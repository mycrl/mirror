use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use clap::Parser;
use srt::{cleanup, startup, Listener, Socket, SrtOptions};
use tokio::{sync::Mutex, time::sleep};

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
async fn main() -> Result<(), anyhow::Error> {
    startup();

    let mut index: u8 = 0;
    let args = Args::parse();
    let tables: Arc<Mutex<HashMap<u8, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut options = SrtOptions::default();
    options.latency = 20;
    options.fc = 32;

    if args.kind == "server" {
        let mut server = Listener::bind(args.addr, options, 100).await?;
        while let Ok((socket, _addr)) = server.accept().await {
            let mut buf = [0u8; 2000];
            while let Ok(size) = socket.read(&mut buf).await {
                if size == 0 {
                    break;
                }

                socket.send(&buf[..size]).await?;
            }
        }
    } else {
        let socket = Socket::connect(args.addr, options).await?;
        let socket = Arc::new(socket);

        let tables_ = tables.clone();
        let socket_ = socket.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 2000];
            while let Ok(size) = socket_.read(&mut buf).await {
                if size == 0 {
                    break;
                }

                let index = buf[0];
                if let Some(instant) = tables_.lock().await.remove(&index) {
                    println!(
                        "delay={}, stats={:#?}",
                        instant.elapsed().as_millis() / 2,
                        socket_.get_stats()
                    );
                }
            }
        });

        let mut buf = [0u8; 1300];
        loop {
            buf[0] = index;
            tables.lock().await.insert(index, Instant::now());
            index = if index + 1 >= u8::MAX { 0 } else { index + 1 };

            socket.send(&buf).await?;
            sleep(Duration::from_millis(1000)).await;
        }
    }

    cleanup();
    Ok(())
}
