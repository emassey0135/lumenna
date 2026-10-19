//! Planning a whole day in one go: the core's greedy plan said in full, then applied as the
//! person accepts it. Always optional, and never applied by itself.


use lumenna_core::edit::{self, Edit};
use lumenna_core::model::BlockRef;

use crate::error::{LumennaError, Result};
use crate::replan::{TimeStyle, now_or};
use crate::types::Change;
use crate::words::count_line;
use crate::{Lumenna, repaired, resolve};
pub use lumenna_records::proposals::{DayPins, DayPlan, DayPlanned, ProposedSitting};


impl Lumenna {
    /// What planning `date` (a date phrase; today when absent) proposes: each work block still
    /// to come given the tasks it is suggested, until their estimates fill it. Nothing changes
    /// until [`Lumenna::apply_day_plan`]. `pins` are what the person decided: tasks left out,
    /// sittings turned down. `now` is for tests, a local date and time.
    ///
    /// # Errors
    ///
    /// If the date or a task cannot be read.
    pub fn propose_day_plan(
        &self,
        date: Option<String>,
        style: TimeStyle,
        pins: DayPins,
        now: Option<String>,
    ) -> Result<DayPlan> {
        let now = now_or(now.as_deref())?;
        self.with(|store| {
            store.load_all_years()?;
            lumenna_records::dayplan::propose(&repaired(store), date.as_deref(), style, &pins, &now)
        })
    }

    /// Puts in the sittings of `plan` the person accepted, all or some, and plans again: the
    /// time a sitting turned down leaves is offered to another task, and the task to another
    /// block, so the rest are kept to (each turned down in its block) and the day planned
    /// afresh. Each is checked against the store as it is now: a task done, gone, or already in
    /// that block is refused, so the client asks for a new plan rather than acting on a stale
    /// one. One undoable change. `now` is for tests.
    ///
    /// # Errors
    ///
    /// If a sitting no longer applies.
    pub fn apply_day_plan(&self, plan: DayPlan, accepted: Vec<ProposedSitting>, now: Option<String>) -> Result<DayPlanned> {
        let mut pins = plan.pins.clone();
        for declined in plan.sittings.iter().filter(|s| !accepted.contains(s)) {
            let series = declined.block.split('@').next().unwrap_or_default();
            pins.declined.push(format!("{}@{series}", declined.task));
        }
        let change = self.put_in_sittings(&accepted)?;
        let next = self.propose_day_plan(Some(plan.date.clone()), plan.style, pins, now)?;
        let announcement = match (change.changed, next.sittings.is_empty()) {
            (true, true) => format!("{}.", change.announcement.trim_end_matches('.')),
            (true, false) => format!("{}. Now: {}", change.announcement.trim_end_matches('.'), next.announcement),
            (false, _) => next.announcement.clone(),
        };
        Ok(DayPlanned { announcement, notices: Vec::new(), change, next })
    }
}

impl Lumenna {
    fn put_in_sittings(&self, sittings: &[ProposedSitting]) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let facts = snapshot.facts();
            let stale = |why: &str| LumennaError::new(format!("{why} since this was planned"));
            let mut edits: Vec<Edit> = Vec::new();
            for sitting in sittings {
                let task_id = resolve::task_id(&snapshot, &sitting.task)?;
                let task = snapshot.tasks.get(&task_id).ok_or_else(|| stale("a task is gone"))?;
                if task.is_deleted() || facts.is_completed(task) {
                    return Err(stale(&format!("{} was done or deleted", task.title)));
                }
                let (series, day) = sitting.block.split_once('@').ok_or_else(|| LumennaError::new("a sitting names no day"))?;
                let series_id = resolve::series_id(&snapshot, series)?;
                let block = snapshot.series.get(&series_id).ok_or_else(|| stale("a block is gone"))?;
                let day: jiff::civil::Date =
                    day.parse().map_err(|_| LumennaError::new(format!("'{day}' is not a day")))?;
                let (target, year) = if block.is_recurring() {
                    (BlockRef::Occurrence(series_id, day), day.year())
                } else {
                    (BlockRef::OneOff(series_id), block.start_date.year())
                };
                if snapshot.assignments.values().any(|a| a.block_ref == target && a.task_id == task_id) {
                    return Err(stale(&format!("{} was put in {}", task.title, block.title)));
                }
                let mut assign = edit::assign_task(&snapshot, task_id, target, year)?;
                if let Some(edit::Change::Assignment { transition, .. }) = assign.changes.first_mut()
                    && let Some(new) = transition.after.as_mut()
                {
                    new.planned_mins = Some(sitting.minutes);
                }
                edits.push(assign);
            }
            if edits.is_empty() {
                return Ok(Change::unchanged("nothing was planned"));
            }
            let description = format!("Planned {}", count_line(edits.len(), "task"));
            let change = Edit { description, changes: edits.into_iter().flat_map(|e| e.changes).collect() };
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }
}
