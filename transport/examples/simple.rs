use std::{
    io::Write,
    net::SocketAddr,
    process::{Command, Stdio},
    sync::{Arc, Weak},
    thread::{self, sleep},
    time::Duration,
};

use bytes::Bytes;
use clap::Parser;
use transport::{
    adapter::{
        ReceiverAdapterFactory, StreamBufferInfo, StreamKind, StreamReceiverAdapter,
        StreamSenderAdapter,
    },
    Transport, TransportOptions,
};

struct SimpleReceiverAdapterFactory;

impl ReceiverAdapterFactory for SimpleReceiverAdapterFactory {
    fn connect(
        &self,
        _id: u8,
        _ip: SocketAddr,
        _description: &[u8],
    ) -> Option<Weak<StreamReceiverAdapter>> {
        let adapter = StreamReceiverAdapter::new();
        let adapter_ = Arc::downgrade(&adapter);
        thread::spawn(move || {
            let child = Command::new("ffplay")
                .args(&[
                    "-vcodec",
                    "h264",
                    "-fflags",
                    "nobuffer",
                    "-flags",
                    "low_delay",
                    "-framedrop",
                    "-i",
                    "pipe:0",
                ])
                .stdin(Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin {
                while let Some((buf, kind, _)) = adapter.next() {
                    if kind == StreamKind::Video {
                        if let Err(e) = stdin.write_all(&buf) {
                            println!("{:?}", e);
                            break;
                        }
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
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

fn main() -> anyhow::Result<()> {
    simple_logger::init_with_level(log::Level::Info).unwrap();

    let mut args = Args::parse();
    let transport = Transport::new(
        "239.0.0.1".parse()?,
        Some(TransportOptions {
            adapter_factory: SimpleReceiverAdapterFactory,
            bind: args.addr,
        }),
    )?;

    if args.kind == "client" {
        args.addr.set_port(args.addr.port() + 1);
        let adapter = StreamSenderAdapter::new();
        transport.create_sender(0, 1500, args.addr, vec![], &adapter)?;

        let buf = Bytes::from_static(&[0u8; 3000]);
        loop {
            sleep(Duration::from_millis(100));
            if !adapter.send(buf.clone(), StreamBufferInfo::Video(0, 0)) {
                break;
            }
        }
    } else {
        sleep(Duration::from_secs(9999));
        drop(transport);
    }

    Ok(())
}
