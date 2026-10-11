//! How urgent a task is: a number, computed and never stored, after Taskwarrior's.
//!
//! It orders what is suggested for a block and what a list sorts first. It never schedules
//! anything by itself. Additive terms: when it is due (rising, and still rising while
//! overdue, up to a ceiling), its priority, its age (a little), whether it is partly worked
//! (finishing beats starting), whether other tasks wait on it, whether it waits on another
//! (strongly negative: it cannot be done yet), and whether it is already in a block today
//! (mildly negative). **The project's weight multiplies the sum**: added, a heavy project's
//! trivia would outrank another's emergency; multiplied, the order within each project holds
//! and only the scale between projects changes.

use jiff::{Zoned, civil};

use crate::model::{BlockRef, Task};
use crate::snapshot::Snapshot;
use crate::state::Facts;

/// How much each term counts. The defaults are Taskwarrior's where it has the term.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    /// The most a due date adds, at a week overdue or more.
    pub due: f32,
    /// What each priority adds, from 1 (highest) to 4.
    pub priority: [f32; 4],
    /// The most age adds, at a year old.
    pub age: f32,
    /// Partly worked: time logged, or a timer running.
    pub started: f32,
    /// Another open task waits on it.
    pub blocking: f32,
    /// It waits on an open task: negative.
    pub blocked: f32,
    /// Already in a block today: negative.
    pub assigned_today: f32,
}

impl Default for Coefficients {
    fn default() -> Self {
        Self {
            due: 12.0,
            priority: [6.0, 3.9, 1.8, 0.0],
            age: 2.0,
            started: 4.0,
            blocking: 8.0,
            blocked: -5.0,
            assigned_today: -1.0,
        }
    }
}

/// One term of a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Term {
    /// When it is due.
    Due,
    /// Its priority.
    Priority,
    /// How old it is.
    Age,
    /// Partly worked.
    Started,
    /// Others wait on it.
    Blocking,
    /// It waits on another.
    Blocked,
    /// Already in a block today.
    AssignedToday,
}

/// A task's urgency, and what it is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct Urgency {
    /// The score: the terms' sum, times `weight`.
    pub score: f32,
    /// Each term that counts, before the weight.
    pub terms: Vec<(Term, f32)>,
    /// The project's weight, which multiplies the sum.
    pub weight: f32,
}

/// The day a sitting is on: its occurrence's, or a one-off block's own.
pub(crate) fn sitting_day(snapshot: &Snapshot, block: BlockRef) -> Option<civil::Date> {
    match block {
        BlockRef::Occurrence(_, date) => Some(date),
        BlockRef::OneOff(series) => snapshot.series.get(&series).map(|s| s.start_date),
    }
}

/// How far a due date counts, 0.2 a fortnight or more away, rising to 1.0 at a week overdue.
fn due_scale(days_until: f32) -> f32 {
    (0.2 + 0.8 * (14.0 - days_until) / 21.0).clamp(0.2, 1.0)
}

/// How urgent `task` is at `now`.
#[must_use]
pub fn urgency(snapshot: &Snapshot, facts: &Facts<'_>, task: &Task, now: &Zoned, c: &Coefficients) -> Urgency {
    let mut terms = Vec::new();
    let mut add = |term, value: f32| {
        if value != 0.0 {
            terms.push((term, value));
        }
    };

    if let Some(due) = &task.due {
        // A date without a time is due by the end of that day.
        let time = due.time.unwrap_or(civil::time(23, 59, 0, 0));
        let at = crate::remind::instant(due.date, time, now.time_zone());
        let days = at.duration_since(now.timestamp()).as_secs_f64() / 86_400.0;
        #[expect(clippy::cast_possible_truncation, reason = "a day count, rounded by far less than it means")]
        add(Term::Due, c.due * due_scale(days as f32));
    }

    let priority = usize::from(task.priority.as_u8().clamp(1, 4)) - 1;
    add(Term::Priority, c.priority[priority]);

    let age_days = now.timestamp().duration_since(task.created_at).as_secs_f64() / 86_400.0;
    #[expect(clippy::cast_possible_truncation, reason = "a fraction of a year")]
    add(Term::Age, c.age * (age_days / 365.0).clamp(0.0, 1.0) as f32);

    let sittings: Vec<_> = snapshot.assignments.values().filter(|a| a.task_id == task.id).collect();
    if sittings.iter().any(|a| a.accumulated_mins > 0 || a.is_running()) {
        add(Term::Started, c.started);
    }
    let open = |id| snapshot.tasks.get(id).is_some_and(|t: &Task| !t.is_deleted() && !facts.is_completed(t));
    if snapshot.tasks.values().any(|t| t.depends.contains(&task.id) && open(&t.id)) {
        add(Term::Blocking, c.blocking);
    }
    if task.depends.iter().any(open) {
        add(Term::Blocked, c.blocked);
    }
    let today = now.date();
    if sittings.iter().any(|a| sitting_day(snapshot, a.block_ref) == Some(today)) {
        add(Term::AssignedToday, c.assigned_today);
    }

    let weight = snapshot.effective_weight(task.project_id);
    let score = terms.iter().map(|(_, v)| v).sum::<f32>() * weight;
    Urgency { score, terms, weight }
}

/// Whole days from `now` to a task's due date, negative when overdue: for words about it.
#[must_use]
pub fn days_until_due(task: &Task, now: &Zoned) -> Option<i64> {
    let due = task.due.as_ref()?;
    let span = now.date().until(due.date).ok()?;
    Some(i64::from(span.get_days()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_due_date_counts_more_as_it_nears_and_more_still_while_overdue_up_to_a_ceiling() {
        assert!((due_scale(30.0) - 0.2).abs() < f32::EPSILON);
        assert!(due_scale(7.0) < due_scale(1.0));
        assert!(due_scale(1.0) < due_scale(-3.0));
        assert!((due_scale(-7.0) - 1.0).abs() < 1e-6);
        assert!((due_scale(-30.0) - 1.0).abs() < f32::EPSILON);
    }
}
