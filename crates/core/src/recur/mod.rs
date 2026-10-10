//! Recurrence: two systems, which share a syntax and nothing else.
//!
//! **Recurring tasks** are one task whose due date advances. There is no generated series
//! and no list of future instances — completing it moves it forward, and the history lives
//! in [`TaskCompletion`](crate::model::TaskCompletion) records. This is the opposite
//! of how calendar events work, and getting it backwards is the standard mistake in this
//! category. [`advance`] is that whole system.
//!
//! **Recurring blocks** are a series definition plus sparse exceptions, expanded on read.
//! This *is* how calendar events work: unmodified occurrences are never stored, because a
//! daily routine would otherwise generate thousands of rows and make every sync a bulk
//! transfer. [`expand`] is that system.
//!
//! Both go through [`Rule`], which wraps RFC 5545 expansion. Note what is *not* here:
//! turning English into an RRULE. No mature Rust implementation of that exists, so it is
//! written by hand as part of the quick-add grammar; this module takes the RFC string
//! that grammar produces, and the ones calendar import brings in from other systems.

mod convert;

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use jiff::civil;
use rrule::{Frequency, RRule, RRuleSet, Unvalidated};

use crate::id::SeriesId;
use crate::model::{BlockFlags, BlockKind, BlockSeries, Due, ExceptionAction};

/// How many occurrences a single expansion will produce before giving up.
///
/// A bound rather than a hope. `FREQ=SECONDLY` is rejected outright, but a legitimate rule
/// over a wide range can still ask for more than any caller wants, and a screen reader
/// facing a frozen application is worse off than one facing an error it can read.
const MAX_OCCURRENCES: usize = 10_000;

/// How far the iterator will run past the anchor looking for the next occurrence.
///
/// Reached only by a rule whose matches are extremely sparse — `BYMONTHDAY=29;BYMONTH=2`
/// skips three years at a time — or by one that matches nothing at all after its anchor.
const MAX_SEARCH: usize = 100_000;

/// A recurrence rule that could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecurError {
    /// The string is not a recurrence rule this build understands.
    #[error("not a valid recurrence rule ({rrule:?}): {detail}")]
    Invalid {
        /// The rule as given.
        rrule: String,
        /// What the parser said.
        detail: String,
    },

    /// A sub-daily frequency, which this model has no representation for.
    #[error("{frequency} recurrence is not supported: a block or task occurs on a day")]
    SubDaily {
        /// The frequency that was asked for.
        frequency: &'static str,
    },

    /// A date outside the range the recurrence engine can represent.
    #[error("{date} is outside the range recurrence can be computed over")]
    DateOutOfRange {
        /// The offending date.
        date: civil::Date,
    },

    /// Expansion produced more occurrences than any caller can want.
    #[error("recurrence rule {rrule:?} produced more than {limit} occurrences")]
    TooMany {
        /// The rule.
        rrule: String,
        /// The cap it hit.
        limit: usize,
    },
}

/// A parsed RFC 5545 recurrence rule.
///
/// Holds the rule but **not** its anchor. That is deliberate: a recurring task's rule is
/// re-anchored every time it advances (see [`advance`]), so binding the two together at
/// parse time would mean re-parsing on every completion.
#[derive(Debug, Clone)]
pub struct Rule {
    text: String,
    parsed: RRule<Unvalidated>,
}

impl Rule {
    /// Parses a rule, rejecting what this model cannot represent.
    ///
    /// # Errors
    ///
    /// [`RecurError::Invalid`] if it is not RFC 5545, or [`RecurError::SubDaily`] for
    /// hourly and finer frequencies.
    pub fn parse(text: &str) -> Result<Self, RecurError> {
        let parsed: RRule<Unvalidated> = text.parse().map_err(|e: rrule::RRuleError| {
            RecurError::Invalid { rrule: text.to_owned(), detail: e.to_string() }
        })?;
        // Sub-daily frequencies are not merely unsupported, they are unrepresentable: a
        // block has one start time and a task has one due time, so an hourly rule would
        // expand to the same date twenty-four times over and the duplicates would be
        // silently collapsed. Refusing is the only honest answer.
        let frequency = match parsed.get_freq() {
            Frequency::Hourly => Some("hourly"),
            Frequency::Minutely => Some("minutely"),
            Frequency::Secondly => Some("secondly"),
            Frequency::Yearly | Frequency::Monthly | Frequency::Weekly | Frequency::Daily => None,
        };
        if let Some(frequency) = frequency {
            return Err(RecurError::SubDaily { frequency });
        }
        Ok(Self { text: text.to_owned(), parsed })
    }

    /// The rule as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The rule's `COUNT`, if it sets one.
    #[must_use]
    pub fn count(&self) -> Option<u32> {
        self.parsed.get_count()
    }

    /// Whether the rule ever stops on its own.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.parsed.get_count().is_some() || self.parsed.get_until().is_some()
    }

    fn anchored(&self, anchor: civil::Date) -> Result<RRuleSet, RecurError> {
        let dtstart =
            convert::to_engine(anchor).ok_or(RecurError::DateOutOfRange { date: anchor })?;
        let validated = self.parsed.clone().validate(dtstart).map_err(|e| RecurError::Invalid {
            rrule: self.text.clone(),
            detail: e.to_string(),
        })?;
        Ok(RRuleSet::new(dtstart).rrule(validated))
    }

    /// The first occurrence strictly after `after`, with the rule anchored at `anchor`.
    ///
    /// `None` means the rule has run out — `UNTIL` has passed, or `COUNT` is exhausted.
    ///
    /// # Errors
    ///
    /// If the rule cannot be anchored at that date, or matches nothing within
    /// [`MAX_SEARCH`] occurrences.
    pub fn next_after(
        &self,
        anchor: civil::Date,
        after: civil::Date,
    ) -> Result<Option<civil::Date>, RecurError> {
        let set = self.anchored(anchor)?;
        for (steps, datetime) in set.into_iter().enumerate() {
            if steps >= MAX_SEARCH {
                return Err(RecurError::TooMany { rrule: self.text.clone(), limit: MAX_SEARCH });
            }
            let Some(date) = convert::from_engine(&datetime) else {
                continue;
            };
            if date > after {
                return Ok(Some(date));
            }
        }
        Ok(None)
    }

    /// The first occurrence on or after `anchor`.
    ///
    /// Usually the anchor itself, but not always: RFC 5545 starts at the first date matching
    /// the rule *on or after* DTSTART, so `FREQ=WEEKLY;BYDAY=MO` anchored on a Tuesday first
    /// occurs the following Monday. That gap is what quick add needs to close when someone
    /// types *"every monday"* without a date.
    ///
    /// # Errors
    ///
    /// If the rule cannot be anchored, or matches nothing within [`MAX_SEARCH`] steps.
    pub fn first_from(&self, anchor: civil::Date) -> Result<Option<civil::Date>, RecurError> {
        let set = self.anchored(anchor)?;
        for (steps, datetime) in set.into_iter().enumerate() {
            if steps >= MAX_SEARCH {
                return Err(RecurError::TooMany { rrule: self.text.clone(), limit: MAX_SEARCH });
            }
            if let Some(date) = convert::from_engine(&datetime) {
                return Ok(Some(date));
            }
        }
        Ok(None)
    }

    /// Every occurrence within `range`, with the rule anchored at `anchor`.
    ///
    /// Occurrences before `anchor` do not exist — an RRULE's first occurrence is its
    /// anchor — so a range starting earlier simply yields fewer dates.
    ///
    /// # Errors
    ///
    /// If the rule cannot be anchored, or produces more than [`MAX_OCCURRENCES`] dates
    /// inside the range.
    pub fn occurrences(
        &self,
        anchor: civil::Date,
        range: &RangeInclusive<civil::Date>,
    ) -> Result<Vec<civil::Date>, RecurError> {
        let set = self.anchored(anchor)?;
        let mut dates = Vec::new();
        for (steps, datetime) in set.into_iter().enumerate() {
            if steps >= MAX_SEARCH {
                return Err(RecurError::TooMany { rrule: self.text.clone(), limit: MAX_SEARCH });
            }
            let Some(date) = convert::from_engine(&datetime) else {
                continue;
            };
            if date > *range.end() {
                break;
            }
            if date >= *range.start() {
                if dates.len() >= MAX_OCCURRENCES {
                    return Err(RecurError::TooMany {
                        rrule: self.text.clone(),
                        limit: MAX_OCCURRENCES,
                    });
                }
                dates.push(date);
            }
        }
        Ok(dates)
    }
}

/// What completing a recurring task does to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advanced {
    /// The task does not repeat; completing it is the end of it.
    NotRecurring,
    /// The task comes back on this date.
    Next(Due),
    /// The rule has produced its last occurrence. The task is done for good.
    Finished,
}

/// Moves a recurring task's due date forward after a completion.
///
/// # The two modes
///
/// `from_completion` is Todoist's `every day` versus `every! day`, and the difference shows
/// up exactly when you are late:
///
/// - **`false`** — advance from the *scheduled* date. A task due Monday and finished on
///   Friday comes back on Tuesday, in the past, and shows as overdue. For a bill or a
///   medication that is right: the schedule is the point, and falling behind is information.
/// - **`true`** — advance from *completion*. That same task comes back next Monday.
///   For "water the plants every 3 days" that is right, because the interval is what
///   matters and the calendar is incidental.
///
/// # Why `occurrences_completed` has to be passed in
///
/// The rule is re-anchored on each advance, because the model keeps only the *current* due
/// date — a recurring task is one task whose date moves, not a series with a
/// remembered origin. Re-anchoring is exact for `UNTIL`, which is an absolute date, and for
/// every `BY*` part, since the current due date is itself on the sequence.
///
/// It is *not* exact for `COUNT`, which counts from an origin the model no longer has. So
/// the caller supplies how many occurrences have been completed — the task's
/// [`TaskCompletion`](crate::model::TaskCompletion) records, which exist precisely because a
/// recurring task accumulates them — and this counts the one being recorded now.
///
/// # Errors
///
/// If the stored rule cannot be parsed or anchored.
pub fn advance(
    due: &Due,
    completed_on: civil::Date,
    occurrences_completed: u32,
) -> Result<Advanced, RecurError> {
    let Some(recurrence) = &due.recurrence else {
        return Ok(Advanced::NotRecurring);
    };
    let rule = Rule::parse(&recurrence.rrule)?;

    if let Some(count) = rule.count()
        && occurrences_completed >= count
    {
        return Ok(Advanced::Finished);
    }

    let anchor = if recurrence.from_completion { completed_on } else { due.date };
    match rule.next_after(anchor, anchor)? {
        Some(date) => Ok(Advanced::Next(Due { date, ..due.clone() })),
        None => Ok(Advanced::Finished),
    }
}

/// One occurrence of a block, with its series' fields and any exception already applied.
///
/// A resolved value rather than a reference into the series, because an exception may
/// override almost any of it and callers should never have to remember to check.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    /// Which series it came from.
    pub series_id: SeriesId,
    /// The day it falls on. This is the date the *rule* produced, which is what a
    /// [`BlockException`](crate::model::BlockException) is keyed by even when the exception
    /// moves the block's time.
    pub date: civil::Date,
    /// Effective start.
    pub start_time: civil::Time,
    /// Effective duration.
    pub duration_mins: u32,
    /// How far re-flow may compress it.
    pub min_duration_mins: u32,
    /// Effective title.
    pub title: String,
    /// Effective kind.
    pub kind: BlockKind,
    /// Effective flags.
    pub flags: BlockFlags,
    /// Whether an exception changed this occurrence from what the series says.
    pub modified: bool,
}

impl Occurrence {
    /// How an assignment refers to this occurrence.
    ///
    /// A one-off block's assignments name the series alone, so that moving the block
    /// carries them with it. A recurring block's occurrence has no identifier of its own
    /// until it is excepted, so it is named by series and date.
    #[must_use]
    pub fn block_ref(&self, series: &BlockSeries) -> crate::model::BlockRef {
        if series.is_recurring() {
            crate::model::BlockRef::Occurrence(self.series_id, self.date)
        } else {
            crate::model::BlockRef::OneOff(self.series_id)
        }
    }

    /// When it ends, as a civil time. Wraps past midnight for a block that runs over.
    #[must_use]
    pub fn end_time(&self) -> civil::Time {
        self.start_time + jiff::SignedDuration::from_mins(i64::from(self.duration_mins))
    }

    /// Whether it has ended by `time` on its own day. Compared in minutes since midnight,
    /// because [`end_time`](Self::end_time) wraps: a block ending at midnight ends at 00:00,
    /// and would otherwise read as over all day.
    #[must_use]
    pub fn is_over_at(&self, time: civil::Time) -> bool {
        let minutes = |t: civil::Time| i64::from(t.hour()) * 60 + i64::from(t.minute());
        minutes(self.start_time) + i64::from(self.duration_mins) <= minutes(time)
    }
}

/// Expands a block series across a date range, applying its exceptions.
///
/// `exception_for` supplies the exception for a given occurrence date, if there is one.
/// Cancelled occurrences are dropped; modified ones come back with their overrides applied
/// and [`Occurrence::modified`] set.
///
/// A series with no rule occurs once, on its start date.
///
/// # Errors
///
/// If the series' rule cannot be parsed, anchored, or bounded.
pub fn expand<'a>(
    series: &BlockSeries,
    exception_for: impl Fn(civil::Date) -> Option<&'a ExceptionAction>,
    range: &RangeInclusive<civil::Date>,
) -> Result<Vec<Occurrence>, RecurError> {
    // The series' own end date bounds the rule independently of any UNTIL it carries, and
    // the requested range bounds both.
    let last = series.end_date.map_or(*range.end(), |end| end.min(*range.end()));
    if last < *range.start() || last < series.start_date {
        return Ok(Vec::new());
    }
    let window = (*range.start()).max(series.start_date)..=last;

    let dates = match &series.rrule {
        Some(text) => Rule::parse(text)?.occurrences(series.start_date, &window)?,
        None if window.contains(&series.start_date) => vec![series.start_date],
        None => Vec::new(),
    };

    let mut occurrences = Vec::with_capacity(dates.len());
    for date in dates {
        match exception_for(date) {
            Some(ExceptionAction::Cancelled) => {}
            Some(ExceptionAction::Modified { start_time, duration_mins, title, kind, flags }) => {
                let kind = kind.unwrap_or(series.kind);
                let duration = duration_mins.unwrap_or(series.duration_mins);
                occurrences.push(Occurrence {
                    series_id: series.id,
                    date,
                    start_time: start_time.unwrap_or(series.start_time),
                    duration_mins: duration,
                    min_duration_mins: floor_for(series, kind, duration),
                    title: title.clone().unwrap_or_else(|| series.title.clone()),
                    kind,
                    flags: flags.unwrap_or(series.flags),
                    modified: true,
                });
            }
            None => occurrences.push(Occurrence {
                series_id: series.id,
                date,
                start_time: series.start_time,
                duration_mins: series.duration_mins,
                min_duration_mins: series.compression_floor_mins(),
                title: series.title.clone(),
                kind: series.kind,
                flags: series.flags,
                modified: false,
            }),
        }
    }
    Ok(occurrences)
}

/// The compression floor for an occurrence whose duration or kind an exception changed.
///
/// The series' explicit override still applies when it fits, since it says something about
/// this block that the kind's default does not — a two-hour work block useless under an
/// hour stays useless under an hour when shortened to ninety minutes. When it no longer
/// fits, the kind decides, which is what keeps a shortened break incompressible.
fn floor_for(series: &BlockSeries, kind: BlockKind, duration_mins: u32) -> u32 {
    series
        .min_duration_mins
        .filter(|m| *m <= duration_mins)
        .unwrap_or_else(|| kind.default_min_duration(duration_mins))
}

/// Expands every series in a snapshot across a range, in the order a day is lived.
///
/// Deleted series are skipped. Sorting is by start time, then duration, then series
/// identifier — the last two only so that two blocks starting at the same minute have a
/// stable order rather than one that changes between devices.
///
/// # Errors
///
/// If any series' rule cannot be expanded. One bad rule fails the range rather than
/// silently omitting a block, because a day view missing a meeting is worse than one that
/// says it could not be built.
pub fn expand_all(
    series: &BTreeMap<SeriesId, BlockSeries>,
    exceptions: &BTreeMap<(SeriesId, civil::Date), crate::model::BlockException>,
    range: &RangeInclusive<civil::Date>,
) -> Result<Vec<Occurrence>, RecurError> {
    let mut out = Vec::new();
    for one in series.values().filter(|s| s.deleted_at.is_none()) {
        out.extend(expand(
            one,
            |date| exceptions.get(&(one.id, date)).map(|e| &e.action),
            range,
        )?);
    }
    out.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then(a.start_time.cmp(&b.start_time))
            .then(a.duration_mins.cmp(&b.duration_mins))
            .then(a.series_id.cmp(&b.series_id))
    });
    Ok(out)
}
