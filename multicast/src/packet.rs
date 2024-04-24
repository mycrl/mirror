use std::ops::Range;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};

pub enum Packet {
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Nack { range: Range<u64> },
    Bytes { sequence: u64, chunk: Bytes },
}

impl Packet {
    pub const fn get_max_size(size: usize) -> usize {
        size - 15
    }
}

impl Into<Bytes> for Packet {
    fn into(self) -> Bytes {
        let mut bytes = BytesMut::with_capacity(
            5 + match &self {
                Self::Bytes { chunk, .. } => 8 + chunk.len(),
                Self::Ping { .. } | Self::Pong { .. } => 16,
                Self::Nack { .. } => 4,
            },
        );

        bytes.put_u32(0);
        bytes.put_u8(match &self {
            Self::Ping { .. } => 0,
            Self::Pong { .. } => 1,
            Self::Nack { .. } => 2,
            Self::Bytes { .. } => 3,
        });

        match self {
            Self::Bytes { chunk, sequence } => {
                bytes.put_u64(sequence);
                bytes.put(chunk);
            }
            Self::Ping { timestamp } | Self::Pong { timestamp } => {
                bytes.put_u64(timestamp);
            }
            Self::Nack { range } => {
                bytes.put_u64(range.start);
                bytes.put_u64(range.end);
            }
        }

        let crc = fingerprint(&bytes[4..]);
        (&mut bytes[0..4]).copy_from_slice(&crc.to_be_bytes());

        bytes.freeze()
    }
}

impl TryFrom<&[u8]> for Packet {
    type Error = ();

    fn try_from(mut bytes: &[u8]) -> Result<Self, Self::Error> {
        // Check if the current slice is damaged.
        let crc = bytes.get_u32();
        if crc != fingerprint(&bytes[..]) {
            return Err(());
        }

        Ok(match bytes.get_u8() {
            0 => Self::Ping {
                timestamp: bytes.get_u64(),
            },
            1 => Self::Pong {
                timestamp: bytes.get_u64(),
            },
            2 => Self::Nack {
                range: bytes.get_u64()..bytes.get_u64(),
            },
            3 => Self::Bytes {
                sequence: bytes.get_u64(),
                chunk: Bytes::copy_from_slice(bytes),
            },
            _ => return Err(()),
        })
    }
}

/// CRC32 Fingerprint.
///
/// # Unit Test
///
/// ```
/// assert_eq!(faster_stun::util::fingerprint(b"1"), 3498621689);
/// ```
fn fingerprint(buf: &[u8]) -> u32 {
    Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(buf) ^ 0x5354_554e
}

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct PacketEncoder {
    packets: Vec<BytesMut>,
    max_size: usize,
    sequence: u16,
}

impl PacketEncoder {
    pub fn new(max_size: usize) -> Self {
        Self {
            packets: Default::default(),
            max_size: max_size - 6,
            sequence: 0,
        }
    }

    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn encode(&mut self, bytes: &[u8]) -> &[BytesMut] {
        if bytes.len() == 0 {
            return &[];
        }

        let mut size = 0;
        for (i, chunk) in bytes.chunks(self.max_size).enumerate() {
            {
                if self.packets.get(i).is_none() {
                    self.packets.push(BytesMut::with_capacity(self.max_size * 2));
                }
            }

            if let Some(buf) = self.packets.get_mut(i) {
                buf.clear();
                buf.put_u16(self.sequence);
                buf.put_u32(bytes.len() as u32);
                buf.put(chunk);

                size += 1;
            }
        }

        self.sequence = if self.sequence == u16::MAX {
            0
        } else {
            self.sequence + 1
        };

        &self.packets[..size]
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct PacketDecoder {
    bytes: BytesMut,
    sequence: i16,
    length: usize,
}

impl PacketDecoder {
    pub fn new() -> Self {
        Self {
            bytes: BytesMut::with_capacity(1024 * 1024),
            sequence: -1,
            length: 0,
        }
    }

    pub fn decode(&mut self, mut bytes: &[u8]) -> Option<Bytes> {
        if bytes.len() == 0 {
            return None;
        }

        let sequence = bytes.get_u16() as i16;
        let length = bytes.get_u32() as usize;
        log::info!("========================== {}, {}", bytes.len(), length);

        let mut results = None;
        if sequence != self.sequence {
            if self.bytes.len() >= self.length {
                results = Some(Bytes::copy_from_slice(&self.bytes[..self.length]));
            }

            self.bytes.clear();
        }

        self.bytes.put(bytes);
        self.sequence = sequence;
        self.length = length;
        results
    }
}
