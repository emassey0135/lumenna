//! The task in hand's details, as a form: its fields as text, kept as typed until saved.
//!
//! The fields follow the store while nobody is editing them; typing not yet saved is kept when
//! the store changes underneath, and for each task apart, so typing in one task's details keeps
//! another's. Saving sends only what changed, against the task as it was when the typing began
//! (`task_edit`), so a concurrent edit to another field elsewhere holds.

use std::collections::BTreeMap;

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::fields::DetailsLine;
use lumenna_records::types::TaskDetail;
use lumenna_records::{ActionKind, Choice, FormField, TaskFields};
use serde::{Deserialize, Serialize};

use crate::host::HostRequest;
use crate::model::Screen;
use crate::{Effect, Event, Lumenna, say, write};

/// What the person does in the details.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetailsEvent {
    /// Show this task's details, whatever is in hand: a sitting's task opened from the day.
    Show(String),
    /// A field typed or chosen, by its key in the core's task form: the priority as its number.
    Field {
        /// The field's key: `title`, `due`, `priority`, ….
        key: String,
        /// What it now holds.
        value: String,
    },
    /// Save what changed.
    Save,
    /// Run the first of these kinds the task shown offers, or say why it has none: a key or a
    /// menu while the details show a sitting's task, whose row in hand is the sitting.
    Run(Vec<ActionKind>),
    /// A command of the shared table (`keys::runs`) run on the task the details show, by its
    /// `id`: what it runs is the core's, as `Event::Key` on a row.
    Key(String),
}

/// Typing not yet saved.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// The task, by identifier.
    pub task: String,
    /// The task as it was when the typing began: what saving compares with.
    pub original: TaskDetail,
    /// The fields as typed.
    pub fields: TaskFields,
}

/// A save on its way to the host.
#[derive(Debug, Clone, PartialEq)]
pub struct Saving {
    /// The task.
    pub task: String,
    /// The write it went out as.
    pub number: u64,
    /// The fields as they were sent: typing since is kept once the save is answered.
    pub fields: TaskFields,
}

/// The details' state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Details {
    /// A task asked for by name, in place of the one in hand, until focus moves.
    pub shown: Option<String>,
    /// Typing not yet saved, by task.
    pub drafts: BTreeMap<String, Draft>,
    /// A save on its way to the host.
    pub saving: Option<Saving>,
}

/// The details as shown.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailsView {
    /// The task as the store has it.
    pub task: TaskDetail,
    /// The fields: as typed, or as the store has them.
    pub fields: TaskFields,
    /// Whether anything typed is not yet saved.
    pub dirty: bool,
    /// The form's fields, in order, in the core's words.
    pub form: Vec<FormField>,
    /// The projects the Project field offers.
    pub projects: Vec<Choice>,
    /// What the details list besides the form, line by line, in the core's words: the task's
    /// state, how it repeats when the form cannot say, what it waits for, its history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<DetailsLine>,
}

/// `fields` with `key` set to `value`.
pub fn set(mut fields: TaskFields, key: &str, value: String) -> TaskFields {
    match key {
        "title" => fields.title = value,
        "due" => fields.due = value,
        "repeat" => fields.repeat = value,
        "priority" => fields.priority = value.trim().parse().unwrap_or(fields.priority),
        "estimate" => fields.estimate = value,
        "project" => fields.project = value,
        "labels" => fields.labels = value,
        "notes" => fields.notes = value,
        _ => {}
    }
    fields
}

impl Screen {
    /// A save of the details answered: the draft goes, unless the person typed on since, whose
    /// typing stays, compared from now on with the task as saved.
    pub(crate) fn saved_details(&mut self, saving: Saving) {
        let Some(draft) = self.details.drafts.remove(&saving.task) else { return };
        if draft.fields == saving.fields {
            return;
        }
        let original = self
            .store
            .as_ref()
            .and_then(|store| lumenna_records::plan::task_shown(&store.snapshot, &saving.task, &store.now).ok())
            .map_or(draft.original, |shown| shown.task);
        self.details.drafts.insert(saving.task, Draft { original, ..draft });
    }
}

impl Lumenna {
    pub(crate) fn details(&self, event: DetailsEvent, model: &mut Screen) -> Command<Effect, Event> {
        match event {
            DetailsEvent::Show(task) => {
                model.details.shown = Some(task);
                render()
            }
            DetailsEvent::Run(kinds) => {
                let Some(task) = model.details_shown() else { return Command::done() };
                self.run_among(&task.actions, &kinds, lumenna_records::Subject::Task, model)
            }
            DetailsEvent::Key(id) => {
                let Some(task) = model.details_shown() else { return Command::done() };
                match crate::keys::runs(&id) {
                    Some(kinds) => self.run_among(&task.actions, kinds, lumenna_records::Subject::Task, model),
                    None => Command::done(),
                }
            }
            DetailsEvent::Field { key, value } => {
                let Some(task) = model.details_shown() else { return Command::done() };
                // Typing begins: what saving will compare with is the task as it is now.
                let draft = model.details.drafts.remove(&task.id).unwrap_or_else(|| Draft {
                    task: task.id.clone(),
                    fields: lumenna_records::task_fields(task.clone()),
                    original: task,
                });
                let fields = set(draft.fields, &key, value);
                model.details.drafts.insert(draft.task.clone(), Draft { fields, ..draft });
                render()
            }
            DetailsEvent::Save => {
                let Some(task) = model.details_shown() else { return Command::done() };
                let (original, fields) = match model.details.drafts.get(&task.id) {
                    Some(draft) => (draft.original.clone(), draft.fields.clone()),
                    None => (task.clone(), lumenna_records::task_fields(task.clone())),
                };
                match lumenna_records::task_edit(original, fields.clone()) {
                    None => {
                        model.details.drafts.remove(&task.id);
                        render().and(say(lumenna_records::fields::nothing_changed()))
                    }
                    Some(edit) => {
                        let (number, sent) = write(model, HostRequest::EditTask { id: task.id.clone(), edit }, None);
                        model.details.saving = Some(Saving { task: task.id, number, fields });
                        sent
                    }
                }
            }
        }
    }
}
