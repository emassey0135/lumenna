//! What a Lumenna announcement says, as DNS-SD TXT keys, and the names it goes by.
//!
//! One format for every platform, because the point is that a phone, a Mac, a Windows laptop
//! and a BTSpeak each hear what the others announce:
//!
//! - the service type is `_lumenna._udp` for devices and `_lumenna-pair._udp` for pairing
//!   sessions;
//! - the instance is `lumenna-` and the first sixteen characters of the key — a DNS label
//!   holds 63 bytes, and a key written out is 64;
//! - TXT `id` is the whole key, and `a0`, `a1`, … each one address, `ip:port`.
//!
//! The addresses are in TXT, not only in SRV and A records, because an endpoint may listen on
//! different ports for IPv4 and IPv6 and an SRV record holds one. The SRV and A records are
//! still published, so any DNS-SD browser sees a well-formed service.

use std::net::SocketAddr;

use iroh::EndpointId;

/// The TXT key for the whole key.
const ID: &str = "id";

/// The most addresses one announcement carries. Each TXT string is at most 255 bytes, and a
/// device rarely has more than a handful of addresses worth trying.
const MOST_ADDRESSES: usize = 12;

/// A device or pairing session, as announced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Announcement {
    /// Its key.
    pub id: EndpointId,
    /// Where it listens.
    pub addrs: Vec<SocketAddr>,
}

/// The DNS-SD service type for a service name: `lumenna` becomes `_lumenna._udp`.
pub(crate) fn service_type(service: &str) -> String {
    format!("_{service}._udp")
}

/// The instance name an endpoint announces under.
#[cfg_attr(
    all(target_family = "wasm", target_os = "unknown"),
    expect(dead_code, reason = "only a responder hears, and a browser has none")
)]
pub(crate) fn instance(id: &EndpointId) -> String {
    let text = id.to_string();
    format!("lumenna-{}", &text[..text.len().min(16)])
}

impl Announcement {
    /// The TXT key-value pairs.
    pub(crate) fn properties(&self) -> Vec<(String, String)> {
        std::iter::once((ID.to_owned(), self.id.to_string()))
            .chain(
                self.addrs
                    .iter()
                    .take(MOST_ADDRESSES)
                    .enumerate()
                    .map(|(n, addr)| (format!("a{n}"), addr.to_string())),
            )
            .collect()
    }

    /// Reads an announcement back from TXT key-value pairs. Anything not an id or an
    /// address is ignored, so a later version may add keys.
    pub(crate) fn from_properties<'a>(
        properties: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Option<Self> {
        let mut id = None;
        let mut addrs = Vec::new();
        for (key, value) in properties {
            if key.eq_ignore_ascii_case(ID) {
                id = value.parse().ok();
            } else if key.starts_with('a') && key[1..].chars().all(|c| c.is_ascii_digit()) {
                addrs.extend(value.parse::<SocketAddr>().ok());
            }
        }
        Some(Self { id: id?, addrs })
    }

    /// The TXT record as DNS-SD carries it: each `key=value` prefixed with its length.
    /// Apple's API takes the wire form; `mdns-sd` takes the pairs.
    #[cfg_attr(not(target_vendor = "apple"), allow(dead_code, reason = "used by the Apple backend"))]
    pub(crate) fn to_record(&self) -> Vec<u8> {
        let mut record = Vec::new();
        for (key, value) in self.properties() {
            let entry = format!("{key}={value}");
            let bytes = &entry.as_bytes()[..entry.len().min(255)];
            #[expect(clippy::cast_possible_truncation, reason = "capped at 255 just above")]
            record.push(bytes.len() as u8);
            record.extend_from_slice(bytes);
        }
        record
    }

    /// Reads a TXT record in its wire form.
    #[cfg_attr(not(target_vendor = "apple"), allow(dead_code, reason = "used by the Apple backend"))]
    pub(crate) fn from_record(record: &[u8]) -> Option<Self> {
        let mut entries = Vec::new();
        let mut rest = record;
        while let Some((&len, tail)) = rest.split_first() {
            let len = usize::from(len).min(tail.len());
            let (entry, next) = tail.split_at(len);
            if let Ok(text) = std::str::from_utf8(entry) {
                entries.push(text.split_once('=').unwrap_or((text, "")));
            }
            rest = next;
        }
        Self::from_properties(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> EndpointId {
        iroh::SecretKey::generate().public()
    }

    #[test]
    fn an_announcement_survives_the_wire_both_ways() {
        let announced = Announcement {
            id: key(),
            addrs: vec!["192.168.1.20:4433".parse().unwrap(), "[fe80::1]:4434".parse().unwrap()],
        };
        assert_eq!(Announcement::from_record(&announced.to_record()), Some(announced.clone()));
        let pairs = announced.properties();
        let borrowed = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str()));
        assert_eq!(Announcement::from_properties(borrowed), Some(announced));
    }

    #[test]
    fn keys_a_later_version_adds_are_ignored_and_a_missing_id_is_no_announcement() {
        let id = key();
        let found = Announcement::from_properties([("id", &*id.to_string()), ("colour", "teal")]);
        assert_eq!(found, Some(Announcement { id, addrs: vec![] }));
        assert_eq!(Announcement::from_properties([("a0", "10.0.0.1:1")]), None);
    }

    #[test]
    fn an_instance_name_fits_in_a_dns_label() {
        assert!(instance(&key()).len() <= 63);
    }
}
