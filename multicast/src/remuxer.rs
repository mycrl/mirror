use std::{collections::BTreeMap, time::Instant};

/// Unmixed packets.
///
/// Reordering of packets is included internally, but introduces some fixed
/// delays.
pub struct Remuxer {
    packets: BTreeMap<u16, (Vec<u8>, Instant)>,
    remove_keys: Vec<u16>,
    dequeue: Vec<Vec<u8>>,
    timeout: usize,

    seq: u16,
}

impl Remuxer {
    /// Creates a remuxer and specifies a packet transmission delay.
    pub fn new(timeout: usize) -> Self {
        Self {
            remove_keys: Vec::with_capacity(10),
            dequeue: Vec::with_capacity(10),
            packets: BTreeMap::new(),
            timeout,

            seq: 0,
        }
    }

    /// Processes incoming packets and returns all packets that have been
    /// sorted.
    ///
    /// Note that this function reorders received packets.
    pub fn remux(&mut self, buf: &[u8]) -> &[Vec<u8>] {
        assert!(!buf.is_empty());

        // The received packets are written to the b-tree by sequence number, and the
        // b-tree data structure automatically sorts the packets. Mark the current time
        // for each packet when writing to the b-tree.
        self.packets.insert(
            u16::from_be_bytes([buf[0], buf[1]]),
            ((&buf[2..]).to_vec(), Instant::now()),
        );

        if !self.dequeue.is_empty() {
            self.dequeue.clear();
        }

        if !self.remove_keys.is_empty() {
            self.remove_keys.clear();
        }

        // Scan all timeout packets inside the b-tree in order of sequence number from
        // smallest to largest.
        for (seq, (_, time)) in self.packets.iter() {
            if time.elapsed().as_millis() as usize >= self.timeout {
                self.remove_keys.push(*seq);
            } else {
                break;
            }
        }

        // Transfer all timeout packets to the outgoing queue.
        for seq in &self.remove_keys {
            if self.seq + 1 != *seq {
                log::info!("packet loss, old seq={}, seq={}", self.seq, seq);
            }

            self.seq = *seq;

            if let Some((packet, _)) = self.packets.remove(seq) {
                self.dequeue.push(packet);
            }
        }

        &self.dequeue
    }
}
