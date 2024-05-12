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
#[derive(Default)]
pub struct Muxer {
    sequence: u64,
}

impl Muxer {
    const HEAD_SIZE: usize = 18;

    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn mux(&mut self, info: PacketInfo, buf: &[u8]) -> Option<Bytes> {
        if buf.is_empty() {
            return None;
        }

        let mut bytes = BytesMut::with_capacity(buf.len() + Self::HEAD_SIZE);
        bytes.put_u64(self.sequence);
        bytes.put_u8(info.kind as u8);
        bytes.put_u8(info.flags);
        bytes.put_u64(info.timestamp);
        bytes.put(buf);

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
    pub fn remux(&mut self, mut buf: &[u8]) -> Option<(usize, PacketInfo)> {
        let seq = buf.get_u64() as i128;
        let kind = StreamKind::try_from(buf.get_u8()).ok()?;
        let flags = buf.get_u8();
        let timestamp = buf.get_u64();

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
