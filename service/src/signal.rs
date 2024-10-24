use crate::route::Route;

use std::{io::Error, net::SocketAddr, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::sleep,
};

use mirror_transport::Signal;

pub async fn start_server(bind: SocketAddr, route: Arc<Route>) -> Result<(), Error> {
    let listener = TcpListener::bind(bind).await?;

    let route_ = route.clone();
    tokio::spawn(async move {
        loop {
            route_.ping();
            sleep(Duration::from_secs(5)).await;
        }
    });

    loop {
        match listener.accept().await {
            Ok((mut socket, addr)) => {
                log::info!("new signal socket, addr={}", addr);

                let route = route.clone();
                tokio::spawn(async move {
                    if socket.set_nodelay(true).is_err() {
                        return;
                    }

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

                    // Every time a new publisher comes online, the current connection is notified
                    let mut buf = [0u8; 1];
                    let mut changer = route.get_changer();
                    loop {
                        tokio::select! {
                            Ok(signal) = changer.recv() => {
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
            Err(e) => {
                log::error!("{:?}", e);

                break;
            }
        }
    }

    Ok(())
}
