//! Planning a whole day in one go: the core's greedy plan said in full. Applying it is the
//! host's.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;
use lumenna_core::dayplan::propose_day;
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::plan::minutes_of;
use crate::proposals::{DayPins, DayPlan, ProposedSitting, TimeStyle};
use crate::replan::clock;
use crate::resolve;
use crate::words::{count_line, duration, listed, suggested_because};

/// What planning `date` (a date phrase; today when absent) proposes: each work block still to
/// come given the tasks it is suggested, until their estimates fill it, keeping to `pins`.
///
/// # Errors
///
/// If the date or a pinned task cannot be read.
pub fn propose(snapshot: &Snapshot, date: Option<&str>, style: TimeStyle, pins: &DayPins, now: &Zoned) -> Result<DayPlan> {
    let day = resolve::date(date, now, snapshot.settings.week_start)?;
    let mut excluded = BTreeSet::new();
    for task in &pins.exclude {
        excluded.insert(resolve::task_id(snapshot, task)?);
    }
    let mut declined = BTreeSet::new();
    for pair in &pins.declined {
        let (task, series) =
            pair.split_once('@').ok_or_else(|| LumennaError::new(format!("'{pair}' is not <task>@<block>")))?;
        declined.insert((resolve::task_id(snapshot, task)?, resolve::series_id(snapshot, series)?));
    }
    // A filter that no longer reads admits nothing, as Put in a Block treats it.
    let mut filters = BTreeMap::new();
    let mut unreadable = BTreeSet::new();
    for series in snapshot.series.values() {
        match series.task_filter.as_deref().map(str::trim) {
            None | Some("") => {}
            Some(text) => match resolve::query(snapshot, text) {
                Ok(expr) => {
                    filters.insert(series.id, expr);
                }
                Err(_) => {
                    unreadable.insert(series.id);
                }
            },
        }
    }
    let mut proposal = propose_day(snapshot, day, now, &filters, &excluded, &declined)
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
}
