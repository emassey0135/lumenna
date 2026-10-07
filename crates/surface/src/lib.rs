//! Lumenna's command surface: every operation once, typed, for every client.
//!
//! [`Lumenna`] is one open store, and its methods are the operations — `add_task(text) ->
//! Change`, `list_tasks(query) -> Rows`, and so on — taking and returning the plain records
//! in [`types`]. Everything a client can do is here, and nothing a client does is anywhere
//! else: no date parsing, no rule about what completing a task does to its subtasks. That is
//! what keeps eleven targets from becoming eleven implementations.
//!
//! # Every client, one definition
//!
//! - **The command line** links this crate and renders what it returns (`apps/cli`).
//! - **iOS, macOS, Android and Wear OS** get the same object through UniFFI (the `uniffi`
//!   feature, built by `crates/ffi`): a Swift `Lumenna` class whose methods return Swift
//!   structs, checked by the compiler on both sides. Nothing crosses the boundary as JSON.
//! - **Emacs and the BTSpeak app**, which cannot link Rust, reach it over `lum rpc`, which
//!   serialises these same records. The JSON-RPC layer is an adapter over this crate, not
//!   the definition of the surface.
//!
//! A method added here is on every client that links the crate the next time it builds, and
//! a reshaped record fails to compile everywhere that used the old shape — the drift a second
//! definition would allow cannot happen.
//!
//! # Identifiers
//!
//! Methods take identifiers as text: a whole one, or a prefix long enough to name one record,
//! git-style. Row numbers are a terminal affordance and the CLI resolves them before it
//! calls in; here a bare number is refused rather than mistaken for a prefix.

mod durability;
#[cfg(any(unix, windows))]
pub mod endpoint;
mod error;
mod form;
mod organise;
mod planning;
mod resolve;
#[cfg(all(feature = "rpc", any(unix, windows)))]
pub mod rpc;
mod settings;
#[cfg(feature = "sync")]
mod sync;
mod tasks;
mod text;
pub mod types;
pub mod words;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lumenna_core::edit;
use lumenna_core::snapshot::Snapshot;
use lumenna_store::Store;

pub use durability::{DEVICE_KEYS, cloud_warning};
pub use error::{LumennaError, Result};
pub use form::{parse_weight, BlockDefaults, BlockFields, TaskFields, block_defaults, block_edit, block_fields, day_block_fields, new_block, label_reference, project_reference, sitting_status, task_edit, task_fields};
pub use settings::{parse_every, parse_keep};
#[cfg(feature = "sync")]
pub use sync::{PairingPrompt, SyncListener, SyncLoop, keep_in_sync};
#[cfg(all(feature = "sync", not(all(target_family = "wasm", target_os = "unknown"))))]
pub use sync::SyncService;
pub use types::*;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

/// The version of the shapes in [`types`], as `--json` and `lum rpc` carry them: a
/// compatibility contract. Bumped when a shape changes in a way a reader could not survive.
pub const CONTRACT: u32 = 1;

/// One open store, and everything that can be done with it.
///
/// Safe to share between threads: each operation takes the store for as long as it runs.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct Lumenna {
    // Shared, so that a sync endpoint can work on the same connection the operations do:
    // what it brings in is visible at once, and what they write it can see to send on.
    store: Arc<Mutex<Store>>,
    directory: PathBuf,
    // The native service, if one is running; a browser runs its loop itself.
    #[cfg(all(feature = "sync", not(all(target_family = "wasm", target_os = "unknown"))))]
    sync: sync::SyncState,
    /// The documents' heads after the last operation, so a repair is looked for only when
    /// something came in from outside: an operation's own writes cannot make a loop.
    looked: Mutex<Option<Vec<lumenna_store::ChangeHash>>>,
    /// Repairs made and not yet said, for the next record that can carry a notice.
    unsaid: Mutex<Vec<String>>,
    /// Set by an operation that merged another store's changes in — a restore, an import —
    /// which can make a loop, so the repair is looked for straight after it.
    merged: std::sync::atomic::AtomicBool,
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Opens the profile in `directory`, creating it if this is the first run.
    ///
    /// Does not take a backup: call [`back_up_if_due`](Self::back_up_if_due) at launch,
    /// where a failure can be said.
    ///
    /// # Errors
    ///
    /// If the directory cannot be created or the store cannot be opened.
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn open(directory: &str) -> Result<Arc<Self>> {
        Self::open_at(Path::new(directory)).map(Arc::new)
    }

    /// Takes in what another process or a sync wrote since last asked, and says whether
    /// anything did.
    ///
    /// Every operation does this first, so an answer is never staler than the last change
    /// on disk; a client calls it on its own to learn that it should redraw.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn refresh(&self) -> Result<bool> {
        Ok(self.store().refresh()?)
    }

    /// Takes in what another process wrote, and returns a number that moves whenever another
    /// process — `lum`, the daemon — has written since, and for nothing else.
    ///
    /// This, not [`refresh`](Self::refresh), is how an app learns `lum` wrote. `refresh` says
    /// whether *that call* took anything in, and every operation refreshes first, as does the
    /// app's own sync loop each tick: whichever call comes first after `lum` writes is the one
    /// told, and a once-a-second check of `refresh` misses the change whenever another call
    /// got there first. No call can use this up.
    ///
    /// It is for an app that already redraws for its own edits as it makes them, and hears of
    /// a sync's arrivals from its `SyncService`: what is left to notice is another process. A
    /// version that also moved for the app's own edits would redraw everything a second time
    /// a moment later.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn outside_version(&self) -> Result<i64> {
        let mut store = self.store();
        store.refresh()?;
        Ok(store.outside_version()?)
    }

    /// The profile directory.
    #[must_use]
    pub fn directory(&self) -> String {
        self.directory.display().to_string()
    }
}

impl Lumenna {
    /// Opens the profile in `directory`, creating it if this is the first run.
    ///
    /// # Errors
    ///
    /// If the directory cannot be created or the store cannot be opened.
    pub fn open_at(directory: &Path) -> Result<Self> {
        // A browser's store is a file in OPFS's pool, named by its path, with no directory
        // to make.
        if !cfg!(all(target_family = "wasm", target_os = "unknown")) {
            std::fs::create_dir_all(directory)?;
        }
        let mut store = Store::open(&directory.join("lumenna.sqlite"))?;
        // Every store has exactly one Inbox, and the store creates it under the same
        // identifier everywhere. A store from before that minted its own, which is
        // folded into the shared one here, once.
        let adopt = edit::adopt_inbox(&repaired(&store));
        if !adopt.is_empty() {
            store.apply(&adopt)?;
        }
        let lumenna = Self {
            store: Arc::new(Mutex::new(store)),
            directory: directory.to_path_buf(),
            #[cfg(all(feature = "sync", not(all(target_family = "wasm", target_os = "unknown"))))]
            sync: sync::SyncState::default(),
            looked: Mutex::new(None),
            unsaid: Mutex::new(Vec::new()),
            merged: std::sync::atomic::AtomicBool::new(false),
        };
        // A device that took a new build says so in the device list, for the others to see.
        // Failing to is no reason not to open.
        #[cfg(feature = "sync")]
        let _ = lumenna.note_own_version();
        Ok(lumenna)
    }

    /// The store itself, for the parts of a client that are not operations on it — the sync
    /// endpoint, which drives Automerge's protocol directly.
    ///
    /// Held for as long as the guard lives, and every method here takes it too: drop it
    /// before calling one.
    pub fn store(&self) -> MutexGuard<'_, Store> {
        // A panic mid-operation leaves nothing half-written that the next one cannot read:
        // the store commits whole changes or none.
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The store as the sync crate takes it, on the same connection.
    #[cfg(feature = "sync")]
    fn shared(&self) -> lumenna_sync::SharedStore {
        Arc::clone(&self.store)
    }

    /// The profile directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.directory
    }

    /// The store's file.
    #[must_use]
    pub fn store_path(&self) -> PathBuf {
        self.directory.join("lumenna.sqlite")
    }

    /// Runs one operation against a fresh view of the store.
    fn with<T>(&self, operation: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        let mut store = self.store();
        // Another process may have written since, and answering from a stale document would
        // be a wrong answer rather than a slow one.
        store.refresh()?;
        self.write_repairs(&mut store)?;
        let result = operation(&mut store);
        if self.merged.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.write_repairs(&mut store)?;
        } else {
            *self.looked.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(store.version());
        }
        result
    }

    /// Marks the operation running as one that merged changes in from elsewhere.
    pub(crate) fn merged(&self) {
        self.merged.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// As [`with`](Self::with), and says any repair made since on the record it returns.
    fn told<T: Announced>(&self, operation: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        let mut record = self.with(operation)?;
        let unsaid = std::mem::take(&mut *self.unsaid.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
        record.notices_mut().extend(unsaid);
        Ok(record)
    }

    /// Writes down any repair a merge made necessary, once, keeping what to say about it.
    fn write_repairs(&self, store: &mut Store) -> Result<()> {
        let version = store.version();
        let mut looked = self.looked.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if looked.as_ref() == Some(&version) {
            return Ok(());
        }
        let (snapshot, _) = store.snapshot();
        let (repair, said) = edit::repair(&snapshot);
        if !repair.is_empty() {
            store.apply(&repair)?;
            self.unsaid.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend(said);
        }
        *looked = Some(store.version());
        Ok(())
    }
}

/// The repaired snapshot every query runs against.
#[must_use]
pub fn repaired(store: &Store) -> Snapshot {
    let (mut snapshot, _) = store.snapshot();
    // Merge can produce cycles and dangling references that no single device ever
    // wrote. Repairing on read means every query can assume a tree.
    snapshot.repair();
    snapshot
}
