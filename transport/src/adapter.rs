use std::{
    fmt,
    net::IpAddr,
    sync::{Arc, Weak},
};

use async_trait::async_trait;
use bytes::Bytes;
use codec::video::VideoStreamSenderProcesser;
use tokio::sync::{
    broadcast::{channel, Receiver, Sender},
    Mutex, RwLock,
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
        ip: IpAddr,
        description: &[u8],
    ) -> Option<Weak<StreamReceiverAdapter>>;
}

pub struct StreamSenderAdapter {
    video: VideoStreamSenderProcesser,
    tx: Sender<(Bytes, StreamKind)>,
    rx: Mutex<Receiver<(Bytes, StreamKind)>>,
}

impl StreamSenderAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = channel(10);
        Arc::new(Self {
            video: VideoStreamSenderProcesser::new(),
            rx: Mutex::new(rx),
            tx,
        })
    }

    pub async fn send(&self, buf: Bytes, info: StreamBufferInfo) -> bool {
        if let StreamBufferInfo::Video(flags) = info {
            self.video.apply(buf.clone(), flags);
        }

        self.tx
            .send((
                buf,
                match info {
                    StreamBufferInfo::Video(_) => StreamKind::Video,
                    StreamBufferInfo::Audio(_) => StreamKind::Audio,
                },
            ))
            .is_ok()
    }

    pub(crate) async fn next(&self) -> Option<(Bytes, StreamKind)> {
        self.rx.lock().await.recv().await.ok()
    }

    pub(crate) fn get_config(&self) -> Vec<(&[u8], StreamKind)> {
        [
            (
                self.video.get_config_buffer().unwrap_or_else(|| &[]),
                StreamKind::Video,
            ),
            (
                self.video.get_key_buffer().unwrap_or_else(|| &[]),
                StreamKind::Video,
            ),
        ]
        .to_vec()
    }
}

pub struct StreamReceiverAdapter {
    tx: RwLock<Option<Sender<(Bytes, StreamKind)>>>,
    rx: Mutex<Receiver<(Bytes, StreamKind)>>,
}

impl StreamReceiverAdapter {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = channel(10);
        Arc::new(Self {
            tx: RwLock::new(Some(tx)),
            rx: Mutex::new(rx),
        })
    }

    pub async fn next(&self) -> Option<(Bytes, StreamKind)> {
        self.rx.lock().await.recv().await.ok()
    }

    pub(crate) async fn send(&self, buf: Bytes, kind: StreamKind) -> bool {
        if let Some(sender) = self.tx.read().await.as_ref() {
            sender.send((buf, kind)).is_ok()
        } else {
            false
        }
    }

    pub(crate) async fn close(&self) {
        drop(self.tx.write().await.take())
    }
}
