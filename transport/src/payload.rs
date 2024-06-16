use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use xxhash_rust::xxh3::xxh3_64;

#[derive(Debug)]
pub struct PacketInfo {
    pub kind: StreamKind,
    pub flags: u8,
    pub timestamp: u64,
}

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct Muxer;

impl Muxer {
    const HEAD_SIZE: usize = 18;

    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn mux(info: PacketInfo, buf: &[u8]) -> Bytes {
        let mut bytes = BytesMut::with_capacity(buf.len() + Self::HEAD_SIZE);
        bytes.put_u64(0);
        bytes.put_u8(info.kind as u8);
        bytes.put_u8(info.flags);
        bytes.put_u64(info.timestamp);
        bytes.put(buf);

        let hash = xxh3_64(&bytes[8..]);
        bytes[0..8].copy_from_slice(&hash.to_be_bytes());

        bytes.freeze()
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct Remuxer;

impl Remuxer {
    pub fn remux(mut bytes: &[u8]) -> Option<(usize, PacketInfo)> {
        if bytes.get_u64() == xxh3_64(bytes) {
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
    }
}
