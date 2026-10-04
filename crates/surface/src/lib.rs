//! Lumenna's command surface (§12): every operation once, typed, for every client.
//!
//! [`Lumenna`] is one open store, and its methods are the operations — `add_task(text) ->
//! Change`, `list_tasks(query) -> Rows`, and so on — taking and returning the plain records
//! in [`types`]. Everything a client can do is here, and nothing a client does is anywhere
//! else: no date parsing, no rule about what completing a task does to its subtasks. That is
//! principle 2, and it is what keeps eleven targets from becoming eleven implementations.
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
//! git-style. Row numbers are a terminal affordance (§15) and the CLI resolves them before it
//! calls in; here a bare number is refused rather than mistaken for a prefix.

mod durability;
mod error;
mod organise;
mod planning;
mod resolve;
mod settings;
mod tasks;
mod text;
pub mod types;
pub mod words;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use lumenna_core::edit;
use lumenna_core::snapshot::Snapshot;
use lumenna_store::Store;

pub use durability::{DEVICE_KEYS, cloud_warning};
pub use error::{LumennaError, Result};
pub use settings::{parse_every, parse_keep};
pub use types::*;

#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

/// The version of the shapes in [`types`], as `--json` and `lum rpc` carry them — §15's
/// compatibility contract. Bumped when a shape changes in a way a reader could not survive.
pub const CONTRACT: u32 = 1;

/// One open store, and everything that can be done with it.
///
/// Safe to share between threads: each operation takes the store for as long as it runs.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct Lumenna {
    store: Mutex<Store>,
    directory: PathBuf,
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
    pub fn open(directory: &str) -> Result<std::sync::Arc<Self>> {
        Self::open_at(Path::new(directory)).map(std::sync::Arc::new)
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
        std::fs::create_dir_all(directory)?;
        let mut store = Store::open(&directory.join("lumenna.sqlite"))?;
        // Every store has exactly one Inbox, and the store creates it under the same
        // identifier everywhere (§3.4). A store from before that minted its own, which is
        // folded into the shared one here, once.
        let adopt = edit::adopt_inbox(&repaired(&store));
        if !adopt.is_empty() {
            store.apply(&adopt)?;
        }
        Ok(Self { store: Mutex::new(store), directory: directory.to_path_buf() })
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
        operation(&mut store)
    }
}

/// The repaired snapshot every query runs against.
#[must_use]
pub fn repaired(store: &Store) -> Snapshot {
    let (mut snapshot, _) = store.snapshot();
    // §3.13: merge can produce cycles and dangling references that no single device ever
    // wrote. Repairing on read means every query can assume a tree.
    snapshot.repair();
    snapshot
}
