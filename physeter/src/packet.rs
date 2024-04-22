use std::ops::Range;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};

pub enum Packet {
    // 0
    Ping { timestamp: u64 },
    // 1
    Pong { timestamp: u64 },
    // 2
    Nack { range: Range<u16> },
    // 3
    Payload { group: u16, sequence: u16, data: Bytes },
}

impl Into<Bytes> for Packet {
    fn into(self) -> Bytes {
        let mut bytes = BytesMut::with_capacity(
            5 + match &self {
                Self::Payload { data, .. } => 4 + data.len(),
                Self::Ping { .. } | Self::Pong { .. } => 8,
                Self::Nack { .. } => 4,
            },
        );

        bytes.put_u32(0);
        bytes.put_u8(match &self {
            Self::Ping { .. } => 0,
            Self::Pong { .. } => 1,
            Self::Nack { .. } => 2,
            Self::Payload { .. } => 3,
        });

        match self {
            Self::Ping { timestamp } | Self::Pong { timestamp } => {
                bytes.put_u64(timestamp);
            }
            Self::Nack { range } => {
                bytes.put_u16(range.start);
                bytes.put_u16(range.end);
            }
            Self::Payload { group, sequence, data } => {
                bytes.put_u16(group);
                bytes.put_u16(sequence);
                bytes.put(&data[..]);
            }
        }

        let crc = fingerprint(&bytes[4..]);
        (&mut bytes[0..4]).copy_from_slice(&crc.to_be_bytes());

        bytes.freeze()
    }
}

impl TryFrom<Bytes> for Packet {
    type Error = ();

    fn try_from(mut bytes: Bytes) -> Result<Self, Self::Error> {
        if bytes.get_u32() != fingerprint(&bytes[..]) {
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
                range: bytes.get_u16()..bytes.get_u16(),
            },
            3 => Self::Payload {
                group: bytes.get_u16(),
                sequence: bytes.get_u16(),
                data: bytes,
            },
            _ => return Err(()),
        })
    }
}

pub struct LinearEncoder {
    max_packet_size: usize,
    buffer: BytesMut,
    sequence: u16,
}

impl LinearEncoder {
    pub fn new(max_packet_size: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(max_packet_size * 10),
            // use Packet::Payload header.
            max_packet_size: max_packet_size - 9,
            sequence: 0,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.put(bytes);
    }

    pub fn pop(&mut self) -> Option<Packet> {
        if self.buffer.len() >= self.max_packet_size {
            let sequence = self.sequence;
            self.sequence = self.sequence.wrapping_add(1);

            Some(Packet::Payload {
                data: self.buffer.split_to(self.max_packet_size).freeze(),
                sequence,
                // TODO
                group: 0,
            })
        } else {
            None
        }
    }
}

pub struct LinearDecoder {

}

impl LinearDecoder {
    pub fn new() -> Self {
        Self {

        }
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

struct LinearPakcet;

impl LinearPakcet {
    fn encode(buf: &mut BytesMut, sequence: u16, bytes: &[u8]) {
        
    }
}
