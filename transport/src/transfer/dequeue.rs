use std::{collections::BTreeMap, time::Instant};

use super::packet::Packet;

/// Packet reordering queue.
pub struct Dequeue {
    queue: BTreeMap<u64, (Packet, Instant)>,
    last_queue: u64,
    delay: usize,
}

impl Dequeue {
    pub fn new(delay: usize) -> Self {
        Self {
            queue: BTreeMap::new(),
            last_queue: 0,
            delay,
        }
    }

    /// Add a data packet to the queue, and the queue will sort all the data
    /// packets from small to large according to the sequence number.
    ///
    /// It should be noted that you can ignore the order or whether there are
    /// duplicates, and the internal processing can be normal.
    pub fn push(&mut self, packet: Packet) {
        // Check whether the current sequence number has been dequeued. If so, do not
        // process it.
        if !(self.last_queue >= u64::MAX - 100 && packet.sequence <= 100)
            && self.last_queue >= packet.sequence
        {
            return;
        }

        // To avoid duplicate insertion, check here first.
        if !self.queue.contains_key(&packet.sequence) {
            self.queue.insert(packet.sequence, (packet, Instant::now()));
        } else {
            log::info!(
                "The retransmission packet is received, sequence={:?}",
                packet.sequence
            );
        }
    }

    /// According to the set delay, the data packets are taken out from the
    /// queue in order. You can try to take them out multiple times until there
    /// is no result.
    #[rustfmt::skip]
    pub fn pop(&mut self) -> Option<Packet> {
        // Get the packet with the smallest sequence number in the queue and check
        // whether it has timed out.
        let mut sequence = None;
        if let Some((seq, (_, time))) = self.queue.first_key_value() {
            if time.elapsed().as_millis() as usize >= self.delay {
                sequence.replace(*seq);
                self.last_queue = *seq;
            }
        }

        sequence.and_then(|seq| {
            self.queue.remove(&seq).map(|(packet, _)| packet)
        })
    }
}
