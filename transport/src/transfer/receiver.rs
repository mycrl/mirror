use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        mpsc::{self, channel},
        Arc,
    },
    thread,
};

use bytes::Bytes;
use socket2::Socket;
use thread_priority::{set_current_thread_priority, ThreadPriority};

use super::{
    dequeue::Dequeue,
    packet::{Packet, PacketDecoder},
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
    #[allow(unused)]
    socket: Arc<UdpSocket>,
    rx: mpsc::Receiver<Bytes>,
}

impl Receiver {
    /// Creates a UDP socket from the given address.
    ///
    /// You need to specify the multicast group for the udp session to join to
    /// the specified multicast group.
    ///
    /// Note that only IPV4 is supported.
    pub fn new(multicast: Ipv4Addr, bind: SocketAddr) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(bind)?;
        let socket = Socket::from(socket);
        socket.set_recv_buffer_size(4 * 1024 * 1024)?;

        let socket: Arc<UdpSocket> = Arc::new(socket.into());
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(&multicast, &bind)?;
            socket.set_broadcast(true)?;

            log::info!(
                "multicast receiver join: multicast={}, interface={}",
                multicast,
                bind
            );
        }

        log::info!("multicast receiver bind to: bind={}", bind);

        let (tx, rx) = channel();
        let socket_ = Arc::downgrade(&socket);
        thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            let mut buf = vec![0u8; 2048];
            let mut queue = Dequeue::new(50);
            let mut decoder = PacketDecoder::new();

            'a: while let Some(socket) = socket_.upgrade() {
                if let Ok(size) = socket.recv(&mut buf[..]) {
                    if size == 0 {
                        break;
                    }

                    if let Some(packet) = Packet::try_from(&buf[..size]) {
                        queue.push(packet);

                        while let Some(packet) = queue.pop() {
                            if let Some(payload) = decoder.decode(packet) {
                                if tx.send(payload).is_err() {
                                    break 'a;
                                }
                            }
                        }
                    }
                } else {
                    break;
                };
            }
        });

        Ok(Self { socket, rx })
    }

    /// Reads packets sent from the multicast server.
    ///
    /// Because the packets are reordered, it is possible to read out more than
    /// one packet at a time.
    ///
    /// Note that there may be packet loss.
    pub fn read(&self) -> Result<Bytes, Error> {
        self.rx.recv().map_err(|_| Error::Closed)
    }
}
