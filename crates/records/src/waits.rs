//! Planning against a dependency: a task put in a block before the task it waits for is done,
//! or is planned to be. Allowed, since a plan is only a plan; said, since a list does not show
//! the order the way a timeline does.

use jiff::civil::{Date, DateTime};
use lumenna_core::model::{AssignmentStatus, BlockRef, Task};
use lumenna_core::snapshot::Snapshot;

/// When a block's day starts it: the occurrence's own start on its day.
fn starts(snapshot: &Snapshot, block: BlockRef) -> Option<DateTime> {
    let series = snapshot.series.get(&block.series_id())?;
    let date = snapshot.block_date(block)?;
    let occurrence = snapshot.day(date).ok()?.into_iter().find(|o| o.series_id == series.id)?;
    Some(date.to_datetime(occurrence.start_time))
}

/// What to say of putting `task` in `block`: for each task it waits for that is not done and
/// not in a block before this one, which block holds it, or that none does. Nothing when the
/// order is sound.
#[must_use]
pub fn said(snapshot: &Snapshot, task: &Task, block: BlockRef, today: Date) -> Vec<String> {
    let facts = snapshot.facts();
    let Some(here) = starts(snapshot, block) else { return Vec::new() };
    task.depends
        .iter()
        .filter_map(|id| snapshot.tasks.get(id))
        .filter(|before| before.deleted_at.is_none() && !facts.is_completed(before))
        .filter_map(|before| {
            // Only a sitting still to happen plans it: one skipped, moved on or worked, or on a
            // day gone, says nothing of when it will be done.
            let mut sittings: Vec<(DateTime, BlockRef)> = facts
                .assignments_of(before.id)
                .iter()
                .filter(|a| matches!(a.status, AssignmentStatus::Planned | AssignmentStatus::InProgress))
                .filter_map(|a| Some((starts(snapshot, a.block_ref)?, a.block_ref)))
                .filter(|(at, _)| at.date() >= today)
                .collect();
            sittings.sort_by_key(|(at, _)| *at);
            match sittings.first() {
                // Planned earlier, or in the same block: the plan is in order.
                Some((at, _)) if *at <= here => None,
                Some((at, later)) => {
                    let name = snapshot.series.get(&later.series_id()).map_or("another block", |s| s.title.as_str());
                    let when = if at.date() == here.date() {
                        "later that day".to_owned()
                    } else {
                        crate::words::on_day(at.date(), today)
                    };
                    Some(format!("{} waits for {}, which is in {name} {when}, after this block.", task.title, before.title))
                }
                None => Some(format!("{} waits for {}, which is not done and in no block yet.", task.title, before.title)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumenna_core::model::{BlockAssignment, BlockKind, BlockSeries, Project};
    use lumenna_core::order::OrderKey;

    #[test]
    fn a_skipped_sitting_of_what_it_waits_for_plans_nothing() {
        let today = jiff::civil::date(2026, 10, 20);
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let first = Task::new(inbox.id, "Draft", OrderKey::middle());
        let mut then = Task::new(inbox.id, "Send", OrderKey::middle());
        then.depends.insert(first.id);
        let morning = BlockSeries::one_off("Morning", BlockKind::Work, today, jiff::civil::time(9, 0, 0, 0), 60).unwrap();
        let afternoon = BlockSeries::one_off("Afternoon", BlockKind::Work, today, jiff::civil::time(14, 0, 0, 0), 60).unwrap();
        let mut skipped = BlockAssignment::new(BlockRef::OneOff(morning.id), first.id, OrderKey::middle());
        skipped.status = AssignmentStatus::Skipped;
        let block = BlockRef::OneOff(afternoon.id);
        snapshot.assignments.insert(skipped.id, skipped);
        for series in [morning, afternoon] {
            snapshot.series.insert(series.id, series);
        }
        snapshot.tasks.insert(first.id, first);
        snapshot.projects.insert(inbox.id, inbox);
        assert_eq!(said(&snapshot, &then, block, today), ["Send waits for Draft, which is not done and in no block yet."]);
    }
}
