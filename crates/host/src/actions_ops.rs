//! Running a row's action, and what a pick offers: the half of the actions that reads the
//! store. Which actions a row has, and what each asks, are `lumenna_records::actions`.


use jiff::Zoned;

use crate::actions::{Action, ActionKind, Answer, Choices, Question, Subject};
use crate::error::{LumennaError, Result};
use crate::types::{Change, Direction, MoveTarget};
use crate::{Lumenna, repaired, resolve};
use lumenna_records::resolve::split_day;

// ---------------------------------------------------------------------------------------
// What a picker offers
// ---------------------------------------------------------------------------------------

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

fn timer(timer: crate::types::Timer) -> Change {
    Change {
        announcement: timer.announcement,
        notices: timer.notices,
        changed: timer.changed,
        affected: crate::types::Affected { assignments: vec![timer.assignment], ..Default::default() },
        task: None,
    }
}

impl Lumenna {
    /// What a [`Question::Pick`] offers for `action`, in order: never a task as its own
    /// parent or a project inside itself, never a wait that would close a cycle.
    ///
    /// # Errors
    ///
    /// If the record it acts on cannot be found, or the action picks nothing.
    pub fn choices(&self, action: Action) -> Result<Choices> {
        let now = Zoned::now();
        self.with(|store| {
            store.load_all_years()?;
            lumenna_records::choices::offered(&repaired(store), &action, &now)
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
            (S::Task, K::ErasePermanently) => self.erase_task_permanently(target),
            (S::Store, K::EraseEverythingDeleted) => self.erase_everything_deleted(),
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
            (S::Reminder, K::DeliverOn) => self.deliver_reminder(target.to_owned(), answer_pick(&answer)?.0),
            (S::Task, K::TimeZone) => self.set_task_zone(target, Some(answer_text(&answer)?)),
            (S::Series, K::TimeZone) => self.set_block_zone(target, Some(answer_text(&answer)?)),
            (S::Sitting, K::Skip) => self.skip_sitting(target, true),
            (S::Sitting, K::Unskip) => self.skip_sitting(target, false),
            (S::Sitting, K::MoveUp) => self.reorder_sitting(target, Direction::Up),
            (S::Sitting, K::MoveDown) => self.reorder_sitting(target, Direction::Down),
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
            (S::Label | S::Project | S::Filter, K::Colour) => {
                // A pick of the palette's colours; a line naming one, from a client that asks so.
                let colour = match &answer {
                    Answer::Picked { id, .. } => id.clone(),
                    _ => answer_text(&answer)?,
                };
                let colour = lumenna_records::colours::colour_named(&colour)?;
                match action.subject {
                    S::Project => self.recolour_project(target, colour),
                    S::Filter => self.recolour_filter(target, colour),
                    _ => self.recolour_label(target, colour),
                }
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
        let now = Zoned::now();
        self.with(|store| {
            store.load_all_years()?;
            lumenna_records::choices::blocks_for(&repaired(store), task, from.as_deref(), days, &now)
        })
    }
}
