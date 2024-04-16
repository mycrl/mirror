use std::{
    fmt,
    net::SocketAddr,
    sync::{Arc, Weak},
};

use async_trait::async_trait;
use bytes::Bytes;
use codec::video::{VideoStreamReceiverProcesser, VideoStreamSenderProcesser};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Mutex,
};

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

pub struct StreamSenderAdapter {
    video: VideoStreamSenderProcesser,
    tx: UnboundedSender<Option<(Bytes, StreamKind, u8)>>,
    rx: Mutex<UnboundedReceiver<Option<(Bytes, StreamKind, u8)>>>,
}

impl StreamSenderAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = unbounded_channel();
        Arc::new(Self {
            video: VideoStreamSenderProcesser::new(),
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

    pub fn send(&self, buf: Bytes, info: StreamBufferInfo) -> bool {
        if let StreamBufferInfo::Video(flags) = info {
            self.video.process(buf, flags, |buf, flags| {
                self.tx.send(Some((buf, StreamKind::Video, flags))).is_ok()
            })
        } else {
            self.tx.send(Some((buf, StreamKind::Audio, 0))).is_ok()
        }
    }

    pub async fn next(&self) -> Option<(Bytes, StreamKind, u8)> {
        self.rx.lock().await.recv().await.flatten()
    }
}

pub struct StreamReceiverAdapter {
    video: VideoStreamReceiverProcesser,
    tx: UnboundedSender<Option<(Bytes, StreamKind)>>,
    rx: Mutex<UnboundedReceiver<Option<(Bytes, StreamKind)>>>,
}

impl StreamReceiverAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = unbounded_channel();
        Arc::new(Self {
            video: VideoStreamReceiverProcesser::new(),
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

    pub async fn next(&self) -> Option<(Bytes, StreamKind)> {
        self.rx.lock().await.recv().await.flatten()
    }

    pub fn send(&self, buf: Bytes, kind: StreamKind, flags: u8) -> bool {
        if kind == StreamKind::Video {
            self.video
                .process(buf, flags, |buf| self.tx.send(Some((buf, kind))).is_ok())
        } else {
            self.tx.send(Some((buf, kind))).is_ok()
        }
    }

    pub fn loss_pkt(&self) {
        if cfg!(feature = "frame-drop") {
            self.video.loss_pkt();
        } else {
            if let Some(buf) = self.video.get_key_frame() {
                let _ = self.tx.send(Some((buf, StreamKind::Video)));
            }
        }
    }
}
