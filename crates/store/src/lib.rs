//! Lumenna's persistence layer: Automerge documents, and the SQLite file they live in.
//!
//! `core` holds the model and knows nothing about CRDTs; this crate is the translation.
//! It hydrates Automerge documents into [`Snapshot`](lumenna_core::snapshot::Snapshot) and
//! turns edits back into Automerge operations, so that the read model §8 leaves optional
//! can arrive later without any interface changing — both paths produce the same structs.
//!
//! Two invariants run through everything here, and both come from §3:
//!
//! - **Automerge is the truth.** SQLite stores change chunks as opaque blobs and, later, a
//!   projection; it is never a source of truth for anything, and the projection is strictly
//!   one-directional.
//! - **Merge produces states nobody wrote.** Cycles, dangling references, records from a
//!   version this build has never seen. Loading has to survive all of it.

pub mod backup;
pub mod db;
pub mod doc;
pub mod error;
pub mod export;
mod records;
pub mod store;
pub mod undo;
mod value;

pub use db::Db;
pub use doc::{Doc, DocId, Documents, HydrationReport, Skipped};
pub use store::{Imports, Restored, Store};
pub use error::{Result, StoreError};
