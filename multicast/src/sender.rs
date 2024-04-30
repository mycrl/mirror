use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Arc,
    thread,
};

use crate::{
    packet::{Packet, PacketEncoder},
    Error,
};

use bytes::Bytes;

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
    target: SocketAddr,
    socket: Arc<UdpSocket>,
    encoder: PacketEncoder,
    sequence: u64,
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

        let socket_ = Arc::downgrade(&socket);
        thread::spawn(move || {
            let mut buf = [0u8; 2048];

            while let Some(socket) = socket_.upgrade() {
                if let Ok((size, addr)) = socket.recv_from(&mut buf) {
                    if size == 0 {
                        break;
                    }

                    if let Ok(packet) = Packet::try_from(&buf[..size]) {
                        match packet {
                            Packet::Ping { timestamp } => {
                                let bytes: Bytes = Packet::Pong { timestamp }.into();
                                if socket.send_to(&bytes, addr).is_err() {
                                    break;
                                }
                            }
                            _ => (),
                        }
                    }
                } else {
                    break;
                }
            }
        });

        Ok(Self {
            target: SocketAddr::new(IpAddr::V4(multicast), bind.port()),
            encoder: PacketEncoder::new(Packet::get_max_size(mtu)),
            sequence: 0,
            socket,
        })
    }

    /// Sends data on the socket to the remote address to which it is connected.
    ///
    /// Sends the packet to all members of the multicast group.
    ///
    /// Note that there may be packet loss.
    pub fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() == 0 {
            return Ok(());
        }

        for packet in self.encoder.encode(bytes) {
            let bytes: Bytes = Packet::Bytes {
                sequence: self.sequence,
                chunk: packet,
            }
            .into();

            self.socket.send_to(&bytes, self.target)?;
            if self.sequence == u64::MAX {
                self.sequence = 0;
            } else {
                self.sequence += 1;
            }
        }

        Ok(())
    }
}
