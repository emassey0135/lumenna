//! What a platform's own search finds of Lumenna's: Spotlight's indexed entities, Android's
//! AppSearch, GNOME Shell's search provider and KDE's KRunner. Each indexes, or answers with,
//! the core's [`SearchDocument`]s, and opens one chosen with its `launch` (`Event::Launch`), so
//! no platform decides what matches or how a result reads.
//!
//! A task's document carries what the platform's own entity for it holds too ([`SearchTask`]:
//! its priority, estimate, states and the rest), so Spotlight's entity, App Intents' and
//! Android's AppSearch are made from one call ([`documents`], or [`task_documents`] for the
//! tasks a listing names) and none goes stale beside its search line.

use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::Result;

/// One thing a platform's search can find.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchDocument {
    /// What it is, with its kind, unique among every document: "task:<id>".
    pub id: String,
    /// What kind of thing it is.
    pub kind: SearchKind,
    /// Its name.
    pub title: String,
    /// What a result says under its name: a task's due date, project and labels; a block's
    /// length; a saved filter's query.
    pub line: String,
    /// Words it is also found by, beside its name: a task's notes, project and labels.
    pub keywords: Vec<String>,
    /// When it is due, as an ISO date, for a task that has a date.
    pub due: Option<String>,
    /// What opening it hands the app (`Event::Launch`): its `id`.
    pub launch: String,
    /// For a task, the rest of what a platform's entity for it holds; none for anything else.
    pub task: Option<SearchTask>,
}

/// What a platform's own entity for a task holds beside its search document: Spotlight's and
/// App Intents' `TaskEntity`, Android's AppSearch document. As the task stands when the
/// document was made, so an entity made from it is never older than its line.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchTask {
    /// The project it is in, by name; none for the Inbox.
    pub project: Option<String>,
    /// Its labels' names, without the `@`.
    pub labels: Vec<String>,
    /// 1 to 4, where 1 is highest.
    pub priority: u8,
    /// Its due time, `HH:MM`, beside the document's `due`, for the platform's clock.
    pub due_time: Option<String>,
    /// How long it should take.
    pub estimate_mins: Option<u32>,
    /// How it repeats, in the words quick add takes ("every weekday").
    pub repetition: Option<String>,
    /// Its states, as the filter language names them: `overdue`, `blocked`, `recurring`, …
    pub states: Vec<String>,
    /// Its notes.
    pub notes: String,
    /// When it was made, RFC 3339.
    pub created_at: String,
    /// Whether its current occurrence is done.
    pub done: bool,
    /// Whether it is in the trash.
    pub deleted: bool,
}

/// What kind of thing a search finds.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// An open task.
    Task,
    /// A project.
    Project,
    /// A label.
    Label,
    /// A saved filter.
    Filter,
    /// A block series.
    Block,
}

/// Every document a platform keeps an index of: open tasks, projects, labels, saved filters and
/// blocks. What is done or in the trash is left out, so a search never offers it.
///
/// # Errors
///
/// If the listings cannot be made.
pub fn documents(snapshot: &Snapshot, now: &Zoned) -> Result<Vec<SearchDocument>> {
    let mut found = Vec::new();
    let facts = snapshot.facts();
    for row in crate::lists::tasks(snapshot, "", now)?.rows {
        if let Some(task) = row.id.parse().ok().and_then(|id| snapshot.tasks.get(&id)) {
            found.push(task_document(snapshot, &facts, task, now));
        }
    }
    for project in snapshot.projects.values().filter(|p| !p.archived && !p.is_inbox) {
        found.push(named(SearchKind::Project, "project", &project.name, "Project"));
    }
    for label in snapshot.labels.values() {
        found.push(named(SearchKind::Label, "label", &label.name, "Label"));
    }
    for filter in snapshot.saved_filters.values() {
        found.push(SearchDocument { line: filter.query.clone(), ..named(SearchKind::Filter, "filter", &filter.name, "") });
    }
    for series in snapshot.series.values() {
        let id = format!("block:{}", series.id);
        // How long it runs, which needs no clock: a time of day is the platform's to say.
        let line = crate::words::duration(series.duration_mins);
        found.push(SearchDocument {
            id: id.clone(),
            kind: SearchKind::Block,
            title: series.title.clone(),
            line,
            keywords: Vec::new(),
            due: None,
            launch: id,
            task: None,
        });
    }
    Ok(found)
}

/// The documents of the tasks `ids` names, in that order, done and trashed ones too: what a
/// platform makes its entities of for the tasks a listing or an operation answered with, in one
/// call. An identifier that names no task here is passed over.
#[must_use]
pub fn task_documents(snapshot: &Snapshot, ids: &[String], now: &Zoned) -> Vec<SearchDocument> {
    let facts = snapshot.facts();
    ids.iter()
        .filter_map(|id| crate::resolve::task_id(snapshot, id).ok())
        .filter_map(|id| snapshot.tasks.get(&id))
        .map(|task| task_document(snapshot, &facts, task, now))
        .collect()
}

/// What a search for `text` finds, most fitting first: the tasks the core's own search finds,
/// then projects, labels, saved filters and blocks whose names hold it.
///
/// # Errors
///
/// If the search cannot be made.
pub fn results(snapshot: &Snapshot, text: &str, now: &Zoned) -> Result<Vec<SearchDocument>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let facts = snapshot.facts();
    let mut found: Vec<SearchDocument> = crate::lists::search(snapshot, text, now)?
        .rows
        .iter()
        .filter_map(|row| row.id.parse().ok().and_then(|id| snapshot.tasks.get(&id)))
        .map(|task| task_document(snapshot, &facts, task, now))
        .collect();
    let wanted = text.to_lowercase();
    found.extend(
        documents(snapshot, now)?
            .into_iter()
            .filter(|doc| doc.kind != SearchKind::Task && doc.title.to_lowercase().contains(&wanted)),
    );
    Ok(found)
}

fn named(kind: SearchKind, prefix: &str, name: &str, line: &str) -> SearchDocument {
    let id = format!("{prefix}:{name}");
    SearchDocument { id: id.clone(), kind, title: name.to_owned(), line: line.to_owned(), keywords: Vec::new(), due: None, launch: id, task: None }
}

fn task_document(snapshot: &Snapshot, facts: &lumenna_core::state::Facts<'_>, task: &lumenna_core::model::Task, now: &Zoned) -> SearchDocument {
    let project = snapshot.projects.get(&task.project_id).filter(|p| !p.is_inbox).map(|p| p.name.clone());
    let labels: Vec<String> = snapshot.labels_of(task).iter().map(|l| l.name.clone()).collect();
    let due = task.due.as_ref();
    let mut line = Vec::new();
    // As its row says it: "due tomorrow", the time left to the platform's clock.
    line.extend(due.map(|due| lumenna_core::time::due_words(due.date, now.date())));
    line.extend(project.clone());
    line.extend(labels.iter().map(|l| format!("@{l}")));
    let mut keywords: Vec<String> = project.iter().cloned().chain(labels.iter().cloned()).collect();
    if !task.notes.trim().is_empty() {
        keywords.push(task.notes.trim().to_owned());
    }
    let id = format!("task:{}", task.id);
    let repetition = due
        .and_then(|due| due.recurrence.as_ref())
        .and_then(|r| crate::types::repetition_phrase(&r.rrule, r.from_completion));
    SearchDocument {
        id: id.clone(),
        kind: SearchKind::Task,
        title: task.title.clone(),
        line: line.join(", "),
        keywords,
        due: due.map(|d| d.date.to_string()),
        launch: id,
        task: Some(SearchTask {
            project,
            labels,
            priority: task.priority.as_u8(),
            due_time: due.and_then(|d| d.time).map(lumenna_core::time::time_text),
            estimate_mins: task.estimate_mins,
            repetition,
            states: facts.states_of(task, now).iter().map(|s| s.keyword().to_owned()).collect(),
            notes: task.notes.clone(),
            created_at: task.created_at.to_string(),
            done: facts.is_completed(task),
            deleted: task.is_deleted(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumenna_core::model::{Due, Priority, Project, Task, TaskCompletion};
    use lumenna_core::order::OrderKey;

    #[test]
    fn a_tasks_document_carries_what_its_entity_holds_and_the_tasks_named_come_in_one_call() {
        let now: Zoned = "2026-10-20T09:00:00[UTC]".parse().unwrap();
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let mut open = Task::new(inbox.id, "Call the bank", OrderKey::middle());
        open.priority = Priority::P1;
        open.estimate_mins = Some(15);
        open.due = Some(Due::at(jiff::civil::date(2026, 10, 19), jiff::civil::time(9, 30, 0, 0)));
        let done = Task::new(inbox.id, "Buy milk", OrderKey::middle());
        let completion = TaskCompletion::new(done.id, None);
        snapshot.projects.insert(inbox.id, inbox);
        snapshot.tasks.insert(open.id, open.clone());
        snapshot.tasks.insert(done.id, done.clone());
        snapshot.completions.insert(completion.id, completion);

        let indexed = documents(&snapshot, &now).unwrap();
        let task = indexed.iter().find(|d| d.id == format!("task:{}", open.id)).unwrap().task.clone().unwrap();
        assert_eq!((task.priority, task.estimate_mins, task.due_time.as_deref()), (1, Some(15), Some("09:30")));
        assert!(task.states.iter().any(|s| s == "overdue"), "{:?}", task.states);
        assert!(!indexed.iter().any(|d| d.title == "Buy milk"), "a search never offers what is done");

        let named = task_documents(&snapshot, &[done.id.to_string(), "nothing".to_owned(), open.id.to_string()], &now);
        let titles: Vec<&str> = named.iter().map(|d| d.title.as_str()).collect();
        assert_eq!(titles, ["Buy milk", "Call the bank"], "in the order named, done ones too");
        assert!(named[0].task.as_ref().unwrap().done);
    }
}
