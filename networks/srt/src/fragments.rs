use bytes::{Buf, BufMut, Bytes, BytesMut};
use xxhash_rust::xxh3::xxh3_64;

pub struct FragmentEncoder {
    packets: Vec<BytesMut>,
    sequence: u64,
    mtu: usize,
}

impl FragmentEncoder {
    pub fn new(mtu: usize) -> Self {
        Self {
            packets: Default::default(),
            sequence: 0,
            mtu,
        }
    }

    pub fn encode(&mut self, bytes: &[u8]) -> &[BytesMut] {
        let mut size = 0;
        for (i, chunk) in bytes.chunks(self.mtu - 20).enumerate() {
            {
                if self.packets.get(i).is_none() {
                    self.packets.push(BytesMut::with_capacity(self.mtu));
                }
            }

            if let Some(buf) = self.packets.get_mut(i) {
                buf.clear();

                buf.put_u64(0);
                buf.put_u64(self.sequence);
                buf.put_u32(bytes.len() as u32);
                buf.put(chunk);

                let hash = xxh3_64(&buf[8..]);
                buf[0..8].copy_from_slice(&hash.to_be_bytes());

                size += 1;
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
            bytes: BytesMut::new(),
            sequence: -1,
            size: 0,
        }
    }

    pub fn decode(&mut self, mut bytes: &[u8]) -> Option<(u64, Bytes)> {
        let mut result = None;

        if bytes.get_u64() == xxh3_64(bytes) {
            let sequence = bytes.get_u64();
            let size = bytes.get_u32() as usize;
            if sequence as i128 != self.sequence {
                if !self.bytes.is_empty() && self.bytes.len() >= self.size {
                    result = Some((
                        self.sequence as u64,
                        Bytes::copy_from_slice(&self.bytes[..self.size]),
                    ));
                }

                self.bytes.clear();
            }

            self.sequence = sequence as i128;
            self.size = size;

            self.bytes.put(bytes);
        }

        result
    }
}
