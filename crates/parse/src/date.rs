//! The date grammar, shared by both parsers.
//!
//! §6.2 is blunt about why this is one module and not two: *"two subtly different date
//! parsers in one app is a bug generator."* `due before: next friday` parses exactly as
//! `next friday` does in quick add, because it is the same code.
//!
//! # Why this is written rather than adopted
//!
//! §6.1 surveys the crates — `interim`, `chrono-english`, `two_timer`, `parse_datetime` —
//! and rejects them all for one reason: they expect their input to **be** a date expression,
//! not to *contain* one. Quick add needs to know which span the date consumed so it can be
//! cut out of the title, and no crate exposes that. Writing it also gets recurrence parsing,
//! which nothing off the shelf provides at all.
//!
//! Everything here returns how many words it consumed, which is what makes that span
//! recoverable.

use jiff::civil::{self, Weekday};
use lumenna_core::time::{DateSpec, DueSpec, MonthDay, RecurrenceSpec, RelativeUnit, Which};

use crate::words::Word;

/// A date phrase that matched, and how much of the input it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct When {
    /// What was understood.
    pub spec: DueSpec,
    /// How many words it consumed.
    pub words: usize,
}

/// Matches the longest date, time, and repetition phrase starting at `at`.
///
/// Greedy on purpose, and in this order: a repetition, then a day, then a time. That is what
/// lets *"every weekday at 9am"* and *"tomorrow 3pm"* come out as single phrases rather than
/// leaving half of themselves in the title — which §6.1 calls out as the failure worth
/// avoiding, since a swallowed date is invisible until the task fails to fire.
#[must_use]
pub fn parse_when(words: &[Word], at: usize) -> Option<When> {
    let mut spec = DueSpec::default();
    let mut index = at;

    if let Some((recurrence, from_completion, used)) = parse_recurrence(words, index) {
        spec.recurrence = Some(recurrence);
        spec.from_completion = from_completion;
        index += used;
    }
    if let Some((date, used)) = parse_date(words, index) {
        spec.date = Some(date);
        index += used;
    }
    if let Some((time, used)) = parse_time(words, index) {
        spec.time = Some(time);
        index += used;
    }

    (!spec.is_empty()).then_some(When { spec, words: index - at })
}

/// Matches a day phrase starting at `at`.
#[must_use]
pub fn parse_date(words: &[Word], at: usize) -> Option<(DateSpec, usize)> {
    let word = words.get(at)?;

    if word.any_of(&["today", "tonight"]) {
        return Some((DateSpec::Today, 1));
    }
    if word.any_of(&["tomorrow", "tmr", "tmrw"]) {
        return Some((DateSpec::Tomorrow, 1));
    }
    if word.is("yesterday") {
        return Some((DateSpec::Yesterday, 1));
    }
    if let Ok(date) = word.lower.parse::<civil::Date>() {
        return Some((DateSpec::On(date), 1));
    }

    // "this friday" / "next friday" / "last friday", and the bare weekday.
    let which = match word.lower.as_str() {
        "this" => Some(Which::This),
        "next" => Some(Which::Next),
        "last" => Some(Which::Last),
        _ => None,
    };
    if let Some(which) = which {
        let next = words.get(at + 1)?;
        if let Some(day) = weekday_of(&next.lower) {
            return Some((DateSpec::Weekday { day, which }, 2));
        }
        // "next week", "last month".
        if let Some(unit) = unit_of(&next.lower) {
            let amount = if which == Which::Last { -1 } else { 1 };
            return Some((DateSpec::Offset { amount, unit }, 2));
        }
        return None;
    }
    if let Some(day) = weekday_of(&word.lower) {
        return Some((DateSpec::Weekday { day, which: Which::This }, 1));
    }

    // "in 3 days"
    if word.is("in")
        && let Some(amount) = words.get(at + 1).and_then(|w| integer(&w.lower))
        && let Some(unit) = words.get(at + 2).and_then(|w| unit_of(&w.lower))
    {
        return Some((DateSpec::Offset { amount, unit }, 3));
    }

    // "3 days ago"
    if let Some(amount) = integer(&word.lower)
        && let Some(unit) = words.get(at + 1).and_then(|w| unit_of(&w.lower))
        && words.get(at + 2).is_some_and(|w| w.is("ago"))
    {
        return Some((DateSpec::Offset { amount: -amount, unit }, 3));
    }

    // "sep 1" / "september 1st"
    if let Some(month) = month_of(&word.lower)
        && let Some(day) = words.get(at + 1).and_then(|w| ordinal(&w.lower))
        && (1..=31).contains(&day)
    {
        return Some((DateSpec::MonthDay { month, day: day as i8 }, 2));
    }

    // "1 sep" / "1st september"
    if let Some(day) = ordinal(&word.lower)
        && (1..=31).contains(&day)
        && let Some(month) = words.get(at + 1).and_then(|w| month_of(&w.lower))
    {
        return Some((DateSpec::MonthDay { month, day: day as i8 }, 2));
    }

    None
}

/// Matches a time of day starting at `at`, skipping a leading "at".
///
/// A bare number is deliberately **not** a time. *"Call mum 3"* is not three o'clock, and
/// treating it as one would silently eat a digit out of every title that has one. A time has
/// to look like a time: a meridiem, a colon, or a word.
#[must_use]
pub fn parse_time(words: &[Word], at: usize) -> Option<(civil::Time, usize)> {
    let mut index = at;
    let mut consumed = 0;
    if words.get(index).is_some_and(|w| w.is("at")) {
        index += 1;
        consumed += 1;
    }
    let word = words.get(index)?;

    let time = if word.is("noon") {
        civil::time(12, 0, 0, 0)
    } else if word.is("midnight") {
        civil::time(0, 0, 0, 0)
    } else {
        clock(&word.lower)?
    };
    Some((time, consumed + 1))
}

/// `3pm`, `3:30pm`, `15:00`, `9:05`.
fn clock(text: &str) -> Option<civil::Time> {
    let (body, meridiem) = if let Some(rest) = text.strip_suffix("pm") {
        (rest, Some(true))
    } else if let Some(rest) = text.strip_suffix("am") {
        (rest, Some(false))
    } else {
        (text, None)
    };
    let body = body.trim_end_matches('.').trim();

    let (hour_text, minute_text) = match body.split_once(':') {
        Some((h, m)) => (h, Some(m)),
        // Without a colon it is only a time if a meridiem said so.
        None if meridiem.is_some() => (body, None),
        None => return None,
    };

    let hour: i8 = hour_text.parse().ok()?;
    let minute: i8 = match minute_text {
        Some(m) => m.parse().ok()?,
        None => 0,
    };

    let hour = match meridiem {
        // 12pm is noon and 12am is midnight, which is the one case where the arithmetic is
        // not "add twelve".
        Some(true) if hour == 12 => 12,
        Some(true) if (1..12).contains(&hour) => hour + 12,
        Some(false) if hour == 12 => 0,
        Some(false) if (1..12).contains(&hour) => hour,
        Some(_) => return None,
        None => hour,
    };
    civil::Time::new(hour, minute, 0, 0).ok()
}

/// Matches a repetition starting at `at`, returning whether it counts from completion.
///
/// The trailing `!` is Todoist's, and §3.5 keeps the distinction: `every day` advances from
/// the scheduled date and can fall behind, `every! day` advances from when you actually
/// finished.
#[must_use]
pub fn parse_recurrence(words: &[Word], at: usize) -> Option<(RecurrenceSpec, bool, usize)> {
    let word = words.get(at)?;
    if !word.any_of(&["every", "each"]) {
        // "daily", "weekly" and friends stand alone.
        let standalone = match word.lower.as_str() {
            "daily" => RecurrenceSpec::Daily { interval: 1 },
            "weekly" => RecurrenceSpec::Weekly { interval: 1, days: Vec::new() },
            "monthly" => RecurrenceSpec::Monthly { interval: 1, day: None },
            "yearly" | "annually" => RecurrenceSpec::Yearly { interval: 1 },
            _ => return None,
        };
        return Some((standalone, false, 1));
    }

    let mut index = at + 1;
    // Todoist writes `every!` for "advance from completion" (§3.5). The tokeniser splits the
    // bang into its own word, since in a filter the same character is negation.
    let from_completion = words.get(index).is_some_and(|w| w.is("!"));
    if from_completion {
        index += 1;
    }
    // "every other week" is an interval of two.
    let mut interval: u16 = 1;
    if words.get(index).is_some_and(|w| w.is("other")) {
        interval = 2;
        index += 1;
    } else if let Some(n) = words.get(index).and_then(|w| integer(&w.lower))
        && (1..=1000).contains(&n)
        && words.get(index + 1).is_some_and(|w| unit_of(&w.lower).is_some())
    {
        interval = u16::try_from(n).ok()?;
        index += 1;
    }

    let word = words.get(index)?;

    // "every weekday"
    if word.any_of(&["weekday", "weekdays"]) {
        return Some((RecurrenceSpec::Weekdays, from_completion, index + 1 - at));
    }

    // "every last day (of the month)"
    if word.is("last") && words.get(index + 1).is_some_and(|w| w.any_of(&["day"])) {
        let mut used = index + 2;
        // Swallow an optional "of the month" so it does not land in the title.
        if words.get(used).is_some_and(|w| w.is("of")) {
            used += 1;
            if words.get(used).is_some_and(|w| w.is("the")) {
                used += 1;
            }
            if words.get(used).is_some_and(|w| w.any_of(&["month"])) {
                used += 1;
            }
        }
        return Some((
            RecurrenceSpec::Monthly { interval, day: Some(MonthDay::Last) },
            from_completion,
            used - at,
        ));
    }

    // "every monday", "every mon, wed and fri"
    if weekday_of(&word.lower).is_some() {
        let (days, used) = weekday_list(words, index);
        return Some((
            RecurrenceSpec::Weekly { interval, days },
            from_completion,
            index + used - at,
        ));
    }

    // "every 15th"
    if let Some(day) = ordinal(&word.lower)
        && (1..=31).contains(&day)
    {
        return Some((
            RecurrenceSpec::Monthly { interval, day: Some(MonthDay::Nth(day as i8)) },
            from_completion,
            index + 1 - at,
        ));
    }

    // "every day", "every 3 weeks"
    let spec = match unit_of(&word.lower)? {
        RelativeUnit::Day => RecurrenceSpec::Daily { interval },
        RelativeUnit::Week => RecurrenceSpec::Weekly { interval, days: Vec::new() },
        RelativeUnit::Month => RecurrenceSpec::Monthly { interval, day: None },
        RelativeUnit::Year => RecurrenceSpec::Yearly { interval },
    };
    Some((spec, from_completion, index + 1 - at))
}

/// Reads `mon, wed and fri` as a list, returning the days and words consumed.
fn weekday_list(words: &[Word], at: usize) -> (Vec<Weekday>, usize) {
    let mut days = Vec::new();
    let mut index = at;
    while let Some(day) = words.get(index).and_then(|w| weekday_of(&w.lower)) {
        if !days.contains(&day) {
            days.push(day);
        }
        index += 1;
        // A separator only counts if another weekday follows it; otherwise "every mon and
        // call the dentist" would eat the "and".
        let mut lookahead = index;
        while words.get(lookahead).is_some_and(|w| w.any_of(&[",", "and", "&"])) {
            lookahead += 1;
        }
        if words.get(lookahead).and_then(|w| weekday_of(&w.lower)).is_some() {
            index = lookahead;
        } else {
            break;
        }
    }
    (days, index - at)
}

/// A weekday by full name or three-letter abbreviation.
#[must_use]
pub fn weekday_of(text: &str) -> Option<Weekday> {
    Some(match text.trim_end_matches('s') {
        "monday" | "mon" => Weekday::Monday,
        "tuesday" | "tue" | "tues" => Weekday::Tuesday,
        "wednesday" | "wed" => Weekday::Wednesday,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thursday,
        "friday" | "fri" => Weekday::Friday,
        "saturday" | "sat" => Weekday::Saturday,
        "sunday" | "sun" => Weekday::Sunday,
        _ => return None,
    })
}

/// A month by full name or three-letter abbreviation.
#[must_use]
pub fn month_of(text: &str) -> Option<i8> {
    Some(match text {
        "january" | "jan" => 1,
        "february" | "feb" => 2,
        "march" | "mar" => 3,
        "april" | "apr" => 4,
        "may" => 5,
        "june" | "jun" => 6,
        "july" | "jul" => 7,
        "august" | "aug" => 8,
        "september" | "sep" | "sept" => 9,
        "october" | "oct" => 10,
        "november" | "nov" => 11,
        "december" | "dec" => 12,
        _ => return None,
    })
}

fn unit_of(text: &str) -> Option<RelativeUnit> {
    Some(match text {
        "day" | "days" => RelativeUnit::Day,
        "week" | "weeks" => RelativeUnit::Week,
        "month" | "months" => RelativeUnit::Month,
        "year" | "years" => RelativeUnit::Year,
        _ => return None,
    })
}

fn integer(text: &str) -> Option<i64> {
    text.parse().ok()
}

/// A plain number or an English ordinal: `15`, `15th`, `1st`, `2nd`, `3rd`.
fn ordinal(text: &str) -> Option<i64> {
    let digits = text.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &text[digits.len()..];
    if !matches!(suffix, "" | "st" | "nd" | "rd" | "th") {
        return None;
    }
    digits.parse().ok()
}
