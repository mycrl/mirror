pub mod adapter;
mod discovery;
mod payload;

use std::{collections::HashSet, net::SocketAddr, sync::Arc};

use adapter::StreamReceiverAdapter;
use futures::StreamExt;
use rtp::{RtpError, RtpReceiver, RtpSender, RtpConfig};
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
    RtpError(#[from] RtpError),
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
            discovery = Some(Discovery::new(options.bind).await?);
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

                            match RtpReceiver::new()
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

                                        while let Some(pkt) = socket.next().await {
                                            if let Some(adapter) = adapter.upgrade() {
                                                match decoder.decode(pkt.as_bytes()) {
                                                    DecodeRet::Pkt(chunk, kind, flags) => {
                                                        if !adapter.send(chunk, kind, flags) {
                                                            log::error!("adapter on buf failed.");
                                                            break;
                                                        }
                                                    }
                                                    DecodeRet::Loss => {
                                                        adapter.loss_pkt();
                                                    }
                                                    _ => (),
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
        bind: SocketAddr,
        dest: SocketAddr,
        description: Vec<u8>,
        adapter: &Arc<StreamSenderAdapter>,
    ) -> Result<(), TransportError> {
        let server = RtpSender::new(RtpConfig { bind, dest })?;
        let max_pkt_size = RtpSender::max_packet_size();
        log::info!("sender server bind to port={}", dest.port());

        let service = Service {
            port: dest.port(),
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
                            if let Err(e) = server.send(payload) {
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
        addr: SocketAddr,
        adapter: &Arc<StreamReceiverAdapter>,
    ) -> Result<(), TransportError> {
        let mut socket = RtpReceiver::new(addr)?;
        log::info!(
            "connected to remote service, ip={}, port={}",
            addr.ip(),
            addr.port(),
        );

        let runtime = Handle::current();
        let adapter = Arc::downgrade(adapter);
        let discovery = self.discovery.as_ref().map(Arc::downgrade);
        tokio::spawn(async move {
            let mut decoder = Decoder::default();

            while let Some(pkt) = socket.next().await {
                if let Some(adapter) = adapter.upgrade() {
                    match decoder.decode(pkt.as_bytes()) {
                        DecodeRet::Pkt(chunk, kind, flags) => {
                            if !adapter.send(chunk, kind, flags) {
                                log::error!("adapter on buf failed.");
                                break;
                            }
                        }
                        DecodeRet::Loss => {
                            adapter.loss_pkt();
                        }
                        _ => (),
                    }
                } else {
                    log::warn!("adapter is droped!");
                    break;
                }
            }

            log::warn!("receiver is closed, ip={}, port={}", addr.ip(), addr.port());

            if let Some(discovery) = discovery.as_ref().map(|item| item.upgrade()).flatten() {
                runtime.block_on(discovery.remove(&addr));
            }

            if let Some(adapter) = adapter.upgrade() {
                adapter.close();
            }
        });

        Ok(())
    }
}
