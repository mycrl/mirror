use std::{
    thread::{self, sleep},
    time::Duration,
};

use rand::prelude::*;
use rist::{Receiver, Sender};

fn main() -> anyhow::Result<()> {
    let mut sender = Sender::new("127.0.0.1:8084".parse()?)?;
    let receiver = Receiver::new("127.0.0.1:8084".parse()?)?;

    thread::spawn(move || {
        while let Some(packet) = receiver.read() {
            println!("size={}", packet.as_slice().len())
        }
    });

    let mut rng = rand::thread_rng();
    let mut buf = vec![0u8; 1000];
    loop {
        buf.shuffle(&mut rng);
        sender.send(&buf)?;
        sleep(Duration::from_secs(1));
    }
}
