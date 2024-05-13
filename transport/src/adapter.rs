use std::{
    fmt,
    net::SocketAddr,
    sync::{
        atomic::AtomicBool,
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, Weak,
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

pub trait ReceiverAdapterFactory: Send + Sync {
    fn connect(
        &self,
        id: u8,
        addr: SocketAddr,
        description: &[u8],
    ) -> Option<Weak<StreamReceiverAdapter>>;
}

impl ReceiverAdapterFactory for () {
    fn connect(&self, _: u8, _: SocketAddr, _: &[u8]) -> Option<Weak<StreamReceiverAdapter>> {
        None
    }
}

/// Video Streaming Send Processing
///
/// Because the receiver will normally join the stream in the middle of the
/// stream, and in the face of this situation, it is necessary to process the
/// sps and pps as well as the key frame information.
#[allow(clippy::type_complexity)]
pub struct StreamSenderAdapter {
    config: AtomicOption<Bytes>,
    tx: Sender<Option<(Bytes, StreamKind, u8, u64)>>,
    rx: Mutex<Receiver<Option<(Bytes, StreamKind, u8, u64)>>>,
}

impl StreamSenderAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = channel();
        Arc::new(Self {
            config: AtomicOption::new(None),
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

    // h264 decoding any p-frames and i-frames requires sps and pps
    // frames, so the configuration frames are saved here, although it
    // should be noted that the configuration frames will only be
    // generated once.
    pub fn send(&self, mut buf: Bytes, info: StreamBufferInfo) -> bool {
        match info {
            StreamBufferInfo::Video(flags, timestamp) => {
                if flags == BufferFlag::Config as i32 {
                    self.config.swap(Some(buf.clone()));
                }

                // Add SPS and PPS units in front of each keyframe (only use android)
                if flags == BufferFlag::KeyFrame as i32 {
                    if let Some(config) = self.config.get() {
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
            StreamBufferInfo::Audio(flags, timestamp) => self
                .tx
                .send(Some((buf, StreamKind::Audio, flags as u8, timestamp)))
                .is_ok(),
        }
    }

    pub fn next(&self) -> Option<(Bytes, StreamKind, u8, u64)> {
        self.rx.lock().unwrap().recv().ok().flatten()
    }
}

/// Video Streaming Receiver Processing
///
/// The main purpose is to deal with cases where packet loss occurs at the
/// receiver side, since the SRT communication protocol does not completely
/// guarantee no packet loss.
pub struct StreamReceiverAdapter {
    readable: AtomicBool,
    tx: Sender<Option<(Bytes, StreamKind, u64)>>,
    rx: Mutex<Receiver<Option<(Bytes, StreamKind, u64)>>>,
}

impl StreamReceiverAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = channel();
        Arc::new(Self {
            readable: AtomicBool::new(false),
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

    pub fn next(&self) -> Option<(Bytes, StreamKind, u64)> {
        self.rx.lock().unwrap().recv().ok().flatten()
    }

    /// As soon as a keyframe is received, the keyframe is cached, and when a
    /// packet loss occurs, the previous keyframe is retransmitted directly into
    /// the decoder.
    pub fn send(&self, buf: Bytes, kind: StreamKind, flags: u8, timestamp: u64) -> bool {
        if kind == StreamKind::Video {
            // When keyframes are received, the video stream can be played back
            // normally without corruption.
            let mut readable = self.readable.get();
            if flags == BufferFlag::KeyFrame as u8 && !readable {
                self.readable.update(true);
                readable = true;
            }

            // In case of packet loss, no packet is sent to the decoder.
            if !readable {
                return true;
            }
        }

        self.tx.send(Some((buf, kind, timestamp))).is_ok()
    }

    /// Marks that the video packet has been lost.
    pub fn loss_pkt(&self) {
        self.readable.update(false);
    }
}
