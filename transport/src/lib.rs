pub mod adapter;
mod discovery;
mod payload;

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use adapter::StreamReceiverAdapter;
use srt::{Listener, Socket, SrtError, SrtOptions};
use thiserror::Error;
use tokio::{
    sync::{Mutex, RwLock},
    time::sleep,
};

use crate::{
    adapter::{ReceiverAdapterFactory, StreamSenderAdapter},
    discovery::{Discovery, DiscoveryError, Service},
    payload::{Decoder, Encoder},
};

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    TransportError(#[from] SrtError),
    #[error(transparent)]
    DiscoveryError(#[from] DiscoveryError),
}

#[derive(Debug, Clone)]
pub struct TransportOptions {
    pub srt: SrtOptions,
    pub bind: SocketAddr,
}

#[derive(Debug)]
pub struct Transport {
    services: Mutex<HashSet<Service>>,
    discovery: Arc<Discovery>,
    options: TransportOptions,
}

impl Transport {
    pub async fn new<T>(
        options: TransportOptions,
        adapter_factory: Option<T>,
    ) -> Result<Self, TransportError>
    where
        T: ReceiverAdapterFactory + 'static,
    {
        let discovery = Discovery::new(&options.bind).await?;
        log::info!("discovery service create done.");

        if let Some(adapter_factory) = adapter_factory {
            let options_ = options.clone();
            let discovery = Arc::downgrade(&discovery);
            tokio::spawn(async move {
                loop {
                    let discovery = if let Some(discovery) = discovery.upgrade() {
                        discovery
                    } else {
                        break;
                    };

                    if let Some((service, addr)) = discovery.recv_online().await {
                        log::info!(
                            "discovery recv online service, id={}, port={}, addr={}",
                            service.id,
                            service.port,
                            addr
                        );

                        if let Some(adapter) = adapter_factory
                            .connect(service.id, addr.ip(), &service.description)
                            .await
                        {
                            log::info!("adapter factory created a adapter.");

                            match Socket::connect(
                                SocketAddr::new(addr.ip(), service.port),
                                options_.srt.clone(),
                            )
                            .await
                            {
                                Ok(socket) => {
                                    log::info!(
                                        "connected to remote service, ip={}, port={}",
                                        addr.ip(),
                                        service.port
                                    );

                                    tokio::spawn(async move {
                                        let mut buf = [0u8; 2048];
                                        let mut decoder = Decoder::default();

                                        while let Ok(size) = socket.read(&mut buf).await {
                                            if size == 0 {
                                                log::error!(
                                                    "read zero buf from socket, ip={}, port={}",
                                                    addr.ip(),
                                                    service.port,
                                                );

                                                break;
                                            }

                                            if let Some(adapter) = adapter.upgrade() {
                                                for (chunk, kind) in decoder.decode(&buf[..size]) {
                                                    if !adapter.send(chunk, kind).await {
                                                        log::error!("adapter on buf failed.");
                                                        break;
                                                    }
                                                }
                                            } else {
                                                log::warn!("adapter is droped!");
                                                break;
                                            }
                                        }

                                        log::warn!("socket is closed, ip={}", addr.ip());

                                        discovery.remove(&addr).await;
                                        if let Some(adapter) = adapter.upgrade() {
                                            adapter.close().await;
                                        }
                                    });
                                }
                                Err(e) => {
                                    log::error!(
                                        "connect to remote service failed, ip={}, port={}, err={:?}",
                                        addr.ip(),
                                        service.port,
                                        e,
                                    );
                                }
                            }
                        } else {
                            log::info!("adapter factory not create adapter.");
                        }
                    } else {
                        break;
                    }
                }

                log::info!("discovery is drop, maybe is released.");
            });
        }

        Ok(Self {
            services: Default::default(),
            discovery,
            options,
        })
    }

    pub async fn create_sender(
        &self,
        id: u8,
        description: Vec<u8>,
        adapter: &Arc<StreamSenderAdapter>,
    ) -> Result<u16, TransportError> {
        let sockets = Arc::new(RwLock::new(HashMap::with_capacity(100)));
        let mut server = Listener::bind(
            SocketAddr::new(self.options.bind.ip(), 0),
            self.options.srt.clone(),
            100,
        )
        .await?;

        let port = server.local_addr().unwrap().port();
        log::info!("srt server bind to port={}", port);

        {
            let mut services = self.services.lock().await;
            services.insert(Service {
                description,
                port,
                id,
            });

            self.discovery
                .set_services(services.iter().map(|item| item.clone()).collect())
                .await;
        }

        let sockets_ = sockets.clone();
        let adapter_ = Arc::downgrade(adapter);
        let accept_task = tokio::spawn(async move {
            let mut encoder = Encoder::default();

            while let Ok((socket, addr)) = server.accept().await {
                // Since the connection has just been called back, the status may
                // not have changed to Connected yet, so simply wait a bit here.
                sleep(Duration::from_millis(100)).await;

                if let Some(adapter) = adapter_.upgrade() {
                    for (buf, kind) in adapter.get_config() {
                        if let Some(payload) = encoder.encode(kind, buf) {
                            if let Err(e) = socket.send(payload).await {
                                log::error!(
                                    "failed to send buf in socket, addr={}, err={:?}",
                                    addr,
                                    e
                                );
                            }
                        }
                    }
                }

                sockets_.write().await.insert(addr, socket);

                log::info!("srt server accept socket, addr={}", addr);
            }
        });

        let discovery = self.discovery.clone();
        let adapter = Arc::downgrade(adapter);
        tokio::spawn(async move {
            let mut closed = Vec::with_capacity(10);
            let mut encoder = Encoder::default();

            while let Some(adapter) = adapter.upgrade() {
                if let Some((buf, kind)) = adapter.next().await {
                    {
                        let sockets = sockets.read().await;
                        if !closed.is_empty() {
                            closed.clear();
                        }

                        if sockets.is_empty() {
                            continue;
                        }

                        if let Some(payload) = encoder.encode(kind, buf.as_ref()) {
                            for (addr, socket) in sockets.iter() {
                                if let Err(e) = socket.send(payload).await {
                                    closed.push(*addr);

                                    log::error!(
                                        "failed to send buf in socket, addr={}, err={:?}",
                                        addr,
                                        e
                                    );
                                }
                            }
                        }
                    }

                    for addr in &closed {
                        let _ = sockets.write().await.remove(addr);
                    }
                }
            }

            log::info!("adapter recv a none, close the worker.");

            accept_task.abort();
            sockets.write().await.clear();
            discovery.set_services(Vec::new()).await;
        });

        Ok(port)
    }

    pub async fn create_receiver(
        &self,
        port: u16,
        adapter: &Arc<StreamReceiverAdapter>,
    ) -> Result<(), TransportError> {
        let addr = SocketAddr::new(self.options.bind.ip(), port);
        let socket = Socket::connect(addr, self.options.srt.clone()).await?;
        log::info!(
            "connected to remote service, ip={}, port={}",
            addr.ip(),
            port
        );

        let adapter = Arc::downgrade(adapter);
        let discovery = Arc::downgrade(&self.discovery);
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            let mut decoder = Decoder::default();

            while let Ok(size) = socket.read(&mut buf).await {
                if size == 0 {
                    log::error!("read zero buf from socket, ip={}", addr.ip());

                    break;
                }

                if let Some(adapter) = adapter.upgrade() {
                    for (chunk, kind) in decoder.decode(&buf[..size]) {
                        if !adapter.send(chunk, kind).await {
                            log::error!("adapter on buf failed.");
                            break;
                        }
                    }
                } else {
                    log::warn!("adapter is droped!");
                    break;
                }
            }

            log::warn!("socket is closed, ip={}, port={}", addr.ip(), port);

            if let Some(discovery) = discovery.upgrade() {
                discovery.remove(&addr).await;
            }

            if let Some(adapter) = adapter.upgrade() {
                adapter.close().await;
            }
        });

        Ok(())
    }
}
