use std::{fmt::Debug, net::Ipv4Addr, thread};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error(transparent)]
    MdnsError(#[from] mdns_sd::Error),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
}

/// LAN service discovery.
///
/// which exposes its services through the MDNS protocol
/// and can allow other nodes or clients to discover the current service.
pub struct DiscoveryService(ServiceDaemon);

impl DiscoveryService {
    /// Register the service, the service type is fixed, you can customize the
    /// port number, in properties you can add
    /// customized data to the published service.
    pub fn register<P: Serialize + Debug>(
        port: u16,
        properties: &P,
    ) -> Result<Self, DiscoveryError> {
        let mdns = ServiceDaemon::new()?;
        mdns.disable_interface(IfKind::IPv6)?;

        let id = Uuid::new_v4().to_string();
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

    /// Query the registered service, the service type is fixed, when the query
    /// is published the callback function will call back all the network
    /// addresses of the service publisher as well as the attribute information.
    pub fn query<P: DeserializeOwned + Debug, T: Fn(Vec<Ipv4Addr>, P) + Send + 'static>(
        func: T,
    ) -> Result<Self, DiscoveryError> {
        let mdns = ServiceDaemon::new()?;
        mdns.disable_interface(IfKind::IPv6)?;

        let receiver = mdns.browse("_hylarana._udp.local.")?;
        thread::spawn(move || {
            let process = |info: ServiceInfo| {
                let properties =
                    serde_json::from_str(info.get_property("properties")?.val_str()).ok()?;
                let addrs = info
                    .get_addresses_v4()
                    .into_iter()
                    .map(|it| *it)
                    .collect::<Vec<_>>();

                log::info!(
                    "discovery service query a sender, host={}, address={:?}, properties={:?}",
                    info.get_hostname(),
                    addrs,
                    properties,
                );

                func(addrs, properties);
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
        let _ = self.0.unregister("_hylarana._udp.local.");
        let _ = self.0.stop_browse("_hylarana._udp.local.");
    }
}
