use crate::adapter::StreamKind;

use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Because of the need to transmit both audio and video data in srt, it is
/// necessary to identify the type of packet, this encoder is used to packetize
/// specific types of data for transmission over the network.
#[derive(Default)]
pub struct Encoder(Vec<u8>);

impl Encoder {
    /// The result of the encoding may be null, this is because an empty packet
    /// may be passed in from outside.
    pub fn encode(&mut self, kind: StreamKind, buf: &[u8]) -> Option<&[u8]> {
        self.0.clear();
        if buf.len() == 0 {
            return None;
        }

        self.0.put_u8(kind as u8);
        self.0.put_u32(buf.len() as u32);
        self.0.put(buf);
        Some(&self.0[..])
    }
}

/// Decode the packets received from the network and separate out the different
/// types of data.
#[derive(Default)]
pub struct Decoder(BytesMut);

impl Decoder {
    pub fn decode(&mut self, buf: &[u8]) -> Vec<(Bytes, StreamKind)> {
        self.0.put(buf);
        let mut ret = Vec::with_capacity(5);

        loop {
            if self.0.len() < 5 {
                break;
            }

            // Gets the length of the current packet data and checks that the packet
            // contents arrived in full.
            let size = u32::from_be_bytes(self.0[1..5].try_into().unwrap()) as usize;
            // log::info!("payload decoder chunk size={}", size);
            if size + 5 > self.0.len() {
                break;
            }

            // Gets the type of the packet and consumes the already read header in
            // the buffer.
            let kind = StreamKind::try_from(self.0[0]).unwrap();
            let _ = self.0.get_uint(5);

            ret.push((self.0.split_to(size).freeze(), kind));
        }

        ret
    }
}
