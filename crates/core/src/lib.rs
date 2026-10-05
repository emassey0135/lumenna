//! Lumenna's domain core: the model, the queries over it, and the scheduling that reads it.
//!
//! Everything here is pure. There is no filesystem, no network, no database, and no
//! Automerge — `store` hydrates documents into [`Snapshot`](snapshot::Snapshot) and
//! translates edits back, `sync` moves changes between devices, and this crate is what both
//! of them are about. Keeping the boundary there is what makes the interesting logic
//! testable headlessly and shippable to eleven UI targets, one of which is a watch and one of
//! which is a browser.
//!
//! Where a decision looks arbitrary, the reasoning is recorded next to the code.

pub mod edit;
pub mod filter;
pub mod id;
pub mod model;
pub mod order;
pub mod recur;
pub mod repair;
pub mod row;
pub mod snapshot;
pub mod suggest;
pub mod state;
pub mod time;

pub use id::{
    AssignmentId, CompletionId, FilterId, LabelId, NodeId, ProjectId, ReminderId, SeriesId,
    TaskId,
};
pub use order::OrderKey;
pub use snapshot::{Repairs, Snapshot};
pub use row::{Role, Row, RowId};
pub use state::State;
