//! Projects, labels, and saved filters.

use std::collections::BTreeSet;

use lumenna_core::edit::{self, ProjectDeletion};
use lumenna_core::id::FilterId;
use lumenna_core::model::{Label, Project, SavedFilter};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::resolve::{self, order_after};
use crate::tasks::{record, record_or};
use crate::types::{Announced, Change, FilterView, Filters, Rows, Weight};
use crate::words::count_line;
use crate::{Lumenna, repaired};

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    // -----------------------------------------------------------------------------------
    // Projects
    // -----------------------------------------------------------------------------------

    /// Adds a project, at the top level or under `parent`.
    ///
    /// # Errors
    ///
    /// If there is already a project by that name, or no parent by that one.
    pub fn add_project(&self, name: &str, parent: Option<String>) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            if snapshot.project_by_name(name).is_some() {
                return Err(LumennaError::new(format!("there is already a project '{name}'")));
            }
            let mut project =
                Project::new(name, order_after(snapshot.projects.values().map(|p| p.order.clone())));
            if let Some(parent) = parent {
                project.parent_id = Some(resolve::project(&snapshot, &parent)?.id);
            }
            record(store, &edit::create_project(project))
        })
    }

    /// Every project, in order, with how many tasks each holds.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn list_projects(&self) -> Result<Rows> {
        self.with(|store| {
            let snapshot = repaired(store);
            let mut live: Vec<&Project> =
                snapshot.projects.values().filter(|p| p.deleted_at.is_none()).collect();
            live.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
            let count = to_u32(live.len());
            let rows: Vec<Row> = live
                .iter()
                .enumerate()
                .map(|(index, project)| {
                    let tasks = snapshot
                        .tasks
                        .values()
                        .filter(|t| t.project_id == project.id && !t.is_deleted())
                        .count();
                    let mut value = count_line(tasks, "task");
                    let weight = snapshot.effective_weight(project.id);
                    if (weight - Project::NEUTRAL_WEIGHT).abs() > f32::EPSILON {
                        value.push_str(&format!(", weight {weight}"));
                    }
                    if project.archived {
                        value.push_str(", archived");
                    }
                    Row {
                        id: RowId::Project(project.id),
                        role: Role::Project,
                        depth: depth_of(&snapshot, project),
                        index: to_u32(index) + 1,
                        count,
                        expanded: None,
                        checked: None,
                        title: project.name.clone(),
                        state: Vec::new(),
                        value: Some(value),
                        hint: None,
                    }
                })
                .collect();
            Ok(Rows::new(&rows, "project"))
        })
    }

    /// Renames a project.
    ///
    /// # Errors
    ///
    /// If there is no project by the first name, or already one by the second.
    pub fn rename_project(&self, name: &str, to: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::project(&snapshot, name)?.clone();
            if snapshot.project_by_name(to).is_some_and(|other| other.id != before.id) {
                return Err(LumennaError::new(format!("there is already a project '{to}'")));
            }
            let mut after = before.clone();
            after.name = to.to_owned();
            let change = edit::update_project(before, after);
            store.apply_recorded(&change)?;
            Ok(Change::announced(format!("Renamed {name} to {to}"), &change))
        })
    }

    /// Archives a project, keeping its tasks out of active views — or unarchives it, if it
    /// already was.
    ///
    /// # Errors
    ///
    /// If there is no project by that name.
    pub fn archive_project(&self, name: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::project(&snapshot, name)?.clone();
            let mut after = before.clone();
            after.archived = !before.archived;
            let archived = after.archived;
            let change = edit::update_project(before, after);
            store.apply_recorded(&change)?;
            Ok(Change::announced(
                if archived { format!("Archived {name}") } else { format!("Unarchived {name}") },
                &change,
            ))
        })
    }

    /// Deletes a project. Its tasks go to the trash with it, or to the Inbox when
    /// `keep_tasks` is set.
    ///
    /// # Errors
    ///
    /// If there is no project by that name, or it is the Inbox.
    pub fn delete_project(&self, name: &str, keep_tasks: bool) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let target = resolve::project(&snapshot, name)?;
            if target.is_inbox {
                return Err(LumennaError::new("the Inbox cannot be deleted"));
            }
            let disposition = if keep_tasks {
                ProjectDeletion::MoveTasksToInbox
            } else {
                ProjectDeletion::TrashTasks
            };
            let change = edit::trash_project(&snapshot, target.id, disposition)?;
            record(store, &change)
        })
    }

    /// Sets a project's urgency multiplier, or has it inherit its parent's again (§3.4).
    ///
    /// This is not a second priority. Task priority is how much one item matters; weight is
    /// how much a whole area matters right now.
    ///
    /// # Errors
    ///
    /// If there is no project by that name, or the weight is not a positive number.
    pub fn weigh_project(&self, name: &str, weight: Weight) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::project(&snapshot, name)?.clone();
            let mut after = before.clone();
            let Weight::Value { value } = weight else {
                let project_id = before.id;
                after.weight = None;
                let change = edit::update_project(before, after);
                if change.is_empty() {
                    return Ok(Change::unchanged("it already inherits its weight"));
                }
                store.apply_recorded(&change)?;
                let inherited = repaired(store).effective_weight(project_id);
                return Ok(Change::announced(
                    format!("{name} now inherits its weight, which is {inherited}"),
                    &change,
                ));
            };
            if !value.is_finite() || value <= 0.0 {
                return Err(LumennaError::new("a weight has to be a positive number"));
            }
            after.weight = Some(value);
            let change = edit::update_project(before, after);
            store.apply_recorded(&change)?;

            let result = Change::announced(format!("{name} now weighs {value}"), &change);
            let (low, high) = Project::WEIGHT_RANGE;
            Ok(if value < low || value > high {
                result.note(format!(
                    "{value} is outside the usual range of {low} to {high}; a wider range \
                     lets one project dominate every ranking"
                ))
            } else {
                result
            })
        })
    }

    // -----------------------------------------------------------------------------------
    // Labels
    // -----------------------------------------------------------------------------------

    /// Adds a label. A leading `@` is ignored.
    ///
    /// # Errors
    ///
    /// If there is already a label by that name.
    pub fn add_label(&self, name: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let name = name.trim_start_matches('@');
            if snapshot.label_by_name(name).is_some() {
                return Err(LumennaError::new(format!("there is already a label '{name}'")));
            }
            let change = edit::create_label(Label::new(
                name,
                order_after(snapshot.labels.values().map(|l| l.order.clone())),
            ));
            record(store, &change)
        })
    }

    /// Every label, in order, with how many tasks wear each.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn list_labels(&self) -> Result<Rows> {
        self.with(|store| {
            let snapshot = repaired(store);
            let mut live: Vec<&Label> =
                snapshot.labels.values().filter(|l| l.deleted_at.is_none()).collect();
            live.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
            let count = to_u32(live.len());
            let rows: Vec<Row> = live
                .iter()
                .enumerate()
                .map(|(index, label)| {
                    let used = snapshot
                        .tasks
                        .values()
                        .filter(|t| t.labels.contains(&label.id) && !t.is_deleted())
                        .count();
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
                        value: Some(count_line(used, "task")),
                        hint: None,
                    }
                })
                .collect();
            Ok(Rows::new(&rows, "label"))
        })
    }

    /// Renames a label. Every task wearing it follows.
    ///
    /// # Errors
    ///
    /// If there is no label by the first name, or already one by the second — which is what
    /// [`merge_labels`](Self::merge_labels) is for.
    pub fn rename_label(&self, name: &str, to: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::label(&snapshot, name)?.clone();
            let to = to.trim_start_matches('@');
            if snapshot.label_by_name(to).is_some_and(|other| other.id != before.id) {
                return Err(LumennaError::new(format!(
                    "there is already a label '{to}'; merge them to fold one into the other"
                )));
            }
            let mut after = before.clone();
            after.name = to.to_owned();
            record(store, &edit::update_label(before, after))
        })
    }

    /// Folds one label into another, for when a typo made a near-duplicate.
    ///
    /// # Errors
    ///
    /// If either label does not exist.
    pub fn merge_labels(&self, from: &str, into: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let loser = resolve::label(&snapshot, from)?.id;
            let winner = resolve::label(&snapshot, into)?.id;
            let change = edit::merge_labels(&snapshot, loser, winner)?;
            record(store, &change)
        })
    }

    /// Deletes a label. Tasks wearing it simply stop showing it.
    ///
    /// # Errors
    ///
    /// If there is no label by that name.
    pub fn delete_label(&self, name: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let target = resolve::label(&snapshot, name)?.id;
            let change = edit::trash_label(&snapshot, target)?;
            store.apply_recorded(&change)?;
            // Worth stating, because it surprises people: the tasks are untouched.
            Ok(Change::announced(
                format!("{}; tasks that wore it are unchanged", change.description),
                &change,
            ))
        })
    }

    // -----------------------------------------------------------------------------------
    // Saved filters
    // -----------------------------------------------------------------------------------

    /// Saves a filter query under a name.
    ///
    /// Checked now, so a broken query is caught here rather than the first time it is used,
    /// but stored as text: a saved filter containing `today` has to mean today at
    /// evaluation time (§6.2).
    ///
    /// # Errors
    ///
    /// If the query cannot be read.
    pub fn add_filter(&self, name: &str, query: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let expr = resolve::query(&snapshot, query)?;
            let change = edit::create_filter(SavedFilter {
                id: FilterId::new(),
                name: name.to_owned(),
                query: query.to_owned(),
                order: order_after(snapshot.saved_filters.values().map(|f| f.order.clone())),
                color: None,
                deleted_at: None,
            });
            store.apply_recorded(&change)?;
            Ok(Change::announced(format!("Saved {name}: {}", expr.describe()), &change))
        })
    }

    /// The saved filters, in order.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn list_filters(&self) -> Result<Filters> {
        self.with(|store| {
            let snapshot = repaired(store);
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
                    })
                    .collect(),
            })
        })
    }

    /// Deletes a saved filter by name.
    ///
    /// # Errors
    ///
    /// If there is no filter by that name.
    pub fn delete_filter(&self, name: &str) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let target = snapshot
                .saved_filters
                .values()
                .find(|f| f.deleted_at.is_none() && f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| LumennaError::new(format!("no filter called '{name}'")))?;
            let change = edit::trash_filter(&snapshot, target.id)?;
            record_or(store, &change, "that filter is already gone")
        })
    }
}

/// How deep a project sits, stopping at a cycle merge may have made (§3.13).
fn depth_of(snapshot: &Snapshot, project: &Project) -> u32 {
    let mut depth = 0;
    let mut current = project.parent_id;
    let mut seen = BTreeSet::from([project.id]);
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        depth += 1;
        current = snapshot.projects.get(&id).and_then(|p| p.parent_id);
    }
    depth
}
