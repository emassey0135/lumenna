//! A whole day planned in one go: each work block still to come is given the tasks its
//! suggestions rank first, greedily, until their estimates fill it. A proposal, always
//! optional, never applied by itself. Greedy and editable is worth more than optimal packing:
//! a person can follow why each task went where it did, and change it.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;

use crate::filter::Expr;
use crate::id::{SeriesId, TaskId};
use crate::model::{AssignmentStatus, BlockRef};
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
    /// Its planned length: what is left of the task's estimate, or [`UNESTIMATED_MINS`].
    pub minutes: u32,
    /// Whether the task had no estimate, or has used it up, so `minutes` is assumed.
    pub assumed: bool,
    /// The minutes already worked on the task, from its earlier sittings.
    pub spent: u32,
    /// What the task still needed when this sitting was planned: `minutes` is a part of it
    /// when less.
    pub rest: u32,
    /// Which of the task's sittings this plan gives it that day, from 1: a second is there
    /// because its due date leaves too few days for one a day.
    pub sitting: u32,
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
    /// Tasks due by the day planned that were given only part of what they need, with the
    /// minutes no block had room for.
    pub short: Vec<(TaskId, u32)>,
}

fn minutes(time: jiff::civil::Time) -> i64 {
    crate::time::minutes_of_day(time)
}

/// A plan for `day`'s work blocks still to come at `now`, each given what its filter (from
/// `filters`, read by the caller; a block absent from it takes any task) admits, best first,
/// leaving out `excluded` (tasks the person said not to plan today) and each of `declined`
/// in that block (a sitting the person turned down, which may still go elsewhere).
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
    declined: &BTreeSet<(TaskId, SeriesId)>,
) -> Result<DayProposal, RecurError> {
    let mut occurrences: Vec<Occurrence> = snapshot.day(day)?;
    occurrences.sort_by_key(|o| (o.start_time, o.series_id));
    let today = now.date();
    let now_mins = minutes(now.time());
    let mut taken: BTreeSet<TaskId> = excluded.clone();
    // A task with a sitting still to come that day is planned already. One worked earlier and
    // still open is not: a task takes as many sittings as it needs.
    for assignment in snapshot.assignments.values() {
        if matches!(assignment.status, AssignmentStatus::Planned | AssignmentStatus::InProgress)
            && snapshot.assignment_date(assignment) == Some(day)
        {
            taken.insert(assignment.task_id);
        }
    }
    let facts = snapshot.facts();
    // Each block that takes tasks, with the time it has free: only the time still to come,
    // none of a day gone, what is left of the block under way, less the work already in it.
    let mut rooms: Vec<(&Occurrence, BlockRef, u32)> = Vec::new();
    for occurrence in &occurrences {
        if !occurrence.flags.accepts_tasks {
            continue;
        }
        let start = minutes(occurrence.start_time);
        let finish = start + i64::from(occurrence.duration_mins);
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
        let left = usable.saturating_sub(already);
        if left >= SHORTEST_SITTING {
            rooms.push((occurrence, block, left));
        }
    }
    // A task bigger than every block is given its first part where there is the most room, not
    // a scrap of whichever block comes first: only where no later block has more free.
    let most_room = rooms.iter().map(|(_, _, left)| *left).max().unwrap_or(0);
    let later_room: Vec<u32> =
        (0..rooms.len()).map(|i| rooms[i + 1..].iter().map(|(_, _, left)| *left).max().unwrap_or(0)).collect();
    // What each task has been given so far: its sittings and their minutes.
    let mut given: BTreeMap<TaskId, (u32, u32)> = BTreeMap::new();
    let mut proposal = DayProposal::default();
    for (index, (occurrence, block, mut left)) in rooms.into_iter().enumerate() {
        let filter = filters.get(&occurrence.series_id);
        for suggestion in suggest(snapshot, filter, block, left, now, usize::MAX) {
            if left < SHORTEST_SITTING {
                break;
            }
            // What cannot be finished yet is not planned; it is still offered by hand.
            if taken.contains(&suggestion.task)
                || suggestion.reasons.contains(&Reason::Blocked)
                || declined.contains(&(suggestion.task, occurrence.series_id))
            {
                continue;
            }
            let Some(task) = snapshot.tasks.get(&suggestion.task) else { continue };
            let spent = spent_mins(snapshot, &facts, task, now);
            let (rest, assumed) = needs(task, spent);
            let (sittings, planned) = given.get(&task.id).copied().unwrap_or_default();
            let need = rest.saturating_sub(planned);
            if need == 0 || sittings >= sittings_today(task, rest, most_room, day) {
                continue;
            }
            let chosen = task.sitting_mins.filter(|s| *s > 0);
            let too_big = chosen.is_none() && rest > most_room;
            // A sitting of the length chosen, or the whole of what is left where it fits.
            let mut length = need.min(chosen.unwrap_or(need));
            if length > left {
                // A task too big for any block takes all the room there is: where no later
                // block has more for its first part, in any block for another its due date needs.
                if too_big && (sittings > 0 || left >= later_room[index]) {
                    length = left;
                } else {
                    continue;
                }
            }
            left -= length;
            given.insert(task.id, (sittings + 1, planned + length));
            let part = length < need;
            let reasons = suggestion
                .reasons
                .into_iter()
                // Too long for the block is no reason against a part planned to fit it.
                .filter(|reason| !(part && matches!(reason, Reason::Longer { .. })))
                .map(|reason| match reason {
                    // Said against the time left when it was chosen, not the block's whole.
                    Reason::Fits { estimate, .. } => Reason::Fits { estimate, remaining: left + length },
                    other => other,
                })
                .collect();
            proposal.sittings.push(PlannedSitting {
                block,
                task: task.id,
                minutes: length,
                assumed,
                spent,
                rest: rest - planned,
                sitting: sittings + 1,
                reasons,
            });
        }
    }
    for task in snapshot.tasks.values() {
        if task.is_deleted()
            || facts.is_completed(task)
            || taken.contains(&task.id)
            || day < today
            || !task.due.as_ref().is_some_and(|due| due.date <= day)
        {
            continue;
        }
        let (rest, _) = needs(task, spent_mins(snapshot, &facts, task, now));
        match given.get(&task.id) {
            None => proposal.unfit.push(task.id),
            Some((_, planned)) if *planned < rest => proposal.short.push((task.id, rest - planned)),
            Some(_) => {}
        }
    }
    Ok(proposal)
}

/// What `task` still needs, having had `spent` minutes: what is left of its estimate, or
/// [`UNESTIMATED_MINS`] with no estimate or one used up, and whether that length is assumed.
fn needs(task: &crate::model::Task, spent: u32) -> (u32, bool) {
    match task.estimate_mins {
        Some(estimate) if spent < estimate => ((estimate - spent).max(1), false),
        _ => (UNESTIMATED_MINS, true),
    }
}

/// How many sittings `task` may have on `day`: one, unless its due date leaves too few days
/// for the sittings it needs, of the length it chose or (too big for any block) the most room a
/// block has; due by `day`, as many as it needs.
fn sittings_today(task: &crate::model::Task, rest: u32, most_room: u32, day: jiff::civil::Date) -> u32 {
    let length = task.sitting_mins.filter(|s| *s > 0).unwrap_or(if rest <= most_room { rest } else { most_room }).max(1);
    let needed = rest.div_ceil(length);
    let Some(due) = task.due.as_ref().map(|due| due.date) else { return 1 };
    if due <= day {
        return needed.max(1);
    }
    // The days after this one, up to and including the day it is due, each take one.
    let after = u32::try_from((due - day).get_days()).unwrap_or(u32::MAX);
    needed.saturating_sub(after).max(1)
}

/// The minutes already worked on `task`'s current occurrence: every sitting of a one-off
/// task, and of a repeating one, those after the day it was last done, since an earlier
/// occurrence's time says nothing about this one's.
#[must_use]
pub fn spent_mins(snapshot: &Snapshot, facts: &crate::state::Facts<'_>, task: &crate::model::Task, now: &Zoned) -> u32 {
    let last_done = facts
        .completions_of(task.id)
        .iter()
        .map(|c| c.completed_at.to_zoned(now.time_zone().clone()).date())
        .max();
    facts
        .assignments_of(task.id)
        .iter()
        .filter(|a| {
            last_done.is_none_or(|done| snapshot.assignment_date(a).is_none_or(|date| date > done))
        })
        .map(|a| a.accumulated_mins)
        .sum()
}
