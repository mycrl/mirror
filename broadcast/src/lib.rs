mod packet;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use packet::{PacketMuxer, PakcetRemuxer};
use tokio::net::UdpSocket;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("udp socket closed")]
    Closed,
}

pub struct Sender {
    muxer: PacketMuxer,
    socket: UdpSocket,
    target: SocketAddr,
}

impl Sender {
    pub async fn new(multicast: Ipv4Addr, bind: SocketAddr, mtu: usize) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(SocketAddr::new(bind.ip(), 0)).await?;
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(multicast, bind)?;

            log::info!(
                "udp socket join multicast {}, interface {}",
                multicast,
                bind
            );
        }

        log::info!("udp socket bind to {}", bind);

        Ok(Self {
            target: SocketAddr::new(IpAddr::V4(multicast), bind.port()),
            muxer: PacketMuxer::new(mtu),
            socket,
        })
    }

    pub async fn send(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.socket
            .send_to(self.muxer.mux(buf), &self.target)
            .await?;
        Ok(())
    }

    pub fn max_packet_size(&self) -> usize {
        self.muxer.max_payload_size()
    }
}

pub struct Receiver {
    remuxer: PakcetRemuxer,
    buffer: [u8; 2048],
    socket: UdpSocket,
}

impl Receiver {
    pub async fn new(multicast: Ipv4Addr, bind: SocketAddr) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(bind).await?;
        if let IpAddr::V4(bind) = bind.ip() {
            socket.join_multicast_v4(multicast, bind)?;

            log::info!(
                "udp socket join multicast {}, interface {}",
                multicast,
                bind
            );
        }

        log::info!("udp socket bind to {}", bind);

        Ok(Self {
            remuxer: PakcetRemuxer::new(20),
            buffer: [0u8; 2048],
            socket,
        })
    }

    pub async fn read(&mut self) -> Result<&[Vec<u8>], Error> {
        let size = self.socket.recv(&mut self.buffer[..]).await?;
        if size == 0 {
            return Err(Error::Closed);
        }

        Ok(self.remuxer.remux(&self.buffer[..size]))
    }
}
