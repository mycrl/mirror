mod dequeue;
mod fragments;

use std::{
    io::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        mpsc::{self, channel},
        Arc,
    },
    thread,
};

use bytes::Bytes;
use fragments::FragmentEncoder;
use thread_priority::{set_current_thread_priority, ThreadPriority};

use crate::{
    dequeue::Dequeue,
    fragments::{Fragment, FragmentDecoder},
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
pub struct Socket {
    #[allow(unused)]
    socket: Arc<UdpSocket>,
    rx: mpsc::Receiver<(u64, Bytes)>,
}

impl Socket {
    /// Creates a UDP socket from the given address.
    ///
    /// You need to specify the multicast group for the udp session to join to
    /// the specified multicast group.
    ///
    /// Note that only IPV4 is supported.
    pub fn new(multicast: Ipv4Addr, bind: SocketAddr) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(bind)?;
        let socket = socket2::Socket::from(socket);
        socket.set_recv_buffer_size(4 * 1024 * 1024)?;

        let socket: Arc<UdpSocket> = Arc::new(socket.into());
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(&multicast, &bind)?;
            socket.set_broadcast(true)?;
        }

        let (tx, rx) = channel();
        let socket_ = Arc::downgrade(&socket);
        thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            let mut buf = vec![0u8; 2048];
            let mut queue = Dequeue::new(50);
            let mut decoder = FragmentDecoder::new();

            'a: while let Some(socket) = socket_.upgrade() {
                if let Ok(size) = socket.recv(&mut buf[..]) {
                    if size == 0 {
                        break;
                    }

                    if let Ok(packet) = Fragment::try_from(&buf[..size]) {
                        queue.push(packet);

                        while let Some(chunk) = queue.pop() {
                            if let Some(packet) = decoder.decode(chunk) {
                                if tx.send(packet).is_err() {
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
    pub fn read(&self) -> Option<(u64, Bytes)> {
        self.rx.recv().ok()
    }
}

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
pub struct Server {
    target: SocketAddr,
    socket: UdpSocket,
    encoder: FragmentEncoder,
}

impl Server {
    pub fn local_addr(&self) -> SocketAddr {
        self.target
    }

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

        let socket = UdpSocket::bind(bind)?;
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(&multicast, &bind)?;
            socket.set_multicast_loop_v4(false)?;
        }

        Ok(Self {
            encoder: FragmentEncoder::new(mtu),
            target: socket.local_addr()?,
            socket,
        })
    }

    /// Sends data on the socket to the remote address to which it is connected.
    ///
    /// Sends the packet to all members of the multicast group.
    ///
    /// Note that there may be packet loss.
    pub fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.is_empty() {
            return Ok(());
        }

        for chunk in self.encoder.encode(bytes) {
            self.socket.send_to(chunk, self.target)?;
        }

        Ok(())
    }
}
