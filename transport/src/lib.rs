pub mod adapter;
mod discovery;
mod payload;

use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    thread,
};

use adapter::StreamReceiverAdapter;
use multicast::{Receiver, Sender};
use thiserror::Error;
use thread_priority::{set_current_thread_priority, ThreadPriority};

use crate::{
    adapter::{ReceiverAdapterFactory, StreamSenderAdapter},
    discovery::{Discovery, DiscoveryError, Service},
    payload::{Muxer, Remuxer},
};

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    NetError(#[from] multicast::Error),
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
    multicast: Ipv4Addr,
}

impl Transport {
    pub fn new<T>(
        multicast: Ipv4Addr,
        options: Option<TransportOptions<T>>,
    ) -> Result<Self, TransportError>
    where
        T: ReceiverAdapterFactory + 'static,
    {
        let mut discovery = None;
        if let Some(options) = options {
            discovery = Some(Discovery::new(options.bind)?);
            let discovery = discovery.as_ref().map(Arc::downgrade);

            thread::spawn(move || loop {
                let discovery = if let Some(discovery) =
                    discovery.as_ref().map(|item| item.upgrade()).flatten()
                {
                    discovery
                } else {
                    log::info!("discovery is drop, maybe is released.");

                    break;
                };

                if let Some((service, addr)) = discovery.recv_online() {
                    log::info!(
                        "discovery recv online service, id={}, port={}, addr={}",
                        service.id,
                        service.port,
                        addr
                    );

                    let bind = SocketAddr::new(addr.ip(), service.port);
                    if let Some(adapter) =
                        options
                            .adapter_factory
                            .connect(service.id, bind, &service.description)
                    {
                        log::info!(
                            "adapter factory created a adapter, ip={}, port={}",
                            options.bind.ip(),
                            service.port
                        );

                        let bind = SocketAddr::new(options.bind.ip(), service.port);
                        match Receiver::new(multicast, bind, 50) {
                            Ok(mut receiver) => {
                                log::info!(
                                    "connected to remote service, ip={}, port={}",
                                    addr.ip(),
                                    service.port,
                                );

                                thread::spawn(move || {
                                    let _ = set_current_thread_priority(ThreadPriority::Max);

                                    'a: while let Ok(packet) = receiver.read() {
                                        if let Some(adapter) = adapter.upgrade() {
                                            if let Some((kind, flags, timestamp)) =
                                                Remuxer::remux(&packet)
                                            {
                                                if !adapter.send(packet, kind, flags, timestamp) {
                                                    log::error!("adapter on buf failed.");

                                                    break 'a;
                                                }
                                            } else {
                                                adapter.loss_pkt();
                                            }
                                        } else {
                                            log::warn!("adapter is droped!");
                                            break 'a;
                                        }
                                    }

                                    log::warn!("socket is closed, ip={}", addr.ip());

                                    discovery.remove(&addr);
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
            });
        }

        Ok(Self {
            services: Default::default(),
            multicast,
            discovery,
        })
    }

    pub fn create_sender(
        &self,
        id: u8,
        mtu: usize,
        bind: SocketAddr,
        description: Vec<u8>,
        adapter: &Arc<StreamSenderAdapter>,
    ) -> Result<(), TransportError> {
        let mut sender = Sender::new(self.multicast, bind, mtu)?;
        let service = Service {
            port: bind.port(),
            description,
            id,
        };

        log::info!("sender bind to port={}", bind.port());

        {
            let mut services = self.services.lock().unwrap();
            services.insert(service.clone());

            if let Some(discovery) = &self.discovery {
                discovery.set_services(services.iter().map(|item| item.clone()).collect());
            }
        }

        let services_ = Arc::downgrade(&self.services);
        let discovery_ = self.discovery.as_ref().map(Arc::downgrade);
        let adapter_ = Arc::downgrade(adapter);
        thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            while let Some(adapter) = adapter_.upgrade() {
                if let Some((buf, kind, flags, timestamp)) = adapter.next() {
                    if let Some(payload) = Muxer::mux(kind, flags, timestamp, buf.as_ref()) {
                        if let Err(e) = sender.send(payload) {
                            log::error!("failed to send buf in socket, err={:?}", e);
                        }
                    }
                } else {
                    break;
                }
            }

            log::info!("adapter recv a none, close the worker.");

            if let Some(discovery) = discovery_.as_ref().map(|item| item.upgrade()).flatten() {
                if let Some(services) = services_.upgrade() {
                    let mut services = services.lock().unwrap();
                    services.remove(&service);

                    discovery.set_services(services.iter().map(|item| item.clone()).collect());
                }
            }
        });

        Ok(())
    }

    pub fn create_receiver(
        &self,
        bind: SocketAddr,
        adapter: &Arc<StreamReceiverAdapter>,
    ) -> Result<(), TransportError> {
        let mut receiver = Receiver::new(self.multicast, bind, 50)?;
        log::info!("receiver listening, port={}", bind.port(),);

        let adapter = Arc::downgrade(adapter);
        thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            'a: while let Ok(packet) = receiver.read() {
                if let Some(adapter) = adapter.upgrade() {
                    if let Some((kind, flags, timestamp)) = Remuxer::remux(&packet) {
                        if !adapter.send(packet, kind, flags, timestamp) {
                            log::error!("adapter on buf failed.");
                            break 'a;
                        }
                    } else {
                        adapter.loss_pkt();
                    }
                } else {
                    log::warn!("adapter is droped!");
                    break 'a;
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
