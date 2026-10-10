//! Tasks: capture, listing, editing, completion, and the trash.

use std::collections::BTreeSet;

use jiff::Zoned;
use lumenna_core::edit::{self, Edit, EditError, MoveTo};
use lumenna_core::filter::{Context, Expr, Predicate};
use lumenna_core::id::LabelId;
use lumenna_core::model::{Due, Label, Priority, Recurrence, Task};
use lumenna_core::order::OrderKey;
use lumenna_parse::quickadd::{Known, Severity, parse_quick_add};
use lumenna_store::Store;

use crate::error::{LumennaError, Result};
use crate::resolve::{self, order_after};
use crate::types::{
    Announced, Change, MoveTarget, Query, Rows, TaskDetail, TaskEdit, TaskShown, Unresolved,
    name_kind,
};
use crate::{Lumenna, repaired};

/// Applies a person's edit, so it can be undone, and says what it did.
pub(crate) fn record(store: &mut Store, change: &Edit) -> Result<Change> {
    store.apply_recorded(change)?;
    Ok(Change::of(change))
}

/// The same, unless there was nothing to do — in which case nothing is written, and the
/// undo history does not fill with entries that appear to do nothing when reversed.
pub(crate) fn record_or(store: &mut Store, change: &Edit, otherwise: &str) -> Result<Change> {
    if change.is_empty() {
        return Ok(Change::unchanged(otherwise));
    }
    record(store, change)
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Adds a task written the way it would be said: a title, and optionally a date phrase,
    /// `p1` to `p4`, `#project`, `@label` and an estimate like `45m`.
    ///
    /// The announcement is the readback — what was saved, with the resolved date — and
    /// [`Change::task`] is the task itself. Notices say what was understood but not used.
    ///
    /// # Errors
    ///
    /// If the text is empty, or names a project that does not exist. An unknown label is
    /// not an error: it becomes a new label.
    pub fn add_task(&self, text: &str) -> Result<Change> {
        if text.trim().is_empty() {
            return Err(LumennaError::new("nothing to add"));
        }
        let now = Zoned::now();
        self.told(|store| {
            let snapshot = repaired(store);
            let preview =
                parse_quick_add(text, &Known::from_snapshot(&snapshot)).resolve(&snapshot, &now);

            // An unknown project is an error; an unknown label is a new label. A mistyped
            // project would file the task somewhere unexpected, while a label is cheap to make.
            if preview.has_errors() {
                let messages: Vec<&str> =
                    preview.diagnostics.iter().map(|d| d.message.as_str()).collect();
                return Err(LumennaError::new(messages.join("; ")));
            }

            let inbox = snapshot
                .inbox()
                .ok_or_else(|| LumennaError::new("this store has no Inbox"))?;
            let project_id = preview.project.unwrap_or(inbox.id);

            let mut changes = Vec::new();
            let mut labels: BTreeSet<LabelId> = preview.labels.iter().copied().collect();
            let mut order = order_after(snapshot.labels.values().map(|l| l.order.clone()));
            for name in &preview.new_labels {
                let label = Label::new(name, order.clone());
                order = OrderKey::after(&order);
                labels.insert(label.id);
                changes.extend(edit::create_label(label).changes);
            }

            let mut task = Task::new(
                project_id,
                &preview.title,
                order_after(
                    snapshot
                        .tasks
                        .values()
                        .filter(|t| t.project_id == project_id && t.parent_id.is_none())
                        .map(|t| t.order.clone()),
                ),
            );
            task.due = preview.due.clone();
            task.priority = preview.priority;
            task.estimate_mins = preview.estimate_mins;
            task.labels = labels;
            let task_id = task.id;
            changes.extend(edit::create_task(task).changes);

            let change = Edit { description: format!("Added {}", preview.title), changes };
            store.apply_recorded(&change)?;

            // The announcement stands in for the inline highlighting a sighted user gets as
            // they type — and always names the resolved date, since "Friday" is the
            // ambiguous part and confirming the phrase back would confirm nothing.
            let mut result = Change::announced(preview.announcement(), &change);
            // Read after applying: labels created alongside it are only nameable once they
            // exist.
            let after = repaired(store);
            result.task = after.tasks.get(&task_id).map(|t| TaskDetail::of(t, &after, &now));
            for diagnostic in &preview.diagnostics {
                if diagnostic.severity == Severity::Notice {
                    result = result.note(diagnostic.message.clone());
                }
            }
            Ok(result)
        })
    }

    /// Tasks matching a filter query, or every open task for an empty one.
    ///
    /// [`Rows::query`] says how the query was understood, and names anything in it that
    /// matched nothing.
    ///
    /// # Errors
    ///
    /// If the query cannot be read; the message says where and what was expected.
    pub fn list_tasks(&self, query: &str) -> Result<Rows> {
        let now = Zoned::now();
        self.told(|store| {
            let snapshot = repaired(store);
            let expr = resolve::query(&snapshot, query)?;
            let cx = Context::new(&snapshot, &now);

            let unresolved: Vec<Unresolved> = expr
                .unresolved(&snapshot)
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

            let mut rows = Rows::new(&snapshot.task_rows(&expr, &cx), "task");
            if !query.trim().is_empty() {
                rows.query = Some(Query {
                    text: query.to_owned(),
                    description: expr.describe(),
                    unresolved,
                });
            }
            rows.notices = notices;
            rows.empty = match query.trim() {
                "" => "No open tasks.".to_owned(),
                "deleted" => "The trash is empty.".to_owned(),
                _ => "No tasks match this filter.".to_owned(),
            };
            crate::actions::fill_rows(&mut rows, &snapshot);
            Ok(rows)
        })
    }

    /// Tasks whose title or notes contain `text`.
    ///
    /// # Errors
    ///
    /// If there is nothing to search for.
    pub fn search_tasks(&self, text: &str) -> Result<Rows> {
        if text.trim().is_empty() {
            return Err(LumennaError::new("nothing to search for"));
        }
        let now = Zoned::now();
        self.told(|store| {
            let snapshot = repaired(store);
            let expr = Expr::Predicate(Predicate::Search(text.to_owned()));
            let cx = Context::new(&snapshot, &now);
            let mut rows = Rows::new(&snapshot.task_rows(&expr, &cx), "task");
            rows.empty = format!("No task has {text} in it.");
            crate::actions::fill_rows(&mut rows, &snapshot);
            Ok(rows)
        })
    }

    /// Everything about one task.
    ///
    /// # Errors
    ///
    /// If no task matches `id`.
    pub fn show_task(&self, id: &str) -> Result<TaskShown> {
        let now = Zoned::now();
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, id)?;
            let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
            let task = TaskDetail::of(task, &snapshot, &now);
            Ok(TaskShown { announcement: task.title.clone(), notices: Vec::new(), task })
        })
    }

    /// Changes the fields of a task that `edit` sets.
    ///
    /// A new project moves the task the way [`move_task`](Self::move_task) does, so its
    /// subtasks follow.
    ///
    /// # Errors
    ///
    /// If no task matches `id`, or a field cannot be read.
    pub fn edit_task(&self, id: &str, edit: TaskEdit) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, id)?;
            let before =
                snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?.clone();
            let mut after = before.clone();

            if let Some(title) = edit.title {
                after.title = title;
            }
            if let Some(due) = &edit.due {
                let kept = after.due.as_ref().and_then(|d| d.recurrence.clone());
                after.due = resolve::due(due, &now)?;
                // A new date that names no repetition moves the task; it does not stop it
                // repeating. `none`, or `repeat: none`, is how that is said.
                if let Some(due) = after.due.as_mut()
                    && due.recurrence.is_none()
                {
                    due.recurrence = kept;
                }
            }
            if let Some(repeat) = &edit.repeat {
                if repeat.trim().eq_ignore_ascii_case("none") {
                    if let Some(due) = after.due.as_mut() {
                        due.recurrence = None;
                    }
                } else {
                    let (spec, from_completion) = resolve::repetition(repeat)?;
                    let recurrence = Recurrence { rrule: spec.to_rrule(), from_completion };
                    match after.due.as_mut() {
                        Some(due) => due.recurrence = Some(recurrence),
                        None => {
                            // Due on the first day it lands on, as quick add does with
                            // "every monday" alone.
                            let first = lumenna_core::recur::Rule::parse(&recurrence.rrule)?
                                .first_from(now.date())?
                                .ok_or_else(|| {
                                    LumennaError::new(format!("'{repeat}' never happens"))
                                })?;
                            after.due = Some(Due {
                                date: first,
                                time: None,
                                timezone: None,
                                recurrence: Some(recurrence),
                            });
                        }
                    }
                }
            }
            if let Some(priority) = edit.priority {
                after.priority = Priority::from_u8(priority);
            }
            if let Some(estimate) = &edit.estimate {
                after.estimate_mins = if estimate.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(resolve::minutes(estimate)?)
                };
            }
            if let Some(notes) = edit.notes {
                after.notes = notes;
            }
            // Names that are not labels yet become labels, as in quick add, and are
            // created in the same edit so one undo takes both back.
            let mut created = Vec::new();
            let mut notices = Vec::new();
            if let Some(names) = &edit.labels {
                let mut order = order_after(snapshot.labels.values().map(|l| l.order.clone()));
                let mut wearing = BTreeSet::new();
                for name in names.iter().map(|n| n.trim().trim_start_matches('@')).filter(|n| !n.is_empty()) {
                    if let Some(label) = snapshot.label_by_name(name) {
                        wearing.insert(label.id);
                    } else {
                        let label = Label::new(name, order.clone());
                        order = OrderKey::after(&order);
                        wearing.insert(label.id);
                        notices.push(format!("new label '{name}'"));
                        created.extend(edit::create_label(label).changes);
                    }
                }
                after.labels = wearing;
            }
            // A new project goes through the move, so subtasks follow and a parent left
            // behind is let go of. The other fields are laid over the task's half of it.
            let moved = match &edit.project {
                Some(project) => {
                    let project = resolve::project(&snapshot, project)?.id;
                    Some(edit::move_task(&snapshot, id, MoveTo::Project(project))?)
                }
                None => None,
            };
            let change = match moved.filter(|m| !m.is_empty()) {
                Some(mut moved) => {
                    for change in &mut moved.changes {
                        if let edit::Change::Task(transition) = change
                            && let Some(task) = transition.after.as_mut()
                            && task.id == id
                        {
                            let (project, parent) = (task.project_id, task.parent_id);
                            *task = after.clone();
                            task.project_id = project;
                            task.parent_id = parent;
                        }
                    }
                    moved.description = format!("Edited {}", after.title);
                    moved
                }
                None => edit::update_task(before, after),
            };
            let change = if created.is_empty() || change.is_empty() {
                change
            } else {
                Edit { description: change.description, changes: [created, change.changes].concat() }
            };
            let mut result = record_or(store, &change, "nothing changed")?;
            if result.changed {
                result.notices = notices;
            }
            Ok(result)
        })
    }

    /// Marks a task done, cascading to its subtasks per the setting and moving a
    /// recurring one to its next date.
    ///
    /// # Errors
    ///
    /// If no task matches `id`.
    pub fn complete_task(&self, id: &str) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            let snapshot = repaired(store);
            let change = edit::complete_task(&snapshot, resolve::task_id(&snapshot, id)?, &now)?;
            record(store, &change)
        })
    }

    /// Takes back a task's most recent completion.
    ///
    /// # Errors
    ///
    /// If no task matches `id`, or it has no completion to take back.
    pub fn uncomplete_task(&self, id: &str) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let change = edit::uncomplete_task(&snapshot, resolve::task_id(&snapshot, id)?)?;
            record(store, &change)
        })
    }

    /// Moves a task, and its subtasks, to the trash — recoverable with
    /// [`restore_task`](Self::restore_task).
    ///
    /// # Errors
    ///
    /// If no task matches `id`.
    pub fn trash_task(&self, id: &str) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let change = edit::trash_task(&snapshot, resolve::task_id(&snapshot, id)?)?;
            record(store, &change)
        })
    }

    /// Takes a task back out of the trash.
    ///
    /// # Errors
    ///
    /// If no task matches `id`.
    pub fn restore_task(&self, id: &str) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let change = edit::restore_task(&snapshot, resolve::task_id(&snapshot, id)?)?;
            record(store, &change)
        })
    }

    /// Deletes a task outright rather than moving it to the trash, so a client asks first —
    /// nothing here can. Undo brings it back, and its content stays in the document's history
    /// and in backups until a real erasure exists.
    ///
    /// # Errors
    ///
    /// If no task matches `id`.
    pub fn erase_task(&self, id: &str) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let change = edit::purge_task(&snapshot, resolve::task_id(&snapshot, id)?)?;
            record(store, &change)
        })
    }

    /// Moves a task under another, into a project, or to the top of its project.
    ///
    /// # Errors
    ///
    /// If either task cannot be found, the project does not exist, or the move would put a
    /// task under itself.
    pub fn move_task(&self, id: &str, to: MoveTarget) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, id)?;
            let destination = match to {
                MoveTarget::Parent { id } => {
                    MoveTo::Parent(Some(resolve::task_id(&snapshot, &id)?))
                }
                MoveTarget::Project { name } => {
                    MoveTo::Project(resolve::project(&snapshot, &name)?.id)
                }
                MoveTarget::Top => MoveTo::Parent(None),
            };
            let change = edit::move_task(&snapshot, id, destination)?;
            record_or(store, &change, "it is already there")
        })
    }

    /// Says that `id` cannot start until `on` is done.
    ///
    /// # Errors
    ///
    /// If either task cannot be found, or the dependency would close a cycle.
    pub fn add_dependency(&self, id: &str, on: &str) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, id)?;
            let dependency = resolve::task_id(&snapshot, on)?;
            if id == dependency {
                return Err(LumennaError::new("a task cannot wait for itself"));
            }
            // A local edge that closes a cycle of any length is a mistake this device can
            // see, even though merge can still produce one and `repair` exists for that.
            // Core refuses it, so every client does.
            let change = edit::add_dependency(&snapshot, id, dependency)?;
            record_or(store, &change, "nothing changed")
        })
    }

    /// Takes away a dependency.
    ///
    /// # Errors
    ///
    /// If either task cannot be found.
    pub fn remove_dependency(&self, id: &str, on: &str) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, id)?;
            let dependency = resolve::task_id(&snapshot, on)?;
            let change = edit::remove_dependency(&snapshot, id, dependency)?;
            record_or(store, &change, "nothing changed")
        })
    }
}
