use std::{
    collections::BTreeMap,
    ops::Range,
    sync::{atomic::AtomicU64, Arc, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
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
    pub fn new<F>(delay: usize, nack: F) -> Self
    where
        F: Fn(Range<u64>) + Send + 'static,
    {
        let rtt = Arc::new(AtomicU64::new(delay as u64 / 2));
        let queue: Arc<RwLock<BTreeMap<u64, (Bytes, Instant)>>> =
            Arc::new(RwLock::new(BTreeMap::new()));

        let queue_ = Arc::downgrade(&queue);
        let rtt_ = Arc::downgrade(&rtt);
        thread::spawn(move || {
            while let (Some(queue), Some(rtt)) = (queue_.upgrade(), rtt_.upgrade()) {
                thread::sleep(Duration::from_millis(rtt.get()));

                let mut index: i64 = -1;
                let mut range = 0..0;
                let mut loss = false;
                for seq in queue.read().unwrap().keys() {
                    let seq = *seq as i64;
                    if index != -1 {
                        if index + 1 == seq {
                            if loss {
                                range.end = index as u64;
                                if range.start == range.end {
                                    loss = false;
                                }

                                break;
                            }
                        } else {
                            if !loss {
                                range.start = seq as u64;
                                loss = true;
                            }
                        }
                    }

                    index = seq;
                }

                if loss {
                    nack(range);
                }
            }
        });

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

        sequence
            .map(|seq| {
                self.queue
                    .write()
                    .unwrap()
                    .remove(&seq)
                    .map(|(bytes, _)| bytes)
            })
            .flatten()
    }
}

pub struct Queue {
    queue: Mutex<BTreeMap<u64, (Bytes, Instant)>>,
    delay: usize,
}

impl Queue {
    pub fn new(delay: usize) -> Self {
        Self {
            queue: Default::default(),
            delay,
        }
    }

    pub fn push(&self, sequence: u64, bytes: Bytes) {
        let mut queue = self.queue.lock().unwrap();

        queue.insert(sequence, (bytes, Instant::now()));

        while let Some(item) = queue.first_entry() {
            if item.get().1.elapsed().as_millis() as usize >= self.delay {
                item.remove();
            } else {
                break;
            }
        }
    }

    pub fn get(&self, sequence: u64) -> Option<Bytes> {
        self.queue
            .lock()
            .unwrap()
            .get(&sequence)
            .map(|(bytes, _)| bytes.clone())
    }
}
