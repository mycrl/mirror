use std::time::Duration;

use broadcast::{Receiver, Sender, SenderOptions};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tokio::spawn(async {
        let mut receiver = Receiver::new("0.0.0.0:8080".parse()?).await?;
        while let Ok(packets) = receiver.read().await {
            for packet in packets {
                println!("{}", packet.len())
            }
        }

        Ok::<(), anyhow::Error>(())
    });

    let mut sender = Sender::new(SenderOptions {
        bind: "0.0.0.0:0".parse()?,
        mtu: 1500,
        to: 8080,
    })
    .await?;

    let buf = [0u8; 1000];
    loop {
        sender.send(&buf).await?;
        sleep(Duration::from_secs(1)).await;
    }
}
