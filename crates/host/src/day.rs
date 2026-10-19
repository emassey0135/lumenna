//! Changing the day as it goes: ending the block under way now or giving it longer, ending
//! the day or clearing the rest of it, and carrying the rest of a sitting forward. Each is the
//! person's own decision, so none is a proposal; each is one change, which one undo takes back.


use jiff::{SignedDuration, Timestamp, Zoned};
use lumenna_core::edit::{self, Edit};
use lumenna_core::id::SeriesId;
use lumenna_core::model::ExceptionAction;
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::types::Change;
use crate::words::count_line;
use crate::{Lumenna, repaired, resolve};

pub use lumenna_records::day::{EXTEND_MINS, carries, day_actions};




/// The edit that makes one day of a block `minutes` long: an exception for a repeating one,
/// keeping what the day already changed; the block itself for one that happens once.
fn length_edit(snapshot: &Snapshot, series: SeriesId, day: jiff::civil::Date, minutes: u32) -> Result<Edit> {
    let block = snapshot.series.get(&series).ok_or_else(|| LumennaError::new("that block is not here"))?;
    if !block.is_recurring() {
        let mut after = block.clone();
        after.duration_mins = minutes;
        return Ok(edit::update_series(block.clone(), after));
    }
    let action = match snapshot.exceptions.get(&(series, day)).map(|e| &e.action) {
        Some(ExceptionAction::Modified { start_time, title, kind, flags, .. }) => ExceptionAction::Modified {
            start_time: *start_time,
            duration_mins: Some(minutes),
            title: title.clone(),
            kind: *kind,
            flags: *flags,
        },
        _ => ExceptionAction::Modified { start_time: None, duration_mins: Some(minutes), title: None, kind: None, flags: None },
    };
    Ok(edit::except_occurrence(snapshot, series, day, action)?)
}

/// When a day's occurrence of `series` started, and how long it lasts.
fn occurrence_on(snapshot: &Snapshot, series: SeriesId, day: jiff::civil::Date) -> Result<(Timestamp, u32)> {
    let occurrence = snapshot
        .day(day)?
        .into_iter()
        .find(|o| o.series_id == series)
        .ok_or_else(|| LumennaError::new("that block does not happen today"))?;
    let zone = snapshot
        .series
        .get(&series)
        .and_then(|s| s.timezone.as_ref())
        .and_then(|z| z.get().ok())
        .unwrap_or_else(jiff::tz::TimeZone::system);
    Ok((lumenna_core::remind::instant(occurrence.date, occurrence.start_time, &zone), occurrence.duration_mins))
}

fn merged(description: String, edits: Vec<Edit>) -> Edit {
    Edit { description, changes: edits.into_iter().flat_map(|e| e.changes).collect() }
}

impl Lumenna {
    /// Ends the block under way now (`<series>@<date>`): that day of it is as long as it has
    /// run, a minute at least.
    ///
    /// # Errors
    ///
    /// If the block is not under way.
    pub fn end_block_now(&self, id: &str) -> Result<Change> {
        let now = Timestamp::now();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let series = resolve::series_id(&snapshot, id)?;
            let day = Zoned::now().date();
            let (start, length) = occurrence_on(&snapshot, series, day)?;
            let ran = u32::try_from(now.duration_since(start).as_mins().max(1)).unwrap_or(1);
            if ran >= length {
                return Ok(Change::unchanged("it has already ended"));
            }
            let change = length_edit(&snapshot, series, day, ran)?;
            let title = snapshot.series.get(&series).map(|s| s.title.clone()).unwrap_or_default();
            let change = Edit { description: format!("Ended {title}"), ..change };
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }

    /// Gives the block under way `minutes` more on this day alone.
    ///
    /// # Errors
    ///
    /// If the block does not happen today.
    pub fn extend_block(&self, id: &str, minutes: u32) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let series = resolve::series_id(&snapshot, id)?;
            let day = Zoned::now().date();
            let (_, length) = occurrence_on(&snapshot, series, day)?;
            let change = length_edit(&snapshot, series, day, length + minutes)?;
            let title = snapshot.series.get(&series).map(|s| s.title.clone()).unwrap_or_default();
            let change = Edit { description: format!("Gave {title} {} more", crate::words::duration(minutes)), ..change };
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }

    /// Clears what is left of today: each block still to come is cancelled for today if it
    /// repeats, or goes to the trash if it happens once. With `end_current`, the block under
    /// way ends now too. One change, which one undo takes back.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn end_day(&self, end_current: bool) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let day = now.date();
            let mut edits = Vec::new();
            let mut ended = None;
            let mut cleared = 0;
            for occurrence in snapshot.day(day)? {
                let Some(series) = snapshot.series.get(&occurrence.series_id) else { continue };
                let (start, length) = occurrence_on(&snapshot, series.id, day)?;
                let end = start.checked_add(SignedDuration::from_mins(i64::from(length))).unwrap_or(start);
                if start > now.timestamp() {
                    edits.push(if series.is_recurring() {
                        edit::except_occurrence(&snapshot, series.id, day, ExceptionAction::Cancelled)?
                    } else {
                        edit::trash_series(&snapshot, series.id)?
                    });
                    cleared += 1;
                } else if end > now.timestamp() && end_current {
                    let ran = u32::try_from(now.timestamp().duration_since(start).as_mins().max(1)).unwrap_or(1);
                    edits.push(length_edit(&snapshot, series.id, day, ran)?);
                    ended = Some(series.title.clone());
                }
            }
            let rest = match cleared {
                0 => String::new(),
                n => format!("cleared {}", count_line(n, "block")),
            };
            let description = match (ended, rest.is_empty()) {
                (None, true) => return Ok(Change::unchanged("nothing is left of the day")),
                (Some(title), true) => format!("Ended {title}"),
                (Some(title), false) => format!("Ended {title} and {rest}"),
                (None, false) => crate::words::capitalised(&rest),
            };
            let change = merged(description, edits);
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }

    /// Puts the rest of a sitting's planned time in `block` (`<series>@<date>`), the old
    /// sitting marked as moved on. One change.
    ///
    /// # Errors
    ///
    /// If either cannot be found, or nothing is left to carry.
    pub fn carry_forward(&self, assignment: &str, block: &str) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let sitting = snapshot.assignments.get(&id).ok_or_else(|| LumennaError::new("that sitting is not here"))?.clone();
            let left = sitting.planned_mins.unwrap_or(0).saturating_sub(sitting.accumulated_mins);
            if left == 0 {
                return Err(LumennaError::new("nothing is left of it to carry"));
            }
            let year = lumenna_store::doc::assignment_year(&sitting, snapshot.series.get(&sitting.block_ref.series_id()))
                .ok_or_else(|| LumennaError::new("that sitting's block is not in this store"))?;
            let series_id = resolve::series_id(&snapshot, block)?;
            let series = snapshot.series.get(&series_id).ok_or_else(|| LumennaError::new("that block is not here"))?;
            let day: jiff::civil::Date = block
                .split_once('@')
                .and_then(|(_, d)| d.parse().ok())
                .unwrap_or(series.start_date);
            let (target, target_year) = if series.is_recurring() {
                (lumenna_core::model::BlockRef::Occurrence(series_id, day), day.year())
            } else {
                (lumenna_core::model::BlockRef::OneOff(series_id), series.start_date.year())
            };
            let mut assign = edit::assign_task(&snapshot, sitting.task_id, target, target_year)?;
            if let Some(edit::Change::Assignment { transition, .. }) = assign.changes.first_mut()
                && let Some(new) = transition.after.as_mut()
            {
                new.planned_mins = Some(left);
            }
            let title = snapshot.tasks.get(&sitting.task_id).map(|t| t.title.clone()).unwrap_or_default();
            let change = merged(
                format!("Carried the remaining {} of {title} to {}", crate::words::duration(left), series.title),
                vec![assign, edit::defer_sitting(&snapshot, id, year)?],
            );
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }
}
