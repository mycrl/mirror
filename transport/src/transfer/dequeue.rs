use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicU64, Arc, RwLock},
    time::Instant,
};

use bytes::Bytes;
use common::atomic::EasyAtomic;

pub struct Dequeue {
    queue: Arc<RwLock<BTreeMap<u64, (Bytes, Instant)>>>,
    rtt: Arc<AtomicU64>,
    time: Instant,
    delay: usize,
}

impl Dequeue {
    pub fn new(delay: usize) -> Self {
        let rtt = Arc::new(AtomicU64::new(delay as u64 / 2));
        let queue: Arc<RwLock<BTreeMap<u64, (Bytes, Instant)>>> =
            Arc::new(RwLock::new(BTreeMap::new()));

        Self {
            time: Instant::now(),
            rtt,
            queue,
            delay,
        }
    }

    pub fn get_time(&self) -> u64 {
        self.time.elapsed().as_millis() as u64
    }

    pub fn update(&self, time: u64) {
        let rtt = self.time.elapsed().as_millis() as u64 - time;
        self.rtt.update(rtt);

        log::info!("Network latency detection, rtt={}", rtt);
    }

    pub fn push(&self, sequence: u64, bytes: Bytes) {
        if !self.queue.read().unwrap().contains_key(&sequence) {
            self.queue
                .write()
                .unwrap()
                .insert(sequence, (bytes, Instant::now()));
        } else {
            log::info!(
                "The retransmission packet is received, sequence={:?}",
                sequence
            );
        }
    }

    pub fn pop(&self) -> Option<Bytes> {
        let mut sequence = None;
        if let Some((seq, (_, time))) = self.queue.read().unwrap().first_key_value() {
            if time.elapsed().as_millis() as usize >= self.delay {
                sequence.replace(*seq);
            }
        }

        sequence.and_then(|seq| {
            self.queue
                .write()
                .unwrap()
                .remove(&seq)
                .map(|(bytes, _)| bytes)
        })
    }
}
