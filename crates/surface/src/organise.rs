//! Projects, labels, and saved filters.

use std::collections::{BTreeMap, BTreeSet};

use lumenna_core::edit::{self, EditError, ProjectDeletion};
use lumenna_core::id::{FilterId, ProjectId};
use lumenna_core::model::{Label, Project, SavedFilter};
use lumenna_core::order::OrderKey;
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::resolve::{self, order_after};
use crate::tasks::{record, record_or};
use crate::types::{Announced, Change, Direction, FilterView, Filters, Rows, Weight};
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
        named("project", name)?;
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
            let live = in_tree_order(&snapshot);
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
            Ok(rows)
        })
    }

    /// Renames a project.
    ///
    /// # Errors
    ///
    /// If there is no project by the first name, or already one by the second.
    pub fn rename_project(&self, name: &str, to: &str) -> Result<Change> {
        named("project", to)?;
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

    /// Sets a project's urgency multiplier, or has it inherit its parent's again.
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
        named("label", name)?;
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
                        value: Some(value),
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
        named("label", to)?;
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
    /// evaluation time.
    ///
    /// # Errors
    ///
    /// If the query cannot be read.
    pub fn add_filter(&self, name: &str, query: &str) -> Result<Change> {
        named("saved filter", name)?;
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

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Moves a project under another, or to the top level when `parent` is absent.
    ///
    /// # Errors
    ///
    /// If either project does not exist, or the move would put a project inside itself.
    pub fn move_project(&self, name: &str, parent: Option<String>) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::project(&snapshot, name)?.clone();
            let parent = parent.map(|p| resolve::project(&snapshot, &p).map(|p| p.id)).transpose()?;
            // A local move that closes a cycle is a mistake this device can see; merge can
            // still make one, which `repair` handles.
            let mut ancestor = parent;
            while let Some(id) = ancestor {
                if id == before.id {
                    return Err(EditError::WouldCycle.into());
                }
                ancestor = snapshot.projects.get(&id).and_then(|p| p.parent_id);
            }
            let mut after = before.clone();
            after.parent_id = parent;
            after.order = order_after(
                snapshot
                    .projects
                    .values()
                    .filter(|p| p.parent_id == parent && p.id != before.id)
                    .map(|p| p.order.clone()),
            );
            let change = edit::update_project(before, after);
            record_or(store, &change, "it is already there")
        })
    }

    /// Moves a project one place up or down among its siblings.
    ///
    /// # Errors
    ///
    /// If there is no project by that name.
    pub fn reorder_project(&self, name: &str, direction: Direction) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::project(&snapshot, name)?.clone();
            let siblings: Vec<(lumenna_core::id::ProjectId, OrderKey)> = snapshot
                .projects
                .values()
                .filter(|p| p.deleted_at.is_none() && p.parent_id == before.parent_id)
                .map(|p| (p.id, p.order.clone()))
                .collect();
            let Some(order) = moved(&siblings, before.id, direction)? else {
                return Ok(Change::unchanged(edge(direction)));
            };
            let mut after = before.clone();
            after.order = order;
            record(store, &edit::update_project(before, after))
        })
    }

    /// Moves a label one place up or down.
    ///
    /// # Errors
    ///
    /// If there is no label by that name.
    pub fn reorder_label(&self, name: &str, direction: Direction) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::label(&snapshot, name)?.clone();
            let siblings: Vec<_> = snapshot
                .labels
                .values()
                .filter(|l| l.deleted_at.is_none())
                .map(|l| (l.id, l.order.clone()))
                .collect();
            let Some(order) = moved(&siblings, before.id, direction)? else {
                return Ok(Change::unchanged(edge(direction)));
            };
            let mut after = before.clone();
            after.order = order;
            record(store, &edit::update_label(before, after))
        })
    }

    /// Gives a label a colour by name — `red`, `teal` — or takes it away. A colour is never
    /// the only thing that says which label is which: the name always shows.
    ///
    /// # Errors
    ///
    /// If there is no label by that name.
    pub fn recolour_label(&self, name: &str, colour: Option<String>) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = resolve::label(&snapshot, name)?.clone();
            let mut after = before.clone();
            after.color = colour.map(|c| c.trim().to_lowercase()).filter(|c| !c.is_empty());
            let change = edit::update_label(before, after);
            record_or(store, &change, "it is already that colour")
        })
    }

    /// Renames a saved filter or changes its query; `None` leaves either alone.
    ///
    /// # Errors
    ///
    /// If there is no filter by that name, another already has the new one, or the new query
    /// cannot be read.
    pub fn edit_filter(
        &self,
        name: &str,
        rename: Option<String>,
        query: Option<String>,
    ) -> Result<Change> {
        if let Some(to) = &rename {
            named("saved filter", to)?;
        }
        self.with(|store| {
            let snapshot = repaired(store);
            let before = live_filter(&snapshot, name)?.clone();
            let mut after = before.clone();
            if let Some(rename) = rename {
                if snapshot.saved_filters.values().any(|f| {
                    f.deleted_at.is_none() && f.id != before.id && f.name.eq_ignore_ascii_case(&rename)
                }) {
                    return Err(LumennaError::new(format!("there is already a filter '{rename}'")));
                }
                after.name = rename;
            }
            let mut readback = None;
            if let Some(query) = query {
                // Checked now, stored as text.
                readback = Some(resolve::query(&snapshot, &query)?.describe());
                after.query = query;
            }
            let change = edit::update_filter(before, after);
            if change.is_empty() {
                return Ok(Change::unchanged("nothing changed"));
            }
            store.apply_recorded(&change)?;
            let result = Change::of(&change);
            Ok(match readback {
                Some(understood) => result.note(understood),
                None => result,
            })
        })
    }

    /// Moves a saved filter one place up or down.
    ///
    /// # Errors
    ///
    /// If there is no filter by that name.
    pub fn reorder_filter(&self, name: &str, direction: Direction) -> Result<Change> {
        self.with(|store| {
            let snapshot = repaired(store);
            let before = live_filter(&snapshot, name)?.clone();
            let siblings: Vec<_> = snapshot
                .saved_filters
                .values()
                .filter(|f| f.deleted_at.is_none())
                .map(|f| (f.id, f.order.clone()))
                .collect();
            let Some(order) = moved(&siblings, before.id, direction)? else {
                return Ok(Change::unchanged(edge(direction)));
            };
            let mut after = before.clone();
            after.order = order;
            record(store, &edit::update_filter(before, after))
        })
    }
}

fn live_filter<'a>(snapshot: &'a Snapshot, name: &str) -> Result<&'a SavedFilter> {
    snapshot
        .saved_filters
        .values()
        .find(|f| f.deleted_at.is_none() && f.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| LumennaError::new(format!("no filter called '{name}'")))
}

const fn edge(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "it is already first",
        Direction::Down => "it is already last",
    }
}

/// The order key that puts `id` one place up or down among `siblings`, or `None` at the end
/// of the list. Only the moved record gets a new key — a single last-writer-wins string, so a
/// concurrent reorder elsewhere merges rather than fighting over a list.
fn moved<I: Ord + Copy>(
    siblings: &[(I, OrderKey)],
    id: I,
    direction: Direction,
) -> Result<Option<OrderKey>> {
    let mut sorted: Vec<&(I, OrderKey)> = siblings.iter().collect();
    sorted.sort_by(|a, b| a.1.cmp_with(&a.0, &b.1, &b.0));
    let Some(at) = sorted.iter().position(|(sibling, _)| *sibling == id) else {
        return Ok(None);
    };
    let (before, after) = match direction {
        Direction::Up if at == 0 => return Ok(None),
        Direction::Up => (at.checked_sub(2).map(|i| &sorted[i].1), Some(&sorted[at - 1].1)),
        Direction::Down if at + 1 == sorted.len() => return Ok(None),
        Direction::Down => (Some(&sorted[at + 1].1), sorted.get(at + 2).map(|s| &s.1)),
    };
    OrderKey::between(before, after)
        .map(Some)
        .map_err(|e| LumennaError::new(e.to_string()))
}

/// How deep a project sits, stopping at a cycle merge may have made.
/// Live projects with their depths, each followed by its own subprojects, siblings in their
/// order. A list sorted by order alone puts a subproject wherever its key falls, which in a
/// list that shows depth reads as a child of whatever is above it. A project whose parent is
/// gone is at the top.
fn in_tree_order(snapshot: &Snapshot) -> Vec<(&Project, u32)> {
    let live: Vec<&Project> = snapshot.projects.values().filter(|p| p.deleted_at.is_none()).collect();
    let ids: BTreeSet<ProjectId> = live.iter().map(|p| p.id).collect();
    let mut children: BTreeMap<Option<ProjectId>, Vec<&Project>> = BTreeMap::new();
    for project in &live {
        let parent = project.parent_id.filter(|id| ids.contains(id) && *id != project.id);
        children.entry(parent).or_default().push(project);
    }
    for siblings in children.values_mut() {
        siblings.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
    }
    let mut ordered = Vec::with_capacity(live.len());
    let mut seen = BTreeSet::new();
    let mut stack: Vec<(&Project, u32)> =
        children.get(&None).into_iter().flatten().rev().map(|p| (*p, 0)).collect();
    while let Some((project, depth)) = stack.pop() {
        if !seen.insert(project.id) {
            continue;
        }
        ordered.push((project, depth));
        if let Some(under) = children.get(&Some(project.id)) {
            stack.extend(under.iter().rev().map(|p| (*p, depth + 1)));
        }
    }
    // A cycle that repair has not broken yet is unreachable from the top; list it anyway.
    for project in live {
        if seen.insert(project.id) {
            ordered.push((project, 0));
        }
    }
    ordered
}

/// Refuses a blank name: nothing could be listed, said or typed to reach it.
fn named(kind: &str, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(LumennaError::new(format!("a {kind} needs a name")));
    }
    Ok(())
}
