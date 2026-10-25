//! What a platform's own search finds of Lumenna's: Spotlight's indexed entities, Android's
//! AppSearch, GNOME Shell's search provider and KDE's KRunner. Each indexes, or answers with,
//! the core's [`SearchDocument`]s, and opens one chosen with its `launch` (`Event::Launch`), so
//! no platform decides what matches or how a result reads.

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
    for row in crate::lists::tasks(snapshot, "", now)?.rows {
        found.push(task_document(snapshot, &row));
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
        });
    }
    Ok(found)
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
    let mut found: Vec<SearchDocument> =
        crate::lists::search(snapshot, text, now)?.rows.iter().map(|row| task_document(snapshot, row)).collect();
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
    SearchDocument { id: id.clone(), kind, title: name.to_owned(), line: line.to_owned(), keywords: Vec::new(), due: None, launch: id }
}

fn task_document(snapshot: &Snapshot, row: &crate::RowView) -> SearchDocument {
    let task = row.id.parse().ok().and_then(|id| snapshot.tasks.get(&id));
    let project = task.and_then(|t| snapshot.projects.get(&t.project_id)).filter(|p| !p.is_inbox).map(|p| p.name.clone());
    let labels: Vec<String> =
        task.map(|t| t.labels.iter().filter_map(|l| snapshot.labels.get(l)).map(|l| l.name.clone()).collect()).unwrap_or_default();
    let mut line = Vec::new();
    line.extend(row.due.clone());
    line.extend(project.clone());
    line.extend(labels.iter().map(|l| format!("@{l}")));
    let mut keywords: Vec<String> = project.into_iter().chain(labels).collect();
    if let Some(notes) = task.map(|t| t.notes.trim()).filter(|n| !n.is_empty()) {
        keywords.push(notes.to_owned());
    }
    let id = format!("task:{}", row.id);
    SearchDocument {
        id: id.clone(),
        kind: SearchKind::Task,
        title: row.title.clone(),
        line: line.join(", "),
        keywords,
        due: task.and_then(|t| t.due.as_ref()).map(|d| d.date.to_string()),
        launch: id,
    }
}
