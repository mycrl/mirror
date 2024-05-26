mod proxy;

use std::{net::SocketAddr, process::exit, sync::Arc, thread};

use anyhow::Result;
use clap::Parser;
use service::route::Route;

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
    // Parse command line parameters. Note that if the command line parameters are
    // incorrect, panic will occur.
    let config = Configure::parse();

    // Initialize srt and logger
    srt::startup();
    simple_logger::init_with_level(log::Level::Info)?;

    log::info!("configure: {:?}", config);

    let route = Arc::new(Route::default());
    let route_ = route.clone();

    // Start the signaling server. If the signaling server exits, the entire process
    // will exit. This is because if the signaling exits, it is meaningless to
    // continue running.
    thread::spawn(move || {
        if let Err(e) = service::signal::start_server(config.bind, route_) {
            log::error!("{:?}", e);
            exit(-1);
        }
    });

    // Start the forwarding server
    proxy::start_server(config, route)?;
    srt::cleanup();
    Ok(())
}
