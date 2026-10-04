//! Lumenna's domain core: the model, the queries over it, and the scheduling that reads it.
//!
//! Everything here is pure. There is no filesystem, no network, no database, and no
//! Automerge — `store` hydrates documents into [`Snapshot`](snapshot::Snapshot) and
//! translates edits back, `sync` moves changes between devices, and this crate is what both
//! of them are about. Keeping the boundary there is what makes the interesting logic
//! testable headlessly (§14) and shippable to eleven UI targets, one of which is a watch and
//! one of which is a browser.
//!
//! The design document (`PLAN.md`) is the reference; section numbers in these docs point
//! into it. Where a decision looks arbitrary, the reasoning is recorded next to the code
//! rather than only there.
//!
//! # What is here so far
//!
//! - [`id`] — typed UUIDv7 identifiers, ordered by creation time.
//! - [`order`] — fractional indexing, the ordering scheme for every sibling list.
//! - [`edit`] — mutations, computed here and applied by `store`.
//! - [`filter`] — the query language's AST, evaluation, and readback.
//! - [`model`] — the records of §3.
//! - [`recur`] — §5's two recurrence systems.
//! - [`row`] — §13's list projection, computed once for all eleven targets.
//! - [`repair`] — the two graphs CRDT merge can corrupt, and how they are put right.
//! - [`suggest`] — nearest-name matching, for "did you mean".
//! - [`state`] — computed states, shared by filters and the accessibility layer.
//! - [`snapshot`] — the materialized view queries run over.
//! - [`time`] — the clock conventions every record obeys.

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
