use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::net::UdpSocket;

use crate::{remuxer::Remuxer, Error};

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
pub struct Client {
    buffer: Vec<u8>,
    remuxer: Remuxer,
    socket: UdpSocket,
}

impl Client {
    /// Creates a UDP socket from the given address.
    ///
    /// You need to specify the multicast group for the udp session to join to
    /// the specified multicast group.
    ///
    /// Note that only IPV4 is supported.
    pub async fn new(multicast: Ipv4Addr, bind: SocketAddr, timeout: usize) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(bind).await?;
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(multicast, bind)?;

            log::info!(
                "udp socket join: multicast={}, interface={}",
                multicast,
                bind
            );
        }

        log::info!("udp socket bind to: bind={}", bind);

        Ok(Self {
            remuxer: Remuxer::new(timeout),
            buffer: vec![0u8; 2048],
            socket,
        })
    }

    /// Reads packets sent from the multicast server.
    ///
    /// Because the packets are reordered, it is possible to read out more than
    /// one packet at a time.
    ///
    /// Note that there may be packet loss.
    pub async fn read(&mut self) -> Result<&[Vec<u8>], Error> {
        let size = self.socket.recv(&mut self.buffer[..]).await?;
        if size == 0 {
            return Err(Error::Closed);
        }

        Ok(self.remuxer.remux(&self.buffer[..size]))
    }
}
