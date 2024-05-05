use std::{
    collections::HashMap,
    io::ErrorKind::ConnectionReset,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        mpsc::{channel, Receiver},
        Arc, Mutex, RwLock,
    },
    thread::{self, sleep},
    time::{Duration, SystemTime},
};

use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    pub fn new(addr: SocketAddr) -> Result<Arc<Self>, DiscoveryError> {
        let (tx, rx) = channel();
        let socket = Arc::new(UdpSocket::bind(addr)?);
        socket.set_broadcast(true)?;

        log::info!("Discovery create udp socket, listening={}", addr);

        let this = Arc::new(Self {
            local_services: RwLock::new(Services(vec![])),
            services: Default::default(),
            receiver: Mutex::new(rx),
            id: Uuid::new(),
            socket,
            addr,
        });

        let this_ = Arc::downgrade(&this);
        thread::spawn(move || {
            let mut buf = [0u8; 2048];

            let tx_ = &tx;
            let notify_service = move |addr: SocketAddr, services: Vec<Service>| {
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

                let (size, addr) = match this.socket.recv_from(&mut buf) {
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
                            if id == this.id.0 {
                                continue;
                            }

                            let mut services_ = this.services.lock().unwrap();
                            if let Some(service) = services_.get_mut(&addr) {
                                if let Some(diffs) = service.diff(&services) {
                                    notify_service(addr, diffs);
                                }

                                *service = Services(services);
                            } else {
                                notify_service(addr, services.clone());
                                services_.insert(addr, Services(services));
                            }
                        }
                        Message::Query { id } => {
                            if id == this.id.0 {
                                continue;
                            }

                            if let Ok(pkt) = rmp_serde::encode::to_vec(&Message::Notify {
                                services: this.local_services.read().unwrap().0.clone(),
                                id: this.id.0,
                            }) {
                                this.broadcast(pkt, None);
                            }
                        }
                    }
                } else {
                    log::warn!("Discovery received to a invalid message.")
                }
            }
        });

        if let Ok(pkt) = rmp_serde::encode::to_vec(&Message::Query { id: this.id.0 }) {
            this.broadcast(pkt, Some(2));
        }

        Ok(this)
    }

    pub fn set_services(&self, services: Vec<Service>) {
        log::info!("Discovery set services, services={:?}", services);

        self.local_services.write().unwrap().0.clone_from(&services);
        if let Ok(pkt) = rmp_serde::encode::to_vec(&Message::Notify {
            id: self.id.0,
            services,
        }) {
            self.broadcast(pkt, None);
        }
    }

    pub fn recv_online(&self) -> Option<(Service, SocketAddr)> {
        self.receiver.lock().unwrap().recv().ok()
    }

    pub fn remove(&self, addr: &SocketAddr) {
        log::info!("Discovery remove a remote service, addr={:?}", addr);

        self.services.lock().unwrap().remove(addr);
    }

    fn broadcast(&self, pkt: Vec<u8>, count: Option<u8>) {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), self.addr.port());
        let socket = Arc::downgrade(&self.socket);

        log::info!("Discovery start broadcast, target={:?}", addr);

        thread::spawn(move || {
            for _ in 0..count.unwrap_or(5) {
                if let Some(socket) = socket.upgrade() {
                    if let Err(e) = socket.send_to(&pkt, addr) {
                        if e.kind() != ConnectionReset {
                            log::error!("udp socket error: {}, addr={}", e, addr);
                            break;
                        }
                    } else {
                        sleep(Duration::from_millis(100));
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
        id: [u8; 10],
    },
    #[serde(alias = "n")]
    Notify {
        #[serde(alias = "i")]
        id: [u8; 10],
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
            if !self.0.iter().any(|value| value == item) {
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

#[derive(Debug)]
struct Uuid([u8; 10]);

impl Uuid {
    fn new() -> Self {
        let mut uid = [0u8; 10];
        uid[..8].copy_from_slice(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_be_bytes()
                .as_slice(),
        );

        thread_rng().fill(&mut uid[8..]);
        Self(uid)
    }
}
