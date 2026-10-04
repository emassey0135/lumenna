//! DNS-SD through Avahi, the system's own responder on most Linux systems — the BTSpeak's
//! included — over D-Bus.
//!
//! Where Avahi runs, a second responder beside it is no good: on the BTSpeak, `mdns-sd`
//! could announce but heard nothing at all while Avahi held port 5353, not even itself. So
//! where Avahi answers on the system bus it does the work, as `mDNSResponder` does on Apple
//! platforms, and `mdns-sd` is only for systems without one.
//!
//! The API is Avahi's `org.freedesktop.Avahi` interface, version 0.8: a service browser
//! whose `ItemNew` and `ItemRemove` signals report instances, `ResolveService` to read one,
//! and an entry group to announce this endpoint.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::OwnedObjectPath;

use super::txt::{Announcement, instance};
use super::{Heard, Hearing};

const AVAHI: &str = "org.freedesktop.Avahi";
const SERVER: &str = "org.freedesktop.Avahi.Server";
const BROWSER: &str = "org.freedesktop.Avahi.ServiceBrowser";
const ENTRY_GROUP: &str = "org.freedesktop.Avahi.EntryGroup";
/// `AVAHI_IF_UNSPEC` and `AVAHI_PROTO_UNSPEC`: every interface, both IPv4 and IPv6.
const ANY: i32 = -1;

fn failed(error: zbus::Error) -> std::io::Error {
    std::io::Error::other(format!("Avahi: {error}"))
}

/// What `ResolveService` returns.
type Resolved = (i32, i32, String, String, String, String, i32, String, u16, Vec<Vec<u8>>, u32);

/// Reads one instance's TXT record and address through Avahi.
fn resolve(
    connection: &Connection,
    interface: i32,
    protocol: i32,
    name: &str,
    service_type: &str,
    domain: &str,
) -> Option<Announcement> {
    let reply = connection
        .call_method(
            Some(AVAHI),
            "/",
            Some(SERVER),
            "ResolveService",
            &(interface, protocol, name, service_type, domain, ANY, 0_u32),
        )
        .ok()?;
    let resolved: Resolved = reply.body().deserialize().ok()?;
    let (address, port, txt) = (resolved.7, resolved.8, resolved.9);
    let entries: Vec<String> = txt.iter().map(|entry| String::from_utf8_lossy(entry).into_owned()).collect();
    let pairs = entries.iter().map(|entry| entry.split_once('=').unwrap_or((entry, "")));
    let mut announced = Announcement::from_properties(pairs)?;
    if announced.addrs.is_empty()
        && let Ok(ip) = address.parse::<std::net::IpAddr>()
    {
        announced.addrs.push((ip, port).into());
    }
    Some(announced)
}

/// This endpoint's entry group, and what it last announced.
struct Group {
    path: OwnedObjectPath,
    name: String,
    port: u16,
}

/// Browses one service type and announces this endpoint under it, through Avahi.
pub(crate) struct Responder {
    connection: Connection,
    service_type: String,
    browser: OwnedObjectPath,
    group: Mutex<Option<Group>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Responder {
    /// Starts browsing `service_type` through Avahi, or fails if Avahi is not running.
    pub(crate) fn start(service_type: &str, heard: Hearing) -> std::io::Result<Self> {
        let connection = Connection::system().map_err(failed)?;
        connection
            .call_method(Some(AVAHI), "/", Some(SERVER), "GetVersionString", &())
            .map_err(failed)?;

        // Subscribed before the browser exists: Avahi reports what it already knows the
        // moment the browser is made, and a subscription made after would miss it.
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(AVAHI)
            .map_err(failed)?
            .interface(BROWSER)
            .map_err(failed)?
            .build();
        let messages = MessageIterator::for_match_rule(rule, &connection, Some(256)).map_err(failed)?;
        let browser: OwnedObjectPath = connection
            .call_method(
                Some(AVAHI),
                "/",
                Some(SERVER),
                "ServiceBrowserNew",
                &(ANY, ANY, service_type, "", 0_u32),
            )
            .map_err(failed)?
            .body()
            .deserialize()
            .map_err(failed)?;

        let (watching, mine) = (connection.clone(), browser.clone());
        let thread = std::thread::Builder::new().name("lumenna-avahi".to_owned()).spawn(move || {
            // Avahi reports an instance once per interface and protocol; it is read once,
            // when first heard, and lost only when gone from all of them.
            let on: Arc<Mutex<HashMap<String, HashSet<(i32, i32)>>>> = Arc::default();
            // Ends when the connection closes, as the responder does on drop.
            for message in messages {
                let Ok(message) = message else { break };
                let header = message.header();
                if header.path().is_none_or(|path| path.as_str() != mine.as_str()) {
                    continue;
                }
                let Some(member) = header.member().map(|m| m.to_string()) else { continue };
                let Ok((interface, protocol, name, kind, domain, _flags)) =
                    message.body().deserialize::<(i32, i32, String, String, String, u32)>()
                else {
                    continue;
                };
                let mut seen = on.lock().unwrap_or_else(PoisonError::into_inner);
                match member.as_str() {
                    "ItemNew" => {
                        let places = seen.entry(name.clone()).or_default();
                        if places.insert((interface, protocol)) && places.len() == 1 {
                            // Read on a thread of its own, so an instance whose device has
                            // gone times out there without holding up the rest.
                            let (connection, heard) = (watching.clone(), Arc::clone(&heard));
                            let _ = std::thread::Builder::new()
                                .name("lumenna-avahi-resolve".to_owned())
                                .spawn(move || {
                                    if let Some(announced) =
                                        resolve(&connection, interface, protocol, &name, &kind, &domain)
                                    {
                                        heard(Heard::Found(name, announced));
                                    }
                                });
                        }
                    }
                    "ItemRemove" => {
                        if let Some(places) = seen.get_mut(&name) {
                            places.remove(&(interface, protocol));
                            if places.is_empty() {
                                seen.remove(&name);
                                heard(Heard::Lost(name));
                            }
                        }
                    }
                    _ => {}
                }
            }
        })?;
        Ok(Self {
            connection,
            service_type: service_type.to_owned(),
            browser,
            group: Mutex::new(None),
            thread: Some(thread),
        })
    }

    /// Announces `announced`, replacing any earlier announcement.
    pub(crate) fn announce(&self, announced: &Announcement) -> std::io::Result<()> {
        let Some(port) = announced.addrs.first().map(std::net::SocketAddr::port) else {
            return Ok(());
        };
        let txt: Vec<Vec<u8>> = announced
            .properties()
            .into_iter()
            .map(|(key, value)| format!("{key}={value}").into_bytes())
            .collect();
        let mut group = self.group.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(current) = group.as_ref().filter(|g| g.port == port) {
            // Same port, new addresses: only the TXT record changes.
            self.connection
                .call_method(
                    Some(AVAHI),
                    current.path.as_str(),
                    Some(ENTRY_GROUP),
                    "UpdateServiceTxt",
                    &(ANY, ANY, 0_u32, current.name.as_str(), self.service_type.as_str(), "", txt),
                )
                .map_err(failed)?;
            return Ok(());
        }
        let path: OwnedObjectPath = match group.take() {
            Some(old) => {
                self.connection
                    .call_method(Some(AVAHI), old.path.as_str(), Some(ENTRY_GROUP), "Reset", &())
                    .map_err(failed)?;
                old.path
            }
            None => self
                .connection
                .call_method(Some(AVAHI), "/", Some(SERVER), "EntryGroupNew", &())
                .map_err(failed)?
                .body()
                .deserialize()
                .map_err(failed)?,
        };
        let name = instance(&announced.id);
        self.connection
            .call_method(
                Some(AVAHI),
                path.as_str(),
                Some(ENTRY_GROUP),
                "AddService",
                &(ANY, ANY, 0_u32, name.as_str(), self.service_type.as_str(), "", "", port, txt),
            )
            .map_err(failed)?;
        self.connection
            .call_method(Some(AVAHI), path.as_str(), Some(ENTRY_GROUP), "Commit", &())
            .map_err(failed)?;
        *group = Some(Group { path, name, port });
        Ok(())
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        // Withdrawing the announcement and the browser; then closing the connection ends the
        // watching thread's message stream.
        if let Some(group) = self.group.lock().unwrap_or_else(PoisonError::into_inner).take() {
            let _ = self.connection.call_method(Some(AVAHI), group.path.as_str(), Some(ENTRY_GROUP), "Free", &());
        }
        let _ = self.connection.call_method(Some(AVAHI), self.browser.as_str(), Some(BROWSER), "Free", &());
        let _ = self.connection.clone().close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
