//! The task form (§16.1: task detail / edit): one task's fields as text, and what saving them
//! sends.

use lumenna_surface::{TaskDetail, TaskEdit};

/// A task's editable fields, as the form shows them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskFields {
    pub title: String,
    /// A date phrase the core reads back in: `2026-10-09 14:00`.
    pub due: String,
    /// In the words quick add takes, `every monday`.
    pub repeat: String,
    /// 1 to 4, where 1 is highest.
    pub priority: u8,
    /// `45m`.
    pub estimate: String,
    /// By name.
    pub project: String,
    /// Names separated by commas.
    pub labels: String,
    pub notes: String,
}

impl TaskFields {
    /// The fields as a task has them.
    pub fn of(task: &TaskDetail) -> Self {
        Self {
            title: task.title.clone(),
            due: [task.due.as_deref(), task.due_time.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" "),
            repeat: task.repetition.clone().unwrap_or_default(),
            priority: task.priority,
            estimate: task.estimate_mins.map(|m| format!("{m}m")).unwrap_or_default(),
            project: task.project.clone().unwrap_or_default(),
            labels: task.labels.join(", "),
            notes: task.notes.clone(),
        }
    }

    /// What saving these fields over `task` sends, or `None` if nothing differs.
    ///
    /// **Only the fields that changed.** Each field sent is written, and each write wins a
    /// last-write-wins race against a concurrent edit from another device — so sending what
    /// the form merely happened to show would silently revert someone else's change.
    pub fn edit(&self, task: &TaskDetail) -> Option<TaskEdit> {
        let before = Self::of(task);
        if *self == before {
            return None;
        }
        let changed = |now: &String, was: &String| now.trim() != was.trim();
        // Cleared, a field means "none". Except repetition by a rule the grammar cannot say:
        // that field started empty, and empty still means leave it alone.
        let cleared = |now: &String| if now.trim().is_empty() { "none".to_owned() } else { now.trim().to_owned() };
        let edit = TaskEdit {
            title: changed(&self.title, &before.title).then(|| self.title.trim().to_owned()),
            due: changed(&self.due, &before.due).then(|| cleared(&self.due)),
            repeat: changed(&self.repeat, &before.repeat).then(|| cleared(&self.repeat)),
            priority: (self.priority != before.priority).then_some(self.priority),
            estimate: changed(&self.estimate, &before.estimate).then(|| cleared(&self.estimate)),
            // Notes are kept exactly as typed, spacing and all.
            notes: (self.notes != before.notes).then(|| self.notes.clone()),
            // A task is always in some project; clearing the name is not a move.
            project: (changed(&self.project, &before.project) && !self.project.trim().is_empty())
                .then(|| self.project.trim().to_owned()),
            labels: (label_names(&self.labels) != label_names(&before.labels))
                .then(|| label_names(&self.labels)),
        };
        (edit != TaskEdit::default()).then_some(edit)
    }
}

/// Label names from the comma-separated field, without a leading `@` someone typed out of
/// habit from quick add.
fn label_names(text: &str) -> Vec<String> {
    text.split(',')
        .map(|name| name.trim().trim_start_matches('@').trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> TaskDetail {
        TaskDetail {
            id: "0199".to_owned(),
            title: "Write report".to_owned(),
            notes: "Two pages.".to_owned(),
            project: Some("Work".to_owned()),
            parent: None,
            priority: 2,
            labels: vec!["desk".to_owned(), "deep".to_owned()],
            depends: Vec::new(),
            due: Some("2026-10-09".to_owned()),
            due_time: Some("14:00".to_owned()),
            recurrence: None,
            repetition: None,
            estimate_mins: Some(45),
            state: vec!["ready".to_owned()],
            created_at: "2026-10-01T09:00:00Z".to_owned(),
        }
    }

    #[test]
    fn the_fields_read_back_as_the_core_takes_them_in() {
        let fields = TaskFields::of(&task());
        assert_eq!(fields.due, "2026-10-09 14:00");
        assert_eq!(fields.estimate, "45m");
        assert_eq!(fields.labels, "desk, deep");
    }

    #[test]
    fn saving_unchanged_fields_sends_nothing() {
        assert_eq!(TaskFields::of(&task()).edit(&task()), None);
    }

    #[test]
    fn only_the_fields_that_changed_are_sent() {
        let mut fields = TaskFields::of(&task());
        fields.title = "Write the report".to_owned();
        fields.priority = 1;
        assert_eq!(
            fields.edit(&task()),
            Some(TaskEdit { title: Some("Write the report".to_owned()), priority: Some(1), ..TaskEdit::default() })
        );
    }

    #[test]
    fn a_cleared_field_says_none() {
        let mut fields = TaskFields::of(&task());
        fields.due.clear();
        fields.estimate = "  ".to_owned();
        let edit = fields.edit(&task()).unwrap();
        assert_eq!(edit.due.as_deref(), Some("none"));
        assert_eq!(edit.estimate.as_deref(), Some("none"));
    }

    #[test]
    fn spacing_around_a_field_is_not_a_change() {
        let mut fields = TaskFields::of(&task());
        fields.title = " Write report ".to_owned();
        fields.labels = "desk,deep".to_owned();
        assert_eq!(fields.edit(&task()), None);
    }

    #[test]
    fn labels_are_names_without_their_sigil() {
        let mut fields = TaskFields::of(&task());
        fields.labels = "desk, @calls".to_owned();
        assert_eq!(fields.edit(&task()).unwrap().labels, Some(vec!["desk".to_owned(), "calls".to_owned()]));
    }

    #[test]
    fn clearing_the_project_does_not_move_the_task() {
        let mut fields = TaskFields::of(&task());
        fields.project.clear();
        assert_eq!(fields.edit(&task()), None);
    }
}
