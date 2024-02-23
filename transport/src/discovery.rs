use std::{
    collections::HashMap,
    io::ErrorKind::ConnectionReset,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    sync::{
        broadcast::{channel, Receiver},
        Mutex, RwLock,
    },
    time::sleep,
};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct Discovery {
    id: Uuid,
    services: Mutex<HashMap<SocketAddr, Services>>,
    receiver: Mutex<Receiver<(Service, SocketAddr)>>,
    local_services: RwLock<Services>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
}

impl Discovery {
    pub async fn new(addr: &SocketAddr) -> Result<Arc<Self>, DiscoveryError> {
        let (tx, rx) = channel(1);
        let socket = Arc::new(UdpSocket::bind(addr).await?);
        socket.set_broadcast(true)?;

        log::info!("Discovery create udp socket, listening={}", addr);

        let this = Arc::new(Self {
            local_services: RwLock::new(Services(vec![])),
            services: Default::default(),
            receiver: Mutex::new(rx),
            id: Uuid::new_v4(),
            addr: *addr,
            socket,
        });

        let this_ = Arc::downgrade(&this);
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];

            let tx_ = &tx;
            let notify_service = |addr: SocketAddr, services: Vec<Service>| async move {
                for service in services {
                    log::info!(
                        "Discovery recv a online service event, id={}, port={}",
                        service.id,
                        service.port
                    );

                    if let Err(e) = tx_.send((service, addr)) {
                        log::error!("unexpected error, this is a bug!, error={}", e);
                    }
                }
            };

            loop {
                let this = if let Some(this) = this_.upgrade() {
                    this
                } else {
                    log::info!(
                        "Discovery socket receiver closed, maybe is because Discovery drop."
                    );

                    break;
                };

                let (size, addr) = match this.socket.recv_from(&mut buf).await {
                    Err(e) if e.kind() != ConnectionReset => break,
                    Ok(ret) => ret,
                    _ => continue,
                };

                if size == 0 {
                    log::info!("Discovery udp socket recv zero buf, close the socket receiver.");

                    break;
                }

                log::info!(
                    "Discovery udp socket recv buf, size={}, addr={}",
                    size,
                    addr
                );

                if let Ok(pkt) = rmp_serde::decode::from_slice::<Message>(&buf[..size]) {
                    log::info!("Discovery recv a message, pkt={:?}", pkt);

                    match pkt {
                        Message::Notify { id, services } => {
                            if id == this.id {
                                continue;
                            }

                            let mut service = this.services.lock().await;
                            if let Some(service) = service.get_mut(&addr) {
                                if let Some(diffs) = service.diff(&services) {
                                    notify_service(addr, diffs).await;
                                }

                                *service = Services(services);
                            } else {
                                notify_service(addr, services.clone()).await;
                                service.insert(addr, Services(services));
                            }
                        }
                        Message::Query { id } => {
                            if id == this.id {
                                continue;
                            }

                            if let Ok(pkt) = rmp_serde::encode::to_vec(&Message::Notify {
                                services: this.local_services.read().await.0.clone(),
                                id: this.id,
                            }) {
                                this.broadcast(pkt, None);
                            }
                        }
                    }
                }
            }
        });

        if let Ok(pkt) = rmp_serde::encode::to_vec(&Message::Query { id: this.id }) {
            this.broadcast(pkt, Some(2));
        }

        Ok(this)
    }

    pub async fn set_services(&self, services: Vec<Service>) {
        log::info!("Discovery set services, services={:?}", services);

        self.local_services.write().await.0 = services.clone();
        if let Ok(pkt) = rmp_serde::encode::to_vec(&Message::Notify {
            id: self.id,
            services,
        }) {
            self.broadcast(pkt, None);
        }
    }

    pub async fn recv_online(&self) -> Option<(Service, SocketAddr)> {
        self.receiver.lock().await.recv().await.ok()
    }

    pub async fn remove(&self, addr: &SocketAddr) {
        log::info!("Discovery remove a remote service, addr={:?}", addr);

        self.services.lock().await.remove(addr);
    }

    fn broadcast(&self, pkt: Vec<u8>, count: Option<u8>) {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), self.addr.port());
        let socket = Arc::downgrade(&self.socket);

        log::info!("Discovery start broadcast, target={:?}", addr);

        tokio::spawn(async move {
            for _ in 0..count.unwrap_or(3) {
                if let Some(socket) = socket.upgrade() {
                    if let Err(e) = socket.send_to(&pkt, addr).await {
                        if e.kind() != ConnectionReset {
                            log::error!("udp socket error: {}, addr={}", e, addr);
                            break;
                        }
                    } else {
                        sleep(Duration::from_millis(1000)).await;
                    }
                } else {
                    break;
                }
            }
        });
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub struct Service {
    #[serde(alias = "i")]
    pub id: u8,
    #[serde(alias = "p")]
    pub port: u16,
    #[serde(alias = "d")]
    pub description: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
enum Message {
    #[serde(alias = "q")]
    Query {
        #[serde(alias = "i")]
        id: Uuid,
    },
    #[serde(alias = "n")]
    Notify {
        #[serde(alias = "i")]
        id: Uuid,
        #[serde(alias = "s")]
        services: Vec<Service>,
    },
}

#[derive(Debug)]
struct Services(Vec<Service>);

impl Services {
    fn diff(&self, services: &[Service]) -> Option<Vec<Service>> {
        let mut diffs = Vec::new();
        for item in services {
            if self.0.iter().find(|value| value == &item).is_none() {
                diffs.push(item.clone());
            }
        }

        if diffs.is_empty() && self.0.len() <= services.len() {
            None
        } else {
            Some(diffs)
        }
    }
}
