//! What a pick offers: the projects a task can move to, the tasks it can go under or wait for,
//! the blocks it fits, the tasks a block is suggested, what to work on now. A function of the
//! snapshot, the action asking and the moment.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;
use lumenna_core::dayplan::SHORTEST_SITTING;
use lumenna_core::filter::Context;
use lumenna_core::id::TaskId;
use lumenna_core::model::{AssignmentStatus, BlockRef, Label, Project, Task};
use lumenna_core::row::RowId;
use lumenna_core::snapshot::Snapshot;

use crate::actions::{Action, ActionKind, Choice, Choices, Subject};
use crate::error::{LumennaError, Result};
use crate::plan::{Now, where_now};
use crate::resolve::{self, split_day};
use crate::words::{count_line, duration, suggested_because};

/// Every task under `root`, at any depth.
fn descendants(snapshot: &Snapshot, root: TaskId) -> BTreeSet<TaskId> {
    let mut children: BTreeMap<TaskId, Vec<TaskId>> = BTreeMap::new();
    for task in snapshot.tasks.values() {
        if let Some(parent) = task.parent_id {
            children.entry(parent).or_default().push(task.id);
        }
    }
    reach(root, |id| children.get(&id).cloned().unwrap_or_default())
}

/// Every task waiting on `root`, directly or through others: what waiting for any of them
/// would close a cycle with.
fn waiting_on(snapshot: &Snapshot, root: TaskId) -> BTreeSet<TaskId> {
    let mut waiters: BTreeMap<TaskId, Vec<TaskId>> = BTreeMap::new();
    for task in snapshot.tasks.values() {
        for on in &task.depends {
            waiters.entry(*on).or_default().push(task.id);
        }
    }
    reach(root, |id| waiters.get(&id).cloned().unwrap_or_default())
}

/// Everything reachable from `root` by `next`, without `root` itself; a visited set keeps an
/// unrepaired cycle finite.
fn reach(root: TaskId, next: impl Fn(TaskId) -> Vec<TaskId>) -> BTreeSet<TaskId> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![root];
    while let Some(id) = queue.pop() {
        for child in next(id) {
            if child != root && seen.insert(child) {
                queue.push(child);
            }
        }
    }
    seen
}

/// `choices` with its count said, or `none` when there are none.
fn listed(choices: Vec<Choice>, noun: &str, none: &str) -> Choices {
    Choices {
        announcement: if choices.is_empty() { none.to_owned() } else { crate::words::count_line(choices.len(), noun) },
        notices: Vec::new(),
        choices,
    }
}

/// Open tasks in tree order, as choices, but for those `skip` refuses.
fn task_choices(snapshot: &Snapshot, query: &str, now: &Zoned, skip: impl Fn(&Task) -> bool) -> Result<Vec<Choice>> {
    let expr = resolve::query(snapshot, query)?;
    let cx = Context::new(snapshot, now);
    Ok(snapshot
        .task_rows(&expr, &cx)
        .into_iter()
        .filter_map(|row| {
            let RowId::Task(id) = row.id else { return None };
            let task = snapshot.tasks.get(&id)?;
            (!skip(task)).then(|| Choice {
                id: id.to_string(),
                title: row.title,
                depth: row.depth,
                detail: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
                ..Choice::default()
            })
        })
        .collect())
}

fn project_choices(snapshot: &Snapshot, skip: impl Fn(&Project) -> bool) -> Vec<Choice> {
    crate::tree::in_tree_order(snapshot)
        .into_iter()
        .filter(|(project, _)| !skip(project))
        .map(|(project, depth)| Choice { id: project.name.clone(), title: project.name.clone(), depth, ..Choice::default() })
        .collect()
}

/// What `action`'s pick offers, in order, each with why where it is suggested; when nothing,
/// why, which is all a client says.
///
/// # Errors
///
/// If what it is done to is not here, or the action picks nothing.
pub fn offered(snapshot: &Snapshot, action: &Action, now: &Zoned) -> Result<Choices> {
    use ActionKind as K;
    if (action.subject, action.kind) == (Subject::Task, K::PutInBlock) {
        return blocks_for(snapshot, Some(&action.target), None, Some(7), now);
    }
    if (action.subject, action.kind) == (Subject::Reminder, K::DeliverOn) {
        let id: lumenna_core::id::ReminderId =
            action.target.parse().map_err(|_| LumennaError::new("that is not a reminder of its own"))?;
        let reminder = snapshot.reminders.get(&id).ok_or_else(|| LumennaError::new("that reminder is not here"))?;
        let choices = crate::reminders::delivery_choices(snapshot, &reminder.delivery);
        return Ok(Choices { announcement: crate::words::count_line(choices.len(), "choice"), notices: Vec::new(), choices });
    }
    if action.kind == K::Colour {
        let current = match action.subject {
            Subject::Project => resolve::project(snapshot, &action.target)?.color.clone(),
            Subject::Label => resolve::label(snapshot, &action.target)?.color.clone(),
            Subject::Filter => snapshot
                .saved_filters
                .values()
                .find(|f| f.deleted_at.is_none() && lumenna_core::suggest::same_name(&f.name, &action.target))
                .ok_or_else(|| LumennaError::new(format!("there is no saved filter called {}", action.target)))?
                .color
                .clone(),
            _ => None,
        };
        let choices = crate::colours::colour_choices(current.as_deref());
        return Ok(Choices { announcement: crate::words::count_line(choices.len(), "choice"), notices: Vec::new(), choices });
    }
    if (action.subject, action.kind) == (Subject::Day, K::WhatNow) {
        return what_now(snapshot, now);
    }
    if (action.subject, action.kind) == (Subject::Sitting, K::CarryForward) {
        let id = resolve::assignment_id(snapshot, &action.target)?;
        let task = snapshot.assignments.get(&id).map(|a| a.task_id.to_string());
        let task = task.ok_or_else(|| LumennaError::new("that sitting is not here"))?;
        return blocks_for(snapshot, Some(&task), None, Some(7), now);
    }
    {
        let task = || -> Result<&Task> {
            let id = resolve::task_id(snapshot, &action.target)?;
            snapshot.tasks.get(&id).ok_or_else(|| LumennaError::new("that task is not here"))
        };
        Ok(match (action.subject, action.kind) {
            (Subject::Task, K::MoveToProject) => {
                let task = task()?;
                listed(
                    project_choices(snapshot, |p| p.id == task.project_id || p.archived),
                    "project",
                    "There is no other project to move it to.",
                )
            }
            (Subject::Task, K::MakeSubtaskOf) => {
                let task = task()?;
                let under = descendants(snapshot, task.id);
                listed(
                    task_choices(snapshot, "", now, |t| {
                        t.id == task.id || under.contains(&t.id) || Some(t.id) == task.parent_id
                    })?,
                    "task",
                    "There is no task it could go under.",
                )
            }
            (Subject::Task, K::WaitFor) => {
                let task = task()?;
                let waiting = waiting_on(snapshot, task.id);
                listed(
                    task_choices(snapshot, "", now, |t| {
                        t.id == task.id || task.depends.contains(&t.id) || waiting.contains(&t.id)
                    })?,
                    "task",
                    "There is no task it could wait for.",
                )
            }
            (Subject::Block, K::AssignTask) => {
                let (series, date) = split_day(&action.target)?;
                let series = snapshot
                    .series
                    .get(&resolve::series_id(snapshot, series)?)
                    .ok_or_else(|| LumennaError::new("that block is not here"))?;
                let day: jiff::civil::Date =
                    date.parse().map_err(|_| LumennaError::new(format!("'{date}' is not a day")))?;
                let occurrence = if series.is_recurring() {
                    lumenna_core::model::BlockRef::Occurrence(series.id, day)
                } else {
                    lumenna_core::model::BlockRef::OneOff(series.id)
                };
                let filter = series.task_filter.clone().unwrap_or_default();
                let none = if filter.trim().is_empty() {
                    "There is no open task to assign.".to_owned()
                } else {
                    format!("No open task matches this block's filter, {filter}.")
                };
                // Best first, each with why: the block's own question, "what goes here?"
                let expr = if filter.trim().is_empty() { None } else { Some(resolve::query(snapshot, &filter)?) };
                let length = snapshot
                    .day(day)
                    .ok()
                    .and_then(|day| day.into_iter().find(|o| o.series_id == series.id).map(|o| o.duration_mins))
                    .unwrap_or(series.duration_mins);
                let offered = lumenna_core::suggestions::suggest(snapshot, expr.as_ref(), occurrence, length, now, usize::MAX)
                    .into_iter()
                    .filter_map(|suggestion| {
                        let task = snapshot.tasks.get(&suggestion.task)?;
                        Some(Choice {
                            id: task.id.to_string(),
                            title: task.title.clone(),
                            detail: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
                            reason: crate::words::suggested_because(&suggestion.reasons, snapshot.settings.verbosity == lumenna_core::model::Verbosity::Terse),
                            ..Choice::default()
                        })
                    })
                    .collect();
                listed(offered, "task", &none)
            }
            (Subject::Project, K::MoveUnder) => {
                let project = resolve::project(snapshot, &action.target)?;
                listed(
                    project_choices(snapshot, |p| {
                        p.id == project.id
                            || p.is_inbox
                            || Some(p.id) == project.parent_id
                            || snapshot.is_within(p.id, project.id)
                    }),
                    "project",
                    "There is no other project to move it under.",
                )
            }
            (Subject::Label, K::MergeInto) => {
                let label = resolve::label(snapshot, &action.target)?;
                let mut live: Vec<&Label> =
                    snapshot.labels.values().filter(|l| l.deleted_at.is_none() && l.id != label.id).collect();
                live.sort_by(|x, y| x.order.cmp_with(&x.id, &y.order, &y.id));
                listed(
                    live.into_iter()
                        .map(|l| Choice { id: l.name.clone(), title: l.name.clone(), ..Choice::default() })
                        .collect(),
                    "label",
                    "There is no other label to merge it into.",
                )
            }
            _ => return Err(LumennaError::new(format!("{} picks nothing", action.title))),
        })
    }
}

/// The work blocks of `days` days from `from` (a date phrase, today when absent) still to
/// come, and, for `task`, only those whose filter admits it: a block that takes #Work tasks is
/// not offered for a call home.
///
/// # Errors
///
/// If the day cannot be read, or `task` is not here.
pub fn blocks_for(snapshot: &Snapshot, task: Option<&str>, from: Option<&str>, days: Option<u32>, now: &Zoned) -> Result<Choices> {
    let first = resolve::date(from, now, snapshot.settings.week_start)?;
    let blocks = crate::plan::work_blocks(snapshot, first, days.unwrap_or(crate::plan::WORK_BLOCK_DAYS), now)?;
    let task = task
        .map(|task| -> Result<Task> {
            snapshot.tasks.get(&resolve::task_id(snapshot, task)?).cloned().ok_or_else(|| LumennaError::new("that task is not here"))
        })
        .transpose()?;
    let admitted: Option<BTreeSet<String>> = task.as_ref().map(|task| {
        let cx = Context::new(snapshot, now);
        snapshot
            .series
            .values()
            .filter(|series| match series.task_filter.as_deref().map(str::trim) {
                None | Some("") => true,
                // A filter that no longer reads (a label renamed away) admits nothing
                // rather than everything.
                Some(text) => resolve::query(snapshot, text).is_ok_and(|expr| expr.matches(task, &cx)),
            })
            .map(|series| series.id.to_string())
            .collect()
    });
    let open: Vec<Choice> = blocks
        .blocks
        .into_iter()
        .filter(|b| b.when != "past")
        .filter(|b| admitted.as_ref().is_none_or(|ids| ids.contains(b.id.split('@').next().unwrap_or(&b.id))))
        .map(|b| {
            // A block before the one holding what the task waits for says so, as its reason.
            let reason = task.as_ref().and_then(|task| {
                let (series, date) = b.id.split_once('@')?;
                let series = snapshot.series.get(&series.parse().ok()?)?;
                let block = if series.is_recurring() {
                    lumenna_core::model::BlockRef::Occurrence(series.id, date.parse().ok()?)
                } else {
                    lumenna_core::model::BlockRef::OneOff(series.id)
                };
                let said: Vec<String> = crate::waits::said(snapshot, task, block, now.date())
                    .into_iter()
                    .filter(|sentence| sentence.ends_with("after this block."))
                    .collect();
                (!said.is_empty()).then(|| said.join(" "))
            });
            Choice { id: b.id, title: b.title, date: Some(b.date), start: Some(b.start), end: Some(b.end), reason, ..Choice::default() }
        })
        .collect();
    let none = match (blocks.days, blocks.from == now.date().to_string()) {
        (7, true) => "There are no work blocks this week. Add one from Today.".to_owned(),
        (1, _) => format!("There are no work blocks on {}.", blocks.from),
        (days, _) => format!("There are no work blocks in the {days} days from {}.", blocks.from),
    };
    Ok(listed(open, "work block", &none))
}

/// What to work on now: in a block under way its own unfinished sittings, then its
/// suggestions; in free time, the suggestions for the time until the next block.
///
/// # Errors
///
/// If a block's filter or a day cannot be read.
pub fn what_now(snapshot: &Snapshot, now: &Zoned) -> Result<Choices> {
    let mut offered = Vec::new();
    let (filter, block, left, none) = match where_now(snapshot, now)? {
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
            let block = crate::plan::block_ref(snapshot, &occurrence);
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
                Some(text) => Some(resolve::query(snapshot, text)?),
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
    for suggestion in lumenna_core::suggestions::suggest(snapshot, filter.as_ref(), block, left, now, usize::MAX) {
        if suggestion.reasons.contains(&lumenna_core::suggestions::Reason::Blocked) {
            continue;
        }
        let Some(task) = snapshot.tasks.get(&suggestion.task) else { continue };
        offered.push(Choice {
            id: format!("task:{}", task.id),
            title: task.title.clone(),
            detail: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
            reason: suggested_because(&suggestion.reasons, snapshot.settings.verbosity == lumenna_core::model::Verbosity::Terse),
            ..Choice::default()
        });
    }
    Ok(Choices {
        announcement: if offered.is_empty() { none } else { match offered.len() { 1 => "1 thing to work on".to_owned(), n => format!("{n} things to work on") } },
        notices: Vec::new(),
        choices: offered,
    })
}
