use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
#[derive(Default)]
pub struct Encoder(Vec<Vec<u8>>);

impl Encoder {
    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn encode(&mut self, unit_len: usize, kind: StreamKind, buf: &[u8]) -> Option<&[Vec<u8>]> {
        if buf.len() == 0 {
            return None;
        }

        let mut size = 0;
        for (i, chunk) in buf.chunks(unit_len - 9).enumerate() {
            {
                if self.0.get(i).is_none() {
                    self.0.push(vec![0u8; unit_len]);
                }
            }

            if let Some(buf) = self.0.get_mut(i) {
                buf.clear();
                buf.put_u32(0);
                buf.put_u16(i as u16);
                buf.put_u8(kind as u8);
                buf.put_u16(chunk.len() as u16);
                buf.put(chunk);

                let crc = fingerprint(&buf[4..]);
                (&mut buf[0..4]).copy_from_slice(&crc.to_be_bytes());
                size += 1;
            }
        }

        Some(&self.0[..size])
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct Decoder {
    kind: Option<StreamKind>,
    buf: BytesMut,
    throw: bool,
    seq: i16,
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            seq: -1,
            kind: None,
            throw: false,
            buf: BytesMut::with_capacity(1024 * 1024),
        }
    }
}

impl Decoder {
    pub fn decode(&mut self, mut buf: &[u8]) -> Option<(Bytes, StreamKind)> {
        // Check if the current slice is damaged.
        let crc = buf.get_u32();
        if crc != fingerprint(&buf[..]) {
            log::warn!("Incorrect packet received.");

            // If the check doesn't pass, then none of the packets in the set
            // can be used because there is no retransmission.
            self.throw = true;
            return None;
        }

        // Get slice header information.
        let seq = buf.get_u16() as i16;
        let kind = StreamKind::try_from(buf.get_u8()).unwrap();
        let size = buf.get_u16() as usize;
        if self.throw {
            // It has entered discard mode, but when it encounters a new group
            // arriving, it begins to receive the new group normally.
            if seq == 0 {
                self.throw = false;
            }
        } else {
            // Normal processing, it is still necessary to check whether the
            // packet sequence number is consecutive, and check whether the
            // current group has lost any packets.
            if seq > 0 && self.seq + 1 != seq {
                log::warn!("Packets are starting to be lost, ignore this set of packets.");

                // has dropped the packet, enters discard mode, and returns the
                // null result immediately.
                self.throw = true;
                return None;
            }
        }

        let mut bytes = None;
        if !self.throw {
            if seq == 0 && !self.buf.is_empty() {
                bytes = Some(Bytes::copy_from_slice(&self.buf[..]));
                self.buf.clear();
            }

            self.buf.put(&buf[..size]);
        }

        self.seq = seq;
        let old_kind = self.kind.replace(kind);

        bytes.map(|it| (it, old_kind.unwrap()))
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
