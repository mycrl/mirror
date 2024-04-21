use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::net::UdpSocket;

use crate::{muxer::Muxer, Error};

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
    muxer: Muxer,
    socket: UdpSocket,
    target: SocketAddr,
}

impl Server {
    /// Creates a UDP socket from the given address.
    ///
    /// You need to specify the multicast group for the udp session to join to
    /// the specified multicast group.
    ///
    /// Note that only IPV4 is supported.
    ///
    /// MTU is used to specify the network unit size, this is used to limit the
    /// maximum size of packets sent.
    pub async fn new(multicast: Ipv4Addr, bind: SocketAddr, mtu: usize) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(SocketAddr::new(bind.ip(), 0)).await?;
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
            target: SocketAddr::new(IpAddr::V4(multicast), bind.port()),
            muxer: Muxer::new(mtu),
            socket,
        })
    }

    /// Sends data on the socket to the remote address to which it is connected.
    ///
    /// Sends the packet to all members of the multicast group.
    ///
    /// Note that there may be packet loss.
    pub async fn send(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.socket
            .send_to(self.muxer.mux(buf), &self.target)
            .await?;
        Ok(())
    }

    /// Gets the maximum length of the packet.
    pub fn max_packet_size(&self) -> usize {
        self.muxer.max_payload_size()
    }
}
