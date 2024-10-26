use std::{
    collections::HashMap,
    io::{Error, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::Arc,
    thread,
};

use bytes::{BufMut, Bytes, BytesMut};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

pub struct Service<T> {
    streams: Arc<RwLock<HashMap<u32, u16>>>,
    onlines: Arc<Mutex<HashMap<u32, T>>>,
}

impl<T> Service<T>
where
    T: FnOnce(u16) -> Result<(), Error> + Send + 'static,
{
    pub fn new(server: SocketAddr) -> Result<Self, Error> {
        let mut socket = TcpStream::connect(server)?;
        let streams: Arc<RwLock<HashMap<u32, u16>>> = Default::default();
        let onlines: Arc<Mutex<HashMap<u32, T>>> = Default::default();

        let streams_ = Arc::downgrade(&streams);
        let onlines_ = Arc::downgrade(&onlines);
        thread::Builder::new()
            .name("MirrorTransportServiceThread".to_string())
            .spawn(move || {
                let mut buf = [0u8; 1024];
                let mut bytes = BytesMut::with_capacity(2000);

                // static pong bytes
                let pong_bytes = Signal::Pong.encode();

                while let Ok(size) = socket.read(&mut buf) {
                    log::trace!("signal socket read buf, size={}", size);

                    if size == 0 {
                        break;
                    }

                    // Try to decode all data received
                    bytes.extend_from_slice(&buf[..size]);
                    if let Some((size, signal)) = Signal::decode(&bytes) {
                        let _ = bytes.split_to(size);

                        log::info!("recv a signal={:?}", signal);

                        if let (Some(onlines), Some(streams)) =
                            (onlines_.upgrade(), streams_.upgrade())
                        {
                            match signal {
                                Signal::Start { id, port } => {
                                    streams.write().insert(id, port);
                                    if let Some(func) = onlines.lock().remove(&id) {
                                        if let Err(e) = func(port) {
                                            log::error!("{:?}", e);
                                        }
                                    }
                                }
                                Signal::Stop { id } => {
                                    streams.write().remove(&id);
                                    if onlines.lock().remove(&id).is_some() {
                                        log::info!("channel is close, id={}", id)
                                    }
                                }
                                Signal::Ping => {
                                    if socket.write_all(&pong_bytes).is_err() {
                                        break;
                                    }
                                }
                                _ => (),
                            }
                        } else {
                            break;
                        }
                    }
                }

                log::error!("MirrorTransportServiceThread, service connection is closed");
            })?;

        Ok(Self { streams, onlines })
    }

    pub fn online(&self, id: u32, handle: T) -> Result<Listener<T>, Error> {
        let mut listener = Listener { onlines: None, id };
        if let Some(port) = self.streams.read().get(&id) {
            return handle(*port).map(|_| listener);
        }

        self.onlines.lock().insert(id, handle);
        listener.onlines = Some(self.onlines.clone());
        Ok(listener)
    }
}

pub struct Listener<T> {
    onlines: Option<Arc<Mutex<HashMap<u32, T>>>>,
    id: u32,
}

impl<T> Drop for Listener<T> {
    fn drop(&mut self) {
        if let Some(onlines) = self.onlines.as_ref() {
            drop(onlines.lock().remove(&self.id));
        }
    }
}

#[repr(u8)]
#[derive(Default, PartialEq, Eq, Debug)]
pub enum SocketKind {
    #[default]
    Subscriber = 0,
    Publisher = 1,
}

#[derive(Default, Debug)]
pub struct StreamInfo {
    pub id: u32,
    pub port: Option<u16>,
    pub kind: SocketKind,
}

impl StreamInfo {
    pub fn decode(value: &str) -> Option<Self> {
        if value.starts_with("#!::") {
            let mut info = Self::default();
            for item in value.split_at(4).1.split(',') {
                if let Some((k, v)) = item.split_once('=') {
                    match k {
                        "i" => {
                            if let Ok(id) = v.parse::<u32>() {
                                info.id = id;
                            }
                        }
                        "k" => {
                            if let Ok(kind) = v.parse::<u8>() {
                                match kind {
                                    0 => {
                                        info.kind = SocketKind::Subscriber;
                                    }
                                    1 => {
                                        info.kind = SocketKind::Publisher;
                                    }
                                    _ => (),
                                }
                            }
                        }
                        "p" => {
                            if let Ok(port) = v.parse::<u16>() {
                                info.port = Some(port);
                            }
                        }
                        _ => (),
                    }
                }
            }

            Some(info)
        } else {
            None
        }
    }

    pub fn encode(self) -> String {
        format!(
            "#!::{}",
            [
                format!("i={}", self.id),
                format!("k={}", self.kind as u8),
                self.port.map(|p| format!("p={}", p)).unwrap_or_default(),
            ]
            .join(",")
        )
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Signal {
    Ping,
    Pong,
    /// Start publishing a channel. The port number is the publisher's multicast
    /// port.
    Start {
        id: u32,
        port: u16,
    },
    /// Stop publishing to a channel
    Stop {
        id: u32,
    },
}

impl Signal {
    pub fn encode(&self) -> Bytes {
        let payload = rmp_serde::to_vec(&self).unwrap();
        let mut buf = BytesMut::with_capacity(payload.len() + 2);
        buf.put_u16(buf.capacity() as u16);
        buf.extend_from_slice(&payload);
        buf.freeze()
    }

    pub fn decode(buf: &[u8]) -> Option<(usize, Self)> {
        if buf.len() > 2 {
            let size = u16::from_be_bytes([buf[0], buf[1]]) as usize;

            if size <= buf.len() {
                return rmp_serde::from_slice(&buf[2..size])
                    .ok()
                    .map(|it| (size, it));
            }
        }

        None
    }
}
