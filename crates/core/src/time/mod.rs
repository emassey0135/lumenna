//! Clock conventions the whole model obeys.
//!
//! The big decision is `jiff` rather than `chrono`, because the distinction between civil
//! (wall-clock) and absolute time is load-bearing throughout this app and `jiff` models it
//! properly. What is here is smaller and easier to get wrong.
//!
//! # Timestamps are milliseconds
//!
//! Every [`Timestamp`] in the model is truncated to millisecond precision, and [`now`] is
//! how you get one.
//!
//! The reason is that a document has to agree with itself. Automerge's timestamp scalar is
//! defined as milliseconds since the epoch, which is also how the JavaScript implementation
//! and every inspection tool will read it — so storing nanoseconds there would be lying
//! about the unit to save a precision nothing in a task manager wants. Truncating on the way
//! *out* instead would be worse in a subtler way: a record would silently stop equalling
//! itself the moment it was saved, and every round-trip test would need to know which fields
//! to forgive.
//!
//! So the truncation happens here, at the point the value is created, and a record in memory
//! is byte-for-byte what a record read back will be.
//!
//! Sub-millisecond precision would buy nothing regardless. Two events a microsecond apart on
//! one device are not ordered by their timestamps — merge order decides, and sibling-order
//! tie-breaks go through UUIDv7 identifiers rather than clocks.
//!
//! # Naming a time you have not resolved yet
//!
//! [`DateSpec`] and friends are the other half of this module: *"next friday"*, *"in three
//! days"*, *"every other monday"* as values, resolved against a [`jiff::Zoned`] only when
//! someone asks. Quick add and the filter parser both produce them, and must — a saved filter
//! containing `today` must mean today *at evaluation time*, and one that resolved to a date
//! range when it was saved silently rots overnight.

mod spec;

pub use spec::{
    DateSpec, DueSpec, MonthDay, RecurrenceSpec, RelativeUnit, Which, weekday_name,
};

use jiff::Timestamp;

/// The current time, at the precision the model stores.
#[must_use]
pub fn now() -> Timestamp {
    truncate(Timestamp::now())
}

/// Drops sub-millisecond precision, so a value equals what a document will give back.
///
/// Use this on any timestamp that came from outside — a platform clock, an import, an FFI
/// caller — before it enters a record.
#[must_use]
pub fn truncate(timestamp: Timestamp) -> Timestamp {
    Timestamp::from_millisecond(timestamp.as_millisecond()).unwrap_or(timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_already_truncated() {
        let t = now();
        assert_eq!(truncate(t), t);
        assert_eq!(t.subsec_nanosecond() % 1_000_000, 0);
    }

    #[test]
    fn truncation_is_idempotent_and_never_moves_forward() {
        let t = Timestamp::from_nanosecond(1_800_000_000_999_999_999).unwrap();
        let truncated = truncate(t);
        assert!(truncated <= t);
        assert_eq!(truncate(truncated), truncated);
        assert_eq!(truncated.as_millisecond(), t.as_millisecond());
    }

    #[test]
    fn truncation_survives_the_epoch() {
        // Timestamps before 1970 are not a use case, but arithmetic that breaks on them
        // tends to break elsewhere too.
        let t = Timestamp::from_nanosecond(-1_500_000_001).unwrap();
        assert_eq!(truncate(truncate(t)), truncate(t));
    }
}
