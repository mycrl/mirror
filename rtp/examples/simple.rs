use std::{thread, time::Duration};

use anyhow::Result;
use futures::StreamExt;
use rtp::*;

#[tokio::main]
async fn main() -> Result<()> {
    // thread::spawn(|| {
        let sender = RtpSender::new(RtpConfig {
            dest: "239.0.0.1:8086".parse()?,
            bind: "0.0.0.0:0".parse()?,
        })?;

        loop {
            let buf = [0u8; 1000];
            sender.send(&buf)?;
            thread::sleep(Duration::from_millis(50))
        }

        // #[allow(unused)]
        // Ok::<(), anyhow::Error>(())
    // });

    // let mut receiver = RtpReceiver::new(RtpConfig {
    //     dest: "224.0.0.100:8082".parse()?,
    //     bind: "0.0.0.0:0".parse()?,
    // })?;

    // while let Some(pkt) = receiver.next().await {
    //     println!("{}", pkt.as_bytes().len())
    // }

    Ok(())
}
