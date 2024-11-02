mod adapter;
mod multi;
mod package;
mod service;
pub mod srt;

pub use self::{
    adapter::{
        BufferFlag, StreamBufferInfo, StreamKind, StreamMultiReceiverAdapter,
        StreamReceiverAdapter, StreamReceiverAdapterExt, StreamSenderAdapter,
    },
    package::{copy_from_slice, with_capacity, Package, PacketInfo, UnPackage},
    service::{Service, Signal, SocketKind, StreamInfo},
};

use std::{
    io::Error,
    net::{Ipv4Addr, SocketAddr},
    sync::{atomic::AtomicU64, Arc},
    thread,
};

use mirror_common::atomic::EasyAtomic;
use parking_lot::Mutex;

/// Initialize the srt communication protocol, mainly initializing some
/// log-related things.
pub fn startup() -> bool {
    srt::startup()
}

/// Clean up the srt environment and prepare to exit.
pub fn shutdown() {
    srt::cleanup()
}

#[derive(Debug, Clone, Copy)]
pub struct TransportDescriptor {
    pub server: SocketAddr,
    pub multicast: Ipv4Addr,
    pub mtu: usize,
}

pub struct Transport {
    options: TransportDescriptor,
    service: Service<Box<dyn FnOnce(u16) -> Result<(), Error> + Send>>,
}

impl Transport {
    pub fn new(options: TransportDescriptor) -> Result<Self, Error> {
        Ok(Self {
            service: Service::new(options.server)?,
            options,
        })
    }

    pub fn create_sender(
        &self,
        stream_id: u32,
        adapter: &Arc<StreamSenderAdapter>,
    ) -> Result<(), Error> {
        let port = multi::alloc_port()?;

        // Create a multicast sender, the port is automatically assigned an idle port by
        // the system
        let mut mcast_sender = multi::Server::new(
            self.options.multicast,
            format!("0.0.0.0:{}", port).parse().unwrap(),
            self.options.mtu,
        )?;

        log::info!("create multicast sender, port={}", port);

        // Create an srt configuration and carry stream information
        let mut opt = srt::Descriptor::default();
        opt.fc = 32;
        opt.latency = 20;
        opt.mtu = self.options.mtu as u32;
        opt.stream_id = Some(
            StreamInfo {
                kind: SocketKind::Publisher,
                port: Some(port),
                id: stream_id,
            }
            .encode(),
        );

        // Create an srt connection to the server
        let mut encoder = srt::FragmentEncoder::new(opt.max_pkt_size());
        let sender = Arc::new(srt::Socket::connect(self.options.server, opt)?);
        log::info!("sender connect to server={}", self.options.server);

        let adapter_ = Arc::downgrade(adapter);
        thread::Builder::new()
            .name("MirrorStreamSenderThread".to_string())
            .spawn(move || {
                // If the adapter has been released, close the current thread
                'a: while let Some(adapter) = adapter_.upgrade() {
                    if let Some((buf, kind, flags, timestamp)) = adapter.next() {
                        if buf.is_empty() {
                            continue;
                        }

                        // Packaging audio and video information
                        let payload = Package::pack(
                            PacketInfo {
                                kind,
                                flags,
                                timestamp,
                            },
                            buf,
                        );

                        // Here we check whether the audio and video data are being multicasted, so
                        // as to dynamically switch the protocol stack.
                        if adapter.get_multicast() {
                            if let Err(e) = mcast_sender.send(&payload) {
                                log::error!("failed to send buf in multicast, err={:?}", e);

                                break 'a;
                            }
                        } else {
                            // SRT does not perform data fragmentation. It needs to be split into
                            // fragments that do not exceed the MTU size.
                            for chunk in encoder.encode(&payload) {
                                if let Err(e) = sender.send(chunk) {
                                    log::error!("failed to send buf in srt, err={:?}", e);

                                    break 'a;
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }

                log::info!("sender is closed, id={}", stream_id);

                if let Some(adapter) = adapter_.upgrade() {
                    adapter.close();
                    sender.close();
                }
            })?;

        Ok(())
    }

    pub fn create_receiver<T, H>(
        &self,
        stream_id: u32,
        adapter: &Arc<T>,
        online_handle: H,
    ) -> Result<(), Error>
    where
        T: StreamReceiverAdapterExt + 'static,
        H: FnOnce() + Send + 'static,
    {
        let current_mcast_rceiver: Arc<Mutex<Option<Arc<multi::Socket>>>> = Default::default();

        // Create an srt configuration and carry stream information
        let mut opt = srt::Descriptor::default();
        opt.fc = 32;
        opt.latency = 20;
        opt.mtu = self.options.mtu as u32;
        opt.stream_id = Some(
            StreamInfo {
                kind: SocketKind::Subscriber,
                id: stream_id,
                port: None,
            }
            .encode(),
        );

        // Create an srt connection to the server
        let sequence = Arc::new(AtomicU64::new(0));
        let mut decoder = srt::FragmentDecoder::new();
        let receiver = Arc::new(srt::Socket::connect(self.options.server, opt)?);
        log::info!("receiver connect to server={}", self.options.server);

        let multicast_ = self.options.multicast;
        let adapter_ = Arc::downgrade(adapter);
        let sequence_ = Arc::downgrade(&sequence);
        let receiver_ = Arc::downgrade(&receiver);
        let current_mcast_rceiver_ = current_mcast_rceiver.clone();
        let service_listener = self.service.online(
            stream_id,
            Box::new(move |port| {
                // Notify external sender that the sender is online.
                online_handle();

                // Creating a multicast receiver
                let socket = match multi::Socket::new(
                    multicast_,
                    SocketAddr::new("0.0.0.0".parse().unwrap(), port),
                ) {
                    Ok(socket) => {
                        let socket = Arc::new(socket);
                        if let Some(socket) = current_mcast_rceiver_.lock().replace(socket.clone())
                        {
                            socket.close()
                        }

                        socket
                    }
                    Err(e) => {
                        if let Some(receiver) = receiver_.upgrade() {
                            receiver.close();
                        }

                        return Err(e);
                    }
                };

                thread::Builder::new()
                    .name("MirrorStreamMulticastReceiverThread".to_string())
                    .spawn(move || {
                        while let Some((seq, bytes)) = socket.read() {
                            if bytes.is_empty() {
                                break;
                            }

                            if let Some(adapter) = adapter_.upgrade() {
                                // Check whether the sequence number is continuous, in
                                // order to check whether packet loss has occurred
                                if let Some(sequence) = sequence_.upgrade() {
                                    if seq == 0 || seq - 1 == sequence.get() {
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

                                    sequence.update(seq);
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }

                        log::warn!("multicast receiver is closed, id={}", stream_id);

                        if let Some(receiver) = receiver_.upgrade() {
                            receiver.close();
                        }
                    })?;

                Ok(())
            }),
        )?;

        let adapter_ = Arc::downgrade(adapter);
        thread::Builder::new()
            .name("MirrorStreamReceiverThread".to_string())
            .spawn(move || {
                let mut buf = [0u8; 2000];

                loop {
                    match receiver.read(&mut buf) {
                        Ok(size) => {
                            if size == 0 {
                                break;
                            }

                            // All the fragments received from SRT are split and need to be
                            // reassembled here
                            if let Some((seq, bytes)) = decoder.decode(&buf[..size]) {
                                if let Some(adapter) = adapter_.upgrade() {
                                    // Check whether the sequence number is continuous, in order to
                                    // check whether packet loss has
                                    // occurred
                                    if seq == 0 || seq - 1 == sequence.get() {
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

                                    sequence.update(seq);
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

                log::warn!("srt receiver is closed, id={}", stream_id);

                // release service online listener.
                drop(service_listener);

                if let Some(adapter) = adapter_.upgrade() {
                    adapter.close();
                    receiver.close();
                }

                if let Some(socket) = current_mcast_rceiver.lock().take() {
                    socket.close()
                }
            })?;

        Ok(())
    }
}
