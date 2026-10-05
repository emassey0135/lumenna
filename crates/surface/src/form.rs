//! What a client's forms share: a task's fields as text, what saving them sends, and the
//! few phrases every client composes the same way.
//!
//! These are free functions rather than methods on [`Lumenna`](crate::Lumenna) because they
//! need no store — only what an operation already returned — but they are rules all the same,
//! and rules belong here rather than in each of eleven clients. The one that
//! matters most is [`task_edit`]: a form that sends a field it did not change wins a
//! last-write-wins race it should have lost, and silently reverts another device's edit.

use lumenna_parse::complete::quoted;
use serde::{Deserialize, Serialize};

use crate::error::{LumennaError, Result};
use crate::types::{BlockEdit, BlockShown, NewBlock, PlanAssignment, PlanBlock, TaskDetail, TaskEdit, Weight};
use crate::words::duration;

/// A task's editable fields, as a form shows them: all text, apart from the priority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
/// last-write-wins race against a concurrent edit from another device — so sending
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

/// The settings a kind of block has unless a block sets them apart: what a block
/// form's check buttons start from, and go back to when the kind is changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BlockDefaults {
    /// Whether tasks can be put in it.
    pub accepts_tasks: bool,
    /// Whether it counts toward the hours available for work.
    pub counts_capacity: bool,
    /// Whether it is fixed in time.
    pub anchored: bool,
}

/// What a kind of block — `work`, `break` or `event` — has unless set apart, or `None` for a
/// word that is not a kind.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn block_defaults(kind: String) -> Option<BlockDefaults> {
    let flags = crate::planning::block_kind(&kind).ok()?.default_flags();
    Some(BlockDefaults {
        accepts_tasks: flags.accepts_tasks,
        counts_capacity: flags.counts_capacity,
        anchored: flags.anchored,
    })
}

/// A block's editable fields, as a form shows them: all text, apart from the three flags.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BlockFields {
    /// What it is called.
    pub title: String,
    /// When it starts: `09:00`, or `9am` as typed.
    pub start: String,
    /// How many minutes it lasts.
    pub minutes: String,
    /// `work`, `break` or `event`.
    pub kind: String,
    /// Whether tasks can be put in it.
    pub accepts_tasks: bool,
    /// Whether it counts toward the hours available for work.
    pub counts_capacity: bool,
    /// Whether it is fixed in time.
    pub anchored: bool,
    /// How it repeats, in the block editor's words: `every weekday`. Empty for once — or for
    /// a rule the words cannot say, which saving then leaves alone.
    pub repeat: String,
    /// The last day a repeating block happens. Empty for for good.
    pub until: String,
    /// How short re-flow may make it, in minutes. Empty for the kind's own.
    pub min_minutes: String,
    /// The filter scoping which tasks are offered for it. Empty for none.
    pub task_filter: String,
    /// A colour, by name. Empty for none.
    pub colour: String,
    /// The notes, exactly as typed.
    pub notes: String,
}

/// A block's fields as the form for every occurrence starts from them.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn block_fields(block: BlockShown) -> BlockFields {
    BlockFields {
        title: block.title,
        start: block.start,
        minutes: block.minutes.to_string(),
        kind: block.kind,
        accepts_tasks: block.accepts_tasks,
        counts_capacity: block.counts_capacity,
        anchored: block.anchored,
        repeat: block.repetition.unwrap_or_default(),
        until: block.until.unwrap_or_default(),
        min_minutes: block.min_minutes.map(|m| m.to_string()).unwrap_or_default(),
        task_filter: block.task_filter.unwrap_or_default(),
        colour: block.colour.unwrap_or_default(),
        notes: block.notes,
    }
}

/// One day's block as the form for that day alone starts from it: its time, length, title,
/// kind and flags, which are all one day can change. The rest stays empty and unchanged.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn day_block_fields(block: PlanBlock) -> BlockFields {
    BlockFields {
        title: block.title,
        start: block.start,
        minutes: block.duration_mins.to_string(),
        kind: block.kind,
        accepts_tasks: block.accepts_tasks,
        counts_capacity: block.counts_capacity,
        anchored: block.anchored,
        ..BlockFields::default()
    }
}

/// What saving `after` over `before` sends to
/// [`edit_block`](crate::Lumenna::edit_block), or nothing if no field changed.
///
/// **Only the fields that changed**, as with a task. Changing the kind gives the
/// block that kind's flags, so a flag is sent where it differs from the *new* kind's, not
/// from what it was. Empty notes, filter or colour clear them; an empty shortest length goes
/// back to the kind's own; an empty last day repeats for good; an empty repetition stops it
/// repeating — unless it was empty to start with, as for a rule the words cannot say.
///
/// # Errors
///
/// If the length or the shortest length is not a whole number of minutes.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn block_edit(before: BlockFields, after: BlockFields) -> Result<Option<BlockEdit>> {
    let changed = |now: &str, was: &str| now.trim() != was.trim();
    let text = |now: &str, was: &str| changed(now, was).then(|| now.trim().to_owned());
    let or_none = |now: &str, was: &str| {
        changed(now, was).then(|| if now.trim().is_empty() { "none".to_owned() } else { now.trim().to_owned() })
    };
    let kind = text(&after.kind, &before.kind);
    let base = match kind.as_deref().and_then(|k| block_defaults(k.to_owned())) {
        Some(defaults) => (defaults.accepts_tasks, defaults.counts_capacity, defaults.anchored),
        None => (before.accepts_tasks, before.counts_capacity, before.anchored),
    };
    let flag = |now: bool, base: bool| (now != base).then_some(now);
    let min_minutes = if changed(&after.min_minutes, &before.min_minutes) {
        Some(if after.min_minutes.trim().is_empty() { 0 } else { whole_minutes(&after.min_minutes, "the shortest length")? })
    } else {
        None
    };
    let edit = BlockEdit {
        title: text(&after.title, &before.title),
        at: text(&after.start, &before.start),
        minutes: if changed(&after.minutes, &before.minutes) { Some(whole_minutes(&after.minutes, "the length")?) } else { None },
        kind,
        repeat: or_none(&after.repeat, &before.repeat),
        notes: (after.notes != before.notes).then(|| after.notes.clone()),
        accepts_tasks: flag(after.accepts_tasks, base.0),
        counts_capacity: flag(after.counts_capacity, base.1),
        anchored: flag(after.anchored, base.2),
        min_minutes,
        task_filter: text(&after.task_filter, &before.task_filter),
        until: or_none(&after.until, &before.until),
        colour: text(&after.colour, &before.colour),
    };
    Ok((edit != BlockEdit::default()).then_some(edit))
}

/// What a new block's form sends to [`add_block`](crate::Lumenna::add_block), starting on
/// `date` — a date phrase, today when absent. A flag the same as its kind's is left to the
/// kind, and an empty field is left out.
///
/// # Errors
///
/// If the length or the shortest length is not a whole number of minutes.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn new_block(fields: BlockFields, date: Option<String>) -> Result<NewBlock> {
    let given = |text: &str| Some(text.trim().to_owned()).filter(|t| !t.is_empty());
    let defaults = block_defaults(fields.kind.clone());
    let flag = |now: bool, default: Option<bool>| (Some(now) != default).then_some(now);
    Ok(NewBlock {
        title: fields.title.trim().to_owned(),
        at: fields.start.trim().to_owned(),
        minutes: whole_minutes(&fields.minutes, "the length")?,
        date: date.as_deref().and_then(given),
        kind: fields.kind.trim().to_owned(),
        repeat: given(&fields.repeat),
        notes: Some(fields.notes.clone()).filter(|n| !n.trim().is_empty()),
        accepts_tasks: flag(fields.accepts_tasks, defaults.map(|d| d.accepts_tasks)),
        counts_capacity: flag(fields.counts_capacity, defaults.map(|d| d.counts_capacity)),
        anchored: flag(fields.anchored, defaults.map(|d| d.anchored)),
        min_minutes: given(&fields.min_minutes).map(|m| whole_minutes(&m, "the shortest length")).transpose()?,
        task_filter: given(&fields.task_filter),
        until: given(&fields.until),
        colour: given(&fields.colour),
    })
}

fn whole_minutes(text: &str, what: &str) -> Result<u32> {
    text.trim().parse().map_err(|_| LumennaError::new(format!("{what} has to be a whole number of minutes")))
}

/// A project's weight as typed: a number above zero, such as `1.5`, or `inherit` to take its
/// parent's again. Anything else is refused rather than read as `inherit`, so a typo — `1,5`
/// — says so instead of quietly undoing a weight.
///
/// # Errors
///
/// If it is neither.
#[cfg_attr(feature = "uniffi", uniffi::export)]
pub fn parse_weight(text: String) -> Result<Weight> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("inherit") {
        return Ok(Weight::Inherit);
    }
    text.parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| Weight::Value { value })
        .ok_or_else(|| LumennaError::new(format!("'{text}' is not a weight; use a number above zero, such as 1.5, or inherit")))
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
            running: false,
            details: Vec::new(),
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

    #[test]
    fn each_kind_of_block_has_its_own_defaults() {
        let work = block_defaults("work".to_owned()).unwrap();
        assert!(work.accepts_tasks && work.counts_capacity && !work.anchored);
        let event = block_defaults("Event".to_owned()).unwrap();
        assert!(!event.accepts_tasks && event.anchored);
        assert!(!block_defaults("break".to_owned()).unwrap().accepts_tasks);
        assert_eq!(block_defaults("nap".to_owned()), None);
    }

    fn work_block() -> BlockFields {
        BlockFields {
            title: "Deep work".to_owned(),
            start: "09:00".to_owned(),
            minutes: "120".to_owned(),
            kind: "work".to_owned(),
            accepts_tasks: true,
            counts_capacity: true,
            anchored: false,
            repeat: "every weekday".to_owned(),
            ..BlockFields::default()
        }
    }

    #[test]
    fn a_block_form_saved_unchanged_sends_nothing() {
        assert_eq!(block_edit(work_block(), work_block()).unwrap(), None);
    }

    #[test]
    fn a_block_edit_sends_only_what_changed() {
        let after = BlockFields { title: "Writing".to_owned(), anchored: true, ..work_block() };
        let edit = block_edit(work_block(), after).unwrap().unwrap();
        assert_eq!(edit, BlockEdit { title: Some("Writing".to_owned()), anchored: Some(true), ..BlockEdit::default() });
    }

    #[test]
    fn a_new_kind_brings_its_flags_so_only_flags_unlike_it_are_sent() {
        // Made a break, which takes no tasks — and told to take them anyway.
        let after = BlockFields { kind: "break".to_owned(), accepts_tasks: true, counts_capacity: false, ..work_block() };
        let edit = block_edit(work_block(), after).unwrap().unwrap();
        assert_eq!(edit.kind.as_deref(), Some("break"));
        assert_eq!(edit.accepts_tasks, Some(true), "unlike a break's");
        assert_eq!(edit.counts_capacity, None, "a break's own, which the kind brings");
        assert_eq!(edit.anchored, None);
    }

    #[test]
    fn emptied_fields_clear_or_go_back_to_the_default() {
        let before = BlockFields {
            until: "2026-12-31".to_owned(),
            min_minutes: "30".to_owned(),
            task_filter: "#Work".to_owned(),
            colour: "teal".to_owned(),
            notes: "Phone off.".to_owned(),
            ..work_block()
        };
        let after = BlockFields { repeat: String::new(), ..work_block() };
        let edit = block_edit(before, after).unwrap().unwrap();
        assert_eq!(edit.repeat.as_deref(), Some("none"));
        assert_eq!(edit.until.as_deref(), Some("none"));
        assert_eq!(edit.min_minutes, Some(0));
        assert_eq!(edit.task_filter.as_deref(), Some(""));
        assert_eq!(edit.colour.as_deref(), Some(""));
        assert_eq!(edit.notes.as_deref(), Some(""));
    }

    #[test]
    fn a_repetition_the_words_cannot_say_is_left_alone() {
        let before = BlockFields { repeat: String::new(), ..work_block() };
        assert_eq!(block_edit(before.clone(), before).unwrap(), None);
    }

    #[test]
    fn minutes_that_are_not_a_number_are_refused() {
        let after = BlockFields { minutes: "two hours".to_owned(), ..work_block() };
        assert_eq!(block_edit(work_block(), after).unwrap_err().message(), "the length has to be a whole number of minutes");
    }

    #[test]
    fn a_new_block_leaves_to_its_kind_whatever_matches_it() {
        let block = new_block(work_block(), Some("tomorrow".to_owned())).unwrap();
        assert_eq!(block.accepts_tasks, None);
        assert_eq!(block.counts_capacity, None);
        assert_eq!(block.anchored, None);
        assert_eq!(block.minutes, 120);
        assert_eq!(block.date.as_deref(), Some("tomorrow"));
        assert_eq!(block.repeat.as_deref(), Some("every weekday"));
        assert_eq!(block.notes, None);
        let anchored = new_block(BlockFields { anchored: true, ..work_block() }, None).unwrap();
        assert_eq!(anchored.anchored, Some(true));
    }

    #[test]
    fn a_days_form_starts_from_that_day() {
        let day = day_block_fields(PlanBlock {
            row: 1,
            id: "s@2026-10-05".to_owned(),
            series: "s".to_owned(),
            title: "Deep work".to_owned(),
            start: "10:00".to_owned(),
            end: "11:00".to_owned(),
            duration_mins: 60,
            kind: "work".to_owned(),
            when: String::new(),
            repeats: true,
            changed_for_this_day: true,
            assignments: Vec::new(),
            accepts_tasks: true,
            counts_capacity: false,
            anchored: false,
            colour: Some("teal".to_owned()),
            notes: "Phone off.".to_owned(),
            details: Vec::new(),
        });
        assert_eq!((day.start.as_str(), day.minutes.as_str()), ("10:00", "60"));
        assert!(!day.counts_capacity);
        assert_eq!(day.colour, "", "a day cannot change it, so the form does not hold it");
    }

    #[test]
    fn a_weight_is_a_number_above_zero_or_inherit_and_nothing_else() {
        assert_eq!(parse_weight(" 1.5 ".to_owned()).unwrap(), Weight::Value { value: 1.5 });
        assert_eq!(parse_weight("Inherit".to_owned()).unwrap(), Weight::Inherit);
        for wrong in ["1,5", "0", "-2", "", "inf", "heavy"] {
            assert!(parse_weight(wrong.to_owned()).is_err(), "{wrong} is not a weight");
        }
    }
}
