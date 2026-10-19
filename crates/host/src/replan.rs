//! Replanning the rest of today: the core's re-flow said in full, accepted whole or a change at
//! a time, and worked out again from what the person pins.
//!
//! **Every proposal is said before anything changes**, since for someone who cannot glance
//! at a timeline the description *is* the check: "Email moves from 10:30 to 10:45 and
//! shortens from 30 to 15 minutes; Review PR does not fit and moves to Deep work tomorrow at
//! 9:00." So the sentences are whole, with times in the clock the client asks for. A change
//! is applied only if the day still matches what it was proposed against; otherwise the
//! client asks again.

use jiff::Zoned;
use lumenna_core::edit::{self, Edit};
use lumenna_core::model::{BlockRef, ExceptionAction};

use crate::error::{LumennaError, Result};
use crate::types::Change;
use crate::{Lumenna, repaired, resolve};
pub(crate) use lumenna_records::plan::minutes_of;
pub use lumenna_records::proposals::{Replan, ReplanAlternative, ReplanChange, ReplanFloor, ReplanPins, Replanned, TimeStyle};








/// Now, or for tests the local time given (`2026-05-06T10:40`).
pub(crate) fn now_or(given: Option<&str>) -> Result<Zoned> {
    given.map_or_else(
        || Ok(Zoned::now()),
        |text| {
            text.parse::<jiff::civil::DateTime>()
                .and_then(|t| t.to_zoned(jiff::tz::TimeZone::system()))
                .map_err(|_| LumennaError::new(format!("'{text}' is not a local date and time")))
        },
    )
}




impl Lumenna {
    /// What replanning the rest of today proposes, said in `style`'s clock: nothing changes
    /// until [`Lumenna::apply_replan`]. `pins` are what the person decided: blocks kept or
    /// skipped, sittings kept. `now` is for tests, a local date and time; a running app
    /// leaves it out.
    ///
    /// # Errors
    ///
    /// If a pinned block cannot be found.
    pub fn replan(
        &self,
        style: TimeStyle,
        pins: ReplanPins,
        now: Option<String>,
    ) -> Result<Replan> {
        {
            let now = now_or(now.as_deref())?;
            self.with(|store| {
                store.load_all_years()?;
                lumenna_records::replan::propose(&repaired(store), style, pins, &now, true)
            })
        }
    }

    /// Makes the changes of `proposal` the person accepted, all or some, and proposes again:
    /// a change taken alone can leave the day still crowded, or crowd it another way, so the
    /// rest are not dropped but kept to (a block not moved is kept where it is, a sitting not
    /// moved stays) and the day as it now is proposed afresh. Each change is checked against
    /// the day as it is now; one that no longer matches is refused, so the client asks for a
    /// new proposal rather than acting on a stale one. One undoable change.
    ///
    /// # Errors
    ///
    /// If the day changed since the proposal was made.
    pub fn apply_replan(&self, proposal: Replan, accepted: Vec<ReplanChange>, now: Option<String>) -> Result<Replanned> {
        let mut pins = proposal.pins.clone();
        for declined in proposal.changes.iter().filter(|c| !accepted.contains(c)) {
            match declined.kind.as_str() {
                "block" => pins.keep.push(declined.target.clone()),
                "skip" => pins.skip.retain(|s| !declined.target.starts_with(s.as_str())),
                "sitting" => pins.sittings.push(declined.target.clone()),
                _ => {}
            }
        }
        let change = self.apply_changes(&accepted, &now_or(now.as_deref())?)?;
        let next = self.replan(proposal.style, pins, now)?;
        let announcement = if change.changed {
            format!("{}. {}", change.announcement.trim_end_matches('.'), next.announcement)
        } else {
            next.announcement.clone()
        };
        Ok(Replanned { announcement, notices: Vec::new(), change, next })
    }
}

impl Lumenna {
    fn apply_changes(&self, changes: &[ReplanChange], now: &Zoned) -> Result<Change> {
        let day = now.date();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let occurrences = snapshot.day(day)?;
            let stale = || LumennaError::new("the day has changed since this was proposed");
            let mut edits: Vec<Edit> = Vec::new();
            for change in changes {
                match change.kind.as_str() {
                    "block" => {
                        let series_id = resolve::series_id(&snapshot, &change.target)?;
                        let occurrence = occurrences.iter().find(|o| o.series_id == series_id).ok_or_else(stale)?;
                        if minutes_of(occurrence.start_time) != change.from_start || occurrence.duration_mins != change.from_mins {
                            return Err(stale());
                        }
                        let start = i8::try_from(change.to_start.div_euclid(60))
                            .ok()
                            .and_then(|h| jiff::civil::Time::new(h, i8::try_from(change.to_start.rem_euclid(60)).ok()?, 0, 0).ok())
                            .ok_or_else(|| LumennaError::new("a block has to start within its day"))?;
                        let series = snapshot.series.get(&series_id).ok_or_else(stale)?;
                        edits.push(if series.is_recurring() {
                            let earlier = snapshot.exceptions.get(&(series_id, day)).map(|e| e.action.clone());
                            let (title, kind, flags) = match earlier {
                                Some(ExceptionAction::Modified { title, kind, flags, .. }) => (title, kind, flags),
                                _ => (None, None, None),
                            };
                            edit::except_occurrence(
                                &snapshot,
                                series_id,
                                day,
                                ExceptionAction::Modified {
                                    start_time: Some(start),
                                    duration_mins: Some(change.to_mins),
                                    title,
                                    kind,
                                    flags,
                                },
                            )?
                        } else {
                            let mut after = series.clone();
                            after.start_time = start;
                            after.duration_mins = change.to_mins;
                            edit::update_series(series.clone(), after)
                        });
                    }
                    "skip" => {
                        let series_id = resolve::series_id(&snapshot, &change.target)?;
                        let series = snapshot.series.get(&series_id).ok_or_else(stale)?;
                        edits.push(if series.is_recurring() {
                            edit::except_occurrence(&snapshot, series_id, day, ExceptionAction::Cancelled)?
                        } else {
                            edit::trash_series(&snapshot, series_id)?
                        });
                    }
                    "sitting" => {
                        let id = resolve::assignment_id(&snapshot, &change.target)?;
                        let sitting = snapshot.assignments.get(&id).ok_or_else(stale)?.clone();
                        let year = lumenna_store::doc::assignment_year(&sitting, snapshot.series.get(&sitting.block_ref.series_id()))
                            .ok_or_else(stale)?;
                        edits.push(edit::unassign(&snapshot, id, year)?);
                        if !change.destination.is_empty() {
                            let (series, date) = change.destination.split_once('@').ok_or_else(stale)?;
                            let series_id = resolve::series_id(&snapshot, series)?;
                            let block = snapshot.series.get(&series_id).ok_or_else(stale)?;
                            let date: jiff::civil::Date = date.parse().map_err(|_| stale())?;
                            let (target, target_year) = if block.is_recurring() {
                                (BlockRef::Occurrence(series_id, date), date.year())
                            } else {
                                (BlockRef::OneOff(series_id), block.start_date.year())
                            };
                            let mut assign = edit::assign_task(&snapshot, sitting.task_id, target, target_year)?;
                            if let Some(edit::Change::Assignment { transition, .. }) = assign.changes.first_mut()
                                && let Some(new) = transition.after.as_mut()
                            {
                                new.planned_mins = sitting.planned_mins;
                            }
                            edits.push(assign);
                        }
                    }
                    other => return Err(LumennaError::new(format!("'{other}' is not a change a replan makes"))),
                }
            }
            if edits.is_empty() {
                return Ok(Change::unchanged("nothing was changed"));
            }
            let description = match changes.len() {
                1 => format!("Replanned: {}", changes[0].sentence),
                n => format!("Replanned the rest of the day: {}", crate::words::count_line(n, "change")),
            };
            let change = Edit { description, changes: edits.into_iter().flat_map(|e| e.changes).collect() };
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }
}
