use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        mpsc::{self, channel},
        Arc, RwLock,
    },
    thread,
    time::Instant,
};

use bytes::Bytes;
use socket2::Socket;
use thread_priority::{set_current_thread_priority, ThreadPriority};

use crate::{
    reliable::{Reliable, ReliableConfig, ReliableObserver},
    Error,
};

/// A UDP socket.
///
/// After creating a UdpSocket by binding it to a socket address, data can be
/// sent to and received from any other socket address.
///
/// Although UDP is a connectionless protocol, this implementation provides an
/// interface to set an address where data should be sent and received from.
/// After setting a remote address with connect, data can be sent to and
/// received from that address with send and recv.
///
/// As stated in the User Datagram Protocol’s specification in IETF RFC 768, UDP
/// is an unordered, unreliable protocol;
///
/// This client is only used to receive multicast packets and does not send
/// multicast packets.
pub struct Receiver {
    rx: mpsc::Receiver<Bytes>,
}

impl Receiver {
    /// Creates a UDP socket from the given address.
    ///
    /// You need to specify the multicast group for the udp session to join to
    /// the specified multicast group.
    ///
    /// Note that only IPV4 is supported.
    pub fn new(multicast: Ipv4Addr, bind: SocketAddr, mtu: usize) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(bind)?;
        let socket = Socket::from(socket);
        socket.set_recv_buffer_size(1024 * 1024)?;

        let socket: Arc<UdpSocket> = Arc::new(socket.into());
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(&multicast, &bind)?;

            log::info!(
                "multicast receiver join: multicast={}, interface={}",
                multicast,
                bind
            );
        }

        log::info!("multicast receiver bind to: bind={}", bind);

        let (tx, rx) = channel();
        let target = Arc::new(RwLock::new(None));
        let mut reliable = Reliable::new(
            ReliableConfig {
                name: socket.local_addr().unwrap().to_string(),
                max_fragment_size: mtu - 100,
                max_packet_size: 200 * 1024,
                fragment_size: mtu - 200,
                max_fragments: 255,
            },
            0.0,
            ReceiverObserver {
                socket: socket.clone(),
                target: target.clone(),
                tx: Some(tx),
            },
        );

        thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            let mut buf = vec![0u8; 2048];
            let time = Instant::now();

            loop {
                if let Ok((size, addr)) = socket.recv_from(&mut buf[..]) {
                    if size == 0 {
                        break;
                    }

                    if target.read().unwrap().is_none() {
                        target.write().unwrap().replace(addr);
                    }

                    reliable.recv(&buf[..size]);
                    reliable.update(time.elapsed().as_millis() as f64);
                } else {
                    break;
                };
            }
        });

        Ok(Self { rx })
    }

    /// Reads packets sent from the multicast server.
    ///
    /// Because the packets are reordered, it is possible to read out more than
    /// one packet at a time.
    ///
    /// Note that there may be packet loss.
    pub fn read(&mut self) -> Result<Bytes, Error> {
        self.rx.recv().map_err(|_| Error::Closed)
    }
}

struct ReceiverObserver {
    socket: Arc<UdpSocket>,
    tx: Option<mpsc::Sender<Bytes>>,
    target: Arc<RwLock<Option<SocketAddr>>>,
}

impl ReliableObserver for ReceiverObserver {
    fn recv(&mut self, _id: u64, _sequence: u16, buf: &[u8]) -> bool {
        if let Some(tx) = &self.tx {
            if tx.send(Bytes::copy_from_slice(buf)).is_err() {
                drop(self.tx.take())
            }
        }

        true
    }

    fn send(&mut self, _id: u64, _sequence: u16, buf: &[u8]) {
        if let Ok(target) = self.target.read() {
            if let Some(addr) = target.as_ref() {
                if self.socket.send_to(buf, addr).is_err() {
                    drop(self.tx.take())
                }
            }
        }
    }
}
