mod proxy;
mod route;
mod signal;

use self::{route::Route, signal::start_server};

use std::{net::SocketAddr, process::exit, sync::Arc, thread};

use anyhow::Result;
use clap::Parser;
use tokio::runtime::Runtime;
use transport::srt::{cleanup, startup};

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
    let route = Arc::new(Route::default());

    log::info!("configure: {:?}", config);

    // Start the forwarding server
    let route_ = route.clone();
    let config_ = config.clone();
    thread::spawn(move || {
        if proxy::start_server(config_, route_).is_err() {
            exit(-11);
        }
    });

    // Start the signaling server. If the signaling server exits, the entire process
    // will exit. This is because if the signaling exits, it is meaningless to
    // continue running.
    Runtime::new()?.block_on(start_server(config.bind, route))?;
    cleanup();

    Ok(())
}
