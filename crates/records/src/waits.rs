//! Planning against a dependency: a task put in a block before the task it waits for is done,
//! or is planned to be. Allowed, since a plan is only a plan; said, since a list does not show
//! the order the way a timeline does.

use jiff::civil::{Date, DateTime};
use lumenna_core::model::{BlockRef, Task};
use lumenna_core::snapshot::Snapshot;

/// When a block's day starts it: the occurrence's own start on its day.
fn starts(snapshot: &Snapshot, block: BlockRef) -> Option<DateTime> {
    let series = snapshot.series.get(&block.series_id())?;
    let date = block.date().unwrap_or(series.start_date);
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
            let mut sittings: Vec<(DateTime, BlockRef)> = snapshot
                .assignments
                .values()
                .filter(|a| a.task_id == before.id)
                .filter_map(|a| Some((starts(snapshot, a.block_ref)?, a.block_ref)))
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
