//! The listings: tasks by query, projects, labels and saved filters, each a function of the
//! snapshot alone.

use jiff::Zoned;
use lumenna_core::filter::{Context, Expr, Predicate};
use lumenna_core::model::{Label, Project, SavedFilter};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;

use crate::error::Result;
use crate::resolve;
use crate::tree::in_tree_order;
use crate::types::{FilterView, Filters, Query, Rows, Unresolved, name_kind};
use crate::words::{count_line, to_u32};


/// Tasks matching a filter query, or every open task for an empty one, most urgent first unless
/// the store or the query says otherwise.
///
/// # Errors
///
/// If the query cannot be read; the message says where and what was expected.
pub fn tasks(snapshot: &Snapshot, query: &str, now: &Zoned) -> Result<Rows> {
    let (expr, sort) = resolve::list_query(snapshot, query)?;
    let cx = Context::new(snapshot, now);

    let unresolved: Vec<Unresolved> = expr
        .unresolved(snapshot)
        .into_iter()
        .map(|name| Unresolved {
            kind: name_kind(name.kind).to_owned(),
            name: name.name,
            suggestion: name.suggestion,
        })
        .collect();
    let notices: Vec<String> = unresolved
        .iter()
        .map(|name| {
            let hint = name
                .suggestion
                .as_deref()
                .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"));
            format!("no {} called '{}'{hint}", name.kind, name.name)
        })
        .collect();

    let order = sort.unwrap_or(snapshot.settings.task_order);
    let mut rows = Rows::new(&snapshot.task_rows_sorted(&expr, &cx, order), "task");
    if !query.trim().is_empty() {
        rows.query = Some(Query {
            text: query.to_owned(),
            description: resolve::described(&expr, sort),
            unresolved,
        });
    }
    rows.notices = notices;
    // A place's own query reads as that place: a project, a label.
    let is = |reference: String| query.trim() == reference;
    rows.empty = match query.trim() {
        "" => "No open tasks.".to_owned(),
        "deleted" => "The trash is empty.".to_owned(),
        _ if snapshot.projects.values().any(|p| p.deleted_at.is_none() && is(crate::project_reference(p.name.clone()))) => {
            "No open tasks in this project.".to_owned()
        }
        _ if snapshot.labels.values().any(|l| l.deleted_at.is_none() && is(crate::label_reference(l.name.clone()))) => {
            "No open task has this label.".to_owned()
        }
        _ => "No tasks match this filter.".to_owned(),
    };
    crate::actions::fill_rows(&mut rows, snapshot);
    Ok(rows)
}

/// Tasks whose title or notes contain `text`.
///
/// # Errors
///
/// Never; a `Result` as every listing is.
pub fn search(snapshot: &Snapshot, text: &str, now: &Zoned) -> Result<Rows> {
    let expr = Expr::Predicate(Predicate::Search(text.to_owned()));
    let cx = Context::new(snapshot, now);
    let mut rows = Rows::new(&snapshot.task_rows(&expr, &cx), "task");
    rows.empty = format!("No task has {text} in it.");
    crate::actions::fill_rows(&mut rows, snapshot);
    Ok(rows)
}

/// Every project, in tree order, with how many open tasks each holds.
///
/// # Errors
///
/// Never; a `Result` as every listing is.
pub fn projects(snapshot: &Snapshot) -> Result<Rows> {
    let live = in_tree_order(snapshot);
    let count = to_u32(live.len());
    let facts = snapshot.facts();
    let rows: Vec<Row> = live
        .iter()
        .enumerate()
        .map(|(index, (project, depth))| {
            // Open tasks: a project of two hundred finished ones is not two hundred
            // tasks' worth of anything.
            let tasks = snapshot
                .tasks
                .values()
                .filter(|t| t.project_id == project.id && !t.is_deleted())
                .filter(|t| !facts.is_completed(t))
                .count();
            let mut value = count_line(tasks, "open task");
            let weight = snapshot.effective_weight(project.id);
            if (weight - Project::NEUTRAL_WEIGHT).abs() > f32::EPSILON {
                value.push_str(&format!(", weight {weight}"));
            }
            Row {
                id: RowId::Project(project.id),
                role: Role::Project,
                depth: *depth,
                index: to_u32(index) + 1,
                count,
                expanded: None,
                checked: None,
                title: project.name.clone(),
                state: Vec::new(),
                due: None,
                due_time: None,
                value: Some(value),
                hint: None,
            }
        })
        .collect();
    let mut rows = Rows::new(&rows, "project");
    // Archived is a state of the row, not a word in its value, so a client can offer
    // to unarchive without reading prose. Core's states are a task's; this is not.
    for (row, (project, _)) in rows.rows.iter_mut().zip(&live) {
        if project.archived {
            row.state.push("archived".to_owned());
        }
    }
    crate::actions::fill_rows(&mut rows, snapshot);
    Ok(rows)
}

/// Every label, in order, with how many open tasks wear it.
///
/// # Errors
///
/// Never; a `Result` as every listing is.
pub fn labels(snapshot: &Snapshot) -> Result<Rows> {
    let mut live: Vec<&Label> =
        snapshot.labels.values().filter(|l| l.deleted_at.is_none()).collect();
    live.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
    let count = to_u32(live.len());
    let facts = snapshot.facts();
    let rows: Vec<Row> = live
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let used = snapshot
                .tasks
                .values()
                .filter(|t| t.labels.contains(&label.id) && !t.is_deleted())
                .filter(|t| !facts.is_completed(t))
                .count();
            let mut value = count_line(used, "open task");
            // Said in words, never colour alone.
            if let Some(colour) = &label.color {
                value.push_str(&format!(", {colour}"));
            }
            Row {
                id: RowId::Label(label.id),
                role: Role::Label,
                depth: 0,
                index: to_u32(index) + 1,
                count,
                expanded: None,
                checked: None,
                title: label.name.clone(),
                state: Vec::new(),
                due: None,
                due_time: None,
                value: Some(value),
                hint: None,
            }
        })
        .collect();
    let mut rows = Rows::new(&rows, "label");
    crate::actions::fill_rows(&mut rows, snapshot);
    Ok(rows)
}

/// Every saved filter, in order.
///
/// # Errors
///
/// Never; a `Result` as every listing is.
pub fn filters(snapshot: &Snapshot) -> Result<Filters> {
    let mut live: Vec<&SavedFilter> =
        snapshot.saved_filters.values().filter(|f| f.deleted_at.is_none()).collect();
    live.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
    Ok(Filters {
        announcement: count_line(live.len(), "filter"),
        notices: Vec::new(),
        count: to_u32(live.len()),
        filters: live
            .iter()
            .enumerate()
            .map(|(index, saved)| FilterView {
                row: to_u32(index + 1),
                id: saved.id.to_string(),
                name: saved.name.clone(),
                query: saved.query.clone(),
                actions: crate::actions::filter(snapshot, saved),
            })
            .collect(),
        empty: crate::places::SidebarGroup::Filters.empty(),
    })
}
