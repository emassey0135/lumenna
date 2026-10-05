//! Blocks as iCalendar (RFC 5545), so a planned day can be read by any calendar.
//!
//! One `VEVENT` per live series. A repeating block carries its `RRULE` as stored, a cancelled
//! occurrence becomes an `EXDATE`, and a changed one becomes its own `VEVENT` with a
//! `RECURRENCE-ID` — the same sparse-exception shape the model uses, which is no accident: it is
//! the calendar world's shape.
//!
//! Times **float** unless the block is anchored to a zone. A floating `DTSTART` — no
//! `Z`, no `TZID` — is iCalendar's own way of saying "9am wherever you are", so the meaning
//! survives the trip. An anchored block names its IANA zone in `TZID`; no `VTIMEZONE` is
//! written, which RFC 5545 asks for but every calendar people use resolves IANA names
//! without.

use std::fmt::Write as _;

use jiff::Timestamp;
use jiff::civil::{Date, Time};
use lumenna_core::model::{BlockKind, BlockSeries, ExceptionAction};
use lumenna_core::snapshot::Snapshot;

/// The live block series as an iCalendar file.
#[must_use]
pub fn ics(snapshot: &Snapshot, now: Timestamp) -> String {
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        "PRODID:-//Lumenna//Lumenna//EN".to_owned(),
        "CALSCALE:GREGORIAN".to_owned(),
    ];
    let stamp = format!("DTSTAMP:{}", now.strftime("%Y%m%dT%H%M%SZ"));

    for series in snapshot.series.values().filter(|s| s.deleted_at.is_none()) {
        let exceptions: Vec<_> = snapshot
            .exceptions
            .values()
            .filter(|e| e.series_id == series.id && series.is_recurring())
            .collect();

        lines.push("BEGIN:VEVENT".to_owned());
        lines.push(format!("UID:{}@lumenna", series.id));
        lines.push(stamp.clone());
        lines.push(start_line("DTSTART", series, series.start_date, series.start_time));
        lines.push(format!("DURATION:PT{}M", series.duration_mins));
        lines.push(format!("SUMMARY:{}", escape(&series.title)));
        if !series.notes.is_empty() {
            lines.push(format!("DESCRIPTION:{}", escape(&series.notes)));
        }
        lines.push(format!("CATEGORIES:{}", kind_word(series.kind)));
        if series.kind == BlockKind::Break {
            // A break is free time as far as anyone else's calendar is concerned.
            lines.push("TRANSP:TRANSPARENT".to_owned());
        }
        if let Some(rrule) = &series.rrule {
            lines.push(format!("RRULE:{}", bounded(rrule, series.end_date)));
            for exception in &exceptions {
                if exception.action == ExceptionAction::Cancelled {
                    lines.push(start_line(
                        "EXDATE",
                        series,
                        exception.original_date,
                        series.start_time,
                    ));
                }
            }
        }
        lines.push("END:VEVENT".to_owned());

        for exception in &exceptions {
            let ExceptionAction::Modified { start_time, duration_mins, title, kind, .. } =
                &exception.action
            else {
                continue;
            };
            lines.push("BEGIN:VEVENT".to_owned());
            lines.push(format!("UID:{}@lumenna", series.id));
            lines.push(stamp.clone());
            lines.push(start_line(
                "RECURRENCE-ID",
                series,
                exception.original_date,
                series.start_time,
            ));
            lines.push(start_line(
                "DTSTART",
                series,
                exception.original_date,
                start_time.unwrap_or(series.start_time),
            ));
            lines.push(format!("DURATION:PT{}M", duration_mins.unwrap_or(series.duration_mins)));
            lines.push(format!("SUMMARY:{}", escape(title.as_deref().unwrap_or(&series.title))));
            lines.push(format!("CATEGORIES:{}", kind_word(kind.unwrap_or(series.kind))));
            lines.push("END:VEVENT".to_owned());
        }
    }
    lines.push("END:VCALENDAR".to_owned());

    let mut out = String::new();
    for line in lines {
        out.push_str(&fold(&line));
    }
    out
}

fn kind_word(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Work => "work",
        BlockKind::Break => "break",
        BlockKind::Event => "event",
    }
}

/// `NAME:20261003T090000` floating, or `NAME;TZID=Europe/London:20261003T090000` anchored.
fn start_line(name: &str, series: &BlockSeries, date: Date, time: Time) -> String {
    let value = format!("{}T{:02}{:02}{:02}", date.strftime("%Y%m%d"), time.hour(), time.minute(), time.second());
    match &series.timezone {
        Some(zone) => format!("{name};TZID={zone}:{value}"),
        None => format!("{name}:{value}"),
    }
}

/// The series' end date, carried into the rule when the rule does not already end itself.
fn bounded(rrule: &str, end: Option<Date>) -> String {
    let ends = rrule.split(';').any(|part| part.starts_with("UNTIL=") || part.starts_with("COUNT="));
    match end {
        Some(end) if !ends => format!("{rrule};UNTIL={}T235959", end.strftime("%Y%m%d")),
        _ => rrule.to_owned(),
    }
}

/// RFC 5545 section 3.3.11 text escaping.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            other => out.push(other),
        }
    }
    out
}

/// RFC 5545 section 3.1: lines end in CRLF and are folded at 75 octets, never inside a character.
fn fold(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    let mut width = 0;
    for c in line.chars() {
        let size = c.len_utf8();
        // A continuation line starts with a space, which counts against its 75.
        if width + size > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += size;
    }
    let _ = write!(out, "\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_escaped() {
        assert_eq!(escape("a, b; c\\d\nnext"), "a\\, b\\; c\\\\d\\nnext");
    }

    #[test]
    fn long_lines_fold_at_75_octets_without_splitting_a_character() {
        let line = format!("SUMMARY:{}", "é".repeat(60));
        let folded = fold(&line);
        for physical in folded.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(physical.len() <= 75, "{} octets", physical.len());
        }
        let unfolded = folded.replace("\r\n ", "").replace("\r\n", "");
        assert_eq!(unfolded, line);
    }

    #[test]
    fn an_end_date_bounds_a_rule_that_does_not_end_itself() {
        let end = jiff::civil::date(2026, 12, 31);
        assert_eq!(bounded("FREQ=DAILY", Some(end)), "FREQ=DAILY;UNTIL=20261231T235959");
        assert_eq!(bounded("FREQ=DAILY;COUNT=3", Some(end)), "FREQ=DAILY;COUNT=3");
        assert_eq!(bounded("FREQ=DAILY", None), "FREQ=DAILY");
    }
}
