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
    runtime::Handle,
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
    services: Arc<Mutex<HashSet<Service>>>,
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
                        log::info!("discovery is drop, maybe is released.");

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
                            .connect(
                                service.id,
                                SocketAddr::new(addr.ip(), service.port),
                                &service.description,
                            )
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

                                    let runtime = Handle::current();
                                    std::thread::spawn(move || {
                                        let mut buf = [0u8; 2048];
                                        let mut decoder = Decoder::default();

                                        while let Ok(size) = socket.read(&mut buf) {
                                            if size == 0 {
                                                log::error!(
                                                    "read zero buf from socket, ip={}, port={}",
                                                    addr.ip(),
                                                    service.port,
                                                );

                                                break;
                                            }

                                            if let Some(adapter) = adapter.upgrade() {
                                                if let Some((chunk, kind)) =
                                                    decoder.decode(&buf[..size])
                                                {
                                                    if !adapter.send(chunk, kind) {
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

                                        runtime.block_on(discovery.remove(&addr));
                                        if let Some(adapter) = adapter.upgrade() {
                                            adapter.close();
                                        }
                                    });
                                }
                                Err(e) => {
                                    log::error!(
                                        "connect to remote service failed, ip={}, port={}, \
                                         err={:?}",
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
                        log::info!("discovery recv online a none, maybe is released.");

                        break;
                    }
                }
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
        let sockets = Arc::new(RwLock::new(HashMap::with_capacity(256)));
        let mut server = Listener::bind(
            SocketAddr::new(self.options.bind.ip(), 0),
            self.options.srt.clone(),
            i32::MAX as u32,
        )
        .await?;

        let max_pkt_size = self.options.srt.max_pkt_size();
        let port = server.local_addr().unwrap().port();
        log::info!("srt server bind to port={}", port);

        let service = Service {
            description,
            port,
            id,
        };

        {
            let mut services = self.services.lock().await;
            services.insert(service.clone());

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
                    let mut is_allow = true;
                    'a: for (buf, kind) in adapter.get_config() {
                        if let Some(payloads) = encoder.encode(max_pkt_size, kind, buf) {
                            for payload in payloads {
                                if let Err(e) = socket.send(payload) {
                                    log::error!(
                                        "failed to send buf in socket, addr={}, err={:?}",
                                        addr,
                                        e
                                    );

                                    is_allow = false;
                                    break 'a;
                                }
                            }
                        }
                    }

                    if is_allow {
                        sockets_.write().await.insert(addr, socket);
                        log::info!("srt server accept socket, addr={}", addr);
                    }
                } else {
                    break;
                }
            }
        });

        let services_ = Arc::downgrade(&self.services);
        let discovery_ = Arc::downgrade(&self.discovery);
        let adapter_ = Arc::downgrade(adapter);
        tokio::spawn(async move {
            let mut closed = Vec::with_capacity(10);
            let mut encoder = Encoder::default();

            while let Some(adapter) = adapter_.upgrade() {
                if let Some((buf, kind)) = adapter.next().await {
                    {
                        let sockets = sockets.read().await;
                        if !closed.is_empty() {
                            closed.clear();
                        }

                        if sockets.is_empty() {
                            continue;
                        }

                        if let Some(payloads) = encoder.encode(max_pkt_size, kind, buf.as_ref()) {
                            for payload in payloads {
                                for (addr, socket) in sockets.iter() {
                                    if let Err(e) = socket.send(payload) {
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
                    }

                    for addr in &closed {
                        let _ = sockets.write().await.remove(addr);
                        log::info!("remove a socket, addr={}", addr)
                    }
                } else {
                    break;
                }
            }

            log::info!("adapter recv a none, close the worker.");

            accept_task.abort();
            sockets.write().await.clear();
            if let Some(discovery) = discovery_.upgrade() {
                if let Some(services) = services_.upgrade() {
                    let mut services = services.lock().await;
                    services.remove(&service);

                    discovery
                        .set_services(services.iter().map(|item| item.clone()).collect())
                        .await;
                }
            }
        });

        Ok(port)
    }

    pub async fn create_receiver(
        &self,
        addr: SocketAddr,
        adapter: &Arc<StreamReceiverAdapter>,
    ) -> Result<(), TransportError> {
        let socket = Socket::connect(addr, self.options.srt.clone()).await?;
        log::info!(
            "connected to remote service, ip={}, port={}",
            addr.ip(),
            addr.port(),
        );

        let runtime = Handle::current();
        let adapter = Arc::downgrade(adapter);
        let discovery = Arc::downgrade(&self.discovery);
        std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            let mut decoder = Decoder::default();

            while let Ok(size) = socket.read(&mut buf) {
                if size == 0 {
                    log::error!("read zero buf from socket, ip={}", addr.ip());

                    break;
                }

                if let Some(adapter) = adapter.upgrade() {
                    if let Some((chunk, kind)) = decoder.decode(&buf[..size]) {
                        if !adapter.send(chunk, kind) {
                            log::error!("adapter on buf failed.");
                            break;
                        }
                    }
                } else {
                    log::warn!("adapter is droped!");
                    break;
                }
            }

            log::warn!("socket is closed, ip={}, port={}", addr.ip(), addr.port());

            if let Some(discovery) = discovery.upgrade() {
                runtime.block_on(discovery.remove(&addr));
            }

            if let Some(adapter) = adapter.upgrade() {
                adapter.close();
            }
        });

        Ok(())
    }
}
