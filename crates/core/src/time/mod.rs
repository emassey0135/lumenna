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

/// *"Friday 15 May 2026"* — the form a screen reader can read without spelling out digits.
#[must_use]
pub fn long_date(date: jiff::civil::Date) -> String {
    format!("{} {} {}", weekday_name(date.weekday()), day_and_month(date), date.year())
}

/// *"15 May"*.
fn day_and_month(date: jiff::civil::Date) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    let month = MONTHS.get((date.month() as usize).wrapping_sub(1)).copied().unwrap_or("");
    format!("{} {month}", date.day())
}

/// *"3:00 PM"*.
#[must_use]
pub fn clock_words(time: jiff::civil::Time) -> String {
    let hour = time.hour();
    let (display, meridiem) = match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    };
    format!("{display}:{:02} {meridiem}", time.minute())
}

/// When something is due, as a row says it next to its title: near dates by name, *"due
/// tomorrow"*, the coming week by weekday, *"due Friday"*, then the date, *"due Friday 23
/// October"*, with the year only when it is not this one. Never digits: a screen reader
/// reads "2026-10-10" as numbers.
#[must_use]
pub fn due_words(date: jiff::civil::Date, today: jiff::civil::Date) -> String {
    // Whole days from today: `until` counts in days between civil dates.
    let days = today.until(date).map_or(i32::MAX, |span| span.get_days());
    let day = match days {
        0 => "today".to_owned(),
        1 => "tomorrow".to_owned(),
        -1 => "yesterday".to_owned(),
        2..=6 => weekday_name(date.weekday()).to_owned(),
        _ if date.year() == today.year() => format!("{} {}", weekday_name(date.weekday()), day_and_month(date)),
        _ => long_date(date),
    };
    format!("due {day}")
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

    #[test]
    fn a_due_date_is_said_in_words_near_by_name_then_by_weekday_then_by_date() {
        use jiff::civil::date;
        let today = date(2026, 10, 9); // a Friday
        assert_eq!(due_words(today, today), "due today");
        assert_eq!(due_words(date(2026, 10, 10), today), "due tomorrow");
        assert_eq!(due_words(date(2026, 10, 8), today), "due yesterday");
        assert_eq!(due_words(date(2026, 10, 14), today), "due Wednesday");
        assert_eq!(due_words(date(2026, 10, 23), today), "due Friday 23 October");
        assert_eq!(due_words(date(2026, 9, 30), today), "due Wednesday 30 September");
        assert_eq!(due_words(date(2027, 1, 4), today), "due Monday 4 January 2027");
    }
}
