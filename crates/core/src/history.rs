//! What has happened to a task, from the records already kept: its sittings and the time in
//! them, how often it was put in a block and left, how old it is, how often a recurring one
//! was done. Repeated putting off says a task is badly defined or secretly unimportant:
//! support for executive function, asked for, never pushed at someone.

use jiff::{Zoned, civil};

use crate::model::{AssignmentStatus, Task};
use crate::snapshot::Snapshot;

/// A task's history, as counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// Sittings it has had, past and planned.
    pub sittings: u32,
    /// Minutes logged across them.
    pub worked_mins: u32,
    /// Its estimate, to say the time against.
    pub estimate_mins: Option<u32>,
    /// Sittings on days already gone that it was never worked in: planned and left, skipped
    /// or moved on.
    pub put_off: u32,
    /// Times its due date was moved later by hand (the store's history, [`Snapshot::postponed`]).
    pub postponed: u32,
    /// Whole days since it was made.
    pub age_days: i64,
    /// Times it was done (a recurring task's occurrences).
    pub completions: u32,
    /// The last day time was logged on it.
    pub last_worked: Option<civil::Date>,
}

/// `task`'s history, as of `now`.
#[must_use]
pub fn history(snapshot: &Snapshot, task: &Task, now: &Zoned) -> History {
    let today = now.date();
    let mut found = History {
        estimate_mins: task.estimate_mins,
        age_days: now.timestamp().duration_since(task.created_at).as_secs() / 86_400,
        completions: snapshot.completion_count(task.id),
        postponed: snapshot.postponed.get(&task.id).copied().unwrap_or(0),
        ..History::default()
    };
    for sitting in snapshot.assignments.values().filter(|a| a.task_id == task.id) {
        found.sittings += 1;
        found.worked_mins += sitting.accumulated_mins;
        let day = snapshot.assignment_date(sitting);
        if sitting.accumulated_mins > 0 {
            found.last_worked = found.last_worked.max(day);
        }
        let gone = day.is_some_and(|d| d < today);
        let left = matches!(sitting.status, AssignmentStatus::Planned | AssignmentStatus::Skipped | AssignmentStatus::Deferred);
        if gone && left && sitting.accumulated_mins == 0 {
            found.put_off += 1;
        }
    }
    found
}
