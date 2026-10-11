//! Planning a whole day in one go: the core's greedy plan said in full, then applied as the
//! person accepts it. Always optional, and never applied by itself.

use std::collections::{BTreeMap, BTreeSet};

use lumenna_core::dayplan::propose_day;
use lumenna_core::edit::{self, Edit};
use lumenna_core::model::BlockRef;
use serde::{Deserialize, Serialize};

use crate::error::{LumennaError, Result};
use crate::replan::{TimeStyle, clock, minutes_of, now_or};
use crate::types::Change;
use crate::words::{count_line, duration, listed, suggested_because};
use crate::{Lumenna, repaired, resolve};

/// One sitting a day plan proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ProposedSitting {
    /// The sitting in a sentence: "Report goes in Morning at 9:00, for 45 minutes".
    pub sentence: String,
    /// Why this task, as suggestions say it: "Suggested because priority 1".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The task.
    pub task: String,
    /// The block, `<series>@<date>`.
    pub block: String,
    /// Its planned length.
    pub minutes: u32,
}

/// What planning a day proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct DayPlan {
    /// The whole plan in a sentence or two, said before anything changes.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// The day planned, `YYYY-MM-DD`.
    pub date: String,
    /// The sittings, in the order of the day, each to accept or not.
    pub sittings: Vec<ProposedSitting>,
    /// What is due by then and fits nowhere, by title.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unfit: Vec<String>,
}

crate::announced!(DayPlan);

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// What planning `date` (a date phrase; today when absent) proposes: each work block still
    /// to come given the tasks it is suggested, until their estimates fill it. Nothing changes
    /// until [`Lumenna::apply_day_plan`]. `exclude` names tasks not to plan that day. `now` is
    /// for tests, a local date and time.
    ///
    /// # Errors
    ///
    /// If the date or a task cannot be read.
    pub fn propose_day_plan(
        &self,
        date: Option<String>,
        style: TimeStyle,
        exclude: Vec<String>,
        now: Option<String>,
    ) -> Result<DayPlan> {
        let now = now_or(now.as_deref())?;
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let day = resolve::date(date.as_deref(), &now)?;
            let mut excluded = BTreeSet::new();
            for task in &exclude {
                excluded.insert(resolve::task_id(&snapshot, task)?);
            }
            // A filter that no longer reads admits nothing, as Put in a Block treats it.
            let mut filters = BTreeMap::new();
            let mut unreadable = BTreeSet::new();
            for series in snapshot.series.values() {
                match series.task_filter.as_deref().map(str::trim) {
                    None | Some("") => {}
                    Some(text) => match resolve::query(&snapshot, text) {
                        Ok(expr) => {
                            filters.insert(series.id, expr);
                        }
                        Err(_) => {
                            unreadable.insert(series.id);
                        }
                    },
                }
            }
            let mut proposal = propose_day(&snapshot, day, &now, &filters, &excluded)
                .map_err(|e| LumennaError::new(e.to_string()))?;
            proposal.sittings.retain(|s| !unreadable.contains(&s.block.series_id()));
            let occurrences = snapshot.day(day)?;
            let mut sittings = Vec::new();
            for planned in &proposal.sittings {
                let series = planned.block.series_id();
                let block = snapshot.series.get(&series).map(|s| s.title.clone()).unwrap_or_default();
                let start = occurrences.iter().find(|o| o.series_id == series).map(|o| minutes_of(o.start_time));
                let task = snapshot.tasks.get(&planned.task).map(|t| t.title.clone()).unwrap_or_default();
                let at = start.map_or_else(String::new, |start| format!(" at {}", clock(start, style)));
                let length = if planned.assumed {
                    format!("for {}, since it has no estimate", duration(planned.minutes))
                } else {
                    format!("for {}", duration(planned.minutes))
                };
                sittings.push(ProposedSitting {
                    sentence: format!("{task} goes in {block}{at}, {length}"),
                    reason: suggested_because(&planned.reasons),
                    task: planned.task.to_string(),
                    block: format!("{series}@{day}"),
                    minutes: planned.minutes,
                });
            }
            let unfit: Vec<String> =
                proposal.unfit.iter().filter_map(|t| snapshot.tasks.get(t).map(|t| t.title.clone())).collect();
            let mut said = Vec::new();
            if sittings.is_empty() {
                said.push("There is nothing to plan: no work block still to come has room for a task that fits it.".to_owned());
            } else {
                // Grouped by block, in the order of the day: "Report and Email in Morning".
                let mut by_block: Vec<(String, Vec<String>)> = Vec::new();
                for planned in &proposal.sittings {
                    let block = snapshot.series.get(&planned.block.series_id()).map(|s| s.title.clone()).unwrap_or_default();
                    let task = snapshot.tasks.get(&planned.task).map(|t| t.title.clone()).unwrap_or_default();
                    match by_block.last_mut() {
                        Some((last, tasks)) if *last == block => tasks.push(task),
                        _ => by_block.push((block, vec![task])),
                    }
                }
                let groups: Vec<String> =
                    by_block.iter().map(|(block, tasks)| format!("{} in {block}", listed(tasks))).collect();
                said.push(format!("Plans {}: {}.", count_line(sittings.len(), "task"), listed(&groups)));
            }
            match unfit.as_slice() {
                [] => {}
                [one] => said.push(format!("{one} is due and fits in no block.")),
                many => said.push(format!("{} are due and fit in no block.", listed(many))),
            }
            Ok(DayPlan { announcement: said.join(" "), notices: Vec::new(), date: day.to_string(), sittings, unfit })
        })
    }

    /// Puts in the sittings of a day plan the person accepted: all, or a few. Each is checked
    /// against the store as it is now: a task done, gone, or already in that block is refused,
    /// so the client asks for a new plan rather than acting on a stale one. One undoable change.
    ///
    /// # Errors
    ///
    /// If a sitting no longer applies.
    pub fn apply_day_plan(&self, sittings: Vec<ProposedSitting>) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let facts = snapshot.facts();
            let stale = |why: &str| LumennaError::new(format!("{why} since this was planned; ask for a new plan"));
            let mut edits: Vec<Edit> = Vec::new();
            for sitting in &sittings {
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
