mod nack;
mod packet;
mod receiver;
mod sender;

pub use receiver::Receiver;
pub use sender::Sender;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("udp socket closed")]
    Closed,
}
