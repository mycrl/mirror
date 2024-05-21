use std::{
    thread::{self, sleep},
    time::Duration,
};

use rist::{Receiver, Sender};

fn main() -> anyhow::Result<()> {
    let mut sender = Sender::new("239.0.0.1:8084".parse()?)?;
    let receiver = Receiver::new("239.0.0.1:8084".parse()?)?;

    thread::spawn(move || {
        while let Some(packet) = receiver.read() {
            println!("size={}", packet.as_slice().len())
        }
    });

    let buf = [0u8; 1000];
    loop {
        sender.send(&buf)?;
        sleep(Duration::from_secs(1));
    }
}
