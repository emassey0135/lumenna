//! When the day slips: a proposal for the rest of it, never a change by itself.
//!
//! **Move the work before moving the day.** A block holding more planned work than the time
//! it has left gives up its last sittings first: an assignment is only a plan, a block a
//! commitment. Then the walk, forward from now: an anchored block (or one the person pinned)
//! stays where it is and the rest route around it; a movable block that is pushed later
//! shrinks if it has time above its minimum, keeping its end, and moves later at its minimum
//! if not, carrying the slip on; a break's minimum is its whole length, so it moves rather
//! than being squeezed. Nothing moves before now or past the day's end. What does not fit is
//! said: the planner cannot make time.
//!
//! Times here are minutes from midnight on the day, civil: the day as the person reads it.

use std::collections::{BTreeMap, BTreeSet};

use crate::id::{AssignmentId, SeriesId, TaskId};
use crate::model::{AssignmentStatus, BlockRef};
use crate::recur::Occurrence;
use crate::snapshot::Snapshot;

/// What the person decided about one block, which the next proposal keeps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    /// Leave it where and as long as it is.
    Keep(SeriesId),
    /// Skip it today, freeing its time.
    Skip(SeriesId),
    /// Leave this sitting in its block, though the block cannot hold it all.
    KeepSitting(AssignmentId),
}

/// One change a proposal makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposed {
    /// A block starts later, and maybe is shorter.
    Block {
        /// Which.
        series: SeriesId,
        /// Its start before, minutes from midnight.
        from_start: i64,
        /// Its start after.
        to_start: i64,
        /// Its length before.
        from_mins: u32,
        /// Its length after.
        to_mins: u32,
    },
    /// A block the person chose to skip today.
    Skip {
        /// Which.
        series: SeriesId,
    },
    /// A sitting leaves a block that cannot hold it; where it goes is the caller's to find.
    MoveSitting {
        /// The sitting.
        assignment: AssignmentId,
        /// Its task.
        task: TaskId,
        /// The block it leaves.
        from: SeriesId,
    },
}

/// What a re-flow proposes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proposal {
    /// The changes, in the order they happen in the day.
    pub changes: Vec<Proposed>,
    /// Blocks that do not fit before the day's end, even at their shortest.
    pub unplaceable: Vec<SeriesId>,
}

fn minutes(time: jiff::civil::Time) -> i64 {
    i64::from(time.hour()) * 60 + i64::from(time.minute())
}

/// A block as the walk sees it.
#[derive(Debug, Clone)]
struct Slot {
    series: SeriesId,
    start: i64,
    length: u32,
    floor: u32,
    fixed: bool,
}

/// A re-flow of `day` from `now` (minutes from midnight) to `end` (the day window's end),
/// keeping to `pins`.
///
/// # Errors
///
/// If a block's rule cannot be expanded.
pub fn propose_reflow(
    snapshot: &Snapshot,
    day: jiff::civil::Date,
    now: i64,
    end: i64,
    pins: &[Pin],
) -> Result<Proposal, crate::recur::RecurError> {
    let kept: BTreeSet<SeriesId> = pins.iter().filter_map(|p| if let Pin::Keep(s) = p { Some(*s) } else { None }).collect();
    let skipped: BTreeSet<SeriesId> = pins.iter().filter_map(|p| if let Pin::Skip(s) = p { Some(*s) } else { None }).collect();
    let staying: BTreeSet<AssignmentId> =
        pins.iter().filter_map(|p| if let Pin::KeepSitting(a) = p { Some(*a) } else { None }).collect();
    let mut occurrences: Vec<Occurrence> = snapshot.day(day)?;
    occurrences.sort_by_key(|o| (o.start_time, o.series_id));
    let reference = |o: &Occurrence| match snapshot.series.get(&o.series_id) {
        Some(series) if series.is_recurring() => BlockRef::Occurrence(o.series_id, o.date),
        _ => BlockRef::OneOff(o.series_id),
    };

    // The block under way, if any: started, not yet over.
    let current = occurrences
        .iter()
        .find(|o| minutes(o.start_time) <= now && now < minutes(o.start_time) + i64::from(o.duration_mins));
    let mut proposal = Proposal::default();

    // Move the work first: a block whose planned sittings outrun its time gives up the last.
    for occurrence in &occurrences {
        let start = minutes(occurrence.start_time);
        let finish = start + i64::from(occurrence.duration_mins);
        if finish <= now || skipped.contains(&occurrence.series_id) {
            continue;
        }
        let available = finish - start.max(now);
        let block = reference(occurrence);
        let mut sittings: Vec<_> = snapshot
            .assignments
            .values()
            .filter(|a| a.block_ref == block && a.status == AssignmentStatus::Planned && a.accumulated_mins == 0)
            .collect();
        sittings.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
        let started_work: i64 = snapshot
            .assignments
            .values()
            .filter(|a| a.block_ref == block && a.status == AssignmentStatus::InProgress)
            .map(|a| i64::from(a.planned_mins.unwrap_or(0).saturating_sub(a.accumulated_mins)))
            .sum();
        let length = |a: &&crate::model::BlockAssignment| {
            i64::from(a.planned_mins.or_else(|| snapshot.tasks.get(&a.task_id).and_then(|t| t.estimate_mins)).unwrap_or(0))
        };
        let mut planned: i64 = started_work + sittings.iter().map(length).sum::<i64>();
        // A sitting the person keeps is counted, never moved: the next to go is the last of the rest.
        while planned > available {
            let Some(at) = sittings.iter().rposition(|a| !staying.contains(&a.id)) else { break };
            let last = sittings.remove(at);
            planned -= length(&last);
            proposal.changes.push(Proposed::MoveSitting { assignment: last.id, task: last.task_id, from: occurrence.series_id });
        }
    }

    // Then the walk, from the end of the block under way, or now.
    let mut cursor = current.map_or(now, |o| (minutes(o.start_time) + i64::from(o.duration_mins)).max(now));
    let mut queue: Vec<Slot> = occurrences
        .iter()
        .filter(|o| Some(o.series_id) != current.map(|c| c.series_id))
        // Still to come, or due to have started during the block under way, which it ran over.
        .filter(|o| minutes(o.start_time) >= now || current.is_some_and(|c| o.start_time >= c.start_time))
        .map(|o| Slot {
            series: o.series_id,
            start: minutes(o.start_time),
            length: o.duration_mins,
            floor: o.min_duration_mins.min(o.duration_mins).max(1),
            fixed: o.flags.anchored || kept.contains(&o.series_id),
        })
        .collect();
    for slot in queue.iter().filter(|s| skipped.contains(&s.series)) {
        proposal.changes.push(Proposed::Skip { series: slot.series });
    }
    queue.retain(|s| !skipped.contains(&s.series));

    let mut placed: BTreeMap<SeriesId, (i64, u32)> = BTreeMap::new();
    // Movables placed since the last fixed block, which a fixed one can push past itself.
    let mut stretch: Vec<SeriesId> = Vec::new();
    let originals: BTreeMap<SeriesId, Slot> = queue.iter().map(|s| (s.series, s.clone())).collect();
    let mut pending: std::collections::VecDeque<Slot> = queue.into_iter().collect();
    while let Some(slot) = pending.pop_front() {
        if slot.fixed {
            // Movables crowding it go after it, latest first, keeping their order.
            while cursor > slot.start {
                let Some(series) = stretch.pop() else { break };
                if let Some((start, _)) = placed.remove(&series) {
                    cursor = start;
                }
                pending.push_front(originals[&series].clone());
            }
            if cursor > slot.start {
                // The block under way runs into it: a conflict to say, not a move to pretend.
                proposal.unplaceable.push(slot.series);
                continue;
            }
            placed.insert(slot.series, (slot.start, slot.length));
            cursor = slot.start + i64::from(slot.length);
            stretch.clear();
            continue;
        }
        let start = slot.start.max(cursor);
        let length = if start == slot.start {
            slot.length
        } else {
            // Pushed later: shrink, keeping the end, down to its floor; past that, move later.
            let natural_end = slot.start + i64::from(slot.length);
            u32::try_from((natural_end - start).max(i64::from(slot.floor))).unwrap_or(slot.floor).min(slot.length)
        };
        // A fixed block still to come that this would run into: it is placed after that one.
        let next_fixed = pending.iter().find(|s| s.fixed).map(|s| s.start);
        if next_fixed.is_some_and(|fixed| start + i64::from(length) > fixed && start < fixed) {
            let at = pending.iter().position(|s| s.fixed).map_or(pending.len(), |i| i + 1);
            pending.insert(at, slot);
            continue;
        }
        // A day's block starts within the day; past the day's end, or past midnight, it does not fit.
        if start + i64::from(length) > end || start >= 24 * 60 {
            proposal.unplaceable.push(slot.series);
            continue;
        }
        placed.insert(slot.series, (start, length));
        stretch.push(slot.series);
        cursor = start + i64::from(length);
    }

    let mut moved: Vec<(i64, Proposed)> = placed
        .iter()
        .filter_map(|(series, &(to_start, to_mins))| {
            let (from_start, from_mins) = (originals[series].start, originals[series].length);
            (to_start != from_start || to_mins != from_mins).then_some((
                to_start,
                Proposed::Block { series: *series, from_start, to_start, from_mins, to_mins },
            ))
        })
        .collect();
    moved.sort_by_key(|(start, _)| *start);
    proposal.changes.extend(moved.into_iter().map(|(_, change)| change));
    Ok(proposal)
}
