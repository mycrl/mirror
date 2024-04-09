pub mod adapter;
mod discovery;
mod payload;

use std::{collections::HashSet, net::SocketAddr, sync::Arc};

use adapter::StreamReceiverAdapter;
use thiserror::Error;
use tokio::{runtime::Handle, sync::Mutex};

use crate::{
    adapter::{ReceiverAdapterFactory, StreamSenderAdapter},
    discovery::{Discovery, DiscoveryError, Service},
    payload::{DecodeRet, Decoder, Encoder},
};

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    RtpError(#[from] broadcast::Error),
    #[error(transparent)]
    DiscoveryError(#[from] DiscoveryError),
}

pub struct TransportOptions<T> {
    pub bind: SocketAddr,
    pub adapter_factory: T,
}

#[derive(Debug)]
pub struct Transport {
    services: Arc<Mutex<HashSet<Service>>>,
    discovery: Option<Arc<Discovery>>,
}

impl Transport {
    pub async fn new<T>(options: Option<TransportOptions<T>>) -> Result<Self, TransportError>
    where
        T: ReceiverAdapterFactory + 'static,
    {
        let mut discovery = None;
        if let Some(options) = options {
            let mut listen = options.bind;
            listen.set_port(listen.port() + 1);

            discovery = Some(Discovery::new(listen).await?);
            let discovery = discovery.as_ref().map(Arc::downgrade);

            tokio::spawn(async move {
                loop {
                    let discovery = if let Some(discovery) =
                        discovery.as_ref().map(|item| item.upgrade()).flatten()
                    {
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

                        if let Some(adapter) = options
                            .adapter_factory
                            .connect(
                                service.id,
                                SocketAddr::new(addr.ip(), service.port),
                                &service.description,
                            )
                            .await
                        {
                            log::info!(
                                "adapter factory created a adapter, ip={}, port={}",
                                options.bind.ip(),
                                service.port
                            );

                            match broadcast::Receiver::new(SocketAddr::new(
                                "0.0.0.0".parse().unwrap(),
                                service.port,
                            )).await
                            {
                                Ok(mut socket) => {
                                    log::info!(
                                        "connected to remote service, ip={}, port={}",
                                        addr.ip(),
                                        service.port,
                                    );

                                    let runtime = Handle::current();
                                    tokio::spawn(async move {
                                        let mut decoder = Decoder::default();

                                        'a: while let Ok(packets) = socket.read().await {
                                            for pkt in packets {
                                                if let Some(adapter) = adapter.upgrade() {
                                                    match decoder.decode(pkt) {
                                                        DecodeRet::Pkt(chunk, kind, flags) => {
                                                            if !adapter.send(chunk, kind, flags) {
                                                                log::error!(
                                                                    "adapter on buf failed."
                                                                );

                                                                break 'a;
                                                            }
                                                        }
                                                        DecodeRet::Loss => {
                                                            adapter.loss_pkt();
                                                        }
                                                        _ => (),
                                                    }
                                                } else {
                                                    log::warn!("adapter is droped!");
                                                    break 'a;
                                                }
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
                                        "connect to remote service failed, ip={}, port={}, error={}",
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
        })
    }

    pub async fn create_sender(
        &self,
        id: u8,
        mtu: usize,
        bind: SocketAddr,
        description: Vec<u8>,
        adapter: &Arc<StreamSenderAdapter>,
    ) -> Result<(), TransportError> {
        let mut sender = broadcast::Sender::new(broadcast::SenderOptions {
            bind: SocketAddr::new(bind.ip(), 0),
            to: bind.port(),
            mtu,
        }).await?;

        let max_pkt_size = sender.max_packet_size();
        log::info!("sender bind to port={}", bind.port());

        let service = Service {
            port: bind.port(),
            description,
            id,
        };

        {
            let mut services = self.services.lock().await;
            services.insert(service.clone());

            if let Some(discovery) = &self.discovery {
                discovery
                    .set_services(services.iter().map(|item| item.clone()).collect())
                    .await;
            }
        }

        let services_ = Arc::downgrade(&self.services);
        let discovery_ = self.discovery.as_ref().map(Arc::downgrade);
        let adapter_ = Arc::downgrade(adapter);
        tokio::spawn(async move {
            let mut encoder = Encoder::default();

            while let Some(adapter) = adapter_.upgrade() {
                if let Some((buf, kind, flags)) = adapter.next().await {
                    if let Some(payloads) = encoder.encode(max_pkt_size, kind, flags, buf.as_ref())
                    {
                        for payload in payloads {
                            if let Err(e) = sender.send(payload).await {
                                log::error!("failed to send buf in socket, err={:?}", e);
                            }
                        }
                    }
                } else {
                    break;
                }
            }

            log::info!("adapter recv a none, close the worker.");

            if let Some(discovery) = discovery_.as_ref().map(|item| item.upgrade()).flatten() {
                if let Some(services) = services_.upgrade() {
                    let mut services = services.lock().await;
                    services.remove(&service);

                    discovery
                        .set_services(services.iter().map(|item| item.clone()).collect())
                        .await;
                }
            }
        });

        Ok(())
    }

    pub async fn create_receiver(
        &self,
        bind: SocketAddr,
        adapter: &Arc<StreamReceiverAdapter>,
    ) -> Result<(), TransportError> {
        let mut socket = broadcast::Receiver::new(bind).await?;
        log::info!("receiver listening, port={}", bind.port(),);

        let adapter = Arc::downgrade(adapter);
        tokio::spawn(async move {
            let mut decoder = Decoder::default();

            'a: while let Ok(packets) = socket.read().await {
                for pkt in packets {
                    if let Some(adapter) = adapter.upgrade() {
                        match decoder.decode(pkt) {
                            DecodeRet::Pkt(chunk, kind, flags) => {
                                if !adapter.send(chunk, kind, flags) {
                                    log::error!("adapter on buf failed.");
                                    break 'a;
                                }
                            }
                            DecodeRet::Loss => {
                                adapter.loss_pkt();
                            }
                            _ => (),
                        }
                    } else {
                        log::warn!("adapter is droped!");
                        break 'a;
                    }
                }
            }

            log::warn!("receiver is closed, addr={}", bind);

            if let Some(adapter) = adapter.upgrade() {
                adapter.close();
            }
        });

        Ok(())
    }
}
