use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};

pub struct PacketInfo {
    pub kind: StreamKind,
    pub flags: u8,
    pub timestamp: u64,
}

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct Muxer {
    sequence: u32,
}

impl Default for Muxer {
    fn default() -> Self {
        Self { sequence: 0 }
    }
}

impl Muxer {
    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn mux(&mut self, info: PacketInfo, buf: &[u8]) -> Option<Bytes> {
        if buf.is_empty() {
            return None;
        }

        let mut bytes = BytesMut::with_capacity(buf.len() + 14);
        bytes.put_u32(self.sequence);
        bytes.put_u8(info.kind as u8);
        bytes.put_u8(info.flags);
        bytes.put_u64(info.timestamp);
        bytes.put(buf);

        if self.sequence == u32::MAX {
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
    sequence: i32,
}

impl Default for Remuxer {
    fn default() -> Self {
        Self { sequence: -1 }
    }
}

impl Remuxer {
    pub fn remux(&mut self, mut buf: &[u8]) -> Option<(usize, PacketInfo)> {
        let seq = buf.get_u32() as i32;
        let kind = StreamKind::try_from(buf.get_u8()).unwrap();
        let flags = buf.get_u8();
        let timestamp = buf.get_u64();

        let is_loss = self.sequence + 1 != seq;
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
                14,
                PacketInfo {
                    kind,
                    flags,
                    timestamp,
                },
            ))
        } else {
            None
        }
    }
}
