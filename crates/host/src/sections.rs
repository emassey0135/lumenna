//! Sections: the headings a project's tasks sit under.

use lumenna_core::edit::{self, MoveTo, SectionDeletion};
use lumenna_core::model::Section;
use lumenna_core::row::{Role, Row, RowId};

use crate::error::{LumennaError, Result};
use crate::resolve::{self, order_after};
use crate::tasks::{record, record_or};
use crate::types::{Change, Direction, Rows};
use crate::{Lumenna, named, repaired};

impl Lumenna {
    /// Adds a section to a project, after its others.
    ///
    /// # Errors
    ///
    /// If there is no project by that name, the name is empty, or the project already has a
    /// section by it.
    pub fn add_section(&self, project: &str, name: &str) -> Result<Change> {
        named("section", "a name", name)?;
        self.told(|store| {
            let snapshot = repaired(store);
            let project = resolve::project(&snapshot, project)?;
            let name = name.trim().trim_start_matches('/');
            let sections = snapshot.sections_of(project.id);
            if sections.iter().any(|s| lumenna_core::suggest::same_name(&s.name, name)) {
                return Err(LumennaError::new(format!("{} already has a section '{name}'", project.name)));
            }
            let section = Section::new(project.id, name, order_after(sections.iter().map(|s| s.order.clone())));
            record(store, &edit::create_section(section))
        })
    }

    /// A project's sections, in order, each with how many open tasks are under it.
    ///
    /// # Errors
    ///
    /// If there is no project by that name.
    pub fn list_sections(&self, project: &str) -> Result<Rows> {
        self.told(|store| {
            let snapshot = repaired(store);
            let project = resolve::project(&snapshot, project)?;
            let facts = snapshot.facts();
            let sections = snapshot.sections_of(project.id);
            let count = u32::try_from(sections.len()).unwrap_or(u32::MAX);
            let rows: Vec<Row> = sections
                .iter()
                .enumerate()
                .map(|(index, section)| {
                    let open = snapshot
                        .tasks
                        .values()
                        .filter(|t| !t.is_deleted() && !facts.is_completed(t))
                        .filter(|t| snapshot.section_of(t).is_some_and(|s| s.id == section.id))
                        .count();
                    Row {
                        id: RowId::Section(section.id),
                        role: Role::Section,
                        depth: 0,
                        index: u32::try_from(index).unwrap_or(u32::MAX) + 1,
                        count,
                        expanded: None,
                        checked: None,
                        title: section.name.clone(),
                        state: Vec::new(),
                        due: None,
                        due_time: None,
                        value: Some(crate::words::count_line(open, "open task")),
                        hint: None,
                        priority: None,
                    }
                })
                .collect();
            let mut listed = Rows::new(&rows, "section");
            listed.empty = format!("{} has no sections.", project.name);
            crate::actions::fill_rows(&mut listed, &snapshot);
            Ok(listed)
        })
    }

    /// Renames a section.
    ///
    /// # Errors
    ///
    /// If the section cannot be found, the name is empty, or its project has another by it.
    pub fn rename_section(&self, section: &str, to: &str) -> Result<Change> {
        named("section", "a name", to)?;
        self.told(|store| {
            let snapshot = repaired(store);
            let before = resolve::section(&snapshot, section, None)?.clone();
            let to = to.trim().trim_start_matches('/');
            if snapshot
                .sections_of(before.project_id)
                .iter()
                .any(|s| s.id != before.id && lumenna_core::suggest::same_name(&s.name, to))
            {
                return Err(LumennaError::new(format!("there is already a section '{to}' in this project")));
            }
            let mut after = before.clone();
            after.name = to.to_owned();
            record_or(store, &edit::update_section(before, after), "it already has that name")
        })
    }

    /// Moves a section one place up or down among its project's.
    ///
    /// # Errors
    ///
    /// If the section cannot be found.
    pub fn reorder_section(&self, section: &str, direction: Direction) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let before = resolve::section(&snapshot, section, None)?.clone();
            let siblings: Vec<_> = snapshot.sections_of(before.project_id).iter().map(|s| (s.id, s.order.clone())).collect();
            let Some(order) = crate::organise::moved(&siblings, before.id, direction)? else {
                return Ok(Change::unchanged(crate::organise::edge(direction)));
            };
            let mut after = before.clone();
            after.order = order;
            record(store, &edit::update_section(before, after))
        })
    }

    /// Deletes a section, its tasks with it or kept in the project under no section.
    ///
    /// # Errors
    ///
    /// If the section cannot be found.
    pub fn delete_section(&self, section: &str, keep_tasks: bool) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::section(&snapshot, section, None)?.id;
            let tasks = if keep_tasks { SectionDeletion::KeepTasks } else { SectionDeletion::TrashTasks };
            record(store, &edit::trash_section(&snapshot, id, tasks)?)
        })
    }

    /// Puts a task under a section, joining the section's project, subtasks and all; or, with
    /// `None`, out of the section it is under.
    ///
    /// # Errors
    ///
    /// If the task or section cannot be found.
    pub fn move_task_to_section(&self, task: &str, section: Option<&str>) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, task)?;
            let project = snapshot.tasks.get(&id).map(|t| t.project_id.to_string());
            let named = section.map(str::trim).filter(|s| !s.is_empty() && *s != lumenna_records::choices::NO_SECTION);
            let section = match named {
                // Named alone, a section of the task's own project first.
                Some(name) => Some(
                    resolve::section(&snapshot, name, project.as_deref())
                        .or_else(|_| resolve::section(&snapshot, name, None))?
                        .id,
                ),
                None => None,
            };
            let change = edit::move_task(&snapshot, id, MoveTo::Section(section))?;
            let unchanged = if section.is_some() { "it is already under that section" } else { "it is under no section" };
            record_or(store, &change, unchanged)
        })
    }

    /// Adds a task from a quick-add line under a section: in its project and under it, unless
    /// the line names a project or a section of its own.
    ///
    /// # Errors
    ///
    /// As [`Lumenna::add_task`], and if the section cannot be found.
    pub fn add_task_in_section(&self, section: &str, text: &str) -> Result<Change> {
        let id = self.with(|store| Ok(resolve::section(&repaired(store), section, None)?.id))?;
        self.add_task_into(text, Some(id))
    }
}
