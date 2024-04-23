use std::{
    thread::{self, sleep},
    time::Duration,
};

use multicast::{Receiver, Sender};

fn main() -> anyhow::Result<()> {
    thread::spawn(|| {
        let mut index: i32 = -1;
        let receiver = Receiver::new("239.0.0.2".parse()?, "0.0.0.0:8080".parse()?)?;
        while let Ok(packet) = receiver.read() {
            let seq = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);

            if index + 1 != seq as i32 {
                println!("packet loss, seq={}", seq)
            } else {
                println!("recv packet, seq={}", seq)
            }

            index = seq as i32;
        }

        Ok::<(), anyhow::Error>(())
    });

    let mut sender = Sender::new("239.0.0.2".parse()?, "0.0.0.0:8080".parse()?, 1400)?;
    let mut buf = [0u8; 1000];
    let mut index: u32 = 0;
    loop {
        (&mut buf[..4]).copy_from_slice(&index.to_be_bytes());
        sender.send(&buf)?;
        println!("send packet, seq={}", index);

        sleep(Duration::from_millis(5));
        index += 1;
    }
}
