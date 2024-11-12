use std::{
    io::Error,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    thread,
};

use uuid::Uuid;

use crate::{
    adapter::StreamSenderAdapter, MulticastServer, Package, PacketInfo, SrtDescriptor,
    SrtFragmentEncoder, SrtSocket, StreamInfo, StreamInfoKind,
};

pub struct Sender {
    id: String,
    adapter: Arc<StreamSenderAdapter>,
}

impl Default for Sender {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            adapter: Arc::new(StreamSenderAdapter::default()),
        }
    }
}

impl Sender {
    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_adapter(&self) -> Arc<StreamSenderAdapter> {
        self.adapter.clone()
    }

    pub fn close(&self) {
        self.adapter.close();
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.close();
    }
}

pub fn create_multicast_sender(addr: SocketAddr, mtu: usize) -> Result<Sender, Error> {
    let sender = Sender::default();

    // Create a multicast sender, the port is automatically assigned an idle port by
    // the system
    let mut server = MulticastServer::new(
        match addr.ip() {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(_) => unimplemented!("not supports ipv6 multicast"),
        },
        format!("0.0.0.0:{}", addr.port()).parse().unwrap(),
        mtu,
    )?;

    log::info!("create multicast sender, id={}, addr={}", sender.id, addr);

    let id = sender.id.to_string();
    let adapter_ = Arc::downgrade(&sender.adapter);
    thread::Builder::new()
        .name("HylaranaStreamMulticastSenderThread".to_string())
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

                    // Here we check whether the audio and video data are being
                    // multicasted, so as to dynamically
                    // switch the protocol stack.
                    if let Err(e) = server.send(&payload) {
                        log::error!("failed to send buf in multicast, err={:?}", e);

                        break 'a;
                    }
                } else {
                    break;
                }
            }

            log::info!("multicast sender is closed, id={}, addr={}", id, addr);

            if let Some(adapter) = adapter_.upgrade() {
                adapter.close();
            }
        })?;

    Ok(sender)
}

pub fn create_relay_sender(addr: SocketAddr, mtu: usize) -> Result<Sender, Error> {
    let sender = Sender::default();

    // Create an srt configuration and carry stream information
    let mut opt = SrtDescriptor::default();
    opt.fc = 32;
    opt.latency = 20;
    opt.mtu = mtu as u32;
    opt.stream_id = Some(
        StreamInfo {
            kind: StreamInfoKind::Publisher,
            id: sender.id.clone(),
        }
        .to_string(),
    );

    // Create an srt connection to the server
    let server = SrtSocket::connect(addr, opt.clone())?;

    log::info!("sender connect to relay server, addr={}", addr);

    let id = sender.id.clone();
    let adapter_ = Arc::downgrade(&sender.adapter);
    thread::Builder::new()
        .name("HylaranaStreamRelaySenderThread".to_string())
        .spawn(move || {
            let mut encoder = SrtFragmentEncoder::new(opt.max_pkt_size());

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

                    // SRT does not perform data fragmentation. It needs to be split
                    // into fragments that do not exceed
                    // the MTU size.
                    for chunk in encoder.encode(&payload) {
                        if let Err(e) = server.send(chunk) {
                            log::error!("failed to send buf in srt, err={:?}", e);

                            break 'a;
                        }
                    }
                } else {
                    break;
                }
            }

            log::info!("srt relay sender is closed, id={}, addr={}", id, addr);

            if let Some(adapter) = adapter_.upgrade() {
                adapter.close();
            }
        })?;

    Ok(sender)
}
