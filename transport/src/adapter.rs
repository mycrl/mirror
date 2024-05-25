use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU8},
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use bytes::{BufMut, Bytes, BytesMut};
use common::atomic::{AtomicOption, EasyAtomic};

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum BufferFlag {
    KeyFrame = 1,
    Config = 2,
    EndOfStream = 4,
    Partial = 8,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Video = 0,
    Audio = 1,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamKindTryFromError;

impl std::error::Error for StreamKindTryFromError {}

impl fmt::Display for StreamKindTryFromError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StreamKindTryFromError")
    }
}

impl TryFrom<u8> for StreamKind {
    type Error = StreamKindTryFromError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Video,
            1 => Self::Audio,
            _ => return Err(StreamKindTryFromError),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamBufferInfo {
    Video(i32, u64),
    Audio(i32, u64),
}

/// Video Audio Streaming Send Processing
///
/// Because the receiver will normally join the stream in the middle of the
/// stream, and in the face of this situation, it is necessary to process the
/// sps and pps as well as the key frame information.
#[allow(clippy::type_complexity)]
pub struct StreamSenderAdapter {
    is_multicast: AtomicBool,
    audio_interval: AtomicU8,
    video_config: AtomicOption<Bytes>,
    audio_config: AtomicOption<Bytes>,
    tx: Sender<Option<(Bytes, StreamKind, u8, u64)>>,
    rx: Mutex<Receiver<Option<(Bytes, StreamKind, u8, u64)>>>,
}

impl StreamSenderAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = channel();
        Arc::new(Self {
            video_config: AtomicOption::new(None),
            audio_config: AtomicOption::new(None),
            is_multicast: AtomicBool::new(false),
            audio_interval: AtomicU8::new(0),
            rx: Mutex::new(rx),
            tx,
        })
    }

    /// Toggle whether to use multicast
    pub fn set_multicast(&self, is_multicast: bool) {
        self.is_multicast.update(is_multicast);
    }

    /// Get whether to use multicast
    pub(crate) fn get_multicast(&self) -> bool {
        self.is_multicast.get()
    }

    pub fn close(&self) {
        self.tx.send(None).expect(
            "Failed to close, this is because it is not possible to send None to the \
             channel, this is a bug.",
        );
    }

    // h264 decoding any p-frames and i-frames requires sps and pps
    // frames, so the configuration frames are saved here, although it
    // should be noted that the configuration frames will only be
    // generated once.
    pub fn send(&self, mut buf: Bytes, info: StreamBufferInfo) -> bool {
        match info {
            StreamBufferInfo::Video(flags, timestamp) => {
                if flags == BufferFlag::Config as i32 {
                    self.video_config.swap(Some(buf.clone()));
                }

                // Add SPS and PPS units in front of each keyframe (only use android)
                if flags == BufferFlag::KeyFrame as i32 {
                    if let Some(config) = self.video_config.get() {
                        let mut bytes = BytesMut::with_capacity(config.len() + buf.len());
                        bytes.put(&config[..]);
                        bytes.put(buf);

                        buf = bytes.freeze();
                    }
                }

                self.tx
                    .send(Some((buf, StreamKind::Video, flags as u8, timestamp)))
                    .is_ok()
            }
            StreamBufferInfo::Audio(flags, timestamp) => {
                if flags == BufferFlag::Config as i32 {
                    self.audio_config.swap(Some(buf.clone()));
                }

                // Insert a configuration package into every 30 audio packages.
                let count = self.audio_interval.get();
                self.audio_interval.update(if count == 30 {
                    if let Some(config) = self.audio_config.get() {
                        if self
                            .tx
                            .send(Some((
                                config.clone(),
                                StreamKind::Audio,
                                BufferFlag::Config as u8,
                                timestamp,
                            )))
                            .is_err()
                        {
                            return false;
                        }
                    }

                    0
                } else {
                    count + 1
                });

                self.tx
                    .send(Some((buf, StreamKind::Audio, flags as u8, timestamp)))
                    .is_ok()
            }
        }
    }

    pub fn next(&self) -> Option<(Bytes, StreamKind, u8, u64)> {
        self.rx.lock().unwrap().recv().ok().flatten()
    }
}

/// Video Audio Streaming Receiver Processing
///
/// The main purpose is to deal with cases where packet loss occurs at the
/// receiver side, since the SRT communication protocol does not completely
/// guarantee no packet loss.
pub struct StreamReceiverAdapter {
    audio_ready: AtomicBool,
    video_readable: AtomicBool,
    tx: Sender<Option<(Bytes, StreamKind, u8, u64)>>,
    rx: Mutex<Receiver<Option<(Bytes, StreamKind, u8, u64)>>>,
}

impl StreamReceiverAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = channel();
        Arc::new(Self {
            video_readable: AtomicBool::new(false),
            audio_ready: AtomicBool::new(false),
            rx: Mutex::new(rx),
            tx,
        })
    }

    pub fn close(&self) {
        self.tx.send(None).expect(
            "Failed to close, this is because it is not possible to send None to the \
             channel, this is a bug.",
        );
    }

    pub fn next(&self) -> Option<(Bytes, StreamKind, u8, u64)> {
        self.rx.lock().unwrap().recv().ok().flatten()
    }

    /// As soon as a keyframe is received, the keyframe is cached, and when a
    /// packet loss occurs, the previous keyframe is retransmitted directly into
    /// the decoder.
    pub fn send(&self, buf: Bytes, kind: StreamKind, flags: u8, timestamp: u64) -> bool {
        match kind {
            StreamKind::Video => {
                // When keyframes are received, the video stream can be played back
                // normally without corruption.
                let mut readable = self.video_readable.get();
                if flags == BufferFlag::KeyFrame as u8 && !readable {
                    self.video_readable.update(true);
                    readable = true;
                }

                // In case of packet loss, no packet is sent to the decoder.
                if !readable {
                    return true;
                }
            }
            StreamKind::Audio => {
                // The audio configuration package only needs to be processed once.
                let ready = self.audio_ready.get();
                if flags == BufferFlag::Config as u8 {
                    if !ready {
                        self.audio_ready.update(true);
                    } else {
                        return true;
                    }
                } else {
                    if !ready {
                        return true;
                    }
                }
            }
        }

        self.tx.send(Some((buf, kind, flags, timestamp))).is_ok()
    }

    /// Marks that the video packet has been lost.
    pub fn loss_pkt(&self) {
        self.video_readable.update(false);
    }
}
