use std::{
    collections::HashMap,
    sync::{
        atomic::AtomicUsize,
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock,
    },
};

use common::atomic::EasyAtomic;
use smallvec::SmallVec;

use crate::signal::Signal;

#[derive(Default)]
pub struct Route {
    index: AtomicUsize,
    nodes: RwLock<HashMap<u32, u16>>,
    channels: Arc<RwLock<HashMap<usize, Sender<Signal>>>>,
}

impl Route {
    pub fn add(&self, id: u32, port: u16) {
        self.nodes.write().unwrap().insert(id, port);
        self.change(Signal::Start { id, port })
    }

    pub fn remove(&self, id: u32) {
        if self.nodes.write().unwrap().remove(&id).is_some() {
            self.change(Signal::Stop { id })
        }
    }

    pub fn get_channels(&self) -> Vec<(u32, u16)> {
        self.nodes
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect()
    }

    pub fn get_changer(&self) -> Changer {
        let (tx, rx) = channel();
        let id = self.index.get();

        {
            self.channels.write().unwrap().insert(id, tx);
            self.index.update(if id == usize::MAX { 0 } else { id + 1 });
        }

        Changer {
            channels: self.channels.clone(),
            rx,
            id,
        }
    }

    fn change(&self, signal: Signal) {
        let mut closeds: SmallVec<[usize; 10]> = SmallVec::with_capacity(10);

        {
            let channels = self.channels.read().unwrap();
            for (id, tx) in channels.iter() {
                if tx.send(signal).is_err() {
                    closeds.push(*id);
                }
            }
        }

        if !closeds.is_empty() {
            let mut channels = self.channels.write().unwrap();
            for id in closeds {
                if let Some(tx) = channels.remove(&id) {
                    drop(tx)
                }
            }
        }
    }
}

pub struct Changer {
    channels: Arc<RwLock<HashMap<usize, Sender<Signal>>>>,
    rx: Receiver<Signal>,
    id: usize,
}

impl Changer {
    pub fn change(&self) -> Option<Signal> {
        self.rx.recv().ok()
    }
}

impl Drop for Changer {
    fn drop(&mut self) {
        if let Some(tx) = self.channels.write().unwrap().remove(&self.id) {
            drop(tx)
        }
    }
}
