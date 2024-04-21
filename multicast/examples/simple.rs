use std::{
    thread::{self, sleep},
    time::Duration,
};

use bytes::Bytes;
use multicast::{Receiver, Sender};

fn main() -> anyhow::Result<()> {
    thread::spawn(|| {
        let mut index: i32 = -1;
        let mut receiver = Receiver::new("239.0.0.1".parse()?, "0.0.0.0:8080".parse()?, 20)?;
        while let Ok(packet) = receiver.read() {
            let seq = u32::from_be_bytes([
                packet[0],
                packet[1],
                packet[2],
                packet[3],
            ]);

            if index + 1 == seq as i32 {
                println!("packet loss, seq={}", seq)
            }

            index = seq as i32;
        }

        Ok::<(), anyhow::Error>(())
    });

    let mut sender = Sender::new("239.0.0.1".parse()?, "0.0.0.0:8080".parse()?, 1500)?;
    let mut buf = [0u8; 1000];
    let mut index: u32 = 0;
    loop {
        (&mut buf[..4]).copy_from_slice(&index.to_be_bytes());
        sender.send(Bytes::copy_from_slice(&buf))?;
        sleep(Duration::from_secs(1));
        index += 1;
    }
}
