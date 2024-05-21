use std::{
    thread::{self, sleep},
    time::Duration,
};

use rist::{Receiver, Sender};

fn main() -> anyhow::Result<()> {
    thread::spawn(|| {
        let receiver = Receiver::new("239.0.0.1:8084".parse()?)?;
        while let Some(packet) = receiver.read() {
            println!("size={}", packet.as_slice().len())
        }

        Ok::<(), anyhow::Error>(())
    });

    let mut sender = Sender::new("239.0.0.1:8084".parse()?)?;
    let buf = [0u8; 1000];
    loop {
        sender.send(&buf)?;
        sleep(Duration::from_secs(1));
    }
}
