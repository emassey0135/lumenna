//! Finding devices and pairing sessions on the local network, by standard DNS-SD (§7).
//!
//! Iroh's own mDNS lookup announces in a form no DNS-SD browser recognises — no PTR record,
//! and every record sent with a time to live of zero, which mDNS reads as "gone". So Bonjour on
//! an iPhone could not hear a laptop, and the laptop's Avahi or Windows' mDNS could not either.
//! This announces and browses ordinary DNS-SD services instead (the names are in [`txt`]),
//! which every platform's mDNS understands:
//!
//! - **Apple** — macOS and iOS — through the system's own responder (`apple`). On iOS that is
//!   Bonjour, and needs no multicast entitlement.
//! - **Linux** through Avahi over D-Bus when it is running (`avahi`) — the BTSpeak, and most
//!   desktops. A second responder beside Avahi heard nothing there.
//! - **Everything else**, and Linux without Avahi, through `mdns-sd`, a standards-compliant
//!   responder of our own (`portable`).
//!
//! [`LocalLookup`] plugs into an endpoint as an Iroh address lookup: Iroh hands it the
//! endpoint's addresses to announce, and asks it where a key is. Pairing also subscribes to it,
//! to meet another waiting session.

mod txt;

#[cfg(target_vendor = "apple")]
mod apple;
#[cfg(target_os = "linux")]
mod avahi;
#[cfg(not(any(target_vendor = "apple", all(target_family = "wasm", target_os = "unknown"))))]
mod portable;

/// The responder in use: the system's own wherever there is one.
enum Responder {
    #[cfg(target_vendor = "apple")]
    Apple(apple::Responder),
    #[cfg(target_os = "linux")]
    Avahi(avahi::Responder),
    #[cfg(not(any(target_vendor = "apple", all(target_family = "wasm", target_os = "unknown"))))]
    Portable(portable::Responder),
}

impl Responder {
    /// Apple's own responder on Apple platforms; Avahi on Linux when it is running; and
    /// otherwise a responder of our own.
    fn start(service_type: &str, heard: Hearing) -> std::io::Result<Self> {
        #[cfg(target_vendor = "apple")]
        {
            apple::Responder::start(service_type, heard).map(Self::Apple)
        }
        // A browser has no local network to browse (§16.12): nothing is heard, as on a network
        // that forbids multicast, and pairing goes by code.
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            let _ = (service_type, heard);
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no local network in a browser"))
        }
        #[cfg(not(any(target_vendor = "apple", all(target_family = "wasm", target_os = "unknown"))))]
        {
            #[cfg(target_os = "linux")]
            if let Ok(avahi) = avahi::Responder::start(service_type, Arc::clone(&heard)) {
                return Ok(Self::Avahi(avahi));
            }
            portable::Responder::start(service_type, heard).map(Self::Portable)
        }
    }

    // Through `*self` with `ref`, so a browser's responder — which has no variants — still
    // matches exhaustively.
    #[cfg_attr(
        all(target_family = "wasm", target_os = "unknown"),
        expect(unused_variables, reason = "a browser has no responder to announce through")
    )]
    fn announce(&self, announced: &Announcement) -> std::io::Result<()> {
        match *self {
            #[cfg(target_vendor = "apple")]
            Self::Apple(ref responder) => responder.announce(announced),
            #[cfg(target_os = "linux")]
            Self::Avahi(ref responder) => responder.announce(announced),
            #[cfg(not(any(target_vendor = "apple", all(target_family = "wasm", target_os = "unknown"))))]
            Self::Portable(ref responder) => responder.announce(announced),
        }
    }
}

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use iroh::EndpointId;
use iroh::address_lookup::{
    AddressLookup, EndpointData, EndpointInfo, Error as LookupError, Item as LookupItem,
};
use n0_future::boxed::BoxStream;
use tokio::sync::broadcast;

use txt::{Announcement, service_type};

/// What the lookup reports as the source of an address, in Iroh's diagnostics.
const PROVENANCE: &str = "lumenna-dnssd";

/// A device or pairing session heard on the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its key.
    pub id: EndpointId,
    /// Where it listens.
    pub addrs: Vec<SocketAddr>,
}

/// What a responder heard: an instance found with what it announces, or an instance gone.
#[cfg_attr(
    all(target_family = "wasm", target_os = "unknown"),
    expect(dead_code, reason = "only a responder hears, and a browser has none")
)]
pub(crate) enum Heard {
    Found(String, Announcement),
    Lost(String),
}

/// Where a responder sends what it hears, from its own thread.
pub(crate) type Hearing = Arc<dyn Fn(Heard) + Send + Sync>;

struct Shared {
    me: EndpointId,
    advertise: bool,
    /// Instance name to what it announced.
    heard: Mutex<HashMap<String, Found>>,
    events: broadcast::Sender<Found>,
    /// `None` when the local network refuses mDNS — no reason to refuse everything else.
    responder: Mutex<Option<Responder>>,
}

/// Local discovery for one endpoint, as an Iroh address lookup.
#[derive(Clone)]
pub struct LocalLookup {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for LocalLookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalLookup").field("me", &self.shared.me).finish_non_exhaustive()
    }
}

impl LocalLookup {
    /// Starts browsing for `service`'s instances, and announcing `me` among them if
    /// `advertise`. A network that forbids multicast leaves it hearing nothing, not failing.
    #[must_use]
    pub fn start(me: EndpointId, service: &str, advertise: bool) -> Self {
        let (events, _) = broadcast::channel(64);
        let shared = Arc::new(Shared {
            me,
            advertise,
            heard: Mutex::default(),
            events,
            responder: Mutex::new(None),
        });
        let weak = Arc::downgrade(&shared);
        let hearing: Hearing = Arc::new(move |heard| {
            if let Some(shared) = weak.upgrade() {
                shared.hear(heard);
            }
        });
        let responder = Responder::start(&service_type(service), hearing).ok();
        *shared.responder.lock().unwrap_or_else(PoisonError::into_inner) = responder;
        Self { shared }
    }

    /// Everything heard so far, then everything heard from now on.
    pub fn subscribe(&self) -> (Vec<Found>, broadcast::Receiver<Found>) {
        // Subscribed before reading the cache, so nothing heard in between is missed; a
        // duplicate is harmless.
        let later = self.shared.events.subscribe();
        let now = self.shared.heard.lock().unwrap_or_else(PoisonError::into_inner).values().cloned().collect();
        (now, later)
    }

    /// Where `id` was last heard, if it was.
    #[must_use]
    pub fn heard(&self, id: EndpointId) -> Option<Vec<SocketAddr>> {
        let heard = self.shared.heard.lock().unwrap_or_else(PoisonError::into_inner);
        heard.values().find(|found| found.id == id).map(|found| found.addrs.clone())
    }
}

impl Shared {
    fn hear(&self, heard: Heard) {
        let mut cache = self.heard.lock().unwrap_or_else(PoisonError::into_inner);
        match heard {
            Heard::Found(instance, announced) if announced.id != self.me => {
                let found = Found { id: announced.id, addrs: announced.addrs };
                cache.insert(instance, found.clone());
                let _ = self.events.send(found);
            }
            Heard::Found(..) => {}
            Heard::Lost(instance) => {
                cache.remove(&instance);
            }
        }
    }
}

fn item(found: &Found) -> LookupItem {
    let data = EndpointData::new(
        found.addrs.iter().copied().map(iroh::TransportAddr::Ip).collect(),
    );
    LookupItem::new(EndpointInfo::from_parts(found.id, data), PROVENANCE, None)
}

impl AddressLookup for LocalLookup {
    fn publish(&self, data: &EndpointData) {
        if !self.shared.advertise {
            return;
        }
        let addrs: Vec<SocketAddr> = data.ip_addrs().copied().collect();
        let announced = Announcement { id: self.shared.me, addrs };
        if let Some(responder) =
            self.shared.responder.lock().unwrap_or_else(PoisonError::into_inner).as_ref()
        {
            let _ = responder.announce(&announced);
        }
    }

    fn resolve(&self, id: EndpointId) -> Option<BoxStream<Result<LookupItem, LookupError>>> {
        if id == self.shared.me {
            return None;
        }
        let (now, later) = self.subscribe();
        let first: Vec<Found> = now.into_iter().filter(|found| found.id == id).collect();
        let stream = n0_future::stream::unfold((first, later), move |(mut first, mut later)| async move {
            if let Some(found) = first.pop() {
                return Some((Ok(item(&found)), (first, later)));
            }
            loop {
                match later.recv().await {
                    Ok(found) if found.id == id => return Some((Ok(item(&found)), (first, later))),
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        });
        Some(Box::pin(stream))
    }
}
