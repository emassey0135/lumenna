//! What to work on in a block: the tasks its filter admits, by urgency, with a task whose
//! estimate fits the time left raised, and the reasons kept, since the reasons are what a
//! person hears ("suggested because overdue by two days, priority 1, and its 30-minute
//! estimate fits your remaining 35 minutes"). Nothing is excluded for not fitting or for being
//! in a block elsewhere today; it is only ranked lower.

use jiff::Zoned;

use crate::filter::{Context, Expr};
use crate::id::TaskId;
use crate::model::BlockRef;
use crate::snapshot::Snapshot;
use crate::urgency::{Term, days_until_due, urgency};

/// What fitting the time left adds to a task's score.
const FITS: f32 = 2.0;

/// One reason a task is suggested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Overdue by this many days (zero: due earlier today).
    Overdue(i64),
    /// Due in this many days (zero: today).
    DueIn(i64),
    /// Its priority, when above the lowest.
    Priority(u8),
    /// Time has gone into it already.
    Started,
    /// This many open tasks wait on it.
    Blocking(usize),
    /// Its estimate fits the minutes left in the block.
    Fits {
        /// The estimate.
        estimate: u32,
        /// What is left.
        remaining: u32,
    },
    /// Its estimate is longer than what is left: still offered, lower.
    Longer {
        /// The estimate.
        estimate: u32,
        /// What is left.
        remaining: u32,
    },
    /// It waits on another task, so it cannot be finished yet.
    Blocked,
    /// It is in a block already today.
    AssignedToday,
}

/// A task suggested for a block.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// The task.
    pub task: TaskId,
    /// Its rank: urgency, plus fitting the time left.
    pub score: f32,
    /// Why, in the order they matter.
    pub reasons: Vec<Reason>,
}

/// The minutes left in `block` (of `length` minutes) once what is in it is counted: each
/// sitting's planned length, else its task's estimate, else nothing.
#[must_use]
pub fn remaining(snapshot: &Snapshot, block: BlockRef, length: u32) -> u32 {
    let planned: u32 = snapshot
        .assignments
        .values()
        .filter(|a| a.block_ref == block)
        .map(|a| a.planned_mins.or_else(|| snapshot.tasks.get(&a.task_id).and_then(|t| t.estimate_mins)).unwrap_or(0))
        .sum();
    length.saturating_sub(planned)
}

/// The tasks to suggest for `block` (of `length` minutes), best first, at most `limit`:
/// open ones `filter` admits (every open one when absent), none already in it.
#[must_use]
pub fn suggest(
    snapshot: &Snapshot,
    filter: Option<&Expr>,
    block: BlockRef,
    length: u32,
    now: &Zoned,
    limit: usize,
) -> Vec<Suggestion> {
    let cx = Context::new(snapshot, now);
    let facts = snapshot.facts();
    let left = remaining(snapshot, block, length);
    let inside: std::collections::BTreeSet<TaskId> =
        snapshot.assignments.values().filter(|a| a.block_ref == block).map(|a| a.task_id).collect();
    let coefficients = snapshot.settings.urgency;
    let mut found: Vec<Suggestion> = snapshot
        .tasks
        .values()
        .filter(|t| !t.is_deleted() && !facts.is_completed(t) && !inside.contains(&t.id))
        .filter(|t| filter.is_none_or(|expr| expr.matches(t, &cx)))
        .map(|task| {
            let scored = urgency(snapshot, &facts, task, now, &coefficients);
            let mut reasons = Vec::new();
            let mut score = scored.score;
            for (term, _) in &scored.terms {
                match term {
                    Term::Due => {
                        if let Some(days) = days_until_due(task, now) {
                            reasons.push(if days < 0 { Reason::Overdue(-days) } else { Reason::DueIn(days) });
                        }
                    }
                    Term::Priority if task.priority.as_u8() < 4 => reasons.push(Reason::Priority(task.priority.as_u8())),
                    Term::Started => reasons.push(Reason::Started),
                    Term::Blocking => reasons.push(Reason::Blocking(
                        snapshot
                            .tasks
                            .values()
                            .filter(|t| t.depends.contains(&task.id) && !t.is_deleted() && !facts.is_completed(t))
                            .count(),
                    )),
                    _ => {}
                }
            }
            if let Some(estimate) = task.estimate_mins {
                if estimate <= left {
                    score += FITS;
                    reasons.push(Reason::Fits { estimate, remaining: left });
                } else {
                    reasons.push(Reason::Longer { estimate, remaining: left });
                }
            }
            for (term, _) in &scored.terms {
                match term {
                    Term::Blocked => reasons.push(Reason::Blocked),
                    Term::AssignedToday => reasons.push(Reason::AssignedToday),
                    _ => {}
                }
            }
            Suggestion { task: task.id, score, reasons }
        })
        .collect();
    // Best first; ties by creation order, the same on every device.
    found.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.task.cmp(&b.task)));
    found.truncate(limit);
    found
}
