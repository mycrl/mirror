mod client;
mod muxer;
mod remuxer;
mod server;

pub use client::Client;
pub use server::Server;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("udp socket closed")]
    Closed,
}
