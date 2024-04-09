use std::{collections::BTreeMap, time::Instant};

use bytes::{BufMut, BytesMut};

pub struct PacketMuxer {
    buffer: BytesMut,
    seq_number: u16,
    mtu: usize,
}

impl PacketMuxer {
    pub fn new(mtu: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(mtu),
            seq_number: 0,
            mtu,
        }
    }

    pub fn max_payload_size(&self) -> usize {
        self.mtu - 2
    }

    pub fn mux(&mut self, buf: &[u8]) -> &[u8] {
        assert!(!buf.is_empty());

        self.buffer.clear();

        self.buffer.put_u16(self.seq_number);
        self.buffer.put(buf);

        if self.seq_number == u16::MAX {
            self.seq_number = 0
        } else {
            self.seq_number += 1;
        }

        &self.buffer[..]
    }
}

struct Packet {
    payload: Vec<u8>,
    time: Instant,
}

impl Packet {
    fn new(payload: Vec<u8>) -> Self {
        Self {
            time: Instant::now(),
            payload,
        }
    }

    fn expired(&self, timeout: usize) -> bool {
        self.time.elapsed().as_millis() as usize >= timeout
    }
}

pub struct PakcetRemuxer {
    packets: BTreeMap<u16, Packet>,
    remove_keys: Vec<u16>,
    dequeue: Vec<Vec<u8>>,
    timeout: usize,
}

impl PakcetRemuxer {
    pub fn new(timeout: usize) -> Self {
        Self {
            remove_keys: Vec::with_capacity(10),
            dequeue: Vec::with_capacity(10),
            packets: BTreeMap::new(),
            timeout,
        }
    }

    pub fn remux(&mut self, buf: &[u8]) -> &[Vec<u8>] {
        assert!(!buf.is_empty());

        self.packets.insert(
            u16::from_be_bytes([buf[0], buf[1]]),
            Packet::new((&buf[2..]).to_vec()),
        );

        if !self.dequeue.is_empty() {
            self.dequeue.clear();
        }

        if !self.remove_keys.is_empty() {
            self.remove_keys.clear();
        }

        for (seq, packet) in self.packets.iter() {
            if packet.expired(self.timeout) {
                self.remove_keys.push(*seq);
            } else {
                break;
            }
        }

        for seq in &self.remove_keys {
            if let Some(packet) = self.packets.remove(seq) {
                self.dequeue.push(packet.payload);
            }
        }

        &self.dequeue
    }
}
