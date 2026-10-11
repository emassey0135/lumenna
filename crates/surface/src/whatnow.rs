//! "What should I work on?", asked of the day: in a block under way, its own unfinished
//! sittings first, then what its suggestions rank first; in free time, what is suggested for
//! the time until the next block. Choosing one starts on it at once: its timer, and for a
//! task not yet in a block, a sitting in the block under way or a block of its own in the
//! free time. One change, which one undo takes back.

use jiff::Zoned;
use lumenna_core::dayplan::{SHORTEST_SITTING, UNESTIMATED_MINS};
use lumenna_core::edit::{self, Edit};
use lumenna_core::model::{AssignmentStatus, BlockKind, BlockRef, BlockSeries};
use lumenna_core::recur::Occurrence;
use lumenna_core::snapshot::Snapshot;

use crate::actions::{Choice, Choices};
use crate::error::{LumennaError, Result};
use crate::replan::minutes_of;
use crate::types::Change;
use crate::words::{count_line, duration, suggested_because};
use crate::{Lumenna, repaired, resolve};

/// Where now falls in today: in a block, or in the free time before the next one.
enum Now {
    /// A block under way, and the minutes left of it.
    Block(Occurrence, u32),
    /// Free time: its minutes until the next block or the day's end, and whether a block is next.
    Free(u32, bool),
}

fn where_now(snapshot: &Snapshot, now: &Zoned) -> Result<Now> {
    let at = minutes_of(now.time());
    let mut day = snapshot.day(now.date())?;
    day.sort_by_key(|o| o.start_time);
    if let Some(current) = day.iter().find(|o| {
        let start = minutes_of(o.start_time);
        start <= at && at < start + i64::from(o.duration_mins)
    }) {
        let left = minutes_of(current.start_time) + i64::from(current.duration_mins) - at;
        return Ok(Now::Block(current.clone(), u32::try_from(left).unwrap_or(0)));
    }
    let next = day.iter().map(|o| minutes_of(o.start_time)).filter(|start| *start > at).min();
    // With nothing after, free time runs to the day's end, or past it to midnight.
    let window = minutes_of(snapshot.settings.day_window.1);
    let end = next.unwrap_or(if at < window { window } else { 24 * 60 });
    Ok(Now::Free(u32::try_from(end - at).unwrap_or(0), next.is_some()))
}

fn reference(snapshot: &Snapshot, occurrence: &Occurrence) -> BlockRef {
    match snapshot.series.get(&occurrence.series_id) {
        Some(series) if series.is_recurring() => BlockRef::Occurrence(occurrence.series_id, occurrence.date),
        _ => BlockRef::OneOff(occurrence.series_id),
    }
}

/// Whether today offers the question now: not inside a block that takes no tasks.
pub(crate) fn offered(snapshot: &Snapshot, now: &Zoned) -> bool {
    match where_now(snapshot, now) {
        Ok(Now::Block(occurrence, _)) => occurrence.flags.accepts_tasks,
        Ok(Now::Free(minutes, _)) => minutes >= SHORTEST_SITTING,
        Err(_) => false,
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// What to work on now, best first, as the day's What Should I Work On? offers it. `now`
    /// is for tests, a local date and time.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn what_should_i_work_on(&self, now: Option<String>) -> Result<Choices> {
        self.what_now_choices(&crate::replan::now_or(now.as_deref())?)
    }

    /// Starts on one of [`Lumenna::what_should_i_work_on`]'s choices now.
    ///
    /// # Errors
    ///
    /// If it is gone.
    pub fn start_on(&self, choice: &str, now: Option<String>) -> Result<Change> {
        self.work_on(choice, &crate::replan::now_or(now.as_deref())?)
    }
}

impl Lumenna {
    /// What to work on now, best first: `sitting:<id>` for a sitting already in the block under
    /// way, `task:<id>` for anything else.
    pub(crate) fn what_now_choices(&self, now: &Zoned) -> Result<Choices> {
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let mut offered = Vec::new();
            let (filter, block, left, none) = match where_now(&snapshot, now)? {
                Now::Block(occurrence, left) => {
                    let series = snapshot.series.get(&occurrence.series_id);
                    let title = series.map(|s| s.title.clone()).unwrap_or_default();
                    if !occurrence.flags.accepts_tasks {
                        return Ok(Choices {
                            announcement: format!("{title} is under way, and takes no tasks."),
                            notices: Vec::new(),
                            choices: Vec::new(),
                        });
                    }
                    let block = reference(&snapshot, &occurrence);
                    let facts = snapshot.facts();
                    // What was planned for this block comes first: the plan already made.
                    let mut sittings: Vec<_> = snapshot
                        .assignments
                        .values()
                        .filter(|a| a.block_ref == block && matches!(a.status, AssignmentStatus::Planned | AssignmentStatus::InProgress))
                        .collect();
                    sittings.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
                    for sitting in sittings {
                        let Some(task) = snapshot.tasks.get(&sitting.task_id) else { continue };
                        if task.is_deleted() || facts.is_completed(task) {
                            continue;
                        }
                        let progress = match sitting.planned_mins {
                            Some(planned) if sitting.accumulated_mins == 0 => {
                                format!("Planned for {title}, {}.", duration(planned))
                            }
                            Some(planned) => format!(
                                "Planned for {title}: {} of {} done.",
                                duration(sitting.accumulated_mins),
                                duration(planned)
                            ),
                            None => format!("Planned for {title}."),
                        };
                        offered.push(Choice {
                            id: format!("sitting:{}", sitting.id),
                            title: task.title.clone(),
                            detail: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
                            reason: Some(progress),
                            ..Choice::default()
                        });
                    }
                    let filter = match series.and_then(|s| s.task_filter.as_deref()).map(str::trim) {
                        None | Some("") => None,
                        Some(text) => Some(resolve::query(&snapshot, text)?),
                    };
                    (filter, block, left, format!("Nothing open fits {title}."))
                }
                Now::Free(left, block_next) => {
                    // A block of its own is made for the free time, so the suggestions are
                    // asked of a block that does not exist yet: any task, the time until the next.
                    let none = if left < SHORTEST_SITTING {
                        let minutes = format!("{} only {}", if left == 1 { "is" } else { "are" }, count_line(left as usize, "minute"));
                        if block_next {
                            format!("There {minutes} before the next block.")
                        } else {
                            format!("There {minutes} left of the day.")
                        }
                    } else {
                        "There is no open task to work on.".to_owned()
                    };
                    if left < SHORTEST_SITTING {
                        return Ok(Choices { announcement: none, notices: Vec::new(), choices: Vec::new() });
                    }
                    (None, BlockRef::OneOff(lumenna_core::id::SeriesId::new()), left, none)
                }
            };
            // The block's own sittings are not among its suggestions, so nothing is offered twice.
            for suggestion in lumenna_core::suggestions::suggest(&snapshot, filter.as_ref(), block, left, now, usize::MAX) {
                if suggestion.reasons.contains(&lumenna_core::suggestions::Reason::Blocked) {
                    continue;
                }
                let Some(task) = snapshot.tasks.get(&suggestion.task) else { continue };
                offered.push(Choice {
                    id: format!("task:{}", task.id),
                    title: task.title.clone(),
                    detail: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
                    reason: suggested_because(&suggestion.reasons),
                    ..Choice::default()
                });
            }
            Ok(Choices {
                announcement: if offered.is_empty() { none } else { match offered.len() { 1 => "1 thing to work on".to_owned(), n => format!("{n} things to work on") } },
                notices: Vec::new(),
                choices: offered,
            })
        })
    }

    /// Starts on `choice` (from [`Lumenna::what_now_choices`]) now.
    pub(crate) fn work_on(&self, choice: &str, now: &Zoned) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let started = lumenna_core::time::truncate(now.timestamp());
            if let Some(sitting) = choice.strip_prefix("sitting:") {
                let id = resolve::assignment_id(&snapshot, sitting)?;
                let year = crate::planning::assignment_year(&snapshot, id)?;
                let change = edit::start_timer(&snapshot, id, year, now)?;
                if change.is_empty() {
                    return Ok(Change::unchanged("its timer is already running"));
                }
                let title = snapshot.assignments.get(&id).and_then(|a| snapshot.tasks.get(&a.task_id)).map(|t| t.title.clone());
                let change = Edit { description: format!("Started on {}", title.unwrap_or_default()), ..change };
                store.apply_recorded(&change)?;
                return Ok(Change::of(&change));
            }
            let task_id = resolve::task_id(&snapshot, choice.strip_prefix("task:").unwrap_or(choice))?;
            let task = snapshot.tasks.get(&task_id).ok_or_else(|| LumennaError::new("that task is not here"))?.clone();
            let mut edits = Vec::new();
            let mut world = snapshot.clone();
            let (block, year, block_title) = match where_now(&snapshot, now)? {
                Now::Block(occurrence, _) => {
                    let title = snapshot.series.get(&occurrence.series_id).map(|s| s.title.clone()).unwrap_or_default();
                    let year = match reference(&snapshot, &occurrence) {
                        BlockRef::Occurrence(_, date) => date.year(),
                        BlockRef::OneOff(series) => snapshot.series.get(&series).map_or(now.date().year(), |s| s.start_date.year()),
                    };
                    (reference(&snapshot, &occurrence), year, title)
                }
                Now::Free(left, _) => {
                    // A block of its own for the free time: as long as the task is meant to take,
                    // or until the next block.
                    let minutes = task.estimate_mins.unwrap_or(UNESTIMATED_MINS).min(left).max(1);
                    let at = now.time();
                    let start = jiff::civil::time(at.hour(), at.minute(), 0, 0);
                    let series = BlockSeries::one_off(&task.title, BlockKind::Work, now.date(), start, minutes)
                        .map_err(|e| LumennaError::new(e.to_string()))?;
                    let id = series.id;
                    world.series.insert(id, series.clone());
                    edits.push(edit::create_series(series));
                    (BlockRef::OneOff(id), now.date().year(), task.title.clone())
                }
            };
            let mut assign = edit::assign_task(&world, task_id, block, year)?;
            if let Some(edit::Change::Assignment { transition, .. }) = assign.changes.first_mut()
                && let Some(new) = transition.after.as_mut()
            {
                new.start(started);
            }
            edits.push(assign);
            let description = if block_title == task.title {
                format!("Started on {} in a block of its own", task.title)
            } else {
                format!("Started on {} in {block_title}", task.title)
            };
            let change = Edit { description, changes: edits.into_iter().flat_map(|e| e.changes).collect() };
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }
}
