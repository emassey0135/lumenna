//! Current-state export and import.
//!
//! Two things are kept visibly apart here:
//!
//! - **A backup** ([`crate::backup`]) is the change log — every task ever created, including
//!   every one deleted. It is for disaster recovery and device migration.
//! - **An export** (this module) is the present state and nothing else: no history, no trash.
//!   It is what someone means when they say "export my tasks", and it is safe to hand to
//!   another program or another person in a way a backup is not.
//!
//! Conflating them is how someone emails a "task list" that turns out to hold everything they
//! ever deleted.
//!
//! # Formats
//!
//! - **JSON** — structured and complete, and the one format [`import_json`] reads back. It is
//!   a compatibility contract like `--json`: versioned, shaped for readers rather than derived
//!   from the model, so the model can change underneath it.
//! - **Markdown and org** ([`markdown`], [`org`]) — tasks, for people.
//! - **iCalendar** ([`ics`]) — blocks, so a planned day can be read by any calendar.
//!
//! Import of our own output is **tested, not assumed**: a recovery path never exercised
//! does not work, and the round-trip test is also what proves every field of the model made
//! it into the format.

mod ics;
mod json;
mod text;

pub use ics::ics;
pub use json::{FORMAT, VERSION, Imported, export_json, parse_json};
pub use text::{markdown, org};
