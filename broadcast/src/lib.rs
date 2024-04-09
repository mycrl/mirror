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

#[derive(Debug, Clone, Copy)]
pub struct SenderOptions {
    pub bind: SocketAddr,
    pub mtu: usize,
    pub to: u16,
}

pub struct Sender {
    muxer: PacketMuxer,
    socket: UdpSocket,
    target: SocketAddr,
}

impl Sender {
    pub async fn new(options: SenderOptions) -> Result<Self, Error> {
        assert!(options.bind.is_ipv4());

        let socket = UdpSocket::bind(options.bind).await?;
        socket.join_multicast_v4("239.0.0.1".parse().unwrap(), "0.0.0.0".parse().unwrap())?;

        Ok(Self {
            target: SocketAddr::new("239.0.0.1".parse().unwrap(), options.to),
            muxer: PacketMuxer::new(options.mtu),
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
    pub async fn new(bind: SocketAddr) -> Result<Self, Error> {
        assert!(bind.is_ipv4());

        let socket = UdpSocket::bind(bind).await?;
        socket.join_multicast_v4("239.0.0.1".parse().unwrap(), "0.0.0.0".parse().unwrap())?;

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
