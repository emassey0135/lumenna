//! Tasks, their completions, and due dates (§3.2, §3.3, §3.5).

use std::collections::BTreeSet;

use jiff::Timestamp;
use jiff::civil;

use super::{ExternalRef, TzName};
use crate::id::{CompletionId, LabelId, ProjectId, TaskId};
use crate::order::OrderKey;

/// A thing to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// Identity.
    pub id: TaskId,
    /// Last-write-wins. Character-level merge is not worth the overhead on a short field.
    pub title: String,
    /// Automerge `Text` in the document, so long-form editing merges properly (§3.2).
    pub notes: String,
    /// Parent task, for subtasks. May dangle or form a cycle after merge; see
    /// [`crate::repair`].
    pub parent_id: Option<TaskId>,
    /// Always set. Inbox is a real [`Project`](super::Project), not a null project (§3.4).
    pub project_id: ProjectId,
    /// How much this one item matters. Project weight is the other axis and is never
    /// called priority (§3.4).
    pub priority: Priority,
    /// Cross-cutting context. Entries whose [`Label`](super::Label) was deleted project as
    /// absent (§3.4).
    pub labels: BTreeSet<LabelId>,
    /// Tasks that must complete before this one can start.
    ///
    /// Deliberately flat: no lag times, no start-to-start variants, no critical path.
    /// What justifies it is the planner, not the list — scheduling B into the morning when
    /// A sits in the afternoon is not a suboptimal plan, it is an impossible one, and
    /// nothing else in the model catches that. It is also what gives `blocked` and `ready`
    /// their meaning (§3.2).
    pub depends: BTreeSet<TaskId>,
    /// When it is due, and whether it recurs.
    pub due: Option<Due>,
    /// What makes multi-sitting work legible: remaining effort is this minus the time
    /// logged across assignments (§3.2).
    pub estimate_mins: Option<u32>,
    /// Position among siblings.
    pub order: OrderKey,
    /// Set when imported from a team system.
    pub external: Option<ExternalRef>,
    /// When it was created.
    pub created_at: Timestamp,
    /// Trash, for undo. A **product feature**, not a sync mechanism — Automerge handles
    /// deletion natively and needs no tombstones to converge (§3.2).
    pub deleted_at: Option<Timestamp>,
}

impl Task {
    /// A new task in a project, with everything optional left out.
    #[must_use]
    pub fn new(project_id: ProjectId, title: impl Into<String>, order: OrderKey) -> Self {
        Self {
            id: TaskId::new(),
            title: title.into(),
            notes: String::new(),
            parent_id: None,
            project_id,
            priority: Priority::default(),
            labels: BTreeSet::new(),
            depends: BTreeSet::new(),
            due: None,
            estimate_mins: None,
            order,
            external: None,
            created_at: crate::time::now(),
            deleted_at: None,
        }
    }

    /// Whether the task is in the trash.
    #[must_use]
    pub const fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// How much one task matters, `P1` highest.
///
/// Todoist's REST API inverts this — their p1 is API priority 4 — so convert at the
/// boundary when importing (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Priority {
    /// Highest.
    P1,
    /// High.
    P2,
    /// Medium.
    P3,
    /// None. The default, and what an unset or unrecognised value reads as.
    #[default]
    P4,
}

impl Priority {
    /// As stored: 1 is highest, 4 is none.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::P1 => 1,
            Self::P2 => 2,
            Self::P3 => 3,
            Self::P4 => 4,
        }
    }

    /// From a stored value, tolerating anything else as "none".
    ///
    /// A document written by a future version, or by a client with a bug, must still load
    /// (§3.1). Refusing to open the store over one bad integer is the wrong trade.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::P1,
            2 => Self::P2,
            3 => Self::P3,
            _ => Self::P4,
        }
    }
}

/// When a task is due (§3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    /// The day.
    pub date: civil::Date,
    /// The time of day, if there is one. "Tuesday" and "Tuesday at 3pm" are genuinely
    /// different states, not the same state with a default.
    pub time: Option<civil::Time>,
    /// Normally absent, meaning the due time floats. Set only when the task is anchored to
    /// a real place, such as a call in another region.
    pub timezone: Option<TzName>,
    /// Set if the task repeats.
    pub recurrence: Option<Recurrence>,
}

impl Due {
    /// A due date with no time and no recurrence.
    #[must_use]
    pub const fn on(date: civil::Date) -> Self {
        Self { date, time: None, timezone: None, recurrence: None }
    }

    /// A due date at a wall-clock time.
    #[must_use]
    pub const fn at(date: civil::Date, time: civil::Time) -> Self {
        Self { date, time: Some(time), timezone: None, recurrence: None }
    }
}

/// A repeat rule (§3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recurrence {
    /// An RFC 5545 `RRULE`.
    pub rrule: String,
    /// Whether the next occurrence counts from when it was actually completed rather than
    /// from when it was scheduled.
    ///
    /// This is Todoist's `every day` versus `every! day`. Scheduled-anchored recurrence can
    /// fall behind and show as overdue, which for some tasks is exactly right and for
    /// others is noise.
    pub from_completion: bool,
}

/// A record that a task was finished (§3.3).
///
/// Separate from the task because a recurring task is **a single task whose due date
/// advances**, not a generated series. It accumulates completions over time; a one-off task
/// has exactly one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCompletion {
    /// Identity.
    pub id: CompletionId,
    /// What was completed.
    pub task_id: TaskId,
    /// When.
    pub completed_at: Timestamp,
    /// Which occurrence, for a recurring task.
    pub occurrence_date: Option<civil::Date>,
    /// Set if a parent's cascade caused this, rather than the user.
    ///
    /// "Completing a parent completes its subtasks" is a setting (§3.10, default on). If
    /// the cascade completes three subtasks and the parent is then uncompleted, a subtask
    /// independently finished last week must not be uncompleted with the rest — so the
    /// uncomplete path only reverses completions it caused.
    ///
    /// Because the setting can change, this is **stored at write time, never derived at
    /// read time**: deriving would retroactively rewrite history when the setting flips.
    pub cascaded_from: Option<TaskId>,
}

impl TaskCompletion {
    /// Records a completion the user performed.
    #[must_use]
    pub fn new(task_id: TaskId, occurrence_date: Option<civil::Date>) -> Self {
        Self {
            id: CompletionId::new(),
            task_id,
            completed_at: crate::time::now(),
            occurrence_date,
            cascaded_from: None,
        }
    }

    /// Records a completion caused by `parent`'s cascade.
    ///
    /// `occurrence_date` is needed for the same reason it is on any other completion: a
    /// recurring subtask is only complete for the occurrence it is currently due on, so a
    /// cascade that recorded no occurrence would leave the subtask reading as unfinished the
    /// moment it was written.
    #[must_use]
    pub fn cascaded(
        task_id: TaskId,
        occurrence_date: Option<civil::Date>,
        parent: TaskId,
    ) -> Self {
        Self { cascaded_from: Some(parent), ..Self::new(task_id, occurrence_date) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_round_trips_and_tolerates_garbage() {
        for p in [Priority::P1, Priority::P2, Priority::P3, Priority::P4] {
            assert_eq!(Priority::from_u8(p.as_u8()), p);
        }
        assert_eq!(Priority::from_u8(0), Priority::P4);
        assert_eq!(Priority::from_u8(9), Priority::P4);
        assert_eq!(Priority::default(), Priority::P4);
    }

    #[test]
    fn priority_sorts_most_important_first() {
        let mut ps = vec![Priority::P4, Priority::P1, Priority::P3];
        ps.sort();
        assert_eq!(ps, vec![Priority::P1, Priority::P3, Priority::P4]);
    }
}
