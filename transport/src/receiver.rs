use std::{
    io::Error,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    thread,
};

use uuid::Uuid;

use crate::{
    adapter::StreamReceiverAdapterExt, MulticastSocket, SrtDescriptor, SrtFragmentDecoder,
    SrtSocket, StreamInfo, StreamInfoKind, UnPackage,
};

enum Socket {
    MulticastSocket(Arc<MulticastSocket>),
    SrtSocket(Arc<SrtSocket>),
}

pub struct Receiver<T> {
    id: String,
    adapter: Arc<T>,
    socket: Option<Socket>,
}

impl<T: Default> Default for Receiver<T> {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            adapter: Arc::new(T::default()),
            socket: None,
        }
    }
}

impl<T> Receiver<T> {
    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_adapter(&self) -> Arc<T> {
        self.adapter.clone()
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        if let Some(socket) = self.socket.as_ref() {
            match socket {
                Socket::MulticastSocket(socket) => socket.close(),
                Socket::SrtSocket(socket) => socket.close(),
            }
        }
    }
}

pub fn create_multicast_receiver<T>(id: String, addr: SocketAddr) -> Result<Receiver<T>, Error>
where
    T: Default + StreamReceiverAdapterExt + 'static,
{
    let mut receiver = Receiver::<T>::default();

    // Creating a multicast receiver
    let socket = Arc::new(MulticastSocket::new(
        match addr.ip() {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(_) => unimplemented!("not supports ipv6 multicast"),
        },
        SocketAddr::new("0.0.0.0".parse().unwrap(), addr.port()),
    )?);

    log::info!("create multicast receiver, id={}, addr={}", id, addr);
    receiver.socket = Some(Socket::MulticastSocket(socket.clone()));

    let mut sequence = 0;
    let adapter_ = Arc::downgrade(&receiver.adapter);
    thread::Builder::new()
        .name("HylaranaStreamMulticastReceiverThread".to_string())
        .spawn(move || {
            while let Some((seq, bytes)) = socket.read() {
                if bytes.is_empty() {
                    break;
                }

                if let Some(adapter) = adapter_.upgrade() {
                    // Check whether the sequence number is continuous, in
                    // order to check whether packet loss has occurred
                    if seq == 0 || seq - 1 == sequence {
                        if let Some((info, package)) = UnPackage::unpack(bytes) {
                            if !adapter.send(package, info.kind, info.flags, info.timestamp) {
                                log::error!("adapter on buf failed.");

                                break;
                            }
                        } else {
                            adapter.loss_pkt();
                        }
                    } else {
                        adapter.loss_pkt()
                    }

                    sequence = seq;
                } else {
                    break;
                }
            }

            log::warn!("multicast receiver is closed, id={}, addr={}", id, addr);

            if let Some(adapter) = adapter_.upgrade() {
                adapter.close();
            }
        })?;

    Ok(receiver)
}

pub fn create_srt_receiver<T>(
    id: String,
    addr: SocketAddr,
    mtu: usize,
) -> Result<Receiver<T>, Error>
where
    T: Default + StreamReceiverAdapterExt + 'static,
{
    let mut receiver = Receiver::<T>::default();

    // Create an srt configuration and carry stream information
    let mut opt = SrtDescriptor::default();
    opt.fc = 32;
    opt.latency = 20;
    opt.mtu = mtu as u32;
    opt.stream_id = Some(
        StreamInfo {
            kind: StreamInfoKind::Subscriber,
            id: id.clone(),
        }
        .to_string(),
    );

    // Create an srt connection to the server
    let socket = Arc::new(SrtSocket::connect(addr, opt)?);

    log::info!("receiver connect to srt server, id={}, addr={}", id, addr);
    receiver.socket = Some(Socket::SrtSocket(socket.clone()));

    let mut sequence = 0;
    let adapter_ = Arc::downgrade(&receiver.adapter);
    thread::Builder::new()
        .name("HylaranaStreamReceiverThread".to_string())
        .spawn(move || {
            let mut buf = [0u8; 2000];
            let mut decoder = SrtFragmentDecoder::new();

            loop {
                match socket.read(&mut buf) {
                    Ok(size) => {
                        if size == 0 {
                            break;
                        }

                        // All the fragments received from SRT are split and need to be
                        // reassembled here
                        if let Some((seq, bytes)) = decoder.decode(&buf[..size]) {
                            if let Some(adapter) = adapter_.upgrade() {
                                // Check whether the sequence number is continuous, in
                                // order to
                                // check whether packet loss has
                                // occurred
                                if seq == 0 || seq - 1 == sequence {
                                    if let Some((info, package)) = UnPackage::unpack(bytes) {
                                        if !adapter.send(
                                            package,
                                            info.kind,
                                            info.flags,
                                            info.timestamp,
                                        ) {
                                            log::error!("adapter on buf failed.");

                                            break;
                                        }
                                    } else {
                                        adapter.loss_pkt();
                                    }
                                } else {
                                    adapter.loss_pkt()
                                }

                                sequence = seq;
                            } else {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("{:?}", e);

                        break;
                    }
                }
            }

            log::warn!("srt receiver is closed, id={}, addr={}", id, addr);

            if let Some(adapter) = adapter_.upgrade() {
                adapter.close();
            }
        })?;

    Ok(receiver)
}
