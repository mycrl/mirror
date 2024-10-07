use bytes::{Buf, BufMut, Bytes, BytesMut};
use xxhash_rust::xxh3::xxh3_64;

pub struct Fragment {
    pub chunk_sequence: u64,
    pub sequence: u64,
    pub size: usize,
    pub bytes: Bytes,
}

impl TryFrom<&[u8]> for Fragment {
    type Error = std::io::Error;

    fn try_from(mut bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.get_u64() == xxh3_64(bytes) {
            Ok(Self {
                chunk_sequence: bytes.get_u64(),
                sequence: bytes.get_u64(),
                size: bytes.get_u32() as usize,
                bytes: Bytes::copy_from_slice(bytes),
            })
        } else {
            Err(std::io::Error::other("invalid data"))
        }
    }
}

pub struct FragmentEncoder {
    packets: Vec<BytesMut>,
    chunk_sequence: u64,
    sequence: u64,
    mtu: usize,
}

impl FragmentEncoder {
    pub fn new(mtu: usize) -> Self {
        Self {
            packets: Default::default(),
            chunk_sequence: 0,
            sequence: 0,
            mtu,
        }
    }

    pub fn encode(&mut self, bytes: &[u8]) -> &[BytesMut] {
        let mut size = 0;
        for (i, chunk) in bytes.chunks(self.mtu - 28).enumerate() {
            {
                if self.packets.get(i).is_none() {
                    self.packets.push(BytesMut::with_capacity(self.mtu));
                }
            }

            if let Some(buf) = self.packets.get_mut(i) {
                buf.clear();

                buf.put_u64(0);
                buf.put_u64(self.chunk_sequence);
                buf.put_u64(self.sequence);
                buf.put_u32(bytes.len() as u32);
                buf.put(chunk);

                let hash = xxh3_64(&buf[8..]);
                buf[0..8].copy_from_slice(&hash.to_be_bytes());

                size += 1;

                self.chunk_sequence = if self.chunk_sequence == u64::MAX {
                    0
                } else {
                    self.chunk_sequence + 1
                };
            }
        }

        self.sequence = if self.sequence == u64::MAX {
            0
        } else {
            self.sequence + 1
        };

        &self.packets[..size]
    }
}

pub struct FragmentDecoder {
    bytes: BytesMut,
    sequence: i128,
    size: usize,
}

impl FragmentDecoder {
    pub fn new() -> Self {
        Self {
            bytes: BytesMut::with_capacity(1024 * 1024),
            sequence: -1,
            size: 0,
        }
    }

    pub fn decode(&mut self, chunk: Fragment) -> Option<(u64, Bytes)> {
        let mut result = None;
        if chunk.sequence as i128 != self.sequence {
            if !self.bytes.is_empty() && self.bytes.len() >= self.size {
                result = Some((
                    self.sequence as u64,
                    Bytes::copy_from_slice(&self.bytes[..self.size]),
                ));
            }

            self.bytes.clear();
        }

        self.sequence = chunk.sequence as i128;
        self.size = chunk.size;

        self.bytes.put(chunk.bytes);

        result
    }
}
