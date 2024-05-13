use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use xxhash_rust::xxh3::xxh3_64;

pub struct PacketInfo {
    pub kind: StreamKind,
    pub flags: u8,
    pub timestamp: u64,
}

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
#[derive(Default)]
pub struct Muxer {
    sequence: u64,
}

impl Muxer {
    const HEAD_SIZE: usize = 26;

    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn mux(&mut self, info: PacketInfo, buf: &[u8]) -> Option<Bytes> {
        if buf.is_empty() {
            return None;
        }

        let mut bytes = BytesMut::with_capacity(buf.len() + Self::HEAD_SIZE);
        bytes.put_u64(0);
        bytes.put_u64(self.sequence);
        bytes.put_u8(info.kind as u8);
        bytes.put_u8(info.flags);
        bytes.put_u64(info.timestamp);
        bytes.put(buf);

        // xxhash
        let hash = xxh3_64(&bytes[8..]);
        bytes[0..8].copy_from_slice(&hash.to_be_bytes());

        if self.sequence == u64::MAX {
            self.sequence = 0;
        } else {
            self.sequence += 1;
        }

        Some(bytes.freeze())
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct Remuxer {
    sequence: i128,
}

impl Default for Remuxer {
    fn default() -> Self {
        Self { sequence: -1 }
    }
}

impl Remuxer {
    pub fn remux(&mut self, mut bytes: &[u8]) -> Option<(usize, PacketInfo)> {
        let hash = bytes.get_u64();
        if hash == xxh3_64(bytes) {
            let seq = bytes.get_u64() as i128;
            let is_loss = seq != 0 && self.sequence + 1 != seq;
            if is_loss {
                log::warn!(
                    "Packet loss, number of lost = {}, current seq={}, previous seq={}",
                    seq - self.sequence,
                    seq,
                    self.sequence
                );
            }

            self.sequence = seq;
            if !is_loss {
                Some((
                    Muxer::HEAD_SIZE,
                    PacketInfo {
                        kind: StreamKind::try_from(bytes.get_u8()).ok()?,
                        flags: bytes.get_u8(),
                        timestamp: bytes.get_u64(),
                    },
                ))
            } else {
                None
            }
        } else {
            None
        }
    }
}
