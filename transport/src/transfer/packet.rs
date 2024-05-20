use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
pub struct PacketEncoder {
    packets: Vec<BytesMut>,
    max_size: usize,
    group_number: u16,
    sequence: u64,
}

impl PacketEncoder {
    pub fn new(max_size: usize) -> Self {
        Self {
            packets: Default::default(),
            max_size: max_size - 18,
            group_number: 0,
            sequence: 0,
        }
    }

    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn encode(&mut self, bytes: &[u8]) -> &[BytesMut] {
        if bytes.is_empty() {
            return &[];
        }

        let mut size = 0;
        for (i, chunk) in bytes.chunks(self.max_size).enumerate() {
            {
                if self.packets.get(i).is_none() {
                    self.packets
                        .push(BytesMut::with_capacity(self.max_size * 2));
                }
            }

            if let Some(buf) = self.packets.get_mut(i) {
                buf.clear();

                // crc check header.
                buf.put_u32(0);
                buf.put_u64(self.sequence);
                buf.put_u16(self.group_number);
                buf.put_u32(bytes.len() as u32);
                buf.put(chunk);

                // calculate crc.
                let crc = fingerprint(&buf[4..]);
                buf[0..4].copy_from_slice(&crc.to_be_bytes());

                size += 1;
                if self.sequence == u64::MAX {
                    self.sequence = 0;
                } else {
                    self.sequence += 1;
                }
            }
        }

        self.group_number = if self.group_number == u16::MAX {
            0
        } else {
            self.group_number + 1
        };

        &self.packets[..size]
    }
}

/// Description of a packet in the transport layer.
pub struct Packet {
    pub sequence: u64,
    pub group_number: u16,
    pub len: usize,
    pub bytes: Bytes,
}

impl Packet {
    /// Try to parse a packet from a raw binary stream.
    pub fn try_from(mut bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 18 {
            if bytes.get_u32() == fingerprint(bytes) {
                Some(Self {
                    sequence: bytes.get_u64(),
                    group_number: bytes.get_u16(),
                    len: bytes.get_u32() as usize,
                    bytes: Bytes::copy_from_slice(bytes),
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
pub struct PacketDecoder {
    group_number: i32,
    bytes: BytesMut,
    length: usize,
}

impl PacketDecoder {
    pub fn new() -> Self {
        Self {
            bytes: BytesMut::with_capacity(1024 * 1024),
            group_number: -1,
            length: 0,
        }
    }

    pub fn decode(&mut self, packet: Packet) -> Option<Bytes> {
        let mut results = None;
        if packet.group_number as i32 != self.group_number {
            if !self.bytes.is_empty() && self.bytes.len() >= self.length {
                results = Some(Bytes::copy_from_slice(&self.bytes[..self.length]));
            }

            self.bytes.clear();
        }

        self.bytes.put(packet.bytes);
        self.group_number = packet.group_number as i32;
        self.length = packet.len;
        results
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
