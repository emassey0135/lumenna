//! The domain model.
//!
//! These are the *materialized* records — what an Automerge document means once read, not
//! how it is stored. `store` hydrates documents into these types and translates edits back
//! into Automerge operations; nothing here knows that Automerge exists. That boundary is
//! what lets an optional SQLite read model be added later without changing an interface:
//! both paths produce the same structs, and every query in this crate runs over them.
//!
//! Two consequences follow, and both are deliberate:
//!
//! - **`notes` is a `String`.** It is Automerge `Text` in the document, merged per
//!   character, but a materialized view of Text is a string. Editing it goes through an
//!   operation that carries a splice, not through assigning the whole field.
//! - **Nothing validates on construction that could be violated by merge.** A parent
//!   pointer into a cycle, a `depends` edge to a task this device has never seen, a label
//!   identifier whose `Label` was deleted — all of these occur and must load.
//!   [`crate::repair`] handles the ones that would otherwise hang or mislead.

use core::fmt;
use core::str::FromStr;

use jiff::Timestamp;

mod block;
mod device;
mod project;
mod reminder;
mod settings;
mod task;

pub use block::{
    BlockAssignment, BlockException, BlockFlags, BlockKind, BlockRef, BlockSeries,
    ExceptionAction, Elapsed, AssignmentStatus,
};
pub use device::Device;

/// The version of the stored format this build writes, raised whenever it changes in a way
/// an older build could misread.
///
/// Every document records the highest version that has written it, and every device records
/// its own in the device list. Builds of different versions sync with each other for good —
/// nothing can make every device update — so a change to the format is made only once every
/// listed device reports at least the version that understands it
/// ([`Snapshot::all_devices_at_least`](crate::snapshot::Snapshot::all_devices_at_least)), and
/// a device on another version is said to be, in words.
pub const SCHEMA_VERSION: u32 = 1;
pub use project::{Label, Project, SavedFilter};
pub use reminder::{
    Delivery, Reminder, ReminderAck, ReminderAction, ReminderAnchor, ReminderTarget, Trigger,
};
pub use settings::{Settings, Verbosity};
pub use task::{Due, Priority, Recurrence, Task, TaskCompletion};

/// A rule the model enforces at construction, because merge cannot violate it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// A reminder anchored to a block's start or end was pointed at a task.
    #[error("a task reminder cannot be anchored to {anchor:?}: tasks have no start or end")]
    AnchorNotApplicable {
        /// The anchor that was asked for.
        anchor: ReminderAnchor,
    },
    /// A block was given a compression floor longer than the block itself.
    #[error("minimum duration {min_duration_mins} exceeds duration {duration_mins}")]
    MinimumExceedsDuration {
        /// The floor as given.
        min_duration_mins: u32,
        /// The block's duration.
        duration_mins: u32,
    },
    /// A block was given a zero duration.
    #[error("a block must last at least one minute")]
    ZeroDuration,
}

/// An IANA time zone identifier, as stored.
///
/// The model holds the *name*, not a resolved [`jiff::tz::TimeZone`], because the name is
/// what is in the document and what has to survive a round trip through a device whose
/// tzdb is older than the one that wrote it. Resolution is [`TzName::get`], and it can
/// fail — on the device that syncs a zone it has never heard of.
///
/// This field is normally **absent**. A due time or a block start
/// with no zone floats: 3pm wherever you are, 9am after you fly to another continent. Set
/// it only when the thing is anchored to a real place.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TzName(String);

impl TzName {
    /// Wraps an identifier without checking it against the tzdb.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The identifier as stored.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Resolves against the platform time zone database.
    ///
    /// # Errors
    ///
    /// If this device's tzdb does not know the zone.
    pub fn get(&self) -> Result<jiff::tz::TimeZone, jiff::Error> {
        jiff::tz::TimeZone::get(&self.0)
    }
}

impl fmt::Display for TzName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for TzName {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

/// Where a record came from, when it was imported rather than created here.
///
/// Blocks are to import from calendars, and tasks from team systems. The field is nearly
/// free and unpleasant to retrofit into a CRDT afterwards, which is why it is on both.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExternalRef {
    /// Which system it came from.
    pub provider: ExternalProvider,
    /// That system's identifier for it.
    pub external_id: String,
    /// Whether local edits are permitted. Import is read-only first; two-way sync is a
    /// separate project.
    pub read_only: bool,
    /// When it was last reconciled with the source.
    pub last_synced: Timestamp,
}

/// The source system behind an [`ExternalRef`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExternalProvider {
    /// Apple's calendar database, on macOS and iOS.
    EventKit,
    /// A CalDAV server.
    CalDav,
    /// A subscribed `.ics` URL.
    IcsUrl,
}
