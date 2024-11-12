mod adapter;
mod multi;
mod package;
mod receiver;
mod sender;
mod srt;

pub use self::{
    adapter::{
        BufferFlag, StreamBufferInfo, StreamKind, StreamMultiReceiverAdapter,
        StreamReceiverAdapter, StreamSenderAdapter,
    },
    multi::{Server as MulticastServer, Socket as MulticastSocket},
    package::{copy_from_slice, with_capacity, Package, PacketInfo, UnPackage},
    receiver::{create_mix_receiver, create_split_receiver, Receiver as TransportReceiver},
    sender::{create_sender, Sender as TransportSender},
    srt::{
        Descriptor as SrtDescriptor, FragmentDecoder as SrtFragmentDecoder,
        FragmentEncoder as SrtFragmentEncoder, Server as SrtServer, Socket as SrtSocket,
    },
};

use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
    str::FromStr,
};

/// Initialize the srt communication protocol, mainly initializing some
/// log-related things.
pub fn startup() -> bool {
    srt::startup()
}

/// Clean up the srt environment and prepare to exit.
pub fn shutdown() {
    srt::cleanup()
}

#[derive(Debug, Clone, Copy)]
pub enum TransportStrategy {
    Direct(SocketAddr),
    /// The IP address and port of the server, in this case the service refers
    /// to the mirror service.
    Relay(SocketAddr),
    /// The multicast address used for multicasting, which is an IP address.
    Multicast(SocketAddr),
}

#[derive(Debug, Clone, Copy)]
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
