//! A whole day planned in one go: each work block still to come is given the tasks its
//! suggestions rank first, greedily, until their estimates fill it. A proposal, always
//! optional, never applied by itself. Greedy and editable is worth more than optimal packing:
//! a person can follow why each task went where it did, and change it.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;

use crate::filter::Expr;
use crate::id::{SeriesId, TaskId};
use crate::model::BlockRef;
use crate::recur::{Occurrence, RecurError};
use crate::snapshot::Snapshot;
use crate::suggestions::{Reason, remaining, suggest};

/// What a task without an estimate is taken to need: something has to be counted, and a
/// task with no estimate is planned as one sitting of this.
pub const UNESTIMATED_MINS: u32 = 30;

/// A block with less than this left is full: a sitting shorter than this is not worth having.
pub const SHORTEST_SITTING: u32 = 15;

/// One sitting the plan proposes.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSitting {
    /// The block, as a sitting names it.
    pub block: BlockRef,
    /// The task.
    pub task: TaskId,
    /// Its planned length: the task's estimate, or [`UNESTIMATED_MINS`].
    pub minutes: u32,
    /// Whether the task had no estimate, so `minutes` is assumed.
    pub assumed: bool,
    /// Why it was chosen, as suggestions say it.
    pub reasons: Vec<Reason>,
}

/// What a whole-day plan proposes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DayProposal {
    /// The sittings, in the order of the day.
    pub sittings: Vec<PlannedSitting>,
    /// Tasks due by the day planned that no block could take: the planner cannot make time.
    pub unfit: Vec<TaskId>,
}

fn minutes(time: jiff::civil::Time) -> i64 {
    i64::from(time.hour()) * 60 + i64::from(time.minute())
}

/// A plan for `day`'s work blocks still to come at `now`, each given what its filter (from
/// `filters`, read by the caller; a block absent from it takes any task) admits, best first,
/// leaving out `excluded` (tasks the person said not to plan today).
///
/// # Errors
///
/// If a block's rule cannot be expanded.
pub fn propose_day(
    snapshot: &Snapshot,
    day: jiff::civil::Date,
    now: &Zoned,
    filters: &BTreeMap<SeriesId, Expr>,
    excluded: &BTreeSet<TaskId>,
) -> Result<DayProposal, RecurError> {
    let mut occurrences: Vec<Occurrence> = snapshot.day(day)?;
    occurrences.sort_by_key(|o| (o.start_time, o.series_id));
    let today = now.date();
    let now_mins = minutes(now.time());
    let mut taken: BTreeSet<TaskId> = excluded.clone();
    // A task already in a block today is planned already.
    for assignment in snapshot.assignments.values() {
        let on = match assignment.block_ref {
            BlockRef::Occurrence(_, date) => Some(date),
            BlockRef::OneOff(series) => snapshot.series.get(&series).map(|s| s.start_date),
        };
        if on == Some(day) {
            taken.insert(assignment.task_id);
        }
    }
    let mut proposal = DayProposal::default();
    for occurrence in &occurrences {
        if !occurrence.flags.accepts_tasks {
            continue;
        }
        let start = minutes(occurrence.start_time);
        let finish = start + i64::from(occurrence.duration_mins);
        // Only the time still to come: none of a day gone, what is left of the block under way.
        let usable = if day < today {
            0
        } else if day == today {
            u32::try_from((finish - start.max(now_mins)).max(0)).unwrap_or(0)
        } else {
            occurrence.duration_mins
        };
        let block = match snapshot.series.get(&occurrence.series_id) {
            Some(series) if series.is_recurring() => BlockRef::Occurrence(occurrence.series_id, occurrence.date),
            _ => BlockRef::OneOff(occurrence.series_id),
        };
        let already = occurrence.duration_mins - remaining(snapshot, block, occurrence.duration_mins);
        let mut left = usable.saturating_sub(already);
        if left < SHORTEST_SITTING {
            continue;
        }
        let filter = filters.get(&occurrence.series_id);
        for suggestion in suggest(snapshot, filter, block, left, now, usize::MAX) {
            if left < SHORTEST_SITTING {
                break;
            }
            // What cannot be finished yet is not planned; it is still offered by hand.
            if taken.contains(&suggestion.task) || suggestion.reasons.contains(&Reason::Blocked) {
                continue;
            }
            let Some(task) = snapshot.tasks.get(&suggestion.task) else { continue };
            let (length, assumed) = task.estimate_mins.map_or((UNESTIMATED_MINS, true), |e| (e.max(1), false));
            if length > left {
                continue;
            }
            left -= length;
            taken.insert(task.id);
            let reasons = suggestion
                .reasons
                .into_iter()
                .map(|reason| match reason {
                    // Said against the time left when it was chosen, not the block's whole.
                    Reason::Fits { estimate, .. } => Reason::Fits { estimate, remaining: left + length },
                    other => other,
                })
                .collect();
            proposal.sittings.push(PlannedSitting { block, task: task.id, minutes: length, assumed, reasons });
        }
    }
    let facts = snapshot.facts();
    proposal.unfit = snapshot
        .tasks
        .values()
        .filter(|t| !t.is_deleted() && !facts.is_completed(t) && !taken.contains(&t.id))
        .filter(|t| day >= today && t.due.as_ref().is_some_and(|due| due.date <= day))
        .map(|t| t.id)
        .collect();
    Ok(proposal)
}
