//! The JSON export: present state, complete enough to rebuild it.
//!
//! Every type here is a mirror of a model record with the field names and encodings fixed for
//! readers: identifiers and dates as their text forms, enums as lowercase words, priority as
//! the number people say. The model can change shape without this changing, and when this has
//! to change, [`VERSION`] does too.
//!
//! **Trash is left out.** A trashed task is history the user asked to be rid of, and an
//! export is the file that leaves the device. So are the completions, assignments and
//! reminders hanging off trashed records, reminder acknowledgements (one device's
//! housekeeping), and the device roster (pairing, not data).

use std::str::FromStr;

use jiff::{Timestamp, Zoned, civil};
use lumenna_core::id::{
    AssignmentId, CompletionId, FilterId, LabelId, NodeId, ProjectId, ReminderId, SeriesId,
    TaskId,
};
use lumenna_core::model::{
    AssignmentStatus, BlockAssignment, BlockException, BlockFlags, BlockKind, BlockRef,
    BlockSeries, Delivery, Due, ExceptionAction, ExternalProvider, ExternalRef, Label, Priority,
    Project, Recurrence, Reminder, ReminderAnchor, ReminderTarget, SavedFilter, Settings, Task,
    TaskCompletion, Trigger, TzName, Verbosity,
};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};

/// What the `format` field of every export says.
pub const FORMAT: &str = "lumenna-export";

/// The export format's version. Raise it on any change a reader could trip over.
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    #[serde(default)]
    exported_at: Option<String>,
    #[serde(default)]
    settings: Option<SettingsOut>,
    #[serde(default)]
    projects: Vec<ProjectOut>,
    #[serde(default)]
    labels: Vec<LabelOut>,
    #[serde(default)]
    filters: Vec<FilterOut>,
    #[serde(default)]
    tasks: Vec<TaskOut>,
    #[serde(default)]
    completions: Vec<CompletionOut>,
    #[serde(default)]
    blocks: Vec<BlockOut>,
    #[serde(default)]
    exceptions: Vec<ExceptionOut>,
    #[serde(default)]
    assignments: Vec<AssignmentOut>,
    #[serde(default)]
    reminders: Vec<ReminderOut>,
}

/// The records an export holds, as model values, ready to be written into a store.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Imported {
    /// Settings, if the export carried them.
    pub settings: Option<Settings>,
    /// Projects.
    pub projects: Vec<Project>,
    /// Labels.
    pub labels: Vec<Label>,
    /// Saved filters.
    pub filters: Vec<SavedFilter>,
    /// Tasks.
    pub tasks: Vec<Task>,
    /// Completions.
    pub completions: Vec<TaskCompletion>,
    /// Block series.
    pub series: Vec<BlockSeries>,
    /// Block exceptions.
    pub exceptions: Vec<BlockException>,
    /// Assignments.
    pub assignments: Vec<BlockAssignment>,
    /// Reminders.
    pub reminders: Vec<Reminder>,
}

impl Imported {
    /// How many records there are in all.
    #[must_use]
    pub fn len(&self) -> usize {
        self.projects.len()
            + self.labels.len()
            + self.filters.len()
            + self.tasks.len()
            + self.completions.len()
            + self.series.len()
            + self.exceptions.len()
            + self.assignments.len()
            + self.reminders.len()
    }

    /// Whether there is nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0 && self.settings.is_none()
    }
}

/// The present state as JSON.
#[must_use]
pub fn export_json(snapshot: &Snapshot, now: Timestamp) -> String {
    let live_task = |id: &TaskId| snapshot.tasks.get(id).is_some_and(|t| !t.is_deleted());
    let live_series =
        |id: &SeriesId| snapshot.series.get(id).is_some_and(|s| s.deleted_at.is_none());

    let file = File {
        format: FORMAT.to_owned(),
        version: VERSION,
        exported_at: Some(now.to_string()),
        settings: Some(SettingsOut::from(&snapshot.settings)),
        projects: snapshot
            .projects
            .values()
            .filter(|p| p.deleted_at.is_none())
            .map(ProjectOut::from)
            .collect(),
        labels: snapshot
            .labels
            .values()
            .filter(|l| l.deleted_at.is_none())
            .map(LabelOut::from)
            .collect(),
        filters: snapshot
            .saved_filters
            .values()
            .filter(|f| f.deleted_at.is_none())
            .map(FilterOut::from)
            .collect(),
        tasks: snapshot.tasks.values().filter(|t| !t.is_deleted()).map(TaskOut::from).collect(),
        completions: snapshot
            .completions
            .values()
            .filter(|c| live_task(&c.task_id))
            .map(CompletionOut::from)
            .collect(),
        blocks: snapshot
            .series
            .values()
            .filter(|s| s.deleted_at.is_none())
            .map(BlockOut::from)
            .collect(),
        exceptions: snapshot
            .exceptions
            .values()
            .filter(|e| live_series(&e.series_id))
            .map(ExceptionOut::from)
            .collect(),
        assignments: snapshot
            .assignments
            .values()
            .filter(|a| live_task(&a.task_id) && live_series(&a.block_ref.series_id()))
            .map(AssignmentOut::from)
            .collect(),
        reminders: snapshot
            .reminders
            .values()
            .filter(|r| r.deleted_at.is_none())
            .filter(|r| match r.target {
                ReminderTarget::Task(id) => live_task(&id),
                ReminderTarget::Block(id) => live_series(&id),
            })
            .map(ReminderOut::from)
            .collect(),
    };
    serde_json::to_string_pretty(&file).unwrap_or_default()
}

/// Reads an export back into model records.
///
/// # Errors
///
/// [`StoreError::Unreadable`] if the text is not a Lumenna export, comes from a newer format,
/// or holds a value that cannot be read — named, with the record it was in, so the message
/// says where to look.
pub fn parse_json(text: &str) -> Result<Imported> {
    let file: File = serde_json::from_str(text)
        .map_err(|e| StoreError::Unreadable(format!("that is not a Lumenna export: {e}")))?;
    if file.format != FORMAT {
        return Err(StoreError::Unreadable(format!(
            "that is not a Lumenna export (its format says '{}')",
            file.format
        )));
    }
    if file.version > VERSION {
        return Err(StoreError::Unreadable(format!(
            "that export was made by a newer version of Lumenna (format {}); update to import it",
            file.version
        )));
    }
    Ok(Imported {
        settings: file.settings.map(SettingsOut::into_model).transpose()?,
        projects: collect(file.projects, ProjectOut::into_model)?,
        labels: collect(file.labels, LabelOut::into_model)?,
        filters: collect(file.filters, FilterOut::into_model)?,
        tasks: collect(file.tasks, TaskOut::into_model)?,
        completions: collect(file.completions, CompletionOut::into_model)?,
        series: collect(file.blocks, BlockOut::into_model)?,
        exceptions: collect(file.exceptions, ExceptionOut::into_model)?,
        assignments: collect(file.assignments, AssignmentOut::into_model)?,
        reminders: collect(file.reminders, ReminderOut::into_model)?,
    })
}

fn collect<A, B>(items: Vec<A>, convert: impl Fn(A) -> Result<B>) -> Result<Vec<B>> {
    items.into_iter().map(convert).collect()
}

/// Parses one field, naming it and the record it belongs to when it fails.
fn field<T: FromStr>(record: &str, name: &str, text: &str) -> Result<T> {
    text.parse().map_err(|_| {
        StoreError::Unreadable(format!("{record}: '{text}' is not a valid {name}"))
    })
}

fn optional<T: FromStr>(record: &str, name: &str, text: Option<&String>) -> Result<Option<T>> {
    text.map(|t| field(record, name, t)).transpose()
}

fn is_false(value: &bool) -> bool {
    !*value
}

// ---------------------------------------------------------------------------------------
// Small values
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct ExternalOut {
    provider: String,
    external_id: String,
    read_only: bool,
    last_synced: String,
}

impl From<&ExternalRef> for ExternalOut {
    fn from(e: &ExternalRef) -> Self {
        Self {
            provider: match e.provider {
                ExternalProvider::EventKit => "eventkit",
                ExternalProvider::CalDav => "caldav",
                ExternalProvider::IcsUrl => "ics_url",
            }
            .to_owned(),
            external_id: e.external_id.clone(),
            read_only: e.read_only,
            last_synced: e.last_synced.to_string(),
        }
    }
}

impl ExternalOut {
    fn into_model(self, record: &str) -> Result<ExternalRef> {
        Ok(ExternalRef {
            provider: match self.provider.as_str() {
                "caldav" => ExternalProvider::CalDav,
                "ics_url" => ExternalProvider::IcsUrl,
                _ => ExternalProvider::EventKit,
            },
            external_id: self.external_id,
            read_only: self.read_only,
            last_synced: field(record, "timestamp", &self.last_synced)?,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct FlagsOut {
    accepts_tasks: bool,
    counts_capacity: bool,
    anchored: bool,
}

impl From<&BlockFlags> for FlagsOut {
    fn from(f: &BlockFlags) -> Self {
        Self { accepts_tasks: f.accepts_tasks, counts_capacity: f.counts_capacity, anchored: f.anchored }
    }
}

impl From<FlagsOut> for BlockFlags {
    fn from(f: FlagsOut) -> Self {
        Self { accepts_tasks: f.accepts_tasks, counts_capacity: f.counts_capacity, anchored: f.anchored }
    }
}

fn kind_word(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Work => "work",
        BlockKind::Break => "break",
        BlockKind::Event => "event",
    }
}

fn kind_of(word: &str) -> BlockKind {
    match word {
        "break" => BlockKind::Break,
        "event" => BlockKind::Event,
        _ => BlockKind::Work,
    }
}

#[derive(Serialize, Deserialize)]
struct TriggerOut {
    anchor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    at: Option<String>,
    #[serde(default)]
    offset_mins: i32,
}

impl From<&Trigger> for TriggerOut {
    fn from(t: &Trigger) -> Self {
        let (anchor, at) = match &t.anchor {
            ReminderAnchor::Due => ("due", None),
            ReminderAnchor::BlockStart => ("block_start", None),
            ReminderAnchor::BlockEnd => ("block_end", None),
            ReminderAnchor::Absolute(at) => ("absolute", Some(at.to_string())),
        };
        Self { anchor: anchor.to_owned(), at, offset_mins: t.offset_mins }
    }
}

impl TriggerOut {
    fn into_model(self, record: &str) -> Result<Trigger> {
        let anchor = match self.anchor.as_str() {
            "due" => ReminderAnchor::Due,
            "block_start" => ReminderAnchor::BlockStart,
            "block_end" => ReminderAnchor::BlockEnd,
            "absolute" => ReminderAnchor::Absolute(field::<Zoned>(
                record,
                "zoned time",
                self.at.as_deref().unwrap_or_default(),
            )?),
            other => {
                return Err(StoreError::Unreadable(format!(
                    "{record}: '{other}' is not a reminder anchor"
                )));
            }
        };
        Ok(Trigger { anchor, offset_mins: self.offset_mins })
    }
}

// ---------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct SettingsOut {
    cascade_complete_subtasks: bool,
    verbosity: String,
    default_task_reminders: Vec<TriggerOut>,
    default_block_reminders: Vec<TriggerOut>,
    all_day_reminder_hour: String,
    day_start: String,
    day_end: String,
    week_start: String,
}

impl From<&Settings> for SettingsOut {
    fn from(s: &Settings) -> Self {
        Self {
            cascade_complete_subtasks: s.cascade_complete_subtasks,
            verbosity: match s.verbosity {
                Verbosity::Terse => "terse",
                Verbosity::Full => "full",
            }
            .to_owned(),
            default_task_reminders: s.default_task_reminders.iter().map(TriggerOut::from).collect(),
            default_block_reminders: s
                .default_block_reminders
                .iter()
                .map(TriggerOut::from)
                .collect(),
            all_day_reminder_hour: s.all_day_reminder_hour.to_string(),
            day_start: s.day_window.0.to_string(),
            day_end: s.day_window.1.to_string(),
            week_start: lumenna_core::time::weekday_name(s.week_start).to_lowercase(),
        }
    }
}

impl SettingsOut {
    fn into_model(self) -> Result<Settings> {
        let record = "settings";
        let triggers = |list: Vec<TriggerOut>| -> Result<Vec<Trigger>> {
            list.into_iter().map(|t| t.into_model(record)).collect()
        };
        Ok(Settings {
            cascade_complete_subtasks: self.cascade_complete_subtasks,
            verbosity: if self.verbosity == "terse" { Verbosity::Terse } else { Verbosity::Full },
            default_task_reminders: triggers(self.default_task_reminders)?,
            default_block_reminders: triggers(self.default_block_reminders)?,
            all_day_reminder_hour: field(record, "time", &self.all_day_reminder_hour)?,
            day_window: (
                field(record, "time", &self.day_start)?,
                field(record, "time", &self.day_end)?,
            ),
            week_start: weekday(&self.week_start).ok_or_else(|| {
                StoreError::Unreadable(format!("settings: '{}' is not a weekday", self.week_start))
            })?,
        })
    }
}

fn weekday(name: &str) -> Option<civil::Weekday> {
    use civil::Weekday::{Friday, Monday, Saturday, Sunday, Thursday, Tuesday, Wednesday};
    [Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday]
        .into_iter()
        .find(|day| lumenna_core::time::weekday_name(*day).eq_ignore_ascii_case(name))
}

// ---------------------------------------------------------------------------------------
// Projects, labels, filters
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct ProjectOut {
    id: String,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    order: String,
    #[serde(default, skip_serializing_if = "is_false")]
    archived: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    inbox: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    weight: Option<f32>,
}

impl From<&Project> for ProjectOut {
    fn from(p: &Project) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            parent: p.parent_id.map(|id| id.to_string()),
            color: p.color.clone(),
            order: p.order.to_string(),
            archived: p.archived,
            inbox: p.is_inbox,
            weight: p.weight,
        }
    }
}

impl ProjectOut {
    fn into_model(self) -> Result<Project> {
        let record = format!("project '{}'", self.name);
        Ok(Project {
            id: field::<ProjectId>(&record, "identifier", &self.id)?,
            parent_id: optional(&record, "identifier", self.parent.as_ref())?,
            order: field::<OrderKey>(&record, "order key", &self.order)?,
            name: self.name,
            color: self.color,
            archived: self.archived,
            is_inbox: self.inbox,
            weight: self.weight,
            deleted_at: None,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct LabelOut {
    id: String,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    order: String,
}

impl From<&Label> for LabelOut {
    fn from(l: &Label) -> Self {
        Self { id: l.id.to_string(), name: l.name.clone(), color: l.color.clone(), order: l.order.to_string() }
    }
}

impl LabelOut {
    fn into_model(self) -> Result<Label> {
        let record = format!("label '{}'", self.name);
        Ok(Label {
            id: field::<LabelId>(&record, "identifier", &self.id)?,
            order: field(&record, "order key", &self.order)?,
            name: self.name,
            color: self.color,
            deleted_at: None,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct FilterOut {
    id: String,
    name: String,
    query: String,
    order: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
}

impl From<&SavedFilter> for FilterOut {
    fn from(f: &SavedFilter) -> Self {
        Self {
            id: f.id.to_string(),
            name: f.name.clone(),
            query: f.query.clone(),
            order: f.order.to_string(),
            color: f.color.clone(),
        }
    }
}

impl FilterOut {
    fn into_model(self) -> Result<SavedFilter> {
        let record = format!("filter '{}'", self.name);
        Ok(SavedFilter {
            id: field::<FilterId>(&record, "identifier", &self.id)?,
            order: field(&record, "order key", &self.order)?,
            name: self.name,
            query: self.query,
            color: self.color,
            deleted_at: None,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Tasks and completions
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct DueOut {
    date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repeat: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    from_completion: bool,
}

impl From<&Due> for DueOut {
    fn from(d: &Due) -> Self {
        Self {
            date: d.date.to_string(),
            time: d.time.map(|t| t.to_string()),
            timezone: d.timezone.as_ref().map(ToString::to_string),
            repeat: d.recurrence.as_ref().map(|r| r.rrule.clone()),
            from_completion: d.recurrence.as_ref().is_some_and(|r| r.from_completion),
        }
    }
}

impl DueOut {
    fn into_model(self, record: &str) -> Result<Due> {
        Ok(Due {
            date: field(record, "date", &self.date)?,
            time: optional(record, "time", self.time.as_ref())?,
            timezone: self.timezone.map(TzName::new),
            recurrence: self
                .repeat
                .map(|rrule| Recurrence { rrule, from_completion: self.from_completion }),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct TaskOut {
    id: String,
    title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    project: String,
    priority: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    depends: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    due: Option<DueOut>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    estimate_mins: Option<u32>,
    order: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    external: Option<ExternalOut>,
    created_at: String,
}

impl From<&Task> for TaskOut {
    fn from(t: &Task) -> Self {
        Self {
            id: t.id.to_string(),
            title: t.title.clone(),
            notes: t.notes.clone(),
            parent: t.parent_id.map(|id| id.to_string()),
            project: t.project_id.to_string(),
            priority: t.priority.as_u8(),
            labels: t.labels.iter().map(ToString::to_string).collect(),
            depends: t.depends.iter().map(ToString::to_string).collect(),
            due: t.due.as_ref().map(DueOut::from),
            estimate_mins: t.estimate_mins,
            order: t.order.to_string(),
            external: t.external.as_ref().map(ExternalOut::from),
            created_at: t.created_at.to_string(),
        }
    }
}

impl TaskOut {
    fn into_model(self) -> Result<Task> {
        let record = format!("task '{}'", self.title);
        Ok(Task {
            id: field::<TaskId>(&record, "identifier", &self.id)?,
            parent_id: optional(&record, "identifier", self.parent.as_ref())?,
            project_id: field(&record, "identifier", &self.project)?,
            priority: Priority::from_u8(self.priority),
            labels: self
                .labels
                .iter()
                .map(|id| field::<LabelId>(&record, "label identifier", id))
                .collect::<Result<_>>()?,
            depends: self
                .depends
                .iter()
                .map(|id| field::<TaskId>(&record, "task identifier", id))
                .collect::<Result<_>>()?,
            due: self.due.map(|due| due.into_model(&record)).transpose()?,
            estimate_mins: self.estimate_mins,
            order: field(&record, "order key", &self.order)?,
            external: self.external.map(|e| e.into_model(&record)).transpose()?,
            created_at: field(&record, "timestamp", &self.created_at)?,
            title: self.title,
            notes: self.notes,
            deleted_at: None,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct CompletionOut {
    id: String,
    task: String,
    completed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    occurrence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cascaded_from: Option<String>,
}

impl From<&TaskCompletion> for CompletionOut {
    fn from(c: &TaskCompletion) -> Self {
        Self {
            id: c.id.to_string(),
            task: c.task_id.to_string(),
            completed_at: c.completed_at.to_string(),
            occurrence: c.occurrence_date.map(|d| d.to_string()),
            cascaded_from: c.cascaded_from.map(|id| id.to_string()),
        }
    }
}

impl CompletionOut {
    fn into_model(self) -> Result<TaskCompletion> {
        let record = format!("completion {}", self.id);
        Ok(TaskCompletion {
            id: field::<CompletionId>(&record, "identifier", &self.id)?,
            task_id: field(&record, "identifier", &self.task)?,
            completed_at: field(&record, "timestamp", &self.completed_at)?,
            occurrence_date: optional(&record, "date", self.occurrence.as_ref())?,
            cascaded_from: optional(&record, "identifier", self.cascaded_from.as_ref())?,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct BlockOut {
    id: String,
    title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    kind: String,
    flags: FlagsOut,
    start_date: String,
    start_time: String,
    duration_mins: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_duration_mins: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repeat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    task_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    external: Option<ExternalOut>,
}

impl From<&BlockSeries> for BlockOut {
    fn from(s: &BlockSeries) -> Self {
        Self {
            id: s.id.to_string(),
            title: s.title.clone(),
            notes: s.notes.clone(),
            kind: kind_word(s.kind).to_owned(),
            flags: FlagsOut::from(&s.flags),
            start_date: s.start_date.to_string(),
            start_time: s.start_time.to_string(),
            duration_mins: s.duration_mins,
            min_duration_mins: s.min_duration_mins,
            end_date: s.end_date.map(|d| d.to_string()),
            repeat: s.rrule.clone(),
            timezone: s.timezone.as_ref().map(ToString::to_string),
            color: s.color.clone(),
            icon: s.icon.clone(),
            task_filter: s.task_filter.clone(),
            external: s.external.as_ref().map(ExternalOut::from),
        }
    }
}

impl BlockOut {
    fn into_model(self) -> Result<BlockSeries> {
        let record = format!("block '{}'", self.title);
        Ok(BlockSeries {
            id: field::<SeriesId>(&record, "identifier", &self.id)?,
            kind: kind_of(&self.kind),
            flags: self.flags.into(),
            start_date: field(&record, "date", &self.start_date)?,
            start_time: field(&record, "time", &self.start_time)?,
            duration_mins: self.duration_mins,
            min_duration_mins: self.min_duration_mins,
            end_date: optional(&record, "date", self.end_date.as_ref())?,
            rrule: self.repeat,
            timezone: self.timezone.map(TzName::new),
            color: self.color,
            icon: self.icon,
            task_filter: self.task_filter,
            external: self.external.map(|e| e.into_model(&record)).transpose()?,
            title: self.title,
            notes: self.notes,
            deleted_at: None,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ExceptionOut {
    block: String,
    date: String,
    #[serde(default, skip_serializing_if = "is_false")]
    cancelled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duration_mins: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    flags: Option<FlagsOut>,
}

impl From<&BlockException> for ExceptionOut {
    fn from(e: &BlockException) -> Self {
        let mut out = Self {
            block: e.series_id.to_string(),
            date: e.original_date.to_string(),
            cancelled: false,
            start_time: None,
            duration_mins: None,
            title: None,
            kind: None,
            flags: None,
        };
        match &e.action {
            ExceptionAction::Cancelled => out.cancelled = true,
            ExceptionAction::Modified { start_time, duration_mins, title, kind, flags } => {
                out.start_time = start_time.map(|t| t.to_string());
                out.duration_mins = *duration_mins;
                out.title = title.clone();
                out.kind = kind.map(|k| kind_word(k).to_owned());
                out.flags = flags.as_ref().map(FlagsOut::from);
            }
        }
        out
    }
}

impl ExceptionOut {
    fn into_model(self) -> Result<BlockException> {
        let record = format!("change to block {} on {}", self.block, self.date);
        let action = if self.cancelled {
            ExceptionAction::Cancelled
        } else {
            ExceptionAction::Modified {
                start_time: optional(&record, "time", self.start_time.as_ref())?,
                duration_mins: self.duration_mins,
                title: self.title,
                kind: self.kind.as_deref().map(kind_of),
                flags: self.flags.map(Into::into),
            }
        };
        Ok(BlockException {
            series_id: field(&record, "identifier", &self.block)?,
            original_date: field(&record, "date", &self.date)?,
            action,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct AssignmentOut {
    id: String,
    block: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    planned_mins: Option<u32>,
    #[serde(default)]
    logged_mins: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    running_since: Option<String>,
    status: String,
    order: String,
    created_at: String,
}

impl From<&BlockAssignment> for AssignmentOut {
    fn from(a: &BlockAssignment) -> Self {
        Self {
            id: a.id.to_string(),
            block: a.block_ref.series_id().to_string(),
            date: a.block_ref.date().map(|d| d.to_string()),
            task: a.task_id.to_string(),
            planned_mins: a.planned_mins,
            logged_mins: a.accumulated_mins,
            running_since: a.running_since.map(|t| t.to_string()),
            status: match a.status {
                AssignmentStatus::Planned => "planned",
                AssignmentStatus::InProgress => "in_progress",
                AssignmentStatus::Worked => "worked",
                AssignmentStatus::Skipped => "skipped",
                AssignmentStatus::Deferred => "deferred",
            }
            .to_owned(),
            order: a.order.to_string(),
            created_at: a.created_at.to_string(),
        }
    }
}

impl AssignmentOut {
    fn into_model(self) -> Result<BlockAssignment> {
        let record = format!("assignment {}", self.id);
        let series: SeriesId = field(&record, "identifier", &self.block)?;
        let block_ref = match optional::<civil::Date>(&record, "date", self.date.as_ref())? {
            Some(date) => BlockRef::Occurrence(series, date),
            None => BlockRef::OneOff(series),
        };
        Ok(BlockAssignment {
            id: field::<AssignmentId>(&record, "identifier", &self.id)?,
            block_ref,
            task_id: field(&record, "identifier", &self.task)?,
            planned_mins: self.planned_mins,
            accumulated_mins: self.logged_mins,
            running_since: optional(&record, "timestamp", self.running_since.as_ref())?,
            status: match self.status.as_str() {
                "in_progress" => AssignmentStatus::InProgress,
                "worked" => AssignmentStatus::Worked,
                "skipped" => AssignmentStatus::Skipped,
                "deferred" => AssignmentStatus::Deferred,
                _ => AssignmentStatus::Planned,
            },
            order: field(&record, "order key", &self.order)?,
            created_at: field(&record, "timestamp", &self.created_at)?,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Reminders
// ---------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct ReminderOut {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    block: Option<String>,
    trigger: TriggerOut,
    delivery: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    devices: Vec<String>,
}

impl From<&Reminder> for ReminderOut {
    fn from(r: &Reminder) -> Self {
        let (task, block) = match r.target {
            ReminderTarget::Task(id) => (Some(id.to_string()), None),
            ReminderTarget::Block(id) => (None, Some(id.to_string())),
        };
        let (delivery, devices) = match &r.delivery {
            Delivery::AllDevices => ("all", Vec::new()),
            Delivery::OnlyDevices(set) => ("only", set.iter().map(ToString::to_string).collect()),
            Delivery::ExceptDevices(set) => {
                ("except", set.iter().map(ToString::to_string).collect())
            }
        };
        Self { id: r.id.to_string(), task, block, trigger: TriggerOut::from(&r.trigger), delivery: delivery.to_owned(), devices }
    }
}

impl ReminderOut {
    fn into_model(self) -> Result<Reminder> {
        let record = format!("reminder {}", self.id);
        let target = match (&self.task, &self.block) {
            (Some(task), _) => ReminderTarget::Task(field(&record, "identifier", task)?),
            (None, Some(block)) => ReminderTarget::Block(field(&record, "identifier", block)?),
            (None, None) => {
                return Err(StoreError::Unreadable(format!("{record}: it names no task or block")));
            }
        };
        let devices = self
            .devices
            .iter()
            .map(|id| field::<NodeId>(&record, "device identifier", id))
            .collect::<Result<_>>()?;
        Ok(Reminder {
            id: field::<ReminderId>(&record, "identifier", &self.id)?,
            target,
            trigger: self.trigger.into_model(&record)?,
            delivery: match self.delivery.as_str() {
                "only" => Delivery::OnlyDevices(devices),
                "except" => Delivery::ExceptDevices(devices),
                _ => Delivery::AllDevices,
            },
            deleted_at: None,
        })
    }
}
