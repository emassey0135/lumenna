//! Tasks, their completions, and due dates.

use std::collections::BTreeSet;

use jiff::Timestamp;
use jiff::civil;

use super::{ExternalRef, TzName};
use crate::id::{CompletionId, LabelId, ProjectId, SectionId, TaskId};
use crate::order::OrderKey;

/// A thing to do.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Task {
    /// Identity.
    pub id: TaskId,
    /// Last-write-wins. Character-level merge is not worth the overhead on a short field.
    pub title: String,
    /// Automerge `Text` in the document, so long-form editing merges properly.
    pub notes: String,
    /// Parent task, for subtasks. May dangle or form a cycle after merge; see
    /// [`crate::repair`].
    pub parent_id: Option<TaskId>,
    /// Always set. Inbox is a real [`Project`](super::Project), not a null project.
    pub project_id: ProjectId,
    /// The heading it sits under in its project, if any. Believed only while that section is
    /// its project's and not deleted (`Snapshot::section_of`); a subtask is listed under its
    /// parent, whatever it names.
    pub section_id: Option<SectionId>,
    /// How much this one item matters. Project weight is the other axis and is never
    /// called priority.
    pub priority: Priority,
    /// Cross-cutting context. Entries whose [`Label`](super::Label) was deleted project as
    /// absent.
    pub labels: BTreeSet<LabelId>,
    /// Tasks that must complete before this one can start.
    ///
    /// Deliberately flat: no lag times, no start-to-start variants, no critical path.
    /// What justifies it is the planner, not the list — scheduling B into the morning when
    /// A sits in the afternoon is not a suboptimal plan, it is an impossible one, and
    /// nothing else in the model catches that. It is also what gives `blocked` and `ready`
    /// their meaning.
    pub depends: BTreeSet<TaskId>,
    /// When it is due, and whether it recurs: the day it is meant to be done.
    pub due: Option<Due>,
    /// When it may be begun: a day before which it is kept out of the listings and the planner,
    /// or [`Start::Someday`], kept but never offered until it is given a date ([`crate::when`]).
    pub start: Option<Start>,
    /// The day it must be done by, which the urgency ranks by beside the due date: the due date
    /// says when it is meant to be done, the deadline when it can wait no longer.
    pub deadline: Option<civil::Date>,
    /// What makes multi-sitting work legible: remaining effort is this minus the time
    /// logged across assignments.
    pub estimate_mins: Option<u32>,
    /// How long one sitting of it should be, for work better done in pieces (drafting an
    /// essay): the planner plans it a sitting of this at a time, one a day unless its due
    /// date needs more. Absent, it is planned whole where it fits.
    pub sitting_mins: Option<u32>,
    /// What it takes of the person's energy, as they rate it: low, medium or high
    /// ([`crate::energy`]). Unrated counts as medium, so rating nothing changes nothing.
    pub energy: Option<crate::energy::Energy>,
    /// When the person last kept it, answering the planner's question about it
    /// ([`crate::review`]): it is not asked about as stale again for
    /// [`crate::review::STALE_DAYS`].
    pub kept_at: Option<Timestamp>,
    /// How often it had been put off when it was kept, so it is asked again only once it is put
    /// off [`crate::review::PUT_OFF_TIMES`] times more.
    pub kept_put_off: Option<u32>,
    /// Position among siblings.
    pub order: OrderKey,
    /// Set when imported from a team system.
    pub external: Option<ExternalRef>,
    /// When it was created.
    pub created_at: Timestamp,
    /// Trash, for undo. A **product feature**, not a sync mechanism — Automerge handles
    /// deletion natively and needs no tombstones to converge.
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
            section_id: None,
            priority: Priority::default(),
            labels: BTreeSet::new(),
            depends: BTreeSet::new(),
            due: None,
            start: None,
            deadline: None,
            estimate_mins: None,
            sitting_mins: None,
            energy: None,
            kept_at: None,
            kept_put_off: None,
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
/// boundary when importing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// A document written by a future version, or by a client with a bug, must still load.
    /// Refusing to open the store over one bad integer is the wrong trade.
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

/// When a task is due.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// When a task may be begun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Start {
    /// Not before this day.
    On(civil::Date),
    /// Kept, but never offered until it is given a date.
    Someday,
}

impl Start {
    /// The word Someday is stored, typed and filtered as.
    pub const SOMEDAY: &'static str = "someday";

    /// The day it starts, if it has one.
    #[must_use]
    pub const fn date(self) -> Option<civil::Date> {
        match self {
            Self::On(date) => Some(date),
            Self::Someday => None,
        }
    }
}

impl std::fmt::Display for Start {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::On(date) => write!(f, "{date}"),
            Self::Someday => f.write_str(Self::SOMEDAY),
        }
    }
}

impl std::str::FromStr for Start {
    type Err = jiff::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.trim().eq_ignore_ascii_case(Self::SOMEDAY) { Ok(Self::Someday) } else { text.trim().parse().map(Self::On) }
    }
}

/// A repeat rule.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// A record that a task was finished.
///
/// Separate from the task because a recurring task is **a single task whose due date
/// advances**, not a generated series. It accumulates completions over time; a one-off task
/// has exactly one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// "Completing a parent completes its subtasks" is a setting (default on). If
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
