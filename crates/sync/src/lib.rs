//! Sync between one person's devices (§7): Automerge documents over Iroh, and pairing.
//!
//! **No server ever sees the data** (§1). Devices dial each other by public key; Iroh finds a
//! path — the local network, a hole punched through a NAT, or a relay that forwards traffic it
//! cannot read — and authenticates both ends. Over that connection each document is reconciled
//! with Automerge's own sync protocol, which sends only what the other side lacks.
//!
//! - [`session`] — the document sync protocol itself, over any byte stream.
//! - [`pairing`] — confirming a new device by comparing words, and the handshake behind it.
//! - [`node`] — this device on the network: dialling and answering its paired devices.
//! - [`invite`] — the short-lived endpoint a pairing runs on.
//!
//! **Membership in the `devices` document is the trust boundary** (§7). A peer that is not
//! listed there is refused on the sync protocol, however it was found. The only way into the
//! list is a pairing whose words a person compared and confirmed on both devices.

pub mod discovery;
mod error;
mod framing;
pub mod invite;
pub mod node;
pub mod pairing;
pub mod session;

pub use error::{Result, SyncError};

/// The protocol a device speaks to its paired devices.
pub const ALPN_SYNC: &[u8] = b"lumenna/sync/0";

/// The protocol a pairing session speaks.
pub const ALPN_PAIR: &[u8] = b"lumenna/pair/0";

/// The store, shared between the tasks of one process.
///
/// A plain mutex, never held across an `.await`: every use locks, does one synchronous thing
/// against SQLite and Automerge, and lets go before touching the network.
pub type SharedStore = std::sync::Arc<std::sync::Mutex<lumenna_store::Store>>;

/// Locks the shared store, turning a poisoned lock into an error rather than a panic.
pub(crate) fn lock(store: &SharedStore) -> Result<std::sync::MutexGuard<'_, lumenna_store::Store>> {
    store.lock().map_err(|_| SyncError::Protocol("the store is wedged after an earlier failure".to_owned()))
}
