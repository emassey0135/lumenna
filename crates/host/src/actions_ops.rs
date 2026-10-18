//! Running a row's action, and what a pick offers: the half of the actions that reads the
//! store. Which actions a row has, and what each asks, are `lumenna_records::actions`.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;
use lumenna_core::filter::Context;
use lumenna_core::id::TaskId;
use lumenna_core::model::{Label, Project, Task};
use lumenna_core::row::RowId;
use lumenna_core::snapshot::Snapshot;

use crate::actions::{Action, ActionKind, Answer, Choice, Choices, Question, Subject};
use crate::error::{LumennaError, Result};
use crate::types::{Change, Direction, MoveTarget};
use crate::{Lumenna, repaired, resolve};

// ---------------------------------------------------------------------------------------
// What a picker offers
// ---------------------------------------------------------------------------------------

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

fn choices(choices: Vec<Choice>, noun: &str, none: &str) -> Choices {
    Choices {
        announcement: if choices.is_empty() { none.to_owned() } else { crate::words::count_line(choices.len(), noun) },
        notices: Vec::new(),
        choices,
    }
}

/// Open tasks in tree order, as choices, but for those `skip` refuses.
fn task_choices(snapshot: &Snapshot, query: &str, skip: impl Fn(&Task) -> bool) -> Result<Vec<Choice>> {
    let now = Zoned::now();
    let expr = resolve::query(snapshot, query)?;
    let cx = Context::new(snapshot, &now);
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

fn answer_text(answer: &Answer) -> Result<String> {
    match answer {
        Answer::Text { text } => Ok(text.trim().to_owned()),
        _ => Err(LumennaError::new("this needs a line of text")),
    }
}

fn answer_pick(answer: &Answer) -> Result<(String, Option<String>)> {
    match answer {
        Answer::Picked { id, length } => Ok((id.clone(), length.clone())),
        _ => Err(LumennaError::new("this needs a choice")),
    }
}

/// An optional length: empty is none.
fn length(text: Option<&str>) -> Result<Option<u32>> {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(None),
        Some(text) => resolve::minutes(text).map(Some),
    }
}

fn split_day(target: &str) -> Result<(&str, &str)> {
    target.split_once('@').ok_or_else(|| LumennaError::new(format!("'{target}' does not name a day of a block")))
}

fn timer(timer: crate::types::Timer) -> Change {
    Change {
        announcement: timer.announcement,
        notices: timer.notices,
        changed: timer.changed,
        affected: crate::types::Affected { assignments: vec![timer.assignment], ..Default::default() },
        task: None,
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// What a [`Question::Pick`] offers for `action`, in order: never a task as its own
    /// parent or a project inside itself, never a wait that would close a cycle.
    ///
    /// # Errors
    ///
    /// If the record it acts on cannot be found, or the action picks nothing.
    pub fn choices(&self, action: Action) -> Result<Choices> {
        use ActionKind as K;
        if (action.subject, action.kind) == (Subject::Task, K::PutInBlock) {
            return self.blocks_for(Some(&action.target), None, Some(7));
        }
        if (action.subject, action.kind) == (Subject::Day, K::WhatNow) {
            return self.what_now_choices(&Zoned::now());
        }
        if (action.subject, action.kind) == (Subject::Sitting, K::CarryForward) {
            let task = self.with(|store| {
                store.load_all_years()?;
                let snapshot = repaired(store);
                let id = resolve::assignment_id(&snapshot, &action.target)?;
                Ok(snapshot.assignments.get(&id).map(|a| a.task_id.to_string()))
            })?;
            let task = task.ok_or_else(|| LumennaError::new("that sitting is not here"))?;
            return self.blocks_for(Some(&task), None, Some(7));
        }
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let task = || -> Result<&Task> {
                let id = resolve::task_id(&snapshot, &action.target)?;
                snapshot.tasks.get(&id).ok_or_else(|| LumennaError::new("that task is not here"))
            };
            Ok(match (action.subject, action.kind) {
                (Subject::Task, K::MoveToProject) => {
                    let task = task()?;
                    choices(
                        project_choices(&snapshot, |p| p.id == task.project_id || p.archived),
                        "project",
                        "There is no other project to move it to.",
                    )
                }
                (Subject::Task, K::MakeSubtaskOf) => {
                    let task = task()?;
                    let under = descendants(&snapshot, task.id);
                    choices(
                        task_choices(&snapshot, "", |t| {
                            t.id == task.id || under.contains(&t.id) || Some(t.id) == task.parent_id
                        })?,
                        "task",
                        "There is no task it could go under.",
                    )
                }
                (Subject::Task, K::WaitFor) => {
                    let task = task()?;
                    let waiting = waiting_on(&snapshot, task.id);
                    choices(
                        task_choices(&snapshot, "", |t| {
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
                        .get(&resolve::series_id(&snapshot, series)?)
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
                    let expr = if filter.trim().is_empty() { None } else { Some(resolve::query(&snapshot, &filter)?) };
                    let length = snapshot
                        .day(day)
                        .ok()
                        .and_then(|day| day.into_iter().find(|o| o.series_id == series.id).map(|o| o.duration_mins))
                        .unwrap_or(series.duration_mins);
                    let now = Zoned::now();
                    let offered = lumenna_core::suggestions::suggest(&snapshot, expr.as_ref(), occurrence, length, &now, usize::MAX)
                        .into_iter()
                        .filter_map(|suggestion| {
                            let task = snapshot.tasks.get(&suggestion.task)?;
                            Some(Choice {
                                id: task.id.to_string(),
                                title: task.title.clone(),
                                detail: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
                                reason: crate::words::suggested_because(&suggestion.reasons),
                                ..Choice::default()
                            })
                        })
                        .collect();
                    choices(offered, "task", &none)
                }
                (Subject::Project, K::MoveUnder) => {
                    let project = resolve::project(&snapshot, &action.target)?;
                    choices(
                        project_choices(&snapshot, |p| {
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
                    let label = resolve::label(&snapshot, &action.target)?;
                    let mut live: Vec<&Label> =
                        snapshot.labels.values().filter(|l| l.deleted_at.is_none() && l.id != label.id).collect();
                    live.sort_by(|x, y| x.order.cmp_with(&x.id, &y.order, &y.id));
                    choices(
                        live.into_iter()
                            .map(|l| Choice { id: l.name.clone(), title: l.name.clone(), ..Choice::default() })
                            .collect(),
                        "label",
                        "There is no other label to merge it into.",
                    )
                }
                _ => return Err(LumennaError::new(format!("{} picks nothing", action.title))),
            })
        })
    }
    /// Runs `action` with the answer to its question: [`Answer::Yes`] for one that asks
    /// nothing or a confirmation.
    ///
    /// # Errors
    ///
    /// If the answer is not the kind its question asks, the action is a client's form, or
    /// the operation it runs refuses.
    pub fn act(&self, action: Action, answer: Answer) -> Result<Change> {
        use ActionKind as K;
        use Subject as S;
        if action.question == Question::Form {
            return Err(LumennaError::new(format!("{} opens a form of the app's own", action.title)));
        }
        if matches!(action.question, Question::Confirm { .. } | Question::Immediate) && answer != Answer::Yes {
            return Ok(Change::unchanged("nothing was done"));
        }
        let target = action.target.as_str();
        match (action.subject, action.kind) {
            (S::Task, K::MarkDone) => self.complete_task(target),
            (S::Task, K::MarkNotDone) => self.uncomplete_task(target),
            (S::Task, K::Delete) => self.trash_task(target),
            (S::Task, K::Restore) => self.restore_task(target),
            (S::Task, K::DeleteForGood) => self.erase_task(target),
            (S::Task, K::MoveToTopLevel) => self.move_task(target, MoveTarget::Top),
            (S::Task, K::MoveToProject) => {
                self.move_task(target, MoveTarget::Project { name: answer_pick(&answer)?.0 })
            }
            (S::Task, K::MakeSubtaskOf) => self.move_task(target, MoveTarget::Parent { id: answer_pick(&answer)?.0 }),
            (S::Task, K::WaitFor) => self.add_dependency(target, &answer_pick(&answer)?.0),
            (S::Task, K::StopWaiting) => self.remove_dependency(target, action.other.as_deref().unwrap_or_default()),
            (S::Task, K::PutInBlock) => {
                let (block, minutes) = answer_pick(&answer)?;
                self.assign(target, &block, None, length(minutes.as_deref())?)
            }
            (S::Block, K::AssignTask) => {
                let (task, minutes) = answer_pick(&answer)?;
                self.assign(&task, target, None, length(minutes.as_deref())?)
            }
            (S::Block, K::CancelDay) => {
                let (series, date) = split_day(target)?;
                self.cancel_occurrence(series, date)
            }
            (S::Block, K::RestoreDay) => {
                let (series, date) = split_day(target)?;
                self.restore_occurrence(series, date)
            }
            (S::Block | S::Series, K::Delete) => self.delete_block(target),
            (S::Sitting, K::StartTimer | K::ResumeTimer) => self.start_timer(target),
            (S::Sitting, K::PauseTimer) => self.pause_timer(target).map(timer),
            (S::Sitting, K::StopTimer) => self.stop_timer(target, None).map(timer),
            (S::Sitting, K::PlannedLength) => self.plan_minutes(target, length(Some(&answer_text(&answer)?))?),
            (S::Sitting, K::LogMinutes) => {
                let text = answer_text(&answer)?;
                if text.is_empty() {
                    return Err(LumennaError::new("say how many minutes, such as 45m"));
                }
                self.stop_timer(target, Some(resolve::minutes(&text)?)).map(timer)
            }
            (S::Sitting, K::Unassign) => self.unassign(target),
            (S::Sitting, K::CarryForward) => self.carry_forward(target, &answer_pick(&answer)?.0),
            (S::Block, K::EndNow) => self.end_block_now(target),
            (S::Block, K::Extend) => self.extend_block(target, crate::day::EXTEND_MINS),
            (S::Block, K::CarryRest) => self.carry_rest_forward(target),
            (S::Day, K::WhatNow) => self.work_on(&answer_pick(&answer)?.0, &Zoned::now()),
            (S::Day, K::EndDay) => self.end_day(true),
            (S::Day, K::ClearDay) => self.end_day(false),
            (S::Project, K::Rename) => self.rename_project(target, &answer_text(&answer)?),
            (S::Project, K::NewInside) => self.add_project(&answer_text(&answer)?, Some(target.to_owned())),
            (S::Project, K::New) => self.add_project(&answer_text(&answer)?, None),
            (S::Project, K::MoveUnder) => self.move_project(target, Some(answer_pick(&answer)?.0)),
            (S::Project, K::MoveToTopLevel) => self.move_project(target, None),
            (S::Project, K::MoveUp) => self.reorder_project(target, Direction::Up),
            (S::Project, K::MoveDown) => self.reorder_project(target, Direction::Down),
            (S::Project, K::Weight) => self.weigh_project(target, crate::form::parse_weight(answer_text(&answer)?)?),
            (S::Project, K::Archive | K::Unarchive) => {
                let archived = resolve_archived(self, target)?;
                if archived == (action.kind == K::Archive) {
                    Ok(Change::unchanged(if archived { "it is already archived" } else { "it is not archived" }))
                } else {
                    self.archive_project(target)
                }
            }
            (S::Project, K::Delete) => self.delete_project(target, answer_pick(&answer)?.0 == "keep"),
            (S::Label, K::Rename) => self.rename_label(target, &answer_text(&answer)?),
            (S::Label, K::New) => self.add_label(&answer_text(&answer)?),
            (S::Label, K::MergeInto) => self.merge_labels(target, &answer_pick(&answer)?.0),
            (S::Label, K::Colour) => {
                let colour = answer_text(&answer)?;
                let colour = (!colour.is_empty() && !colour.eq_ignore_ascii_case("none")).then_some(colour);
                self.recolour_label(target, colour)
            }
            (S::Label, K::MoveUp) => self.reorder_label(target, Direction::Up),
            (S::Label, K::MoveDown) => self.reorder_label(target, Direction::Down),
            (S::Label, K::Delete) => self.delete_label(target),
            (S::Filter, K::Rename) => self.edit_filter(target, Some(answer_text(&answer)?), None),
            (S::Filter, K::ChangeQuery) => {
                let query = answer_text(&answer)?;
                if query.is_empty() {
                    return Err(LumennaError::new("a saved filter needs a query"));
                }
                self.edit_filter(target, None, Some(query))
            }
            (S::Filter, K::MoveUp) => self.reorder_filter(target, Direction::Up),
            (S::Filter, K::MoveDown) => self.reorder_filter(target, Direction::Down),
            (S::Filter, K::Delete) => self.delete_filter(target),
            (S::Reminder, K::New) => self.add_reminder_saying(target.to_owned(), answer_text(&answer)?),
            (S::Reminder, K::Delete) => self.remove_reminder(target.to_owned()),
            #[cfg(feature = "link")]
            (S::Device, K::Rename) => {
                let name = answer_text(&answer)?;
                if name.is_empty() {
                    return Err(LumennaError::new("a device needs a name"));
                }
                self.rename_device(target, &name)
            }
            #[cfg(feature = "link")]
            (S::Device, K::Unpair) => self.unpair_device(target),
            _ => Err(LumennaError::new(format!("{} is not something this build can do", action.title))),
        }
    }
}

fn resolve_archived(lumenna: &Lumenna, name: &str) -> Result<bool> {
    lumenna.with(|store| Ok(resolve::project(&repaired(store), name)?.archived))
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// What Put in a Block offers from another day: `days` days of work blocks from `from`
    /// (a date phrase), but for those already over. [`Lumenna::choices`] offers this week's.
    ///
    /// # Errors
    ///
    /// If `from` is not a date.
    pub fn work_block_choices(&self, from: Option<String>, days: Option<u32>) -> Result<Choices> {
        self.blocks_for(None, from, days)
    }
}

impl Lumenna {
    /// The work blocks of `days` days from `from` still to come, and, for `task`, only those
    /// whose filter admits it: a block that takes #Work tasks is not offered for a call home.
    pub(crate) fn blocks_for(&self, task: Option<&str>, from: Option<String>, days: Option<u32>) -> Result<Choices> {
        let blocks = self.work_blocks(from, days)?;
        let admitted: Option<BTreeSet<String>> = match task {
            None => None,
            Some(task) => Some(self.with(|store| {
                store.load_all_years()?;
                let snapshot = repaired(store);
                let task = snapshot
                    .tasks
                    .get(&resolve::task_id(&snapshot, task)?)
                    .cloned()
                    .ok_or_else(|| LumennaError::new("that task is not here"))?;
                let now = Zoned::now();
                let cx = lumenna_core::filter::Context::new(&snapshot, &now);
                Ok(snapshot
                    .series
                    .values()
                    .filter(|series| match series.task_filter.as_deref().map(str::trim) {
                        None | Some("") => true,
                        // A filter that no longer reads (a label renamed away) admits nothing
                        // rather than everything.
                        Some(text) => resolve::query(&snapshot, text).is_ok_and(|expr| expr.matches(&task, &cx)),
                    })
                    .map(|series| series.id.to_string())
                    .collect())
            })?),
        };
        let open: Vec<Choice> = blocks
            .blocks
            .into_iter()
            .filter(|b| b.when != "past")
            .filter(|b| admitted.as_ref().is_none_or(|ids| ids.contains(b.id.split('@').next().unwrap_or(&b.id))))
            .map(|b| Choice {
                id: b.id,
                title: b.title,
                date: Some(b.date),
                start: Some(b.start),
                end: Some(b.end),
                ..Choice::default()
            })
            .collect();
        let none = match (blocks.days, blocks.from == jiff::Zoned::now().date().to_string()) {
            (7, true) => "There are no work blocks this week. Add one from Today.".to_owned(),
            (1, _) => format!("There are no work blocks on {}.", blocks.from),
            (days, _) => format!("There are no work blocks in the {days} days from {}.", blocks.from),
        };
        Ok(choices(open, "work block", &none))
    }
}
