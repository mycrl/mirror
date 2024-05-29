use std::{io::Error, net::SocketAddr, sync::Arc};

use bytes::{BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use crate::route::Route;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Signal {
    /// Start publishing a channel. The port number is the publisher's multicast
    /// port.
    Start { id: u32, port: u16 },
    /// Stop publishing to a channel
    Stop { id: u32 },
}

impl Signal {
    pub fn encode(&self) -> Bytes {
        let payload = rmp_serde::to_vec(&self).unwrap();
        let mut buf = BytesMut::with_capacity(payload.len() + 2);
        buf.put_u16(buf.capacity() as u16);
        buf.extend_from_slice(&payload);
        buf.freeze()
    }

    #[rustfmt::skip]
    pub fn decode(buf: &[u8]) -> Option<(usize, Self)> {
        if buf.len() > 2 {
            let size = u16::from_be_bytes([
                buf[0],
                buf[1],
            ]) as usize;

            if size <= buf.len() {
                return rmp_serde::from_slice(&buf[2..size]).ok().map(|it| (size, it))
            }
        }

        None
    }
}

pub async fn start_server(bind: SocketAddr, route: Arc<Route>) -> Result<(), Error> {
    let listener = TcpListener::bind(bind).await?;
    while let Ok((mut socket, addr)) = listener.accept().await {
        log::info!("new signal socket, addr={}", addr);

        let route = route.clone();
        tokio::spawn(async move {
            // Every time a new connection comes online, notify the current link of all
            // published channels.
            {
                for (id, port) in route.get_channels() {
                    if socket
                        .write_all(&Signal::Start { id, port }.encode())
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }

            // todo: The link is closed and the thread cannot be released in time

            // Every time a new publisher comes online, the current connection is notified
            let mut buf = [0u8; 1024];
            let mut changer = route.get_changer();
            loop {
                tokio::select! {
                    Some(signal) = changer.change() => {
                        if socket.write_all(&signal.encode()).await.is_err() {
                            break;
                        }
                    }
                    Ok(size) = socket.read(&mut buf) => {
                        if size == 0 {
                            break;
                        }
                    }
                    else => break
                }
            }

            log::info!("signal socket close, addr={}", addr);
        });
    }

    Ok(())
}
