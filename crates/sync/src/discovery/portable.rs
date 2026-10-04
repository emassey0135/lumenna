//! DNS-SD through `mdns-sd`: Linux (BTSpeak included), Windows, Android.
//!
//! A responder of our own, standards-compliant, sharing port 5353 with whatever the system
//! runs — Avahi, Windows' own mDNS — as mDNS responders do.

use std::sync::Mutex;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use super::txt::{Announcement, instance};
use super::{Heard, Hearing};

/// Browses one service type and announces this endpoint under it.
pub(crate) struct Responder {
    daemon: ServiceDaemon,
    service_type: String,
    announced: Mutex<Option<String>>,
}

impl Responder {
    /// Starts browsing `service_type`, telling `heard` about everything found and lost.
    pub(crate) fn start(service_type: &str, heard: Hearing) -> std::io::Result<Self> {
        let service_type = format!("{service_type}.local.");
        let daemon = ServiceDaemon::new().map_err(std::io::Error::other)?;
        let events = daemon.browse(&service_type).map_err(std::io::Error::other)?;
        std::thread::Builder::new().name("lumenna-dnssd".to_owned()).spawn(move || {
            // Ends when the daemon shuts down and closes the channel.
            while let Ok(event) = events.recv() {
                match event {
                    ServiceEvent::ServiceResolved(service) => {
                        let properties = service.get_properties();
                        let pairs = properties.iter().map(|p| (p.key(), p.val_str()));
                        if let Some(mut announced) = Announcement::from_properties(pairs) {
                            // An announcement without addresses in TXT — from some other
                            // implementation — still has its A records and SRV port.
                            if announced.addrs.is_empty() {
                                let port = service.get_port();
                                announced.addrs = service
                                    .get_addresses()
                                    .iter()
                                    .map(|ip| (ip.to_ip_addr(), port).into())
                                    .collect();
                            }
                            heard(Heard::Found(service.get_fullname().to_owned(), announced));
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => heard(Heard::Lost(fullname)),
                    _ => {}
                }
            }
        })?;
        Ok(Self { daemon, service_type, announced: Mutex::new(None) })
    }

    /// Announces `announced`, replacing any earlier announcement.
    pub(crate) fn announce(&self, announced: &Announcement) -> std::io::Result<()> {
        let Some(port) = announced.addrs.first().map(std::net::SocketAddr::port) else {
            return Ok(());
        };
        let name = instance(&announced.id);
        let ips: Vec<std::net::IpAddr> = announced.addrs.iter().map(std::net::SocketAddr::ip).collect();
        let properties: std::collections::HashMap<String, String> =
            announced.properties().into_iter().collect();
        let info = ServiceInfo::new(
            &self.service_type,
            &name,
            &format!("{name}.local."),
            &ips[..],
            port,
            properties,
        )
        .map_err(std::io::Error::other)?;
        let fullname = info.get_fullname().to_owned();
        self.daemon.register(info).map_err(std::io::Error::other)?;
        *self.announced.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(fullname);
        Ok(())
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        // Says goodbye, so the others drop this one at once rather than when it expires.
        if let Some(fullname) =
            self.announced.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take()
        {
            let _ = self.daemon.unregister(&fullname);
        }
        let _ = self.daemon.shutdown();
    }
}
