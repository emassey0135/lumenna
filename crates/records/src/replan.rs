//! Replanning the rest of today: the core's re-flow said in full, and its other options. Making
//! what is accepted is the host's.

use jiff::Zoned;
use lumenna_core::id::SeriesId;
use lumenna_core::reflow::{Pin, Proposed, propose_reflow};
use lumenna_core::snapshot::Snapshot;

use crate::actions::Choice;
use crate::error::{LumennaError, Result};
use crate::proposals::{Replan, ReplanAlternative, ReplanChange, ReplanFloor, ReplanPins, TimeStyle};
use crate::resolve;
use crate::plan::minutes_of;
use crate::words::duration;

/// At most this many other options: more is a menu of strategies, not a choice.
const ALTERNATIVES: usize = 3;

/// The least an option lets a work block shorten to: less is not worth sitting down for.
const WORK_FLOOR: u32 = 15;

/// The least an option lets a break shorten to.
const BREAK_FLOOR: u32 = 10;

/// A time of day as said, in `style`'s clock.
pub fn clock(minutes: i64, style: TimeStyle) -> String {
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

/// Where a sitting that leaves a block goes: the first block of the week from `day` still to
/// come whose filter admits its task, other than the one it leaves.
fn destination_for(snapshot: &Snapshot, task: &str, leaving: SeriesId, day: jiff::civil::Date, now: &Zoned) -> Result<Option<Choice>> {
    let offered = crate::choices::blocks_for(snapshot, Some(task), Some(&day.to_string()), Some(7), now)?.choices;
    Ok(offered.into_iter().find(|c| !c.id.starts_with(&leaving.to_string())))
}

/// The proposal for `pins`, with other options worked out when `alternatives`.
/// What replanning the rest of today proposes, said in `style`'s clock, keeping to `pins`;
/// with its other options when `alternatives`.
///
/// # Errors
///
/// If a pinned block cannot be found.
pub fn propose(snapshot: &Snapshot, style: TimeStyle, pins: ReplanPins, now: &Zoned, alternatives: bool) -> Result<Replan> {
    let ReplanPins { keep, skip, sittings: kept_sittings, floors } = pins.clone();
    let now = now.clone();
    let (proposal, window_end, starts) = (|| -> Result<_> {
        let mut pins = Vec::new();
        for id in &keep {
            pins.push(Pin::Keep(resolve::series_id(snapshot, id)?));
        }
        for id in &skip {
            pins.push(Pin::Skip(resolve::series_id(snapshot, id)?));
        }
        for id in &kept_sittings {
            pins.push(Pin::KeepSitting(resolve::assignment_id(snapshot, id)?));
        }
        for floor in &floors {
            pins.push(Pin::Floor(resolve::series_id(snapshot, &floor.block)?, floor.minutes));
        }
        let day = snapshot.day(now.date())?;
        // The day ends at the window's end; once the person has planned past it, at midnight,
        // or later still if a block already runs past that.
        let window = minutes_of(snapshot.settings.day_window.1);
        let last = day.iter().map(|o| minutes_of(o.start_time) + i64::from(o.duration_mins)).max().unwrap_or(0);
        let window_end = if last > window { last.max(24 * 60) } else { window };
        let proposal = propose_reflow(snapshot, now.date(), minutes_of(now.time()), window_end, &pins)
            .map_err(|e| LumennaError::new(e.to_string()))?;
        let starts: std::collections::BTreeMap<SeriesId, i64> =
            day.iter().map(|o| (o.series_id, minutes_of(o.start_time))).collect();
        Ok((proposal, window_end, starts))
    })()?;
    let mut changes = Vec::new();
    for change in &proposal.changes {
        match change {
            Proposed::Block { series, from_start, to_start, from_mins, to_mins } => {
                let name = title(snapshot, *series);
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
                sentence: format!("{} is skipped today", title(snapshot, *series)),
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
                let destination = destination_for(snapshot, &task.to_string(), *from, now.date(), &now)?;
                let sentence = match &destination {
                    Some(to) => {
                        let day = to
                            .date
                            .as_deref()
                            .and_then(|d| d.parse().ok())
                            .map(|d| crate::words::on_day(d, now.date()))
                            .unwrap_or_default();
                        let at = to.start.as_deref().and_then(|t| t.parse::<jiff::civil::Time>().ok()).map(minutes_of);
                        let at = at.map_or_else(String::new, |at| format!(" at {}", clock(at, style)));
                        format!("{name} does not fit in {} and moves to {} {day}{at}", title(snapshot, *from), to.title)
                    }
                    None => format!(
                        "{name} does not fit in {} and goes back to the list, since no block this week takes it",
                        title(snapshot, *from)
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
            let name = title(snapshot, *series);
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
        other_options(snapshot, style, &pins, &now, (unplaceable.len(), upheaval(&changes)))?
    } else {
        Vec::new()
    };
    Ok(Replan { announcement, notices: Vec::new(), changes, unplaceable, pins, style, alternatives })
}

/// Letting each block not yet over shorten below its minimum, or skipping it, where that
/// does better than `score` (what does not fit, then how far things move and shrink), the
/// cheaper first: what it costs, and what follows.
#[allow(clippy::too_many_lines)]
fn other_options(
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
        let option = propose(snapshot, style, tried.clone(), now, false)?;
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
        let option = propose(snapshot, style, tried.clone(), now, false)?;
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
