use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
    thread,
};

use anyhow::Result;
use clap::Parser;
use hylarana_transport::{
    shutdown, startup, StreamInfo, StreamInfoKind, TransmissionOptions, TransmissionServer,
};
use parking_lot::RwLock;

// #[global_allocator]
// static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser, Clone, Debug)]
#[command(
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
)]
pub struct Configure {
    #[arg(long)]
    pub bind: SocketAddr,
    #[arg(long)]
    pub mtu: usize,
}

fn main() -> Result<()> {
    // Initialize srt and logger
    simple_logger::init_with_level(log::Level::Info)?;
    startup();

    // Parse command line parameters. Note that if the command line parameters are
    // incorrect, panic will occur.
    let config = Configure::parse();
    log::info!("configure: {:?}", config);

    // Configuration of the srt server. Since this suite only works within the LAN,
    // the delay is set to the minimum delay without considering network factors.
    let mut opt = TransmissionOptions::default();
    opt.mtu = config.mtu as u32;
    opt.latency = 40;
    opt.fc = 32;

    // Start the srt server
    let server = TransmissionServer::bind(config.bind, opt, 100)?;
    log::info!("starting srt server...");

    let sockets = Arc::new(RwLock::new(HashMap::with_capacity(200)));
    let subscribers = Arc::new(RwLock::new(HashMap::with_capacity(200)));

    loop {
        match server.accept() {
            Ok((socket, addr)) => {
                let stream_id = socket.get_stream_id();
                log::info!("new srt socket, addr={:?}, stream_id={:?}", addr, stream_id);

                let socket = Arc::new(socket);

                // Get the stream information carried in the srt link. If the stream information
                // does not exist or is invalid, the current connection is rejected. Skipping
                // this step directly will trigger the release of the link and close it.
                let stream_info = if let Some(info) = stream_id
                    .as_ref()
                    .and_then(|it| StreamInfo::from_str(it).ok())
                {
                    info
                } else {
                    log::error!("invalid stream id, addr={:?}", addr);

                    continue;
                };

                log::info!(
                    "accept a srt socket, addr={:?}, info={:?}",
                    addr,
                    stream_info
                );

                {
                    // If it is a subscriber, add the current connection to the subscription
                    // connection pool
                    if stream_info.kind == StreamInfoKind::Subscriber {
                        sockets.write().insert(addr, socket.clone());
                        subscribers
                            .write()
                            .entry(stream_info.id.clone())
                            .or_insert_with(|| HashSet::with_capacity(200))
                            .insert(addr);
                    }
                }

                let socket = socket.clone();
                let sockets = sockets.clone();
                let subscribers = subscribers.clone();
                thread::spawn(move || {
                    let mut buf = [0u8; 2000];
                    let mut closed = Vec::with_capacity(100);

                    loop {
                        match socket.read(&mut buf) {
                            Ok(size) => {
                                if size == 0 {
                                    break;
                                }

                                // Subscribers are not allowed to write any information to the
                                // server!
                                if stream_info.kind == StreamInfoKind::Subscriber {
                                    break;
                                }

                                closed.clear();

                                {
                                    let sockets = sockets.read();
                                    let subscribers = subscribers.read();

                                    // Forwards all packets sent by the publisher to all subscribers
                                    // of the same channel
                                    if let Some(items) = subscribers.get(&stream_info.id) {
                                        for addr in items.iter() {
                                            if let Some(socket) = sockets.get(addr) {
                                                if let Err(e) = socket.send(&buf[..size]) {
                                                    closed.push(*addr);

                                                    log::warn!(
                                                        "not send a buf to srt socket, addr={:?}, err={:?}",
                                                        addr,
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }

                                // Some subscribers have expired, clean up all expired subscribers
                                if !closed.is_empty() {
                                    let mut sockets = sockets.write();
                                    for addr in &closed {
                                        if let Some(socket) = sockets.remove(addr) {
                                            socket.close()
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!(
                                    "not recv a buf to srt socket, addr={:?}, err={:?}",
                                    addr,
                                    e
                                );

                                break;
                            }
                        }
                    }

                    log::info!("srt socket closed, addr={:?}, info={:?}", addr, stream_info);

                    let mut sockets = sockets.write();
                    let mut subscribers = subscribers.write();

                    // If the publisher has exited, it is necessary to close all subscribers of the
                    // current channel and inform the client that the publisher has exited.
                    if stream_info.kind == StreamInfoKind::Publisher {
                        if let Some(items) = subscribers.remove(&stream_info.id) {
                            for addr in items.iter() {
                                if let Some(socket) = sockets.remove(addr) {
                                    socket.close()
                                }
                            }
                        }
                    } else {
                        // Subscriber exits, deletes subscription group record
                        if let Some(items) = subscribers.get_mut(&stream_info.id) {
                            items.remove(&addr);
                        }
                    }
                });
            }
            Err(e) => {
                log::error!("{:?}", e);

                break;
            }
        }
    }

    shutdown();
    Ok(())
}
