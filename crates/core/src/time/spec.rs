//! Dates, times, and repetitions named but not yet resolved.
//!
//! Quick add and the filter language both take date phrases, and they share one grammar:
//! two subtly different date parsers in one app is a bug generator. These types are what
//! that shared grammar produces. Parsing lives in `parse`; the meaning lives here, next to
//! the model the meaning is about.
//!
//! Everything resolves against a caller-supplied [`Zoned`] rather than reading the clock.
//! Quick add anchors to the user's current zoned datetime, never UTC, and a test that
//! cannot choose "now" cannot check what happens on the last day of a month.

use jiff::civil::{self, Weekday};
use jiff::{Span, Zoned};

/// Which occurrence of a weekday a phrase means.
///
/// English is genuinely ambiguous here — *"next Friday"* means different things to
/// different people — so quick add does not try to be clever. It fixes a rule, and then
/// **announces the date it resolved to**, which is the part that actually protects the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Which {
    /// The coming one, today included. On a Friday, *"friday"* means today.
    This,
    /// A week past [`Which::This`]. On a Friday, *"next friday"* means seven days away.
    Next,
    /// The most recent one, strictly in the past.
    Last,
}

/// The unit of a relative offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelativeUnit {
    /// Days.
    Day,
    /// Weeks.
    Week,
    /// Months, with day-of-month constrained to the target month's length.
    Month,
    /// Years.
    Year,
}

/// A day of the month, which may count from the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonthDay {
    /// A fixed day. Months without it are skipped, not clamped.
    Nth(i8),
    /// The last day, whatever length the month is.
    Last,
}

/// A date named without being pinned down.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DateSpec {
    /// An exact date, as typed.
    On(civil::Date),
    /// A month and day, with the year left to inference.
    MonthDay {
        /// 1–12.
        month: i8,
        /// 1–31.
        day: i8,
    },
    /// Today, relative to the anchor.
    Today,
    /// Tomorrow.
    Tomorrow,
    /// Yesterday.
    Yesterday,
    /// A named weekday.
    Weekday {
        /// Which day.
        day: Weekday,
        /// Which occurrence of it.
        which: Which,
    },
    /// An offset from the anchor. Negative counts backwards.
    Offset {
        /// How many.
        amount: i64,
        /// Of what.
        unit: RelativeUnit,
    },
}

impl DateSpec {
    /// Pins the date down against an anchor.
    ///
    /// `None` when the phrase names something that does not exist — the 31st of a month
    /// with thirty days, or an offset that runs past what a date can represent.
    #[must_use]
    pub fn resolve(&self, now: &Zoned) -> Option<civil::Date> {
        let today = now.date();
        match *self {
            Self::On(date) => Some(date),
            Self::MonthDay { month, day } => resolve_month_day(today, month, day),
            Self::Today => Some(today),
            Self::Tomorrow => today.checked_add(Span::new().days(1)).ok(),
            Self::Yesterday => today.checked_sub(Span::new().days(1)).ok(),
            Self::Weekday { day, which } => resolve_weekday(today, day, which),
            Self::Offset { amount, unit } => {
                let span = match unit {
                    RelativeUnit::Day => Span::new().days(amount),
                    RelativeUnit::Week => Span::new().weeks(amount),
                    RelativeUnit::Month => Span::new().months(amount),
                    RelativeUnit::Year => Span::new().years(amount),
                };
                today.checked_add(span).ok()
            }
        }
    }
}

/// A month and day, in the current year or the next one.
///
/// *"sep 1"* typed in October means next September, because a task manager is about what is
/// coming. A date already gone is almost never what someone meant to type, and a due date
/// silently landing in the past is exactly the failure to avoid.
fn resolve_month_day(today: civil::Date, month: i8, day: i8) -> Option<civil::Date> {
    let this_year = civil::Date::new(today.year(), month, day).ok();
    match this_year {
        Some(date) if date >= today => Some(date),
        _ => civil::Date::new(today.year().checked_add(1)?, month, day).ok(),
    }
}

fn resolve_weekday(today: civil::Date, day: Weekday, which: Which) -> Option<civil::Date> {
    let today_index = i64::from(today.weekday().to_monday_zero_offset());
    let target = i64::from(day.to_monday_zero_offset());
    match which {
        Which::This => {
            let ahead = (target - today_index).rem_euclid(7);
            today.checked_add(Span::new().days(ahead)).ok()
        }
        Which::Next => {
            let ahead = (target - today_index).rem_euclid(7) + 7;
            today.checked_add(Span::new().days(ahead)).ok()
        }
        Which::Last => {
            // Strictly in the past: on a Friday, "last friday" is a week ago, not today.
            let behind = (today_index - target).rem_euclid(7);
            let behind = if behind == 0 { 7 } else { behind };
            today.checked_sub(Span::new().days(behind)).ok()
        }
    }
}

impl std::fmt::Display for DateSpec {
    /// Renders the phrase back, for the readback.
    ///
    /// This is the *phrase*, not the date it resolves to. Both belong in an announcement —
    /// the resolved absolute date is always exposed, because "Friday" is ambiguous and the
    /// resolution is the part worth confirming — so a caller says both: *"next friday, that
    /// is the fifteenth of May"*.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const MONTHS: [&str; 12] = [
            "January", "February", "March", "April", "May", "June", "July", "August",
            "September", "October", "November", "December",
        ];
        match self {
            Self::On(date) => write!(f, "{date}"),
            Self::MonthDay { month, day } => {
                match MONTHS.get((*month as usize).wrapping_sub(1)) {
                    Some(name) => write!(f, "{name} {day}"),
                    None => write!(f, "{month}/{day}"),
                }
            }
            Self::Today => f.write_str("today"),
            Self::Tomorrow => f.write_str("tomorrow"),
            Self::Yesterday => f.write_str("yesterday"),
            Self::Weekday { day, which } => {
                let name = weekday_name(*day);
                match which {
                    Which::This => f.write_str(name),
                    Which::Next => write!(f, "next {name}"),
                    Which::Last => write!(f, "last {name}"),
                }
            }
            Self::Offset { amount, unit } => {
                let unit = match unit {
                    RelativeUnit::Day => "day",
                    RelativeUnit::Week => "week",
                    RelativeUnit::Month => "month",
                    RelativeUnit::Year => "year",
                };
                let count = amount.abs();
                let plural = if count == 1 { "" } else { "s" };
                if *amount < 0 {
                    write!(f, "{count} {unit}{plural} ago")
                } else {
                    write!(f, "in {count} {unit}{plural}")
                }
            }
        }
    }
}

/// The English name of a weekday.
#[must_use]
pub fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

/// A repetition named in prose, before it becomes an RRULE.
///
/// No Rust crate turns English into RFC 5545, so this is written by hand as part of the
/// quick-add grammar. Keeping it as a value rather than going straight to a string means
/// the readback can describe what was understood without parsing its own output back.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RecurrenceSpec {
    /// *"every day"*, *"every 3 days"*.
    Daily {
        /// How many days between occurrences.
        interval: u16,
    },
    /// *"every week"*, *"every monday"*, *"every mon, wed and fri"*.
    Weekly {
        /// How many weeks between occurrences.
        interval: u16,
        /// Which days, or empty to repeat on whatever day it starts.
        days: Vec<Weekday>,
    },
    /// *"every weekday"*.
    Weekdays,
    /// *"every month"*, *"every 15th"*, *"every last day of the month"*.
    Monthly {
        /// How many months between occurrences.
        interval: u16,
        /// Which day, or the anchor's own day when absent.
        day: Option<MonthDay>,
    },
    /// *"every year"*.
    Yearly {
        /// How many years between occurrences.
        interval: u16,
    },
}

impl RecurrenceSpec {
    /// The repetition in English, for the readback.
    #[must_use]
    pub fn describe(&self) -> String {
        fn every(interval: u16, unit: &str) -> String {
            match interval {
                0 | 1 => format!("every {unit}"),
                2 => format!("every other {unit}"),
                n => format!("every {n} {unit}s"),
            }
        }
        match self {
            Self::Daily { interval } => every(*interval, "day"),
            Self::Weekly { interval, days } if days.is_empty() => every(*interval, "week"),
            Self::Weekly { interval, days } => {
                let names: Vec<&str> = days.iter().copied().map(weekday_name).collect();
                let list = match names.split_last() {
                    Some((last, [])) => (*last).to_owned(),
                    Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
                    None => String::new(),
                };
                match interval {
                    0 | 1 => format!("every {list}"),
                    2 => format!("every other {list}"),
                    n => format!("every {n} weeks on {list}"),
                }
            }
            Self::Weekdays => "every weekday".to_owned(),
            Self::Monthly { interval, day: None } => every(*interval, "month"),
            Self::Monthly { interval, day: Some(MonthDay::Nth(d)) } => {
                format!("{} on the {d}", every(*interval, "month"))
            }
            Self::Monthly { interval, day: Some(MonthDay::Last) } => {
                format!("{} on the last day", every(*interval, "month"))
            }
            Self::Yearly { interval } => every(*interval, "year"),
        }
    }

    /// Renders the RFC 5545 rule this phrase means.
    ///
    /// `INTERVAL=1` is left off, since it is the default and a rule a user might read should
    /// not be noisier than it needs to be.
    #[must_use]
    pub fn to_rrule(&self) -> String {
        fn interval(n: u16) -> String {
            if n <= 1 { String::new() } else { format!(";INTERVAL={n}") }
        }
        fn day_code(day: Weekday) -> &'static str {
            match day {
                Weekday::Monday => "MO",
                Weekday::Tuesday => "TU",
                Weekday::Wednesday => "WE",
                Weekday::Thursday => "TH",
                Weekday::Friday => "FR",
                Weekday::Saturday => "SA",
                Weekday::Sunday => "SU",
            }
        }
        match self {
            Self::Daily { interval: n } => format!("FREQ=DAILY{}", interval(*n)),
            Self::Weekly { interval: n, days } if days.is_empty() => {
                format!("FREQ=WEEKLY{}", interval(*n))
            }
            Self::Weekly { interval: n, days } => {
                let codes: Vec<&str> = days.iter().copied().map(day_code).collect();
                format!("FREQ=WEEKLY{};BYDAY={}", interval(*n), codes.join(","))
            }
            Self::Weekdays => "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR".to_owned(),
            Self::Monthly { interval: n, day: None } => format!("FREQ=MONTHLY{}", interval(*n)),
            Self::Monthly { interval: n, day: Some(MonthDay::Nth(d)) } => {
                format!("FREQ=MONTHLY{};BYMONTHDAY={d}", interval(*n))
            }
            Self::Monthly { interval: n, day: Some(MonthDay::Last) } => {
                format!("FREQ=MONTHLY{};BYMONTHDAY=-1", interval(*n))
            }
            Self::Yearly { interval: n } => format!("FREQ=YEARLY{}", interval(*n)),
        }
    }

    /// Reads back a rule [`to_rrule`](Self::to_rrule) wrote. A rule it could not have
    /// written — one imported from a calendar, say — is `None`, not approximated: a
    /// repetition described as something close to what it does is a wrong answer.
    #[must_use]
    pub fn from_rrule(rule: &str) -> Option<Self> {
        let (mut freq, mut interval, mut days, mut month_day) = (None, 1_u16, None, None);
        for part in rule.split(';').filter(|part| !part.is_empty()) {
            let (key, value) = part.split_once('=')?;
            match key.to_ascii_uppercase().as_str() {
                "FREQ" => freq = Some(value.to_ascii_uppercase()),
                "INTERVAL" => interval = value.parse().ok().filter(|n| *n >= 1)?,
                "BYDAY" => {
                    let parsed: Option<Vec<Weekday>> = value
                        .split(',')
                        .map(|code| {
                            Some(match code.to_ascii_uppercase().as_str() {
                                "MO" => Weekday::Monday,
                                "TU" => Weekday::Tuesday,
                                "WE" => Weekday::Wednesday,
                                "TH" => Weekday::Thursday,
                                "FR" => Weekday::Friday,
                                "SA" => Weekday::Saturday,
                                "SU" => Weekday::Sunday,
                                _ => return None,
                            })
                        })
                        .collect();
                    days = Some(parsed?);
                }
                "BYMONTHDAY" => {
                    month_day = Some(match value.parse::<i8>().ok()? {
                        -1 => MonthDay::Last,
                        day @ 1..=31 => MonthDay::Nth(day),
                        _ => return None,
                    });
                }
                _ => return None,
            }
        }
        let weekdays = [
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
        ];
        Some(match (freq?.as_str(), days, month_day) {
            ("DAILY", None, None) => Self::Daily { interval },
            ("WEEKLY", Some(days), None) if interval == 1 && days == weekdays => Self::Weekdays,
            ("WEEKLY", days, None) => Self::Weekly { interval, days: days.unwrap_or_default() },
            ("MONTHLY", None, day) => Self::Monthly { interval, day },
            ("YEARLY", None, None) => Self::Yearly { interval },
            _ => return None,
        })
    }

    /// The repetition as the date grammar reads it — what an edit field shows, so that
    /// saving it unchanged means the same thing. `from_completion` writes Todoist's `every!`.
    ///
    /// `None` for what the grammar cannot say, such as every third week on Mondays: such a
    /// rule came from outside, and showing a phrase that would mean something else is how
    /// saving an unrelated field would quietly change it.
    #[must_use]
    pub fn phrase(&self, from_completion: bool) -> Option<String> {
        fn every(interval: u16, unit: &str) -> String {
            match interval {
                0 | 1 => unit.to_owned(),
                2 => format!("other {unit}"),
                n => format!("{n} {unit}s"),
            }
        }
        // Only "other" can come before a day: "every 3 15th" is not English, nor grammar.
        let before_a_day = |interval: u16| match interval {
            0 | 1 => Some(""),
            2 => Some("other "),
            _ => None,
        };
        let body = match self {
            Self::Daily { interval } => every(*interval, "day"),
            Self::Weekly { interval, days } if days.is_empty() => every(*interval, "week"),
            Self::Weekly { interval, days } => {
                let names: Vec<String> =
                    days.iter().map(|day| weekday_name(*day).to_lowercase()).collect();
                let list = match names.split_last() {
                    Some((last, [])) => last.clone(),
                    Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
                    None => String::new(),
                };
                format!("{}{list}", before_a_day(*interval)?)
            }
            Self::Weekdays => "weekday".to_owned(),
            Self::Monthly { interval, day: None } => every(*interval, "month"),
            Self::Monthly { interval, day: Some(MonthDay::Nth(d)) } => {
                format!("{}{}", before_a_day(*interval)?, ordinal_word(*d))
            }
            Self::Monthly { interval, day: Some(MonthDay::Last) } => {
                format!("{}last day", before_a_day(*interval)?)
            }
            Self::Yearly { interval } => every(*interval, "year"),
        };
        Some(format!("every{} {body}", if from_completion { "!" } else { "" }))
    }
}

/// `1st`, `2nd`, `23rd`, `11th`.
fn ordinal_word(n: i8) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A due date as typed, before resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DueSpec {
    /// The day, if one was named.
    pub date: Option<DateSpec>,
    /// The time of day, if one was named. Absent is a real state, not a default of midnight.
    pub time: Option<civil::Time>,
    /// The repetition, if one was named.
    pub recurrence: Option<RecurrenceSpec>,
    /// Whether the repetition counts from completion — Todoist's `every!`.
    pub from_completion: bool,
}

impl DueSpec {
    /// Whether anything at all was named.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.date.is_none() && self.time.is_none() && self.recurrence.is_none()
    }
}

impl DueSpec {
    /// Turns the phrase into a real [`Due`], anchored to `now`.
    ///
    /// Three inferences happen here, and each is the answer to "what did the user leave
    /// out":
    ///
    /// - **A time with no day means today.** *"3pm"* is this afternoon.
    /// - **A repetition with no day starts at its first occurrence from today.** *"every
    ///   monday"* typed on a Wednesday is due next Monday, not Wednesday — because a series
    ///   anchored on a day it never occurs is a trap, and quick add is where it would
    ///   otherwise be set.
    /// - **Nothing at all is no due date**, which is a real state and not a defaulted one.
    ///
    /// # Errors
    ///
    /// If a repetition was named that cannot be expanded.
    pub fn resolve(&self, now: &Zoned) -> Result<Option<crate::model::Due>, crate::recur::RecurError> {
        let recurrence = self.recurrence.as_ref().map(|spec| crate::model::Recurrence {
            rrule: spec.to_rrule(),
            from_completion: self.from_completion,
        });

        let date = match (&self.date, &recurrence) {
            (Some(spec), _) => spec.resolve(now),
            (None, Some(rec)) => {
                crate::recur::Rule::parse(&rec.rrule)?.first_from(now.date())?
            }
            (None, None) if self.time.is_some() => Some(now.date()),
            (None, None) => None,
        };

        Ok(date.map(|date| crate::model::Due {
            date,
            time: self.time,
            timezone: None,
            recurrence,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    /// Wednesday 2026-05-06, mid-afternoon.
    fn now() -> Zoned {
        date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
    }

    #[test]
    fn the_easy_words_resolve() {
        assert_eq!(DateSpec::Today.resolve(&now()), Some(date(2026, 5, 6)));
        assert_eq!(DateSpec::Tomorrow.resolve(&now()), Some(date(2026, 5, 7)));
        assert_eq!(DateSpec::Yesterday.resolve(&now()), Some(date(2026, 5, 5)));
    }

    #[test]
    fn a_weekday_means_the_coming_one_and_today_counts() {
        let spec = |day, which| DateSpec::Weekday { day, which }.resolve(&now());
        // Today is Wednesday.
        assert_eq!(spec(Weekday::Wednesday, Which::This), Some(date(2026, 5, 6)));
        assert_eq!(spec(Weekday::Friday, Which::This), Some(date(2026, 5, 8)));
        assert_eq!(spec(Weekday::Monday, Which::This), Some(date(2026, 5, 11)));
    }

    #[test]
    fn next_is_a_week_past_this_and_last_is_strictly_behind() {
        let spec = |day, which| DateSpec::Weekday { day, which }.resolve(&now());
        assert_eq!(spec(Weekday::Friday, Which::Next), Some(date(2026, 5, 15)));
        // On a Wednesday, "next wednesday" is a week away, never today.
        assert_eq!(spec(Weekday::Wednesday, Which::Next), Some(date(2026, 5, 13)));
        assert_eq!(spec(Weekday::Friday, Which::Last), Some(date(2026, 5, 1)));
        assert_eq!(spec(Weekday::Wednesday, Which::Last), Some(date(2026, 4, 29)));
    }

    #[test]
    fn offsets_count_from_today() {
        let offset = |amount, unit| DateSpec::Offset { amount, unit }.resolve(&now());
        assert_eq!(offset(3, RelativeUnit::Day), Some(date(2026, 5, 9)));
        assert_eq!(offset(2, RelativeUnit::Week), Some(date(2026, 5, 20)));
        assert_eq!(offset(1, RelativeUnit::Month), Some(date(2026, 6, 6)));
        assert_eq!(offset(1, RelativeUnit::Year), Some(date(2027, 5, 6)));
        assert_eq!(offset(-1, RelativeUnit::Day), Some(date(2026, 5, 5)));
    }

    #[test]
    fn a_month_offset_lands_in_a_month_that_has_the_day() {
        let jan31 = date(2026, 1, 31).at(9, 0, 0, 0).in_tz("UTC").unwrap();
        assert_eq!(
            DateSpec::Offset { amount: 1, unit: RelativeUnit::Month }.resolve(&jan31),
            Some(date(2026, 2, 28)),
            "adding a month to the 31st constrains rather than overflowing into March"
        );
    }

    #[test]
    fn a_bare_month_and_day_means_the_next_one_coming() {
        let spec = |month, day| DateSpec::MonthDay { month, day }.resolve(&now());
        // Today is 2026-05-06.
        assert_eq!(spec(9, 1), Some(date(2026, 9, 1)), "later this year");
        assert_eq!(spec(5, 6), Some(date(2026, 5, 6)), "today counts");
        assert_eq!(spec(1, 1), Some(date(2027, 1, 1)), "already gone, so next year");
    }

    #[test]
    fn a_day_that_does_not_exist_resolves_to_nothing() {
        assert_eq!(DateSpec::MonthDay { month: 2, day: 30 }.resolve(&now()), None);
        assert_eq!(DateSpec::MonthDay { month: 13, day: 1 }.resolve(&now()), None);
    }

    #[test]
    fn recurrence_phrases_render_as_rules() {
        let cases = [
            (RecurrenceSpec::Daily { interval: 1 }, "FREQ=DAILY"),
            (RecurrenceSpec::Daily { interval: 3 }, "FREQ=DAILY;INTERVAL=3"),
            (RecurrenceSpec::Weekly { interval: 1, days: vec![] }, "FREQ=WEEKLY"),
            (
                RecurrenceSpec::Weekly { interval: 1, days: vec![Weekday::Monday] },
                "FREQ=WEEKLY;BYDAY=MO",
            ),
            (
                RecurrenceSpec::Weekly {
                    interval: 2,
                    days: vec![Weekday::Monday, Weekday::Wednesday, Weekday::Friday],
                },
                "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE,FR",
            ),
            (RecurrenceSpec::Weekdays, "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR"),
            (RecurrenceSpec::Monthly { interval: 1, day: None }, "FREQ=MONTHLY"),
            (
                RecurrenceSpec::Monthly { interval: 1, day: Some(MonthDay::Nth(15)) },
                "FREQ=MONTHLY;BYMONTHDAY=15",
            ),
            (
                RecurrenceSpec::Monthly { interval: 1, day: Some(MonthDay::Last) },
                "FREQ=MONTHLY;BYMONTHDAY=-1",
            ),
            (RecurrenceSpec::Yearly { interval: 1 }, "FREQ=YEARLY"),
        ];
        for (spec, expected) in cases {
            assert_eq!(spec.to_rrule(), expected);
            // Everything this produces must be something recurrence can actually expand.
            assert!(crate::recur::Rule::parse(&spec.to_rrule()).is_ok(), "{expected}");
        }
    }

    #[test]
    fn a_bare_time_means_today() {
        let spec = DueSpec { time: Some(civil::time(15, 0, 0, 0)), ..DueSpec::default() };
        let due = spec.resolve(&now()).unwrap().unwrap();
        assert_eq!(due.date, date(2026, 5, 6));
        assert_eq!(due.time, Some(civil::time(15, 0, 0, 0)));
    }

    #[test]
    fn a_repetition_with_no_day_starts_at_its_first_real_occurrence() {
        // Typed on a Wednesday. Anchoring the series on Wednesday would produce a weekly
        // Monday rule whose start date is a day it never occurs.
        let spec = DueSpec {
            recurrence: Some(RecurrenceSpec::Weekly {
                interval: 1,
                days: vec![Weekday::Monday],
            }),
            ..DueSpec::default()
        };
        let due = spec.resolve(&now()).unwrap().unwrap();
        assert_eq!(due.date, date(2026, 5, 11));
        assert_eq!(due.recurrence.unwrap().rrule, "FREQ=WEEKLY;BYDAY=MO");
    }

    #[test]
    fn an_explicit_day_wins_over_the_repetitions_own_start() {
        let spec = DueSpec {
            date: Some(DateSpec::Tomorrow),
            recurrence: Some(RecurrenceSpec::Daily { interval: 1 }),
            from_completion: true,
            ..DueSpec::default()
        };
        let due = spec.resolve(&now()).unwrap().unwrap();
        assert_eq!(due.date, date(2026, 5, 7));
        assert!(due.recurrence.unwrap().from_completion);
    }

    #[test]
    fn naming_nothing_is_not_a_due_date() {
        assert!(DueSpec::default().is_empty());
        assert_eq!(DueSpec::default().resolve(&now()).unwrap(), None);
    }
}
