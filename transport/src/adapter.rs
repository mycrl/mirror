use std::{
    fmt,
    net::SocketAddr,
    sync::{atomic::AtomicBool, Arc, Weak},
};

use async_trait::async_trait;
use bytes::Bytes;
use common::atomic::{AtomicOption, EasyAtomic};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Mutex,
};

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
    Video(i32),
    Audio(i32),
}

#[async_trait]
pub trait ReceiverAdapterFactory: Send + Sync {
    async fn connect(
        &self,
        id: u8,
        addr: SocketAddr,
        description: &[u8],
    ) -> Option<Weak<StreamReceiverAdapter>>;
}

#[async_trait]
impl ReceiverAdapterFactory for () {
    async fn connect(&self, _: u8, _: SocketAddr, _: &[u8]) -> Option<Weak<StreamReceiverAdapter>> {
        None
    }
}

/// Video Streaming Send Processing
///
/// Because the receiver will normally join the stream in the middle of the
/// stream, and in the face of this situation, it is necessary to process the
/// sps and pps as well as the key frame information.
pub struct StreamSenderAdapter {
    config: AtomicOption<Bytes>,
    tx: UnboundedSender<Option<(Bytes, StreamKind, u8)>>,
    rx: Mutex<UnboundedReceiver<Option<(Bytes, StreamKind, u8)>>>,
    is_android: bool,
}

impl StreamSenderAdapter {
    pub fn new(is_android: bool) -> Arc<Self> {
        let (tx, rx) = unbounded_channel();
        Arc::new(Self {
            config: AtomicOption::new(None),
            rx: Mutex::new(rx),
            is_android,
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
    pub fn send(&self, buf: Bytes, info: StreamBufferInfo) -> bool {
        if let StreamBufferInfo::Video(flags) = info {
            if flags == BufferFlag::Config as i32 {
                self.config.swap(Some(buf.clone()));
            }

            // Add SPS and PPS units in front of each keyframe (only use android)
            if self.is_android {
                if flags == BufferFlag::KeyFrame as i32 {
                    if let Some(buf) = self.config.get() {
                        if self
                            .tx
                            .send(Some((
                                buf.clone(),
                                StreamKind::Video,
                                BufferFlag::Config as u8,
                            )))
                            .is_err()
                        {
                            return false;
                        }
                    }
                }
            }

            self.tx
                .send(Some((buf, StreamKind::Video, flags as u8)))
                .is_ok()
        } else {
            self.tx.send(Some((buf, StreamKind::Audio, 0))).is_ok()
        }
    }

    pub async fn next(&self) -> Option<(Bytes, StreamKind, u8)> {
        self.rx.lock().await.recv().await.flatten()
    }
}

/// Video Streaming Receiver Processing
///
/// The main purpose is to deal with cases where packet loss occurs at the
/// receiver side, since the SRT communication protocol does not completely
/// guarantee no packet loss.
pub struct StreamReceiverAdapter {
    readable: AtomicBool,
    tx: UnboundedSender<Option<(Bytes, StreamKind)>>,
    rx: Mutex<UnboundedReceiver<Option<(Bytes, StreamKind)>>>,
    is_android: bool,
}

impl StreamReceiverAdapter {
    pub fn new(is_android: bool) -> Arc<Self> {
        let (tx, rx) = unbounded_channel();
        Arc::new(Self {
            readable: AtomicBool::new(false),
            rx: Mutex::new(rx),
            is_android,
            tx,
        })
    }

    pub fn close(&self) {
        self.tx.send(None).expect(
            "Failed to close, this is because it is not possible to send None to the \
             channel, this is a bug.",
        );
    }

    pub async fn next(&self) -> Option<(Bytes, StreamKind)> {
        self.rx.lock().await.recv().await.flatten()
    }

    /// As soon as a keyframe is received, the keyframe is cached, and when a
    /// packet loss occurs, the previous keyframe is retransmitted directly into
    /// the decoder.
    pub fn send(&self, buf: Bytes, kind: StreamKind, flags: u8) -> bool {
        if kind == StreamKind::Video {
            // When keyframes are received, the video stream can be played back
            // normally without corruption.
            let mut readable = self.readable.get();
            if flags
                == if self.is_android {
                    BufferFlag::Config as u8
                } else {
                    BufferFlag::KeyFrame as u8
                }
            {
                if !readable {
                    self.readable.update(true);
                    readable = true;
                }
            }

            // In case of packet loss, no packet is sent to the decoder.
            if !readable {
                return true;
            }
        }

        self.tx.send(Some((buf, kind))).is_ok()
    }

    /// Marks that the video packet has been lost.
    pub fn loss_pkt(&self) {
        self.readable.update(false);
    }
}
