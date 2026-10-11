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
    /// What the person decided that this plan keeps to, handed back to apply it.
    pub pins: DayPins,
    /// The clock its sentences are in, for the plan that follows.
    pub style: TimeStyle,
}

crate::announced!(DayPlan);

/// What the person decided about a day's plan, which the next plan keeps to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct DayPins {
    /// Tasks not to plan that day.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Sittings turned down, `<task>@<series>`: that task may still go in another block.
    #[serde(default)]
    pub declined: Vec<String>,
}

/// A day plan's accepted sittings made, and what is proposed now.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct DayPlanned {
    /// What was planned, then what is proposed now, if anything.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// The change made: one undo takes it back.
    pub change: Change,
    /// The plan for the day as it is now, the sittings turned down kept to.
    pub next: DayPlan,
}

crate::announced!(DayPlanned);

#[cfg_attr(feature = "uniffi", uniffi::export)]
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
            let snapshot = repaired(store);
            let day = resolve::date(date.as_deref(), &now)?;
            let mut excluded = BTreeSet::new();
            for task in &pins.exclude {
                excluded.insert(resolve::task_id(&snapshot, task)?);
            }
            let mut declined = BTreeSet::new();
            for pair in &pins.declined {
                let (task, series) =
                    pair.split_once('@').ok_or_else(|| LumennaError::new(format!("'{pair}' is not <task>@<block>")))?;
                declined.insert((resolve::task_id(&snapshot, task)?, resolve::series_id(&snapshot, series)?));
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
            let mut proposal = propose_day(&snapshot, day, &now, &filters, &excluded, &declined)
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
            Ok(DayPlan {
                announcement: said.join(" "),
                notices: Vec::new(),
                date: day.to_string(),
                sittings,
                unfit,
                pins: pins.clone(),
                style,
            })
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
            let stale = |why: &str| LumennaError::new(format!("{why} since this was planned; ask for a new plan"));
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
