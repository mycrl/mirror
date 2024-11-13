use std::{fmt::Debug, net::Ipv4Addr, thread};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error(transparent)]
    MdnsError(#[from] mdns_sd::Error),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
}

pub struct DiscoveryService(ServiceDaemon);

impl DiscoveryService {
    pub fn new<P: Serialize + Debug>(
        port: u16,
        id: &str,
        properties: &P,
    ) -> Result<Self, DiscoveryError> {
        let mdns = ServiceDaemon::new()?;
        mdns.disable_interface(IfKind::IPv6)?;

        mdns.register(
            ServiceInfo::new(
                "_hylarana._udp.local.",
                "sender",
                &format!("{}._hylarana._udp.local.", id),
                "",
                port,
                &[("properties", serde_json::to_string(properties)?)][..],
            )?
            .enable_addr_auto(),
        )?;

        log::info!(
            "discovery service register sender, port={}, id={}, properties={:?}",
            port,
            id,
            properties
        );

        Ok(Self(mdns))
    }

    pub fn query<P: DeserializeOwned, T: FnOnce(Vec<Ipv4Addr>, P) + Send + 'static>(
        func: T,
    ) -> Result<Self, DiscoveryError> {
        let mdns = ServiceDaemon::new()?;
        mdns.disable_interface(IfKind::IPv6)?;

        let mut func = Some(func);
        let receiver = mdns.browse("_hylarana._udp.local.")?;
        thread::spawn(move || {
            let mut process = |info: ServiceInfo| {
                if let Some(func) = func.take() {
                    func(
                        info.get_addresses_v4()
                            .into_iter()
                            .map(|it| *it)
                            .collect::<Vec<_>>(),
                        serde_json::from_str(info.get_property("properties")?.val_str()).ok()?,
                    );
                }

                Some(())
            };

            loop {
                match receiver.recv() {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        if info.get_fullname() == "sender._hylarana._udp.local." {
                            process(info);
                        }
                    }
                    Err(_) => break,
                    Ok(event) => {
                        log::info!("discovery service query event={:?}", event);
                    }
                }
            }
        });

        Ok(Self(mdns))
    }
}

impl Drop for DiscoveryService {
    fn drop(&mut self) {
        let _ = self.0.unregister("sender._hylarana._udp.local.");
        let _ = self.0.stop_browse("_hylarana._udp.local.");
    }
}
