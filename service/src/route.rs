use std::collections::HashMap;

use mirror_transport::Signal;
use parking_lot::RwLock;
use tokio::sync::broadcast::{channel, Receiver, Sender};

pub struct Route {
    nodes: RwLock<HashMap<u32, u16>>,
    tx: Sender<Signal>,
    rx: Receiver<Signal>,
}

impl Default for Route {
    fn default() -> Self {
        let (tx, rx) = channel(20);
        Self {
            nodes: RwLock::new(HashMap::with_capacity(255)),
            tx,
            rx,
        }
    }
}

impl Route {
    /// Send a ping to all clients.
    pub fn ping(&self) {
        self.tx.send(Signal::Ping).unwrap();
    }

    /// Add a channel to the route, where the port number is the multicast port
    /// on the sender side
    ///
    /// This will trigger an event update, which will broadcast a channel
    /// release event
    pub fn add(&self, id: u32, port: u16) {
        self.nodes.write().insert(id, port);
        self.tx.send(Signal::Start { id, port }).unwrap();
    }

    /// Delete a published channel
    ///
    /// This will trigger an event update, which will broadcast a channel closed
    /// event
    pub fn remove(&self, id: u32) {
        if self.nodes.write().remove(&id).is_some() {
            self.tx.send(Signal::Stop { id }).unwrap();
        }
    }

    /// Get all channels that are publishing
    pub fn get_channels(&self) -> Vec<(u32, u16)> {
        self.nodes.read().iter().map(|(k, v)| (*k, *v)).collect()
    }

    /// Get the event update listener, which can listen to all subsequent events
    /// triggered from the current listener
    pub fn get_changer(&self) -> Receiver<Signal> {
        self.rx.resubscribe()
    }
}
