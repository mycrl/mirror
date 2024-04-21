use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct Muxer;

impl Muxer {
    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn mux(kind: StreamKind, flags: u8, timestamp: u64, buf: &[u8]) -> Option<Bytes> {
        if buf.is_empty() {
            return None;
        }

        let mut bytes = BytesMut::with_capacity(buf.len() + 14);
        bytes.put_u32(0);
        bytes.put_u8(kind as u8);
        bytes.put_u8(flags);
        bytes.put_u64(timestamp);
        bytes.put(buf);

        let crc = fingerprint(&bytes[4..]);
        (&mut bytes[0..4]).copy_from_slice(&crc.to_be_bytes());
        Some(bytes.freeze())
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct Remuxer;

impl Remuxer {
    pub fn remux(mut buf: &[u8]) -> Option<(StreamKind, u8, u64)> {
        // Check if the current slice is damaged.
        let crc = buf.get_u32();
        if crc != fingerprint(&buf[..]) {
            log::warn!("Incorrect packet received.");
            return None;
        }

        // Get slice header information.
        let kind = StreamKind::try_from(buf.get_u8()).unwrap();
        let flags = buf.get_u8();
        let timestamp = buf.get_u64();
        Some((kind, flags, timestamp))
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
