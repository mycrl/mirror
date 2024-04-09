use std::time::Duration;

use broadcast::{Receiver, Sender};
use tokio::time;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tokio::spawn(async {
        let mut receiver = Receiver::new("239.0.0.1".parse()?, "0.0.0.0:8080".parse()?).await?;
        while let Ok(packets) = receiver.read().await {
            for packet in packets {
                println!("{}", packet.len())
            }
        }

        Ok::<(), anyhow::Error>(())
    });

    let mut sender = Sender::new("239.0.0.1".parse()?, "0.0.0.0:8080".parse()?, 1500).await?;
    let buf = [0u8; 1000];
    loop {
        sender.send(&buf).await?;
        time::sleep(Duration::from_secs(1)).await;
    }
}
