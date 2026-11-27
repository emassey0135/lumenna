//! Time reports: the time every sitting logged, summed by week, by project and by label, with
//! the estimates of the tasks worked beside them, so where estimates were wrong can be read.
//!
//! Computed from the sittings, never stored: a sitting's logged time is its own record, and a
//! sum kept beside it would be a second figure that could disagree with the first.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Timestamp;
use jiff::civil::{Date, Weekday};

use crate::id::{LabelId, ProjectId, TaskId};
use crate::snapshot::Snapshot;

/// How many weeks a report covers by default, this week the last of them.
pub const REPORT_WEEKS: u32 = 4;

/// One sum: the time logged, and the estimates of the tasks it was logged on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Total {
    /// Every second logged, a timer running counted to `now`, capped at its block's length as
    /// [`crate::model::BlockAssignment::elapsed`] caps it.
    pub logged_secs: u64,
    /// The estimates of the tasks worked, each task once, those with one.
    pub estimate_mins: u32,
    /// How many tasks were worked.
    pub tasks: u32,
    /// How many of them have no estimate.
    pub unestimated: u32,
    /// How many sittings logged time.
    pub sittings: u32,
}

/// What a report sums, over its weeks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The first day it covers: the first day of its first week.
    pub first: Date,
    /// The last: today.
    pub last: Date,
    /// Each week by its first day, newest first, every one of them, nothing logged or not.
    pub weeks: Vec<(Date, Total)>,
    /// Each project time was logged in, most time first.
    pub projects: Vec<(ProjectId, Total)>,
    /// Each label of a task time was logged on, most time first. A task with two labels counts
    /// in both, so these need not add up to the whole.
    pub labels: Vec<(LabelId, Total)>,
    /// Everything over the weeks.
    pub total: Total,
}

/// The first day of the week `date` falls in, a week starting on `week_start`.
#[must_use]
pub fn week_of(date: Date, week_start: Weekday) -> Date {
    let back = (date.weekday().to_monday_zero_offset() - week_start.to_monday_zero_offset()).rem_euclid(7);
    date.checked_sub(jiff::Span::new().days(i64::from(back))).unwrap_or(date)
}

/// A sum being made: the seconds, the sittings, and the tasks worked.
#[derive(Default)]
struct Sum {
    secs: u64,
    sittings: u32,
    tasks: BTreeSet<TaskId>,
}

impl Sum {
    fn add(&mut self, task: TaskId, secs: u64) {
        self.secs = self.secs.saturating_add(secs);
        self.sittings += 1;
        self.tasks.insert(task);
    }

    fn total(&self, snapshot: &Snapshot) -> Total {
        let estimates: Vec<Option<u32>> = self.tasks.iter().map(|id| snapshot.tasks.get(id).and_then(|t| t.estimate_mins)).collect();
        Total {
            logged_secs: self.secs,
            estimate_mins: estimates.iter().flatten().fold(0u32, |sum, mins| sum.saturating_add(*mins)),
            tasks: u32::try_from(self.tasks.len()).unwrap_or(u32::MAX),
            unestimated: u32::try_from(estimates.iter().filter(|e| e.is_none()).count()).unwrap_or(u32::MAX),
            sittings: self.sittings,
        }
    }
}

/// The report of the `weeks` weeks to `today`, at `now`: every sitting on a day in them that
/// logged time, its task not in the trash.
#[must_use]
pub fn report(snapshot: &Snapshot, today: Date, weeks: u32, now: Timestamp) -> Report {
    let week_start = snapshot.settings.week_start;
    let this_week = week_of(today, week_start);
    let starts: Vec<Date> = (0..weeks.max(1))
        .map(|back| this_week.checked_sub(jiff::Span::new().weeks(i64::from(back))).unwrap_or(this_week))
        .collect();
    let first = *starts.last().unwrap_or(&this_week);
    let mut by_week: BTreeMap<Date, Sum> = starts.iter().map(|start| (*start, Sum::default())).collect();
    let mut by_project: BTreeMap<ProjectId, Sum> = BTreeMap::new();
    let mut by_label: BTreeMap<LabelId, Sum> = BTreeMap::new();
    let mut total = Sum::default();
    for sitting in snapshot.assignments.values() {
        let Some(task) = snapshot.tasks.get(&sitting.task_id).filter(|t| !t.is_deleted()) else { continue };
        let Some(day) = snapshot.assignment_date(sitting) else { continue };
        if day < first || day > today {
            continue;
        }
        let cap = snapshot.series.get(&sitting.block_ref.series_id()).map(|s| s.duration_mins);
        let secs = sitting.elapsed_secs(now, cap);
        if secs == 0 {
            continue;
        }
        by_week.entry(week_of(day, week_start)).or_default().add(task.id, secs);
        by_project.entry(task.project_id).or_default().add(task.id, secs);
        for label in task.labels.iter().filter(|l| snapshot.labels.get(l).is_some_and(|l| l.deleted_at.is_none())) {
            by_label.entry(*label).or_default().add(task.id, secs);
        }
        total.add(task.id, secs);
    }
    Report {
        first,
        last: today,
        weeks: by_week.into_iter().rev().map(|(start, sum)| (start, sum.total(snapshot))).collect(),
        projects: ranked(snapshot, by_project),
        labels: ranked(snapshot, by_label),
        total: total.total(snapshot),
    }
}

/// Each sum, most time first; the identifier, which is creation order, breaking ties alike
/// everywhere.
fn ranked<K: Ord + Copy>(snapshot: &Snapshot, sums: BTreeMap<K, Sum>) -> Vec<(K, Total)> {
    let mut totals: Vec<_> = sums.into_iter().map(|(id, sum)| (id, sum.total(snapshot))).collect();
    totals.sort_by_key(|(id, total)| (std::cmp::Reverse(total.logged_secs), *id));
    totals
}
