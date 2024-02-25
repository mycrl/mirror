use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Weak},
    time::Duration,
};

use async_trait::async_trait;
use bytes::Bytes;
use clap::Parser;
use srt::SrtOptions;
use tokio::{io::AsyncWriteExt, process::Command, time::sleep};
use transport::{
    adapter::{
        ReceiverAdapterFactory, StreamBufferInfo, StreamReceiverAdapter, StreamSenderAdapter,
    },
    Transport, TransportOptions,
};

struct SimpleReceiverAdapterFactory;

#[async_trait]
impl ReceiverAdapterFactory for SimpleReceiverAdapterFactory {
    async fn connect(
        &self,
        _id: u8,
        _ip: IpAddr,
        _description: &[u8],
    ) -> Option<Weak<StreamReceiverAdapter>> {
        let child = Command::new("ffplay")
            .args(&["-i", "pipe:0"])
            .spawn()
            .ok()?;

        let adapter = StreamReceiverAdapter::new();
        let adapter_ = Arc::downgrade(&adapter);
        tokio::spawn(async move {
            if let Some(mut stdin) = child.stdin {
                while let Some((buf, _kind)) = adapter.next().await {
                    if stdin.write_all(&buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        Some(adapter_)
    }
}

#[derive(Parser, Clone)]
#[command(
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
)]
struct Args {
    #[arg(long)]
    addr: SocketAddr,
    #[arg(long)]
    kind: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    simple_logger::init_with_level(log::Level::Info).unwrap();

    let args = Args::parse();
    let transport = Transport::new(
        TransportOptions {
            srt: SrtOptions::default(),
            bind: args.addr,
        },
        Some(SimpleReceiverAdapterFactory),
    )
    .await?;

    if args.kind == "client" {
        let adapter = StreamSenderAdapter::new();
        transport.create_sender(0, vec![], &adapter).await?;

        let buf = Bytes::from_static(&[0u8; 3000]);
        loop {
            sleep(Duration::from_millis(100)).await;
            if !adapter.send(buf.clone(), StreamBufferInfo::Video(0)) {
                break;
            }
        }
    } else {
        std::future::pending::<()>().await;
        drop(transport);
    }

    Ok(())
}
