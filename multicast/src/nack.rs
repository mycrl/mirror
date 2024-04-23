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
    queue: Arc<RwLock<BTreeMap<u16, (Bytes, Instant)>>>,
    latency: Arc<AtomicU64>,
    time: Instant,
    delay: usize,
}

impl Dequeue {
    pub fn new<F>(delay: usize, nack: F) -> Self
    where
        F: Fn(Range<u16>) + Send + 'static,
    {
        let latency = Arc::new(AtomicU64::new(delay as u64 / 2));
        let queue: Arc<RwLock<BTreeMap<u16, (Bytes, Instant)>>> =
            Arc::new(RwLock::new(BTreeMap::new()));

        let queue_ = Arc::downgrade(&queue);
        let latency_ = Arc::downgrade(&latency);
        thread::spawn(move || {
            while let (Some(queue), Some(latency)) = (queue_.upgrade(), latency_.upgrade()) {
                thread::sleep(Duration::from_millis(latency.get()));

                let mut index = 0;
                let mut range = 0..0;
                let mut loss = false;
                for seq in queue.read().unwrap().keys() {
                    if index + 1 == *seq {
                        if loss {
                            range.end = index;
                            break;
                        }
                    } else {
                        if !loss {
                            range.start = *seq;
                            loss = true;
                        }
                    }

                    index = *seq;
                }

                if loss {
                    nack(range);
                }
            }
        });

        Self {
            time: Instant::now(),
            latency,
            queue,
            delay,
        }
    }

    pub fn get_time(&self) -> u64 {
        self.time.elapsed().as_millis() as u64
    }

    pub fn update(&self, time: u64) {
        self.latency
            .update((self.time.elapsed().as_millis() as u64 - time) / 2);
    }

    pub fn push(&self, sequence: u16, bytes: Bytes) {
        if !self.queue.read().unwrap().contains_key(&sequence) {
            self.queue
                .write()
                .unwrap()
                .insert(sequence, (bytes, Instant::now()));
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
    queue: Mutex<BTreeMap<u16, (Bytes, Instant)>>,
    delay: usize,
}

impl Queue {
    pub fn new(delay: usize) -> Self {
        Self {
            queue: Default::default(),
            delay,
        }
    }

    pub fn push(&self, sequence: u16, bytes: Bytes) {
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

    pub fn get(&self, sequence: u16) -> Option<Bytes> {
        self.queue
            .lock()
            .unwrap()
            .get(&sequence)
            .map(|(bytes, _)| bytes.clone())
    }
}
