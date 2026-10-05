//! Quick add, filter queries, and the completion they share.
//!
//! Both features turn typed text into something structured, both need the same date
//! grammar, and both need position-aware errors and completion — so they are one crate with
//! one shared vocabulary.
//!
//! What they produce lives in `core`: [`DueSpec`](lumenna_core::time::DueSpec) for dates,
//! [`Expr`](lumenna_core::filter::Expr) for queries. That split is deliberate. The
//! AST is **the stable interface**, and keeping it in the domain crate means evaluation, the
//! readback, and the accessibility layer never depend on the parser that happened to produce
//! it.
//!
//! # Errors have to be speakable
//!
//! Not "syntax error" but *"unknown label 'lapto' at position 12 — did you mean 'laptop'?"*.
//! A sighted user gets a squiggle under the offending token; here the position and the token
//! must be **in the message text**, because that text is the only channel. Every diagnostic
//! this crate produces carries a span for exactly that reason.

pub mod complete;
pub mod date;
pub mod filter;
pub mod quickadd;
pub mod words;

pub use complete::{Candidate, CandidateKind, Completions, Syntax, complete};
pub use filter::{ParseError, parse_filter};
pub use quickadd::{QuickAdd, parse_quick_add};
pub use words::{Word, words};
