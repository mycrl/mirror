use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct Muxer {
    packets: Vec<Vec<u8>>,
    max_size: usize,
}

impl Muxer {
    pub fn new(max_size: usize) -> Self {
        Self {
            packets: Default::default(),
            max_size,
        }
    }

    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn mux(&mut self, kind: StreamKind, flags: u8, buf: &[u8]) -> Option<&[Vec<u8>]> {
        if buf.len() == 0 {
            return None;
        }

        let mut size = 0;
        for (i, chunk) in buf.chunks(self.max_size - 9).enumerate() {
            {
                if self.packets.get(i).is_none() {
                    self.packets.push(vec![0u8; self.max_size]);
                }
            }

            if let Some(buf) = self.packets.get_mut(i) {
                buf.clear();
                buf.put_u32(0);
                buf.put_u8(i as u8);
                buf.put_u8(kind as u8);
                buf.put_u8(flags);
                buf.put_u16(chunk.len() as u16);
                buf.put(chunk);

                let crc = fingerprint(&buf[4..]);
                (&mut buf[0..4]).copy_from_slice(&crc.to_be_bytes());
                size += 1;
            }
        }

        Some(&self.packets[..size])
    }
}

/// Packet decoder decoding results
pub enum State {
    /// Decode the packet normally.
    Pkt(Bytes, StreamKind, u8),
    /// Need to wait for more data.
    Wait,
    /// There was a loss of transmitted packets.
    Loss,
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct Remuxer {
    mark: Option<(StreamKind, u8)>,
    buf: BytesMut,
    throw: bool,
    seq: i8,
}

impl Default for Remuxer {
    fn default() -> Self {
        Self {
            buf: BytesMut::with_capacity(1024 * 1024),
            throw: false,
            mark: None,
            seq: -1,
        }
    }
}

impl Remuxer {
    pub fn remux(&mut self, mut buf: &[u8]) -> State {
        // Check if the current slice is damaged.
        let crc = buf.get_u32();
        if crc != fingerprint(&buf[..]) {
            log::warn!("Incorrect packet received.");

            // If the check doesn't pass, then none of the packets in the set
            // can be used because there is no retransmission.
            self.throw = true;
            return State::Loss;
        }

        // Get slice header information.
        let seq = buf.get_u8() as i8;
        let kind = StreamKind::try_from(buf.get_u8()).unwrap();
        let flags = buf.get_u8();
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
                return State::Loss;
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
        let previous = self.mark.replace((kind, flags));
        bytes
            .map(|it| {
                let (kind, flags) = previous.unwrap();
                State::Pkt(it, kind, flags)
            })
            .unwrap_or(State::Wait)
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
