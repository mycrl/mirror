use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    process::exit,
    sync::{Arc, RwLock},
    thread,
};

use anyhow::Result;
use clap::Parser;
use service::{route::Route, signal::start_server, SocketKind, StreamInfo};
use srt::{Options, Server};

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
    srt::startup();
    simple_logger::init_with_level(log::Level::Info)?;

    let config = Configure::parse();
    let route = Arc::new(Route::default());
    let sockets = Arc::new(RwLock::new(HashMap::with_capacity(200)));
    let subscribers = Arc::new(RwLock::new(HashMap::with_capacity(200)));

    log::info!("configure: {:?}", config);

    let mut opt = Options::default();
    opt.mtu = config.mtu as u32;
    opt.latency = 20;

    let mut server = Server::bind(config.bind, opt, 100)?;

    log::info!("starting srt server...");

    let route_ = route.clone();
    thread::spawn(move || {
        if let Err(e) = start_server(config.bind, route_.clone()) {
            log::error!("{:?}", e);
            exit(-1);
        }
    });

    while let Ok((socket, info)) = server.accept() {
        log::info!("new srt socket, info={:?}", info);

        let route = route.clone();
        let socket = Arc::new(socket);
        let stream_info = if let Some(info) = info
            .stream_id
            .as_ref()
            .map(|it| StreamInfo::decode(it))
            .flatten()
        {
            info
        } else {
            log::error!("invalid stream id, info={:?}", info);

            continue;
        };

        log::info!(
            "accept a srt socket, addr={:?}, info={:?}",
            info.addr,
            stream_info
        );

        if let Some(port) = stream_info.port {
            route.add(stream_info.id, port)
        }

        {
            if stream_info.kind == SocketKind::Subscriber {
                sockets.write().unwrap().insert(info.addr, socket.clone());
                subscribers
                    .write()
                    .unwrap()
                    .entry(stream_info.id)
                    .or_insert_with(|| HashSet::with_capacity(200))
                    .insert(info.addr);
            }
        }

        let socket = socket.clone();
        let sockets = sockets.clone();
        let subscribers = subscribers.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 2000];
            let mut closed = Vec::with_capacity(100);

            while let Ok(size) = socket.read(&mut buf) {
                if size == 0 {
                    break;
                }

                closed.clear();

                {
                    let sockets = sockets.read().unwrap();
                    let subscribers = subscribers.read().unwrap();

                    if let Some(items) = subscribers.get(&stream_info.id) {
                        for addr in items.iter() {
                            if let Some(socket) = sockets.get(addr) {
                                if socket.send(&buf[..size]).is_err() {
                                    closed.push(*addr);

                                    log::error!("not send a buf to srt socket, addr={:?}", addr);
                                }
                            }
                        }
                    }
                }

                if !closed.is_empty() {
                    let mut sockets = sockets.write().unwrap();
                    for addr in &closed {
                        if let Some(socket) = sockets.remove(addr) {
                            socket.close()
                        }
                    }
                }
            }

            log::info!(
                "srt socket closed, addr={:?}, info={:?}",
                info.addr,
                stream_info
            );

            let mut sockets = sockets.write().unwrap();
            let mut subscribers = subscribers.write().unwrap();

            if stream_info.kind == SocketKind::Publisher {
                if let Some(items) = subscribers.remove(&stream_info.id) {
                    for addr in items.iter() {
                        if let Some(socket) = sockets.remove(addr) {
                            socket.close()
                        }
                    }
                }
            } else {
                if let Some(items) = subscribers.get_mut(&stream_info.id) {
                    items.remove(&info.addr);
                }
            }

            if stream_info.port.is_some() {
                route.remove(stream_info.id)
            }
        });
    }

    srt::cleanup();
    Ok(())
}
