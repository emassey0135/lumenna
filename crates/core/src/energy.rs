//! Energy, as the person rates it: capacity for focus and for starting things, which ADHD and
//! other conditions make uneven. Not calories, and never measured: estimated, as a length is.
//!
//! Three things, each optional, each worth having with only a few rated:
//!
//! - **A task's energy** ([`Task::energy`](crate::model::Task::energy)): what it takes, low,
//!   medium or high. Unrated is medium, so rating nothing changes nothing.
//! - **A day's capacity**: a usual amount ([`Settings::energy_capacity`], minutes of ordinary
//!   work, none by default) and a check-in that changes one day's ([`Settings::energy_today`]:
//!   "I'm low today").
//! - **When energy is good**: a block marked as the person's best or worst hours
//!   ([`BlockSeries::hours`](crate::model::BlockSeries::hours)), else a time of day the settings
//!   name ([`Settings::best_hours`], [`Settings::worst_hours`]).
//!
//! The planner weighs them and only proposes: a demanding task is raised in the good hours and
//! lowered in the worst and on a low day, a light one the other way (`suggestions`); Plan the
//! Day stops before the day's capacity is spent (`dayplan`), and Replan from Now lightens a day
//! planned past it (`reflow`). What it weighed is said in each proposal's reasons.

use std::fmt;
use std::str::FromStr;

use jiff::civil;

use crate::model::{AssignmentStatus, BlockRef, BlockSeries, Settings, Task};
use crate::snapshot::Snapshot;

/// What a task takes, or what a person has, of energy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Energy {
    /// Little: something to do on a bad day.
    Low,
    /// Ordinary. What an unrated task counts as, and a day with no check-in.
    #[default]
    Medium,
    /// A great deal: demanding work, for the good hours.
    High,
}

impl Energy {
    /// Every level, lowest first.
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// As stored and typed: `low`, `medium`, `high`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// What a sitting of this task counts against a day's capacity, in hundredths of its
    /// minutes: a demanding task spends twice its length, a light one half.
    #[must_use]
    pub const fn weight_percent(self) -> u32 {
        match self {
            Self::Low => 50,
            Self::Medium => 100,
            Self::High => 200,
        }
    }
}

impl fmt::Display for Energy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Energy {
    type Err = ();

    /// `low`, `medium` or `high`, in any case, and the words a person might use for them.
    fn from_str(text: &str) -> Result<Self, ()> {
        match text.trim().to_lowercase().as_str() {
            "low" | "light" | "l" => Ok(Self::Low),
            "medium" | "usual" | "normal" | "ordinary" | "m" => Ok(Self::Medium),
            "high" | "demanding" | "h" => Ok(Self::High),
            _ => Err(()),
        }
    }
}

/// The energy a task takes: its rating, or medium unrated.
#[must_use]
pub fn of(task: &Task) -> Energy {
    task.energy.unwrap_or_default()
}

/// What `minutes` of `task` count against a day's capacity.
#[must_use]
pub fn cost(task: &Task, minutes: u32) -> u32 {
    minutes.saturating_mul(of(task).weight_percent()) / 100
}

/// When a person's energy is good or bad: a block, or a time of day, so marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Hours {
    /// The person's best: demanding work goes here.
    Best,
    /// The person's worst: light work, or none.
    Worst,
}

impl Hours {
    /// As stored and typed: `best`, `worst`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Worst => "worst",
        }
    }
}

impl fmt::Display for Hours {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Hours {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        match text.trim().to_lowercase().as_str() {
            "best" | "good" => Ok(Self::Best),
            "worst" | "bad" => Ok(Self::Worst),
            _ => Err(()),
        }
    }
}

/// A stretch of the day, from a time to a later one: the best or worst hours a setting names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeRange {
    /// Its start.
    pub start: civil::Time,
    /// Its end, after the start.
    pub end: civil::Time,
}

impl TimeRange {
    /// Whether `time` falls in it, its start included and its end not.
    #[must_use]
    pub fn holds(&self, time: civil::Time) -> bool {
        self.start <= time && time < self.end
    }
}

impl fmt::Display for TimeRange {
    /// `HH:MM-HH:MM`, as every time is stored.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}-{:02}:{:02}", self.start.hour(), self.start.minute(), self.end.hour(), self.end.minute())
    }
}

impl FromStr for TimeRange {
    type Err = ();

    /// `HH:MM-HH:MM`, its end after its start.
    fn from_str(text: &str) -> Result<Self, ()> {
        let (start, end) = text.split_once('-').ok_or(())?;
        let start: civil::Time = start.trim().parse().map_err(|_| ())?;
        let end: civil::Time = end.trim().parse().map_err(|_| ())?;
        (start < end).then_some(Self { start, end }).ok_or(())
    }
}

/// A day's check-in: how the person said their energy is that day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CheckIn {
    /// The day it is for. A check-in says nothing of any other day.
    pub date: civil::Date,
    /// What they said.
    pub level: Energy,
}

impl fmt::Display for CheckIn {
    /// `YYYY-MM-DD level`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.date, self.level)
    }
}

impl FromStr for CheckIn {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        let (date, level) = text.trim().split_once(' ').ok_or(())?;
        Ok(Self { date: date.parse().map_err(|_| ())?, level: level.parse()? })
    }
}

/// How the person said their energy is on `day`, if they checked in for it.
#[must_use]
pub fn checked_in(settings: &Settings, day: civil::Date) -> Option<Energy> {
    settings.energy_today.filter(|c| c.date == day).map(|c| c.level)
}

/// What a day has room for, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity {
    /// Minutes of ordinary work the day has room for, a demanding task's counting double and a
    /// light one's half.
    pub minutes: u32,
    /// What the person said of the day, if they checked in.
    pub level: Option<Energy>,
    /// The usual amount, if one is set: with none and a low day, the day is half its work
    /// blocks' time.
    pub usual: Option<u32>,
}

/// What a low day has of the usual, and a high one beyond it, in hundredths.
const LOW_PERCENT: u32 = 50;
const HIGH_PERCENT: u32 = 150;

/// What `day` has room for, `work_mins` being the length of its blocks that take tasks: the
/// usual amount, half of it on a day the person said they are low, half again on a high one.
/// With no usual amount a day has no limit, unless the person said they are low: then half its
/// work blocks' time. None is no limit.
#[must_use]
pub fn capacity(settings: &Settings, day: civil::Date, work_mins: u32) -> Option<Capacity> {
    let level = checked_in(settings, day);
    let usual = settings.energy_capacity.filter(|m| *m > 0);
    let minutes = match (level, usual) {
        (Some(Energy::Low), Some(usual)) => usual * LOW_PERCENT / 100,
        (Some(Energy::Low), None) => work_mins * LOW_PERCENT / 100,
        (Some(Energy::High), Some(usual)) => usual * HIGH_PERCENT / 100,
        (Some(Energy::Medium) | None, Some(usual)) => usual,
        (Some(Energy::High | Energy::Medium) | None, None) => return None,
    };
    Some(Capacity { minutes, level, usual })
}

/// What `day` has spent of its capacity: every sitting on it, worked or still to come, at what
/// it took or is planned to take, weighed by its task's energy. A sitting skipped or moved on
/// spent nothing.
#[must_use]
pub fn spent(snapshot: &Snapshot, day: civil::Date) -> u32 {
    snapshot
        .assignments
        .values()
        .filter(|a| snapshot.assignment_date(a) == Some(day))
        .filter_map(|a| {
            let task = snapshot.tasks.get(&a.task_id).filter(|t| !t.is_deleted())?;
            let minutes = match a.status {
                AssignmentStatus::Worked => a.accumulated_mins,
                AssignmentStatus::Planned | AssignmentStatus::InProgress => a
                    .planned_mins
                    .or(task.estimate_mins)
                    .unwrap_or(crate::dayplan::UNESTIMATED_MINS)
                    .max(a.accumulated_mins),
                _ => return None,
            };
            Some(cost(task, minutes))
        })
        .sum()
}

/// The length of `day`'s blocks that take tasks, for a low day with no usual amount.
///
/// # Errors
///
/// If a block's rule cannot be expanded.
pub fn work_mins(snapshot: &Snapshot, day: civil::Date) -> Result<u32, crate::recur::RecurError> {
    Ok(snapshot.day(day)?.iter().filter(|o| o.flags.accepts_tasks).map(|o| o.duration_mins).sum())
}

/// Whether a block starting at `start` is in the person's best or worst hours: its own mark,
/// else the settings' time of day that holds its start.
#[must_use]
pub fn hours_at(settings: &Settings, series: Option<&BlockSeries>, start: civil::Time) -> Option<Hours> {
    if let Some(hours) = series.and_then(|s| s.hours) {
        return Some(hours);
    }
    if settings.best_hours.is_some_and(|r| r.holds(start)) {
        return Some(Hours::Best);
    }
    settings.worst_hours.is_some_and(|r| r.holds(start)).then_some(Hours::Worst)
}

/// Whether `block` is in the person's best or worst hours, on the day it falls.
#[must_use]
pub fn hours_of(snapshot: &Snapshot, block: BlockRef) -> Option<Hours> {
    let series = snapshot.series.get(&block.series_id());
    let date = snapshot.block_date(block)?;
    let start = snapshot
        .day(date)
        .ok()?
        .into_iter()
        .find(|o| o.series_id == block.series_id())
        .map(|o| o.start_time)
        .or_else(|| series.map(|s| s.start_time))?;
    hours_at(&snapshot.settings, series, start)
}

/// How a task's energy meets a block's hours and the day's check-in: what the planner weighs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// A demanding task in the person's best hours: raised.
    DemandingInBest,
    /// A demanding task in their worst hours: lowered.
    DemandingInWorst,
    /// A light task in their worst hours: raised.
    LightInWorst,
    /// A light task in their best hours, which demanding work wants: lowered a little.
    LightInBest,
    /// A demanding task on a day they said they are low: lowered.
    DemandingOnLowDay,
    /// A light task on a day they said they are low: raised.
    LightOnLowDay,
}

impl Fit {
    /// What it adds to a task's rank among suggestions.
    #[must_use]
    pub const fn score(self) -> f32 {
        match self {
            Self::DemandingInBest => 3.0,
            Self::LightInWorst | Self::LightOnLowDay => 2.0,
            Self::LightInBest => -1.0,
            Self::DemandingInWorst | Self::DemandingOnLowDay => -3.0,
        }
    }
}

/// How `task` meets a block in `hours` on a day checked in as `level`: one for the hours and
/// one for the day, each only where it says something. An unrated task meets nothing.
#[must_use]
pub fn fits(task: &Task, hours: Option<Hours>, level: Option<Energy>) -> Vec<Fit> {
    let Some(energy) = task.energy.filter(|e| *e != Energy::Medium) else { return Vec::new() };
    let mut found = Vec::new();
    match (energy, hours) {
        (Energy::High, Some(Hours::Best)) => found.push(Fit::DemandingInBest),
        (Energy::High, Some(Hours::Worst)) => found.push(Fit::DemandingInWorst),
        (Energy::Low, Some(Hours::Worst)) => found.push(Fit::LightInWorst),
        (Energy::Low, Some(Hours::Best)) => found.push(Fit::LightInBest),
        _ => {}
    }
    match (energy, level) {
        (Energy::High, Some(Energy::Low)) => found.push(Fit::DemandingOnLowDay),
        (Energy::Low, Some(Energy::Low)) => found.push(Fit::LightOnLowDay),
        _ => {}
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_in_says_nothing_of_another_day() {
        let mut settings = Settings::default();
        let today = civil::date(2026, 11, 27);
        settings.energy_today = Some(CheckIn { date: today, level: Energy::Low });
        assert_eq!(checked_in(&settings, today), Some(Energy::Low));
        assert_eq!(checked_in(&settings, today.tomorrow().unwrap()), None);
    }

    #[test]
    fn a_day_has_no_limit_until_a_usual_amount_is_set_or_the_person_is_low() {
        let mut settings = Settings::default();
        let today = civil::date(2026, 11, 27);
        assert_eq!(capacity(&settings, today, 240), None);
        settings.energy_today = Some(CheckIn { date: today, level: Energy::Low });
        assert_eq!(capacity(&settings, today, 240).map(|c| c.minutes), Some(120));
        settings.energy_capacity = Some(300);
        assert_eq!(capacity(&settings, today, 240).map(|c| c.minutes), Some(150));
        settings.energy_today = Some(CheckIn { date: today, level: Energy::High });
        assert_eq!(capacity(&settings, today, 240).map(|c| c.minutes), Some(450));
        settings.energy_today = None;
        assert_eq!(capacity(&settings, today, 240).map(|c| c.minutes), Some(300));
    }

    #[test]
    fn an_unrated_task_meets_no_hours_and_no_day() {
        let task = Task::new(crate::ProjectId::INBOX, "Essay", crate::OrderKey::middle());
        assert!(fits(&task, Some(Hours::Best), Some(Energy::Low)).is_empty());
        let demanding = Task { energy: Some(Energy::High), ..task };
        assert_eq!(fits(&demanding, Some(Hours::Best), Some(Energy::Low)), vec![Fit::DemandingInBest, Fit::DemandingOnLowDay]);
    }

    #[test]
    fn stored_forms_read_back() {
        let range: TimeRange = "09:00-12:00".parse().unwrap();
        assert_eq!(range.to_string(), "09:00-12:00");
        assert!("12:00-09:00".parse::<TimeRange>().is_err());
        let check: CheckIn = "2026-11-27 low".parse().unwrap();
        assert_eq!(check.to_string(), "2026-11-27 low");
        assert_eq!("High".parse::<Energy>(), Ok(Energy::High));
    }
}
