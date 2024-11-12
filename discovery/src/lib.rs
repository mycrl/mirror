use std::{collections::HashMap, net::Ipv4Addr, thread};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};
use thiserror::Error;

pub type Properties = HashMap<String, String>;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error(transparent)]
    MdnsError(#[from] mdns_sd::Error),
}

pub struct Discovery(ServiceDaemon);

impl Discovery {
    pub fn new(id: &str, properties: Properties) -> Result<Self, DiscoveryError> {
        let mdns = ServiceDaemon::new()?;
        mdns.disable_interface(IfKind::IPv6)?;

        mdns.register(
            ServiceInfo::new(
                "_hylarana._udp.local.",
                "sender",
                &format!("{}._hylarana._udp.local.", id),
                "",
                3456,
                properties
                    .iter()
                    .map(|(k, v)| (k, v))
                    .collect::<Vec<_>>()
                    .as_slice(),
            )?
            .enable_addr_auto(),
        )?;

        Ok(Self(mdns))
    }

    pub fn query<T: FnOnce(Vec<Ipv4Addr>, Properties) + Send + 'static>(
        func: T,
    ) -> Result<Self, DiscoveryError> {
        let mdns = ServiceDaemon::new()?;
        mdns.disable_interface(IfKind::IPv6)?;

        let mut func = Some(func);
        let receiver = mdns.browse("_hylarana._udp.local.")?;
        thread::spawn(move || {
            while let Ok(ServiceEvent::ServiceResolved(info)) = receiver.recv() {
                if info.get_fullname() == "sender._hylarana._udp.local." {
                    if let Some(func) = func.take() {
                        func(
                            info.get_addresses_v4()
                                .into_iter()
                                .map(|it| *it)
                                .collect::<Vec<_>>(),
                            info.get_properties()
                                .iter()
                                .map(|it| (it.key().to_string(), it.val_str().to_string()))
                                .collect::<Properties>(),
                        )
                    }
                }
            }
        });

        Ok(Self(mdns))
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.0.unregister("sender._hylarana._udp.local.");
        let _ = self.0.stop_browse("_hylarana._udp.local.");
    }
}
