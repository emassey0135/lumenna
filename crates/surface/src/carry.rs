//! Carrying what a block left short forward, offered at its end: "Deep work ended. Essay
//! draft: 40 minutes of 1 hour 30 minutes. Carry the remaining 50 minutes to Deep work
//! tomorrow?" Each sitting goes to the next block whose filter admits its task, and a block it
//! would overfill says so. Never carried by itself: the question is where "the estimate was
//! wrong" gets noticed.

use jiff::Zoned;
use lumenna_core::edit::{self, Edit};
use lumenna_core::filter::Context;
use lumenna_core::id::{AssignmentId, SeriesId};
use lumenna_core::model::{AssignmentStatus, BlockRef};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::types::Change;
use crate::words::{duration, listed};
use crate::{Lumenna, repaired, resolve};

/// How far ahead a sitting's next block is looked for.
const DAYS_AHEAD: i32 = 7;

/// One sitting left short, and where its rest would go.
pub(crate) struct Offer {
    pub assignment: AssignmentId,
    pub year: i16,
    pub task: lumenna_core::id::TaskId,
    pub title: String,
    pub worked: u32,
    pub planned: u32,
    /// The block, its day and title, and by how much the rest would overfill it.
    pub to: Option<(BlockRef, i16, jiff::civil::Date, String, u32)>,
}

fn reference(snapshot: &Snapshot, series: SeriesId, date: jiff::civil::Date) -> Option<(BlockRef, i16)> {
    let block = snapshot.series.get(&series)?;
    Some(if block.is_recurring() {
        (BlockRef::Occurrence(series, date), date.year())
    } else {
        (BlockRef::OneOff(series), block.start_date.year())
    })
}

/// The sittings of `series` on `date` left short with their tasks open, each with the first
/// later block within a week whose filter admits its task.
pub(crate) fn offers(snapshot: &Snapshot, series: SeriesId, date: jiff::civil::Date, now: &Zoned) -> Vec<Offer> {
    let Some((block, year)) = reference(snapshot, series, date) else { return Vec::new() };
    let facts = snapshot.facts();
    let cx = Context::new(snapshot, now);
    let ends = snapshot
        .day(date)
        .ok()
        .and_then(|day| day.into_iter().find(|o| o.series_id == series))
        .map_or((date, jiff::civil::time(23, 59, 0, 0)), |o| {
            let end = o.date.to_datetime(o.start_time) + jiff::SignedDuration::from_mins(i64::from(o.duration_mins));
            (end.date(), end.time())
        });
    let last = date.checked_add(jiff::Span::new().days(DAYS_AHEAD)).unwrap_or(date);
    let mut later: Vec<_> = snapshot
        .occurrences(&(date..=last))
        .unwrap_or_default()
        .into_iter()
        .filter(|o| o.flags.accepts_tasks && (o.date, o.start_time) >= ends && o.series_id != series)
        .filter(|o| snapshot.series.get(&o.series_id).is_some_and(|s| s.deleted_at.is_none()))
        .collect();
    later.sort_by_key(|o| (o.date, o.start_time, o.series_id));
    let mut sittings: Vec<_> = snapshot
        .assignments
        .values()
        .filter(|a| {
            a.block_ref == block
                && matches!(a.status, AssignmentStatus::Planned | AssignmentStatus::InProgress | AssignmentStatus::Worked)
                && a.planned_mins.is_some_and(|planned| a.accumulated_mins < planned)
        })
        .collect();
    sittings.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
    let mut offered = Vec::new();
    for sitting in sittings {
        let Some(task) = snapshot.tasks.get(&sitting.task_id) else { continue };
        if task.is_deleted() || facts.is_completed(task) {
            continue;
        }
        let planned = sitting.planned_mins.unwrap_or(0);
        let left = planned - sitting.accumulated_mins;
        let to = later
            .iter()
            .find(|o| {
                snapshot.series.get(&o.series_id).is_some_and(|s| match s.task_filter.as_deref().map(str::trim) {
                    None | Some("") => true,
                    Some(text) => resolve::query(snapshot, text).is_ok_and(|expr| expr.matches(task, &cx)),
                })
            })
            .and_then(|o| {
                let (target, year) = reference(snapshot, o.series_id, o.date)?;
                let room = lumenna_core::suggestions::remaining(snapshot, target, o.duration_mins);
                let title = snapshot.series.get(&o.series_id).map(|s| s.title.clone()).unwrap_or_default();
                Some((target, year, o.date, title, left.saturating_sub(room)))
            });
        offered.push(Offer {
            assignment: sitting.id,
            year,
            task: task.id,
            title: task.title.clone(),
            worked: sitting.accumulated_mins,
            planned,
            to,
        });
    }
    offered
}

/// Where a day is, said after "to Deep work": "today", "tomorrow", "on Friday".
pub(crate) fn on_day(date: jiff::civil::Date, today: jiff::civil::Date) -> String {
    let words = lumenna_core::time::day_words(date, today);
    match words.as_str() {
        "today" => "later today".to_owned(),
        "tomorrow" | "yesterday" => words,
        _ => format!("on {words}"),
    }
}

/// What is said of the offers, and whether any of them can go anywhere.
pub(crate) fn offer_words(offers: &[Offer], today: jiff::civil::Date) -> Option<String> {
    if offers.is_empty() {
        return None;
    }
    let progress = |offer: &Offer| {
        if offer.worked == 0 {
            format!("{}: none of {}", offer.title, duration(offer.planned))
        } else {
            format!("{}: {} of {}", offer.title, duration(offer.worked), duration(offer.planned))
        }
    };
    let destination = |offer: &Offer| {
        offer.to.as_ref().map(|(_, _, date, title, over)| {
            let over = if *over > 0 { format!(", which puts it {} over", duration(*over)) } else { String::new() };
            format!("{title} {}{over}", on_day(*date, today))
        })
    };
    let movable = offers.iter().filter(|o| o.to.is_some()).count();
    if let [offer] = offers {
        let left = offer.planned - offer.worked;
        return Some(match destination(offer) {
            Some(to) => format!("{}. Carry the remaining {} to {to}?", progress(offer), duration(left)),
            None => format!("{}, and no block this week takes it.", progress(offer)),
        });
    }
    let parts: Vec<String> = offers
        .iter()
        .map(|offer| match destination(offer) {
            Some(to) => format!("{}, the rest to {to}", progress(offer)),
            None => format!("{}, which no block this week takes", progress(offer)),
        })
        .collect();
    let question = match movable {
        0 => String::new(),
        1 => " Carry it forward?".to_owned(),
        _ => " Carry them forward?".to_owned(),
    };
    Some(format!("{}.{question}", listed(&parts)))
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Carries the rest of every sitting `block` (`<series>@<date>`) left short to the next
    /// block that takes its task, as its end offered: each old sitting marked as moved on.
    /// One change.
    ///
    /// # Errors
    ///
    /// If the block is gone, or nothing in it can go anywhere.
    pub fn carry_rest_forward(&self, block: &str) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let (series, date) = block.split_once('@').ok_or_else(|| LumennaError::new("name the block's day: <block>@<date>"))?;
            let series = resolve::series_id(&snapshot, series)?;
            let date: jiff::civil::Date = date.parse().map_err(|_| LumennaError::new(format!("'{date}' is not a day")))?;
            let mut edits = Vec::new();
            let mut moved = Vec::new();
            for offer in offers(&snapshot, series, date, &now) {
                let Some((target, year, _, title, _)) = offer.to else { continue };
                let mut assign = edit::assign_task(&snapshot, offer.task, target, year)?;
                if let Some(edit::Change::Assignment { transition, .. }) = assign.changes.first_mut()
                    && let Some(new) = transition.after.as_mut()
                {
                    new.planned_mins = Some(offer.planned - offer.worked);
                }
                edits.push(assign);
                edits.push(edit::defer_sitting(&snapshot, offer.assignment, offer.year)?);
                moved.push(format!("{} to {title}", offer.title));
            }
            if moved.is_empty() {
                return Err(LumennaError::new("nothing left in it can go to another block"));
            }
            let change = Edit {
                description: format!("Carried forward {}", listed(&moved)),
                changes: edits.into_iter().flat_map(|e| e.changes).collect(),
            };
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }
}
