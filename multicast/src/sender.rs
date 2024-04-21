use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket}, sync::{atomic::AtomicBool, mpsc::{self, channel}, Arc, Mutex}, thread, time::Instant
};

use crate::{
    reliable::{Reliable, ReliableConfig, ReliableObserver},
    Error,
};

use bytes::Bytes;
use common::atomic::EasyAtomic;
use thread_priority::{set_current_thread_priority, ThreadPriority};

/// A UDP server.
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
/// This server is used to send multicast packets to all members of a multicast
/// group.
pub struct Sender {
    closed: Arc<AtomicBool>,
    tx: mpsc::Sender<Bytes>,
}

impl Sender {
    /// Creates a UDP socket from the given address.
    ///
    /// You need to specify the multicast group for the udp session to join to
    /// the specified multicast group.
    ///
    /// Note that only IPV4 is supported.
    ///
    /// MTU is used to specify the network unit size, this is used to limit the
    /// maximum size of packets sent.
    pub fn new(multicast: Ipv4Addr, bind: SocketAddr, mtu: usize) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = Arc::new(UdpSocket::bind(SocketAddr::new(bind.ip(), 0))?);
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(&multicast, &bind)?;

            log::info!(
                "multicast sender join: multicast={}, interface={}",
                multicast,
                bind
            );
        }

        log::info!("multicast sender bind to: bind={}", bind);

        let (tx, rx) = channel::<Bytes>();
        let closed = Arc::new(AtomicBool::new(false));
        let reliable = Arc::new(Mutex::new(Reliable::new(
            ReliableConfig {
                name: socket.local_addr()?.to_string(),
                max_fragment_size: mtu - 100,
                max_packet_size: 32 * 1024,
                fragment_size: mtu - 200,
                max_fragments: 255,
            },
            0.0,
            SenderObserver {
                target: SocketAddr::new(IpAddr::V4(multicast), bind.port()),
                closed: closed.clone(),
                socket: socket.clone(),
            },
        )));

        let closed_ = closed.clone();
        let reliable_ = reliable.clone();
        thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            let mut buf = [0u8; 2048];

            while let Ok((size, _addr)) = socket.recv_from(&mut buf) {
                if size == 0 {
                    break;
                }

                if let Ok(mut reliable) = reliable_.lock() {
                    reliable.recv(&buf[..]);
                }
            }

            closed_.update(true);
        });

        let closed_ = closed.clone();
        thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            let time = Instant::now();

            while let Ok(buf) = rx.recv() {
                if let Ok(mut reliable) = reliable.lock() {
                    reliable.send(&buf[..]);
                    reliable.update(time.elapsed().as_millis() as f64);
                }
            }

            closed_.update(true);
        });

        Ok(Self {
            closed,
            tx,
        })
    }

    /// Sends data on the socket to the remote address to which it is connected.
    ///
    /// Sends the packet to all members of the multicast group.
    ///
    /// Note that there may be packet loss.
    pub fn send(&mut self, buf: Bytes) -> Result<(), Error> {
        if self.closed.get() {
            return Err(Error::Closed);
        }

        self.tx.send(buf).map_err(|_| Error::Closed)
    }
}

struct SenderObserver {
    target: SocketAddr,
    socket: Arc<UdpSocket>,
    closed: Arc<AtomicBool>,
}

impl ReliableObserver for SenderObserver {
    fn send(&mut self, _id: u64, _sequence: u16, buf: &[u8]) {
        if self.socket.send_to(buf, self.target).is_err() {
            self.closed.update(true);
        }
    }
}
