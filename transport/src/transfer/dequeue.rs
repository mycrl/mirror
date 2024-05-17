use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicU64, Arc, RwLock},
    time::Instant,
};

use common::atomic::EasyAtomic;

use super::packet::Packet;

/// Packet reordering queue.
pub struct Dequeue {
    queue: Arc<RwLock<BTreeMap<u64, (Packet, Instant)>>>,
    last_queue: AtomicU64,
    delay: usize,
}

impl Dequeue {
    pub fn new(delay: usize) -> Self {
        Self {
            queue: Arc::new(RwLock::new(BTreeMap::new())),
            last_queue: AtomicU64::new(0),
            delay,
        }
    }

    /// Add a data packet to the queue, and the queue will sort all the data
    /// packets from small to large according to the sequence number.
    ///
    /// It should be noted that you can ignore the order or whether there are
    /// duplicates, and the internal processing can be normal.
    pub fn push(&self, packet: Packet) {
        // Check whether the current sequence number has been dequeued. If so, do not
        // process it.
        let last_seq = self.last_queue.get();
        if !(last_seq >= u64::MAX - 100 && packet.sequence <= 100) && last_seq >= packet.sequence {
            return;
        }

        // To avoid duplicate insertion, check here first.
        if !self.queue.read().unwrap().contains_key(&packet.sequence) {
            self.queue
                .write()
                .unwrap()
                .insert(packet.sequence, (packet, Instant::now()));
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
    pub fn pop(&self) -> Option<Packet> {
        // Get the packet with the smallest sequence number in the queue and check
        // whether it has timed out.
        let mut sequence = None;
        if let Some((seq, (_, time))) = self.queue.read().unwrap().first_key_value() {
            if time.elapsed().as_millis() as usize >= self.delay {
                self.last_queue.update(*seq);
                sequence.replace(*seq);
            }
        }

        sequence.and_then(|seq| {
            self.queue
                .write()
                .unwrap()
                .remove(&seq)
                .map(|(packet, _)| packet)
        })
    }
}
