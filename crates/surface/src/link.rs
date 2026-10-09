//! The Apple Watch's link to its iPhone: sync by message, where the watch has no network.
//!
//! watchOS allows no sockets outside an audio session, so the watch runs no Iroh endpoint.
//! It keeps a whole store of its own and reconciles it with its iPhone's over
//! WatchConnectivity, which carries a message and its reply; the phone passes what the watch
//! wrote on to every other device in its own rounds. Both apps hold a [`PhoneLink`]: the
//! watch starts an exchange and takes each reply, the phone answers.
//!
//! Trust is the pairing of the watch to the phone, which only the system makes; the watch is
//! not in the device list and pairs with nothing.

use std::sync::{Arc, Mutex, PoisonError};

use lumenna_sync::SyncError;
use lumenna_sync::exchange::Exchange;

use crate::{Lumenna, LumennaError, Result};

impl From<SyncError> for LumennaError {
    fn from(error: SyncError) -> Self {
        Self::new(error.to_string())
    }
}

/// One side of the link between a watch and its phone.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct PhoneLink {
    lumenna: Arc<Lumenna>,
    exchange: Mutex<Exchange>,
    took_in: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for PhoneLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhoneLink").finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl PhoneLink {
    /// A link over `lumenna`'s own store connection, so what arrives is what its operations
    /// read next.
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    #[must_use]
    pub fn new(lumenna: Arc<Lumenna>) -> Arc<Self> {
        Arc::new(Self { lumenna, exchange: Mutex::new(Exchange::new()), took_in: false.into() })
    }

    /// Starts an exchange, from the watch: the first message to send. Whatever exchange was
    /// under way is dropped, here and, on reading this, on the phone.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn start(&self) -> Result<Vec<u8>> {
        let mut exchange = self.exchange();
        *exchange = Exchange::new();
        self.lumenna.with(|store| Ok(exchange.outgoing(store)?))
    }

    /// Takes the phone's reply to the last message sent, and returns the next message to
    /// send, or nothing when the two stores agree.
    ///
    /// # Errors
    ///
    /// If the reply does not read, or the store refuses what it brought.
    pub fn take(&self, reply: Vec<u8>) -> Result<Option<Vec<u8>>> {
        let mut exchange = self.exchange();
        self.lumenna.with(|store| {
            self.note(&exchange.incoming(store, &reply)?);
            if exchange.settled() { Ok(None) } else { Ok(Some(exchange.outgoing(store)?)) }
        })
    }

    /// Answers a message from the watch, on the phone: takes in what it brought, and returns
    /// the reply to send back.
    ///
    /// # Errors
    ///
    /// If the message does not read, or the store refuses what it brought.
    pub fn answer(&self, message: Vec<u8>) -> Result<Vec<u8>> {
        let mut exchange = self.exchange();
        self.lumenna.with(|store| {
            self.note(&exchange.incoming(store, &message)?);
            Ok(exchange.outgoing(store)?)
        })
    }

    /// Whether anything came in over the link since this was last asked, so the app redraws:
    /// it arrived on the app's own connection, which `outside_version` does not count.
    pub fn took_in(&self) -> bool {
        self.took_in.swap(false, std::sync::atomic::Ordering::Relaxed)
    }
}

impl PhoneLink {
    fn exchange(&self) -> std::sync::MutexGuard<'_, Exchange> {
        self.exchange.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes that documents changed: for [`took_in`](Self::took_in), and so a loop the merge
    /// made is looked for straight after, as after a restore.
    fn note(&self, changed: &[lumenna_store::DocId]) {
        if !changed.is_empty() {
            self.took_in.store(true, std::sync::atomic::Ordering::Relaxed);
            self.lumenna.merged();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> (tempfile::TempDir, Arc<Lumenna>) {
        let directory = tempfile::tempdir().unwrap();
        let lumenna = Lumenna::open(&directory.path().to_string_lossy()).unwrap();
        (directory, lumenna)
    }

    #[test]
    fn a_task_added_on_the_watch_is_on_the_phone_after_one_exchange_and_back() {
        let ((_w, watch), (_p, phone)) = (open(), open());
        watch.add_task("water the plants").unwrap();
        phone.add_task("call the bank").unwrap();
        let (on_watch, on_phone) = (PhoneLink::new(Arc::clone(&watch)), PhoneLink::new(Arc::clone(&phone)));

        let mut message = on_watch.start().unwrap();
        while let Some(next) = on_watch.take(on_phone.answer(message).unwrap()).unwrap() {
            message = next;
        }

        let titles = |lumenna: &Lumenna| {
            let mut titles: Vec<String> =
                lumenna.list_tasks("").unwrap().rows.into_iter().map(|row| row.title).collect();
            titles.sort();
            titles
        };
        assert_eq!(titles(&watch), ["call the bank", "water the plants"]);
        assert_eq!(titles(&phone), ["call the bank", "water the plants"]);
        assert!(on_watch.took_in() && on_phone.took_in(), "each side is told to redraw");
        assert!(!on_watch.took_in(), "and told once");
    }
}
