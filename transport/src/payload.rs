use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};

pub struct PacketInfo {
    pub kind: StreamKind,
    pub flags: u8,
    pub timestamp: u64,
}

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct Muxer(u32);

impl Default for Muxer {
    fn default() -> Self {
        Self(0)
    }
}

impl Muxer {
    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn mux(&mut self, info: PacketInfo, buf: &[u8]) -> Option<Bytes> {
        if buf.is_empty() {
            return None;
        }

        let mut bytes = BytesMut::with_capacity(buf.len() + 18);
        bytes.put_u32(0);
        bytes.put_u32(self.0);
        bytes.put_u8(info.kind as u8);
        bytes.put_u8(info.flags);
        bytes.put_u64(info.timestamp);
        bytes.put(buf);

        if self.0 == u32::MAX {
            self.0 = 0;
        } else {
            self.0 += 1;
        }

        let crc = fingerprint(&bytes[4..]);
        (&mut bytes[0..4]).copy_from_slice(&crc.to_be_bytes());
        Some(bytes.freeze())
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct Remuxer(i32);

impl Default for Remuxer {
    fn default() -> Self {
        Self(-1)
    }
}

impl Remuxer {
    pub fn remux(&mut self, mut buf: &[u8]) -> Option<(usize, PacketInfo)> {
        // Check if the current slice is damaged.
        let crc = buf.get_u32();
        if crc != fingerprint(&buf[..]) {
            log::warn!("Data corruption. Skip this packet.");
            return None;
        }

        // Get slice header information.
        let seq = buf.get_u32() as i32;
        let kind = StreamKind::try_from(buf.get_u8()).unwrap();
        let flags = buf.get_u8();
        let timestamp = buf.get_u64();

        let is_loss = self.0 + 1 != seq;
        if is_loss {
            log::warn!(
                "Packet loss, number of lost = {}, current seq={}, previous seq={}",
                seq - self.0,
                seq,
                self.0
            );
        }

        self.0 = seq;
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
