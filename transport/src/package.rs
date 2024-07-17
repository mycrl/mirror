use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use xxhash_rust::xxh3::xxh3_64;

#[derive(Debug)]
pub struct PacketInfo {
    pub kind: StreamKind,
    pub flags: i32,
    pub timestamp: u64,
}

pub fn copy_from_slice(src: &[u8]) -> BytesMut {
    let mut bytes = BytesMut::with_capacity(src.len() + Package::HEAD_SIZE);
    bytes.put_bytes(0, Package::HEAD_SIZE);
    bytes.put(src);
    bytes
}

pub fn with_capacity(size: usize) -> BytesMut {
    BytesMut::zeroed(size + Package::HEAD_SIZE)
}

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct Package;

impl Package {
    const HEAD_SIZE: usize = 26;

    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn pack(info: PacketInfo, mut bytes: BytesMut) -> Bytes {
        let size = bytes.len();

        unsafe {
            bytes.set_len(0);
        }

        bytes.put_u64(0);
        bytes.put_u64(size as u64);
        bytes.put_u8(info.kind as u8);
        bytes.put_u8(info.flags as u8);
        bytes.put_u64(info.timestamp);

        unsafe {
            bytes.set_len(size);
        }

        let hash = xxh3_64(&bytes[8..]);
        bytes[0..8].copy_from_slice(&hash.to_be_bytes());
        bytes.freeze()
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct UnPackage;

impl UnPackage {
    pub fn unpack(mut bytes: Bytes) -> Option<(PacketInfo, Bytes)> {
        let count = bytes.len();
        if bytes.get_u64() == xxh3_64(&bytes) {
            if bytes.get_u64() as usize == count {
                Some((
                    PacketInfo {
                        kind: StreamKind::try_from(bytes.get_u8()).ok()?,
                        flags: bytes.get_u8() as i32,
                        timestamp: bytes.get_u64(),
                    },
                    bytes,
                ))
            } else {
                None
            }
        } else {
            None
        }
    }
}
