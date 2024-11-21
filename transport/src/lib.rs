mod adapter;
mod multicast;
mod package;
mod receiver;
mod sender;
mod transmission;

pub use self::{
    adapter::{
        BufferFlag, StreamBufferInfo, StreamKind, StreamMultiReceiverAdapter,
        StreamReceiverAdapter, StreamReceiverAdapterAbstract, StreamSenderAdapter,
    },
    multicast::{Server as MulticastServer, Socket as MulticastSocket},
    package::{copy_from_slice, with_capacity, Package, PacketInfo, UnPackage},
    receiver::{create_mix_receiver, create_split_receiver, Receiver as TransportReceiver},
    sender::{create_sender, Sender as TransportSender},
    transmission::{
        Descriptor as TransmissionDescriptor, FragmentDecoder as TransmissionFragmentDecoder,
        FragmentEncoder as TransmissionFragmentEncoder, Server as TransmissionServer,
        Socket as TransmissionSocket,
    },
};

use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
    str::FromStr,
};

use serde::{Deserialize, Serialize};

/// Initialize the srt communication protocol, mainly initializing some
/// log-related things.
pub fn startup() -> bool {
    transmission::startup()
}

/// Clean up the srt environment and prepare to exit.
pub fn shutdown() {
    transmission::cleanup()
}

/// Transport layer strategies.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TransportStrategy {
    /// In straight-through mode, the sender creates an SRT server and the
    /// receiver connects directly to the sender via the SRT protocol.
    ///
    /// For the sender, the network address is the address to which the SRT
    /// server binds and listens.
    ///
    /// ```text
    /// example: 0.0.0.0:8080
    /// ```
    ///
    /// For the receiving end, the network address is the address of the SRT
    /// server on the sending end.
    ///
    /// ```text
    /// example: 192.168.1.100:8080
    /// ```
    Direct(SocketAddr),
    /// Forwarding mode, where the sender and receiver pass data through a relay
    /// server.
    ///
    /// The network address is the address of the transit server.
    Relay(SocketAddr),
    /// UDP multicast mode, where the sender sends multicast packets into the
    /// current network and the receiver processes the multicast packets.
    ///
    /// The sender and receiver use the same address, which is a combination of
    /// multicast address + port.
    ///
    /// ```text
    /// example: 239.0.0.1:8080
    /// ```
    Multicast(SocketAddr),
}

/// Transport configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TransportDescriptor {
    pub strategy: TransportStrategy,
    /// see: [Maximum_transmission_unit](https://en.wikipedia.org/wiki/Maximum_transmission_unit)
    pub mtu: usize,
}

#[repr(u8)]
#[derive(Default, PartialEq, Eq, Debug, Clone, Copy)]
pub enum StreamInfoKind {
    #[default]
    Subscriber = 0,
    Publisher = 1,
}

#[derive(Default, Debug, Clone)]
pub struct StreamInfo {
    pub id: String,
    pub kind: StreamInfoKind,
}

impl FromStr for StreamInfo {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with("#!::") {
            let mut info = Self::default();
            for item in value.split_at(4).1.split(',') {
                if let Some((k, v)) = item.split_once('=') {
                    match k {
                        "i" => {
                            info.id = v.to_string();
                        }
                        "k" => {
                            if let Ok(kind) = v.parse::<u8>() {
                                match kind {
                                    0 => {
                                        info.kind = StreamInfoKind::Subscriber;
                                    }
                                    1 => {
                                        info.kind = StreamInfoKind::Publisher;
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }

            Ok(info)
        } else {
            Err(Error::new(ErrorKind::InvalidInput, "invalid stream info"))
        }
    }
}

impl ToString for StreamInfo {
    fn to_string(&self) -> String {
        format!("#!::i={},k={}", self.id, self.kind as u8)
    }
}
