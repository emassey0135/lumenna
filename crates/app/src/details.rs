//! The task in hand's details, as a form: its fields as text, kept as typed until saved.
//!
//! The fields follow the store while nobody is editing them; typing not yet saved is kept when
//! the store changes underneath. Saving sends only what changed, against the task as it was
//! when the typing began (`task_edit`), so a concurrent edit to another field elsewhere holds.

use lumenna_records::types::TaskDetail;
use lumenna_records::{Choice, FormField, TaskFields};
use serde::{Deserialize, Serialize};

/// What the person does in the details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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

/// The details' state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Details {
    /// A task asked for by name, in place of the one in hand, until focus moves.
    pub shown: Option<String>,
    /// Typing not yet saved.
    pub draft: Option<Draft>,
    /// A save on its way to the host.
    pub saving: bool,
}

/// The details as shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
