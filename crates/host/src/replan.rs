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
use lumenna_core::id::SeriesId;
use lumenna_core::model::{BlockRef, ExceptionAction};
use lumenna_core::reflow::{Pin, Proposed, propose_reflow};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::types::Change;
use crate::words::duration;
use crate::{Lumenna, repaired, resolve};
pub use lumenna_records::proposals::{Replan, ReplanAlternative, ReplanChange, ReplanFloor, ReplanPins, Replanned, TimeStyle};

/// At most this many other options: more is a menu of strategies, not a choice.
const ALTERNATIVES: usize = 3;

/// The least an option lets a work block shorten to: less is not worth sitting down for.
const WORK_FLOOR: u32 = 15;

/// The least an option lets a break shorten to.
const BREAK_FLOOR: u32 = 10;

pub(crate) fn clock(minutes: i64, style: TimeStyle) -> String {
    let (h, m) = (minutes.div_euclid(60).rem_euclid(24), minutes.rem_euclid(60));
    match style {
        TimeStyle::TwentyFourHour => format!("{h:02}:{m:02}"),
        TimeStyle::TwelveHour => {
            let (twelve, half) = match h {
                0 => (12, "AM"),
                1..=11 => (h, "AM"),
                12 => (12, "PM"),
                _ => (h - 12, "PM"),
            };
            format!("{twelve}:{m:02} {half}")
        }
    }
}


fn title(snapshot: &Snapshot, series: SeriesId) -> String {
    snapshot.series.get(&series).map(|s| s.title.clone()).unwrap_or_default()
}

pub(crate) fn minutes_of(time: jiff::civil::Time) -> i64 {
    i64::from(time.hour()) * 60 + i64::from(time.minute())
}

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

/// How far a proposal moves and shrinks the day, in minutes: what an option is weighed by.
fn upheaval(changes: &[ReplanChange]) -> i64 {
    changes
        .iter()
        .map(|c| (c.to_start - c.from_start).abs() + i64::from(c.from_mins.saturating_sub(c.to_mins)))
        .sum()
}

fn joined(sentences: &[String]) -> String {
    match sentences {
        [] => String::new(),
        [one] => format!("{one}."),
        many => format!("{}.", many.join("; ")),
    }
}

impl Lumenna {
    /// Where a sitting that leaves a block goes: the first block of the week from `day` still
    /// to come whose filter admits its task, other than the one it leaves.
    fn destination_for(&self, task: &str, leaving: SeriesId, day: jiff::civil::Date) -> Result<Option<crate::actions::Choice>> {
        let offered = self.blocks_for(Some(task), Some(day.to_string()), Some(7))?.choices;
        Ok(offered.into_iter().find(|c| !c.id.starts_with(&leaving.to_string())))
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
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
        self.propose(style, pins, &now_or(now.as_deref())?, true)
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

impl Lumenna {
    /// The proposal for `pins`, with other options worked out when `alternatives`.
    fn propose(&self, style: TimeStyle, pins: ReplanPins, now: &Zoned, alternatives: bool) -> Result<Replan> {
        let ReplanPins { keep, skip, sittings: kept_sittings, floors } = pins.clone();
        let now = now.clone();
        let (snapshot, proposal, window_end, starts) = self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let mut pins = Vec::new();
            for id in &keep {
                pins.push(Pin::Keep(resolve::series_id(&snapshot, id)?));
            }
            for id in &skip {
                pins.push(Pin::Skip(resolve::series_id(&snapshot, id)?));
            }
            for id in &kept_sittings {
                pins.push(Pin::KeepSitting(resolve::assignment_id(&snapshot, id)?));
            }
            for floor in &floors {
                pins.push(Pin::Floor(resolve::series_id(&snapshot, &floor.block)?, floor.minutes));
            }
            let day = snapshot.day(now.date())?;
            // The day ends at the window's end; once the person has planned past it, at midnight,
            // or later still if a block already runs past that.
            let window = minutes_of(snapshot.settings.day_window.1);
            let last = day.iter().map(|o| minutes_of(o.start_time) + i64::from(o.duration_mins)).max().unwrap_or(0);
            let window_end = if last > window { last.max(24 * 60) } else { window };
            let proposal = propose_reflow(&snapshot, now.date(), minutes_of(now.time()), window_end, &pins)
                .map_err(|e| LumennaError::new(e.to_string()))?;
            let starts: std::collections::BTreeMap<SeriesId, i64> =
                day.iter().map(|o| (o.series_id, minutes_of(o.start_time))).collect();
            Ok((snapshot, proposal, window_end, starts))
        })?;
        let mut changes = Vec::new();
        for change in &proposal.changes {
            match change {
                Proposed::Block { series, from_start, to_start, from_mins, to_mins } => {
                    let name = title(&snapshot, *series);
                    let moves = from_start != to_start;
                    let shortens = from_mins != to_mins;
                    let sentence = match (moves, shortens) {
                        (true, true) => format!(
                            "{name} moves from {} to {} and shortens from {} to {}",
                            clock(*from_start, style),
                            clock(*to_start, style),
                            duration(*from_mins),
                            duration(*to_mins)
                        ),
                        (true, false) => format!("{name} moves from {} to {}", clock(*from_start, style), clock(*to_start, style)),
                        _ => format!("{name} shortens from {} to {}", duration(*from_mins), duration(*to_mins)),
                    };
                    changes.push(ReplanChange {
                        sentence,
                        kind: "block".to_owned(),
                        target: series.to_string(),
                        from_start: *from_start,
                        to_start: *to_start,
                        from_mins: *from_mins,
                        to_mins: *to_mins,
                        destination: String::new(),
                    });
                }
                Proposed::Skip { series } => changes.push(ReplanChange {
                    sentence: format!("{} is skipped today", title(&snapshot, *series)),
                    kind: "skip".to_owned(),
                    target: series.to_string(),
                    from_start: 0,
                    to_start: 0,
                    from_mins: 0,
                    to_mins: 0,
                    destination: String::new(),
                }),
                Proposed::MoveSitting { assignment, task, from } => {
                    let name = snapshot.tasks.get(task).map(|t| t.title.clone()).unwrap_or_default();
                    let destination = self.destination_for(&task.to_string(), *from, now.date())?;
                    let sentence = match &destination {
                        Some(to) => {
                            let day = to
                                .date
                                .as_deref()
                                .and_then(|d| d.parse().ok())
                                .map(|d| crate::carry::on_day(d, now.date()))
                                .unwrap_or_default();
                            let at = to.start.as_deref().and_then(|t| t.parse::<jiff::civil::Time>().ok()).map(minutes_of);
                            let at = at.map_or_else(String::new, |at| format!(" at {}", clock(at, style)));
                            format!("{name} does not fit in {} and moves to {} {day}{at}", title(&snapshot, *from), to.title)
                        }
                        None => format!(
                            "{name} does not fit in {} and goes back to the list, since no block this week takes it",
                            title(&snapshot, *from)
                        ),
                    };
                    changes.push(ReplanChange {
                        sentence,
                        kind: "sitting".to_owned(),
                        target: assignment.to_string(),
                        from_start: 0,
                        to_start: 0,
                        from_mins: 0,
                        to_mins: 0,
                        destination: destination.map(|d| d.id).unwrap_or_default(),
                    });
                }
            }
        }
        let unplaceable: Vec<String> = proposal
            .unplaceable
            .iter()
            .map(|series| {
                let name = title(&snapshot, *series);
                // An anchored or kept block cannot move, so only what is under way is in its way.
                let fixed = snapshot.series.get(series).is_some_and(|b| b.flags.anchored)
                    || keep.iter().any(|k| series.to_string().starts_with(k.as_str()));
                match starts.get(series) {
                    Some(start) if fixed => format!(
                        "what is under way runs into {name} at {}: stop now to make it, or miss it",
                        clock(*start, style)
                    ),
                    _ if window_end >= 24 * 60 => format!("{name} does not fit before midnight"),
                    _ => format!("{name} does not fit before the day ends at {}", clock(window_end, style)),
                }
            })
            .collect();
        let mut said: Vec<String> = changes.iter().map(|c| c.sentence.clone()).collect();
        said.extend(unplaceable.iter().cloned());
        let announcement = if said.is_empty() {
            "Nothing needs to change: the rest of the day still fits.".to_owned()
        } else {
            crate::words::capitalised(&joined(&said))
        };
        let alternatives = if alternatives && !(changes.is_empty() && unplaceable.is_empty()) {
            self.alternatives(&snapshot, style, &pins, &now, (unplaceable.len(), upheaval(&changes)))?
        } else {
            Vec::new()
        };
        Ok(Replan { announcement, notices: Vec::new(), changes, unplaceable, pins, style, alternatives })
    }

    /// Letting each block not yet over shorten below its minimum, or skipping it, where that
    /// does better than `score` (what does not fit, then how far things move and shrink), the
    /// cheaper first: what it costs, and what follows.
    #[allow(clippy::too_many_lines)]
    fn alternatives(
        &self,
        snapshot: &Snapshot,
        style: TimeStyle,
        pins: &ReplanPins,
        now: &Zoned,
        score: (usize, i64),
    ) -> Result<Vec<ReplanAlternative>> {
        let at = minutes_of(now.time());
        let mut day = snapshot.day(now.date())?;
        day.sort_by_key(|o| (o.start_time, o.series_id));
        // Every block not over yet but the one under way, which re-flow keeps as it is: one due
        // to start during it, and run over, is a choice as much as one still to come.
        let ends = |o: &lumenna_core::recur::Occurrence| minutes_of(o.start_time) + i64::from(o.duration_mins);
        let current = day.iter().find(|o| minutes_of(o.start_time) <= at && at < ends(o)).map(|o| o.series_id);
        let later: Vec<_> = day
            .into_iter()
            .filter(|o| ends(o) > at && Some(o.series_id) != current)
            .filter(|o| !pins.skip.iter().any(|s| o.series_id.to_string().starts_with(s.as_str())))
            .collect();
        let facts = snapshot.facts();
        let mut found = Vec::new();
        for occurrence in &later {
            let mut tried = pins.clone();
            tried.skip.push(occurrence.series_id.to_string());
            let option = self.propose(style, tried.clone(), now, false)?;
            let own = |c: &ReplanChange| c.kind == "skip" && c.target == occurrence.series_id.to_string();
            let rest: Vec<String> = option.changes.iter().filter(|c| !own(c)).map(|c| c.sentence.clone()).chain(option.unplaceable.iter().cloned()).collect();
            let others: Vec<ReplanChange> = option.changes.iter().filter(|c| !own(c)).cloned().collect();
            let better = (option.unplaceable.len(), upheaval(&others));
            if better >= score {
                continue;
            }
            let name = title(snapshot, occurrence.series_id);
            let block = snapshot.series.get(&occurrence.series_id);
            let reference = match block {
                Some(series) if series.is_recurring() => lumenna_core::model::BlockRef::Occurrence(occurrence.series_id, occurrence.date),
                _ => lumenna_core::model::BlockRef::OneOff(occurrence.series_id),
            };
            let planned = snapshot
                .assignments
                .values()
                .filter(|a| a.block_ref == reference && snapshot.tasks.get(&a.task_id).is_some_and(|t| !t.is_deleted() && !facts.is_completed(t)))
                .count();
            let breaks = later.iter().filter(|o| o.kind == lumenna_core::model::BlockKind::Break).count();
            // What it costs, said so the choice is the person's with the price in view.
            let cost = if occurrence.flags.anchored {
                ", which is anchored: this means missing it".to_owned()
            } else if occurrence.kind == lumenna_core::model::BlockKind::Break {
                if breaks == 1 { ", your only break left today".to_owned() } else { format!(", one of {breaks} breaks left today") }
            } else if planned > 0 {
                format!(", and the {} planned in it", crate::words::count_line(planned, "sitting"))
            } else {
                String::new()
            };
            let then = if rest.is_empty() { "nothing else needs to change".to_owned() } else { joined(&rest).trim_end_matches('.').to_owned() };
            found.push((better, 1, ReplanAlternative { sentence: format!("Skip {name} today{cost}. Then {then}."), pins: tried }));
        }
        // Shortening below a block's minimum: the minimum binds the main proposal, never an
        // option, which says the price instead. Costs less than skipping, so it comes first.
        for occurrence in later.iter().filter(|o| !o.flags.anchored && o.kind != lumenna_core::model::BlockKind::Event) {
            let least = if occurrence.kind == lumenna_core::model::BlockKind::Break {
                (occurrence.duration_mins / 2).max(BREAK_FLOOR)
            } else {
                WORK_FLOOR
            };
            if least >= occurrence.min_duration_mins || pins.floors.iter().any(|f| occurrence.series_id.to_string().starts_with(f.block.as_str())) {
                continue;
            }
            let mut tried = pins.clone();
            tried.floors.push(ReplanFloor { block: occurrence.series_id.to_string(), minutes: least });
            let option = self.propose(style, tried.clone(), now, false)?;
            let better = (option.unplaceable.len(), upheaval(&option.changes));
            if better >= score {
                continue;
            }
            // How short it would be, as the option proposes it: only below its minimum counts.
            let Some(to) = option
                .changes
                .iter()
                .find(|c| c.kind == "block" && c.target == occurrence.series_id.to_string())
                .map(|c| c.to_mins)
                .filter(|to| *to < occurrence.min_duration_mins)
            else {
                continue;
            };
            let name = title(snapshot, occurrence.series_id);
            let shorten = if occurrence.kind == lumenna_core::model::BlockKind::Break {
                format!(
                    "Let {name} shorten from {} to {}: {} less break",
                    duration(occurrence.duration_mins),
                    duration(to),
                    duration(occurrence.duration_mins - to)
                )
            } else {
                format!("Let {name} shorten to {}, under its minimum of {}", duration(to), duration(occurrence.min_duration_mins))
            };
            let rest: Vec<String> = option
                .changes
                .iter()
                .filter(|c| c.target != occurrence.series_id.to_string())
                .map(|c| c.sentence.clone())
                .chain(option.unplaceable.iter().cloned())
                .collect();
            let then = if rest.is_empty() { "nothing else needs to change".to_owned() } else { joined(&rest).trim_end_matches('.').to_owned() };
            found.push((better, 0, ReplanAlternative {
                sentence: format!("{shorten}. Then {then}."),
                pins: tried,
            }));
        }
        found.sort_by_key(|((unfit, moved), dearer, _)| (*unfit, *dearer, *moved));
        Ok(found.into_iter().take(ALTERNATIVES).map(|(_, _, alternative)| alternative).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_said_in_the_clock_asked_for() {
        assert_eq!(clock(9 * 60 + 45, TimeStyle::TwelveHour), "9:45 AM");
        assert_eq!(clock(0, TimeStyle::TwelveHour), "12:00 AM");
        assert_eq!(clock(13 * 60 + 5, TimeStyle::TwelveHour), "1:05 PM");
        assert_eq!(clock(13 * 60 + 5, TimeStyle::TwentyFourHour), "13:05");
    }
}
