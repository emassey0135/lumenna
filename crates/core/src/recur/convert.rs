//! The `chrono` boundary.
//!
//! The `rrule` crate speaks `chrono::DateTime<rrule::Tz>` and this crate speaks
//! [`jiff::civil::Date`] (§4). Every conversion between them happens in this file, so that
//! the rest of the crate never sees a `chrono` type and swapping the recurrence engine
//! later touches one module rather than every caller.
//!
//! # Everything is expanded in UTC, on purpose
//!
//! Recurrence asks a **civil-calendar question**: which days does this land on. The answer
//! depends on months, weekdays, and day counts, none of which have anything to do with
//! offsets — and UTC is the one zone with no transitions to perturb them. So a civil date
//! becomes midnight UTC, the expansion runs there, and the result comes back as a civil
//! date with the time discarded.
//!
//! This is not ducking §4's DST question. That question is *"what does a block scheduled in
//! a skipped hour do"*, and it belongs where a civil occurrence is resolved against a real
//! zone to produce an instant — reminder scheduling, timers. Answering it during expansion
//! would mean a recurring 2:30am block silently missing a day each spring, or occurring
//! twice each autumn, which is a calendar bug wearing a time-zone costume.

use chrono::{DateTime, Datelike, TimeZone};
use jiff::civil;
use rrule::Tz;

/// A civil date as midnight UTC, for handing to the recurrence engine.
pub(super) fn to_engine(date: civil::Date) -> Option<DateTime<Tz>> {
    Tz::UTC
        .with_ymd_and_hms(
            i32::from(date.year()),
            u32::try_from(date.month()).ok()?,
            u32::try_from(date.day()).ok()?,
            0,
            0,
            0,
        )
        .single()
}

/// An expanded datetime back as a civil date, discarding the time.
pub(super) fn from_engine(datetime: &DateTime<Tz>) -> Option<civil::Date> {
    civil::Date::new(
        i16::try_from(datetime.year()).ok()?,
        i8::try_from(datetime.month()).ok()?,
        i8::try_from(datetime.day()).ok()?,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_survive_the_round_trip() {
        for date in [
            civil::date(2026, 1, 1),
            civil::date(2026, 2, 29 - 1), // 2026 is not a leap year
            civil::date(2024, 2, 29),
            civil::date(2026, 12, 31),
            civil::date(1970, 1, 1),
        ] {
            let engine = to_engine(date).expect("representable");
            assert_eq!(from_engine(&engine), Some(date));
        }
    }

    #[test]
    fn a_date_in_a_dst_gap_still_converts() {
        // 2026-03-08 02:30 does not exist in America/New_York. In UTC it is unremarkable,
        // which is the entire reason expansion happens there.
        let date = civil::date(2026, 3, 8);
        assert_eq!(from_engine(&to_engine(date).unwrap()), Some(date));
    }
}
