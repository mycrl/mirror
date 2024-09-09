pub mod adapter;
pub mod package;

use std::{
    collections::HashMap,
    io::{Error, Read},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::{
        atomic::{AtomicU32, AtomicU64},
        mpsc::{channel, Sender},
        Arc, Mutex, RwLock, Weak,
    },
    thread,
};

use bytes::BytesMut;
use service::{signal::Signal, SocketKind, StreamInfo};
use smallvec::SmallVec;
use utils::atomic::EasyAtomic;

use crate::{
    adapter::{StreamReceiverAdapterExt, StreamSenderAdapter},
    package::{Package, PacketInfo, UnPackage},
};

pub fn startup() -> bool {
    srt::startup()
}

pub fn shutdown() {
    srt::cleanup()
}

#[derive(Debug, Clone, Copy)]
pub struct TransportDescriptor {
    pub server: SocketAddr,
    pub multicast: Ipv4Addr,
    pub mtu: usize,
}

#[derive(Debug)]
pub struct Transport {
    index: AtomicU32,
    options: TransportDescriptor,
    publishs: Arc<RwLock<HashMap<u32, u16>>>,
    channels: Arc<RwLock<HashMap<u32, Sender<Signal>>>>,
}

impl Transport {
    pub fn new(options: TransportDescriptor) -> Result<Self, Error> {
        let channels: Arc<RwLock<HashMap<u32, Sender<Signal>>>> = Default::default();
        let publishs: Arc<RwLock<HashMap<u32, u16>>> = Default::default();

        // Connecting to a mirror server
        let mut socket = TcpStream::connect(options.server)?;

        // The role of this thread is to forward all received signals to all subscribers
        let channels_ = Arc::downgrade(&channels);
        let publishs_ = Arc::downgrade(&publishs);
        thread::Builder::new()
            .name("MirrorSignalReceiverThread".to_string())
            .spawn(move || {
                let mut buf = [0u8; 1024];
                let mut bytes = BytesMut::with_capacity(2000);

                while let Ok(size) = socket.read(&mut buf) {
                    log::info!("signal socket read buf, size={}", size);

                    if size == 0 {
                        break;
                    }

                    // Try to decode all data received
                    bytes.extend_from_slice(&buf[..size]);
                    if let Some((size, signal)) = Signal::decode(&bytes) {
                        let _ = bytes.split_to(size);

                        log::info!("recv a signal={:?}", signal);

                        if let Some(channels) = channels_.upgrade() {
                            match signal {
                                Signal::Start { id, port } => {
                                    if let Some(publishs) = publishs_.upgrade() {
                                        publishs.write().unwrap().insert(id, port);
                                    }
                                }
                                Signal::Stop { id } => {
                                    if let Some(publishs) = publishs_.upgrade() {
                                        publishs.write().unwrap().remove(&id);
                                    }

                                    if channels.write().unwrap().remove(&id).is_some() {
                                        log::info!("channel is close, id={}", id)
                                    }
                                }
                            }

                            let mut closeds: SmallVec<[u32; 10]> = SmallVec::with_capacity(10);

                            // Forwards the signal to all subscribers
                            {
                                for (id, tx) in channels.read().unwrap().iter() {
                                    if tx.send(signal).is_err() {
                                        closeds.push(*id);
                                    }
                                }
                            }

                            // Clean up closed subscribers
                            if !closeds.is_empty() {
                                for id in closeds {
                                    if channels.write().unwrap().remove(&id).is_some() {
                                        log::info!("channel is close, id={}", id)
                                    }
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }
            })?;

        Ok(Self {
            index: AtomicU32::new(0),
            options,
            channels,
            publishs,
        })
    }

    pub fn create_sender(
        &self,
        stream_id: u32,
        adapter: &Arc<StreamSenderAdapter>,
    ) -> Result<(), Error> {
        let port = multicast::alloc_port()?;

        // Create a multicast sender, the port is automatically assigned an idle port by
        // the system
        let mut mcast_sender = multicast::Server::new(
            self.options.multicast,
            format!("0.0.0.0:{}", port).parse().unwrap(),
            self.options.mtu,
        )?;

        log::info!("create multicast sender, port={}", port);

        // Create an srt configuration and carry stream information
        let mut opt = srt::Descriptor::default();
        opt.fc = 32;
        opt.latency = 40;
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

    pub fn create_receiver<T>(&self, stream_id: u32, adapter: &Arc<T>) -> Result<(), Error>
    where
        T: StreamReceiverAdapterExt + 'static,
    {
        let current_mcast_rceiver = Arc::new(Mutex::new(None));

        // Creating a multicast receiver
        let current_mcast_rceiver_ = current_mcast_rceiver.clone();
        let create_mcast_receiver = move |receiver: Weak<srt::Socket>,
                                          sequence: Arc<AtomicU64>,
                                          adapter: Weak<T>,
                                          multicast,
                                          port| {
            let mcast_rceiver = if let Ok(socket) =
                multicast::Socket::new(multicast, SocketAddr::new("0.0.0.0".parse().unwrap(), port))
            {
                let socket = Arc::new(socket);
                if let Some(socket) = current_mcast_rceiver_
                    .lock()
                    .unwrap()
                    .replace(socket.clone())
                {
                    socket.close()
                }

                socket
            } else {
                if let Some(receiver) = receiver.upgrade() {
                    receiver.close();
                }

                return;
            };

            log::info!("create multicast receiver, port={}", port);

            thread::Builder::new()
                .name("MirrorStreamMulticastReceiverThread".to_string())
                .spawn(move || {
                    while let Some((seq, bytes)) = mcast_rceiver.read() {
                        if bytes.is_empty() {
                            break;
                        }

                        if let Some(adapter) = adapter.upgrade() {
                            // Check whether the sequence number is continuous, in
                            // order to check whether packet loss has occurred
                            if seq == 0 || seq - 1 == sequence.get() {
                                if let Some((info, package)) = UnPackage::unpack(bytes) {
                                    if !adapter.send(package, info.kind, info.flags, info.timestamp)
                                    {
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

                    log::warn!("multicast receiver is closed, id={}", stream_id);

                    if let Some(receiver) = receiver.upgrade() {
                        receiver.close();
                    }
                })
                .unwrap();
        };

        // Create an srt configuration and carry stream information
        let mut opt = srt::Descriptor::default();
        opt.fc = 32;
        opt.latency = 40;
        opt.mtu = self.options.mtu as u32;
        opt.stream_id = Some(
            StreamInfo {
                kind: SocketKind::Subscriber,
                id: stream_id,
                port: None,
            }
            .encode(),
        );

        // Assign a unique ID to each receiver
        let index = self.index.get();
        self.index
            .update(if index == u32::MAX { 0 } else { index + 1 });

        // Create an srt connection to the server
        let sequence = Arc::new(AtomicU64::new(0));
        let mut decoder = srt::FragmentDecoder::new();
        let receiver = Arc::new(srt::Socket::connect(self.options.server, opt)?);
        log::info!("receiver connect to server={}", self.options.server);

        {
            let multicast = self.options.multicast;
            let sequence = sequence.clone();
            let adapter = Arc::downgrade(adapter);
            let receiver = Arc::downgrade(&receiver);
            if let Some(port) = self.publishs.read().unwrap().get(&stream_id) {
                create_mcast_receiver(receiver, sequence, adapter, multicast, *port);
            } else {
                // Add a message receiver to the list
                let (tx, rx) = channel();
                self.channels.write().unwrap().insert(index, tx);

                thread::Builder::new()
                    .name("MirrorReceiverSignalProcessThread".to_string())
                    .spawn(move || {
                        while let Ok(signal) = rx.recv() {
                            if let Signal::Start { id, port } = signal {
                                // Only process messages from the current receiving end
                                if id == stream_id {
                                    create_mcast_receiver(
                                        receiver.clone(),
                                        sequence.clone(),
                                        adapter.clone(),
                                        multicast,
                                        port,
                                    );
                                }
                            }
                        }
                    })?;
            }
        }

        let channels = self.channels.clone();
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

                // Remove the sender, which is intended to stop the signal receiver thread.
                let _ = channels.write().unwrap().remove(&index);

                if let Some(adapter) = adapter_.upgrade() {
                    adapter.close();
                    receiver.close();
                }

                if let Some(socket) = current_mcast_rceiver.lock().unwrap().take() {
                    socket.close()
                }
            })?;

        Ok(())
    }
}
