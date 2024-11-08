mod adapter;
mod multi;
mod package;
pub mod srt;

pub use self::{
    adapter::{
        BufferFlag, StreamBufferInfo, StreamKind, StreamMultiReceiverAdapter,
        StreamReceiverAdapter, StreamReceiverAdapterExt, StreamSenderAdapter,
    },
    package::{copy_from_slice, with_capacity, Package, PacketInfo, UnPackage},
};

use std::{
    io::{Error, ErrorKind},
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::{atomic::AtomicU64, Arc},
    thread,
};

use hylarana_common::atomic::EasyAtomic;
use uuid::Uuid;

/// Initialize the srt communication protocol, mainly initializing some
/// log-related things.
pub fn startup() -> bool {
    srt::startup()
}

/// Clean up the srt environment and prepare to exit.
pub fn shutdown() {
    srt::cleanup()
}

#[repr(u8)]
#[derive(Default, PartialEq, Eq, Debug, Clone, Copy)]
pub enum StreamInfoKind {
    #[default]
    Subscriber = 0,
    Publisher = 1,
}

#[derive(Default, Debug, Clone)]
pub struct StreamInfo {
    pub id: String,
    pub kind: StreamInfoKind,
}

impl FromStr for StreamInfo {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with("#!::") {
            let mut info = Self::default();
            for item in value.split_at(4).1.split(',') {
                if let Some((k, v)) = item.split_once('=') {
                    match k {
                        "i" => {
                            info.id = v.to_string();
                        }
                        "k" => {
                            if let Ok(kind) = v.parse::<u8>() {
                                match kind {
                                    0 => {
                                        info.kind = StreamInfoKind::Subscriber;
                                    }
                                    1 => {
                                        info.kind = StreamInfoKind::Publisher;
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }

            Ok(info)
        } else {
            Err(Error::new(ErrorKind::InvalidInput, "invalid stream info"))
        }
    }
}

impl ToString for StreamInfo {
    fn to_string(&self) -> String {
        format!("#!::i={},k={}", self.id, self.kind as u8)
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamId {
    pub uid: String,
    pub port: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct TransportDescriptor {
    /// The IP address and port of the server, in this case the service refers
    /// to the mirror service.
    pub server: SocketAddr,
    /// The multicast address used for multicasting, which is an IP address.
    pub multicast: Ipv4Addr,
    /// see: [Maximum_transmission_unit](https://en.wikipedia.org/wiki/Maximum_transmission_unit)
    pub mtu: usize,
}

pub struct Transport;

impl Transport {
    pub fn create_sender(
        options: TransportDescriptor,
        adapter: &Arc<StreamSenderAdapter>,
    ) -> Result<StreamId, Error> {
        let stream_id = StreamId {
            uid: Uuid::new_v4().to_string(),
            port: multi::alloc_port()?,
        };

        // Create a multicast sender, the port is automatically assigned an idle port by
        // the system
        let mut mcast_sender = multi::Server::new(
            options.multicast,
            format!("0.0.0.0:{}", stream_id.port).parse().unwrap(),
            options.mtu,
        )?;

        log::info!("create multicast sender, port={}", stream_id.port);

        // Create an srt configuration and carry stream information
        let mut opt = srt::Descriptor::default();
        opt.fc = 32;
        opt.latency = 20;
        opt.mtu = options.mtu as u32;
        opt.stream_id = Some(
            StreamInfo {
                kind: StreamInfoKind::Publisher,
                id: stream_id.uid.clone(),
            }
            .to_string(),
        );

        // Create an srt connection to the server
        let mut encoder = srt::FragmentEncoder::new(opt.max_pkt_size());
        let srt = Arc::new(srt::Socket::connect(options.server, opt)?);
        log::info!("sender connect to server={}", options.server);

        let stream_id_ = stream_id.clone();
        let adapter_ = Arc::downgrade(adapter);
        thread::Builder::new()
            .name("HylaranaStreamSenderThread".to_string())
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
                                if let Err(e) = srt.send(chunk) {
                                    log::error!("failed to send buf in srt, err={:?}", e);

                                    break 'a;
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }

                log::info!("sender is closed, id={:?}", stream_id_);

                if let Some(adapter) = adapter_.upgrade() {
                    adapter.close();
                    srt.close();
                }
            })?;

        Ok(stream_id)
    }

    pub fn create_receiver<T>(
        stream_id: StreamId,
        options: TransportDescriptor,
        adapter: &Arc<T>,
    ) -> Result<(), Error>
    where
        T: StreamReceiverAdapterExt + 'static,
    {
        let sequence = Arc::new(AtomicU64::new(0));

        // Create an srt configuration and carry stream information
        let mut opt = srt::Descriptor::default();
        opt.fc = 32;
        opt.latency = 20;
        opt.mtu = options.mtu as u32;
        opt.stream_id = Some(
            StreamInfo {
                kind: StreamInfoKind::Subscriber,
                id: stream_id.uid.clone(),
            }
            .to_string(),
        );

        // Create an srt connection to the server
        let srt = Arc::new(srt::Socket::connect(options.server, opt)?);
        log::info!("receiver connect to server={}", options.server);

        // Creating a multicast receiver
        let multicast_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), stream_id.port);
        let multicast = Arc::new(multi::Socket::new(options.multicast, multicast_addr)?);
        log::info!("create multicast receiver, port={}", stream_id.port);

        let multicast_ = Arc::downgrade(&multicast);

        let stream_id_ = stream_id.clone();
        let srt_ = Arc::downgrade(&srt);
        let adapter_ = Arc::downgrade(adapter);
        let sequence_ = Arc::downgrade(&sequence);
        thread::Builder::new()
            .name("HylaranaStreamMulticastReceiverThread".to_string())
            .spawn(move || {
                while let Some((seq, bytes)) = multicast.read() {
                    if bytes.is_empty() {
                        break;
                    }

                    if let Some(adapter) = adapter_.upgrade() {
                        // Check whether the sequence number is continuous, in
                        // order to check whether packet loss has occurred
                        if let Some(sequence) = sequence_.upgrade() {
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
                    } else {
                        break;
                    }
                }

                log::warn!("multicast receiver is closed, id={:?}", stream_id_);

                if let Some(srt) = srt_.upgrade() {
                    srt.close();
                }
            })?;

        let adapter_ = Arc::downgrade(adapter);
        thread::Builder::new()
            .name("HylaranaStreamReceiverThread".to_string())
            .spawn(move || {
                let mut buf = [0u8; 2000];
                let mut decoder = srt::FragmentDecoder::new();

                loop {
                    match srt.read(&mut buf) {
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

                log::warn!("srt receiver is closed, id={:?}", stream_id);

                if let Some(adapter) = adapter_.upgrade() {
                    adapter.close();
                    srt.close();
                }

                if let Some(multicast) = multicast_.upgrade() {
                    multicast.close();
                }
            })?;

        Ok(())
    }
}
