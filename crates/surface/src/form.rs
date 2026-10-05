//! What a client's forms share: a task's fields as text, what saving them sends, and the
//! few phrases every client composes the same way.
//!
//! These are free functions rather than methods on [`Lumenna`](crate::Lumenna) because they
//! need no store — only what an operation already returned — but they are rules all the same,
//! and principle 2 puts rules here rather than in each of eleven clients. The one that
//! matters most is [`task_edit`]: a form that sends a field it did not change wins a
//! last-write-wins race it should have lost, and silently reverts another device's edit.

use lumenna_parse::complete::quoted;

use crate::types::{PlanAssignment, TaskDetail, TaskEdit};
use crate::words::duration;

/// A task's editable fields, as a form shows them: all text, apart from the priority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct TaskFields {
    /// The title.
    pub title: String,
    /// The due date as a phrase the core reads back in: `2026-10-09 14:00`. Empty for none.
    pub due: String,
    /// The repetition in quick-add words, `every monday`. Empty when it does not repeat — or
    /// repeats by a rule the grammar cannot say, which saving then leaves alone.
    pub repeat: String,
    /// 1 to 4, where 1 is highest.
    pub priority: u8,
    /// How long it should take, `45m`. Empty for none.
    pub estimate: String,
    /// The project, by name.
    pub project: String,
    /// Label names, separated by commas.
    pub labels: String,
    /// The notes, exactly as typed.
    pub notes: String,
}

/// A task's fields as a form starts from them.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn task_fields(task: TaskDetail) -> TaskFields {
    TaskFields {
        title: task.title,
        due: [task.due, task.due_time].into_iter().flatten().collect::<Vec<_>>().join(" "),
        repeat: task.repetition.unwrap_or_default(),
        priority: task.priority,
        estimate: task.estimate_mins.map(|m| format!("{m}m")).unwrap_or_default(),
        project: task.project.unwrap_or_default(),
        labels: task.labels.join(", "),
        notes: task.notes,
    }
}

/// What saving `fields` over `task` sends to [`edit_task`](crate::Lumenna::edit_task), or
/// nothing if no field changed.
///
/// **Only the fields that changed.** Each field sent is written, and each write wins a
/// last-write-wins race against a concurrent edit from another device (§3.13) — so sending
/// what the form merely showed would revert someone else's change. Spacing around a field is
/// not a change; a field emptied means "none"; an emptied project is not a move, since a task
/// is always in one; a leading `@` typed on a label name out of habit is dropped.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn task_edit(task: TaskDetail, fields: TaskFields) -> Option<TaskEdit> {
    let before = task_fields(task);
    let changed = |now: &str, was: &str| now.trim() != was.trim();
    let or_none = |now: &str| if now.trim().is_empty() { "none".to_owned() } else { now.trim().to_owned() };
    let edit = TaskEdit {
        title: changed(&fields.title, &before.title).then(|| fields.title.trim().to_owned()),
        due: changed(&fields.due, &before.due).then(|| or_none(&fields.due)),
        repeat: changed(&fields.repeat, &before.repeat).then(|| or_none(&fields.repeat)),
        priority: (fields.priority != before.priority).then_some(fields.priority),
        estimate: changed(&fields.estimate, &before.estimate).then(|| or_none(&fields.estimate)),
        notes: (fields.notes != before.notes).then(|| fields.notes.clone()),
        project: (changed(&fields.project, &before.project) && !fields.project.trim().is_empty())
            .then(|| fields.project.trim().to_owned()),
        labels: (label_names(&fields.labels) != label_names(&before.labels)).then(|| label_names(&fields.labels)),
    };
    (edit != TaskEdit::default()).then_some(edit)
}

fn label_names(text: &str) -> Vec<String> {
    text.split(',')
        .map(|name| name.trim().trim_start_matches('@').trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// A project as a filter or a quick-add line names it: `#Work`, or `#"Home Office"`. What a
/// project's task list queries, and what a task added there starts with.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn project_reference(name: String) -> String {
    quoted("#", &name)
}

/// A label as a filter or a quick-add line names it: `@calls`, or `@"deep work"`.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn label_reference(name: String) -> String {
    quoted("@", &name)
}

/// A sitting's status with its planned length beside it: "planned for 45 minutes" before it
/// starts, then "worked" and "45 minutes planned", so the two never read as "planned,
/// planned".
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn sitting_status(sitting: PlanAssignment) -> Vec<String> {
    match sitting.planned_mins {
        None => vec![sitting.status],
        Some(planned) if sitting.status == "planned" => vec![format!("planned for {}", duration(planned))],
        Some(planned) => vec![sitting.status, format!("{} planned", duration(planned))],
    }
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

    fn sitting(status: &str, planned: Option<u32>) -> PlanAssignment {
        PlanAssignment {
            row: 1,
            id: "a".to_owned(),
            task: "t".to_owned(),
            title: "Write report".to_owned(),
            status: status.to_owned(),
            planned_mins: planned,
            minutes: 0,
            capped: false,
        }
    }

    #[test]
    fn the_fields_read_back_as_the_core_takes_them_in() {
        let fields = task_fields(task());
        assert_eq!(fields.due, "2026-10-09 14:00");
        assert_eq!(fields.estimate, "45m");
        assert_eq!(fields.labels, "desk, deep");
    }

    #[test]
    fn saving_unchanged_fields_sends_nothing() {
        assert_eq!(task_edit(task(), task_fields(task())), None);
    }

    #[test]
    fn only_the_fields_that_changed_are_sent() {
        let fields = TaskFields { title: "Write the report".to_owned(), priority: 1, ..task_fields(task()) };
        assert_eq!(
            task_edit(task(), fields),
            Some(TaskEdit { title: Some("Write the report".to_owned()), priority: Some(1), ..TaskEdit::default() })
        );
    }

    #[test]
    fn a_cleared_field_says_none() {
        let fields = TaskFields { due: String::new(), estimate: "  ".to_owned(), ..task_fields(task()) };
        let edit = task_edit(task(), fields).unwrap();
        assert_eq!(edit.due.as_deref(), Some("none"));
        assert_eq!(edit.estimate.as_deref(), Some("none"));
    }

    #[test]
    fn spacing_around_a_field_is_not_a_change() {
        let fields = TaskFields { title: " Write report ".to_owned(), labels: "desk,deep".to_owned(), ..task_fields(task()) };
        assert_eq!(task_edit(task(), fields), None);
    }

    #[test]
    fn notes_keep_their_spacing() {
        let fields = TaskFields { notes: "Two pages. ".to_owned(), ..task_fields(task()) };
        assert_eq!(task_edit(task(), fields).unwrap().notes.as_deref(), Some("Two pages. "));
    }

    #[test]
    fn labels_are_names_without_their_sigil() {
        let fields = TaskFields { labels: "desk, @calls".to_owned(), ..task_fields(task()) };
        assert_eq!(task_edit(task(), fields).unwrap().labels, Some(vec!["desk".to_owned(), "calls".to_owned()]));
    }

    #[test]
    fn clearing_the_project_does_not_move_the_task() {
        let fields = TaskFields { project: String::new(), ..task_fields(task()) };
        assert_eq!(task_edit(task(), fields), None);
    }

    #[test]
    fn a_name_with_a_space_is_quoted_after_its_sigil() {
        assert_eq!(project_reference("Work".to_owned()), "#Work");
        assert_eq!(label_reference("deep work".to_owned()), "@\"deep work\"");
    }

    #[test]
    fn a_planned_length_never_reads_as_planned_twice() {
        assert_eq!(sitting_status(sitting("planned", Some(45))), vec!["planned for 45 minutes"]);
        assert_eq!(sitting_status(sitting("worked", Some(45))), vec!["worked", "45 minutes planned"]);
        assert_eq!(sitting_status(sitting("planned", None)), vec!["planned"]);
    }
}
