//! Undo and redo, kept on this device and nowhere else.
//!
//! **Automerge does not provide undo.** It provides history, and rewinding a document would
//! discard concurrent remote changes along with your own. Undo means applying an *inverse*,
//! which every [`Edit`] already carries: each change holds the record before and after.
//!
//! # Saved, so that it outlives a process
//!
//! A one-shot `lum` process has no session to keep a stack in, so the history is a table in
//! the profile's SQLite file. That makes it **per device**, not per process: `lum undo` in a
//! terminal undoes what the BTSpeak app just did, because both are this device. It is
//! **local-only**: sync exchanges Automerge changes and nothing else, and a backup
//! holds Automerge documents and nothing else, so the table never leaves the device. The
//! change an undo *produces* is an ordinary edit, and syncs like any other.
//!
//! # Undone against what is there now
//!
//! An entry may be undone long after it was recorded — after other edits here, or edits
//! merged in from another device. Swapping before and after unconditionally would overwrite those.
//! So an undo is **rebased**: for each record, only the fields the edit changed are put back,
//! and only where they still hold what the edit set. A field that has changed since is kept
//! and reported, never silently overwritten. Redo is the same thing in the other direction.
//!
//! Because the entry is always rebased, the stack never jams on a conflict: an undo that can
//! only do part of its work does that part and says what it left alone.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use lumenna_core::edit::{Change, Edit, Transition};
use lumenna_core::snapshot::Snapshot;

/// The format of a saved entry. An entry in an older format that no longer reads is dropped
/// with a notice rather than guessed at.
pub(crate) const FORMAT: i64 = 1;

/// How many edits are kept. A long session must not grow without limit.
pub const DEPTH: usize = 100;

/// What an undo or redo did.
#[derive(Debug, Clone, PartialEq)]
pub struct Reverted {
    /// The original edit's description — *"Completed Review PR"* — for *"Undid: …"*, since
    /// an undo is never a silent state change.
    pub description: String,
    /// What was actually written. Empty when everything had changed since.
    pub applied: Edit,
    /// What was left alone, one sentence each.
    pub kept: Vec<String>,
}

/// What [`Store::undo`](crate::Store::undo) and [`Store::redo`](crate::Store::redo) found.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// There was nothing to undo, or nothing to redo.
    Nothing,
    /// An entry was undone or redone.
    Done(Reverted),
    /// The entry was saved by a version whose edits this one cannot read. It has been
    /// dropped; the one before it is next.
    Unreadable,
}

/// Which way an entry is being taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    /// From the edit's `after` back to its `before`.
    Undo,
    /// From its `before` to its `after` again.
    Redo,
}

/// The edit that takes the store from where it is now toward `edit`'s other side.
pub(crate) fn rebase(edit: &Edit, direction: Direction, now: &Snapshot) -> Reverted {
    let mut kept = Vec::new();
    let mut changes = Vec::new();
    let ordered: Box<dyn Iterator<Item = &Change>> = match direction {
        // Undo goes backwards, so a record created and then referred to is unwound in the
        // order that keeps each step consistent.
        Direction::Undo => Box::new(edit.changes.iter().rev()),
        Direction::Redo => Box::new(edit.changes.iter()),
    };
    for change in ordered {
        if let Some(rebased) = rebase_change(change, direction, now, &mut kept) {
            changes.push(rebased);
        }
    }
    Reverted {
        description: edit.description.clone(),
        applied: Edit { description: edit.description.clone(), changes },
        kept,
    }
}

fn rebase_change(
    change: &Change,
    direction: Direction,
    now: &Snapshot,
    kept: &mut Vec<String>,
) -> Option<Change> {
    let k = kept;
    let d = direction;
    match change {
        Change::Task(t) => step(t, d, |r| now.tasks.get(&r.id), "task", k)
            .map(|t| Change::Task(Box::new(t))),
        Change::Completion(t) => step(t, d, |r| now.completions.get(&r.id), "completion", k)
            .map(|t| Change::Completion(Box::new(t))),
        Change::Project(t) => step(t, d, |r| now.projects.get(&r.id), "project", k)
            .map(|t| Change::Project(Box::new(t))),
        Change::Label(t) => step(t, d, |r| now.labels.get(&r.id), "label", k)
            .map(|t| Change::Label(Box::new(t))),
        Change::Filter(t) => step(t, d, |r| now.saved_filters.get(&r.id), "filter", k)
            .map(|t| Change::Filter(Box::new(t))),
        Change::Series(t) => step(t, d, |r| now.series.get(&r.id), "block", k)
            .map(|t| Change::Series(Box::new(t))),
        Change::Exception(t) => step(
            t,
            d,
            |r| now.exceptions.get(&(r.series_id, r.original_date)),
            "change to a block",
            k,
        )
        .map(|t| Change::Exception(Box::new(t))),
        Change::Reminder(t) => step(t, d, |r| now.reminders.get(&r.id), "reminder", k)
            .map(|t| Change::Reminder(Box::new(t))),
        Change::Ack(t) => step(
            t,
            d,
            |r| now.acks.get(&(r.reminder_id, r.occurrence_date)),
            "reminder dismissal",
            k,
        )
        .map(|t| Change::Ack(Box::new(t))),
        Change::Device(t) => step(t, d, |r| now.devices.get(&r.node_id), "device", k)
            .map(|t| Change::Device(Box::new(t))),
        Change::Assignment { year, transition } => {
            step(transition, d, |r| now.assignments.get(&r.id), "sitting", k)
                .map(|t| Change::Assignment { year: *year, transition: Box::new(t) })
        }
        Change::Settings { before, after } => {
            let (from, to) = match d {
                Direction::Undo => (after.as_ref(), before.as_ref()),
                Direction::Redo => (before.as_ref(), after.as_ref()),
            };
            let t = rebase_record(Some(from), Some(to), Some(&now.settings), "settings", k)?;
            match (t.before, t.after) {
                (Some(before), Some(after)) => Some(Change::Settings {
                    before: Box::new(before),
                    after: Box::new(after),
                }),
                _ => None,
            }
        }
    }
}

/// One record's transition, taken in `direction` from wherever `lookup` says it is now.
fn step<'a, T>(
    transition: &Transition<T>,
    direction: Direction,
    lookup: impl Fn(&T) -> Option<&'a T>,
    noun: &str,
    kept: &mut Vec<String>,
) -> Option<Transition<T>>
where
    T: Serialize + DeserializeOwned + PartialEq + Clone + 'a,
{
    let (from, to) = sides(&transition.before, &transition.after, direction);
    let current = from.or(to).and_then(lookup);
    rebase_record(from, to, current, noun, kept)
}

fn sides<'a, T>(
    before: &'a Option<T>,
    after: &'a Option<T>,
    direction: Direction,
) -> (Option<&'a T>, Option<&'a T>) {
    match direction {
        Direction::Undo => (after.as_ref(), before.as_ref()),
        Direction::Redo => (before.as_ref(), after.as_ref()),
    }
}

/// One record's step from `from` toward `to`, given that it is `current` now.
///
/// `None` when there is nothing to write — it is already there, or everything that would
/// have changed has changed since, which `kept` then says.
fn rebase_record<T>(
    from: Option<&T>,
    to: Option<&T>,
    current: Option<&T>,
    noun: &str,
    kept: &mut Vec<String>,
) -> Option<Transition<T>>
where
    T: Serialize + DeserializeOwned + PartialEq + Clone,
{
    let name = || describe(noun, current.or(to).or(from));
    match (from, to, current) {
        // Already where it is going.
        (_, _, now) if now == to => None,

        // Bringing back something the edit removed.
        (None, Some(to), None) => Some(Transition { before: None, after: Some(to.clone()) }),
        (None, Some(_), Some(_)) => {
            kept.push(format!("{} exists again, so it was left as it is", name()));
            None
        }

        // Removing something the edit created.
        (Some(from), None, Some(now)) => {
            if now == from {
                Some(Transition { before: Some(now.clone()), after: None })
            } else {
                kept.push(format!("kept {}, which has changed since", name()));
                None
            }
        }

        // Putting fields back.
        (Some(from), Some(to), Some(now)) => {
            let (target, conflicts) = merge_fields(from, to, now)?;
            if !conflicts.is_empty() {
                kept.push(format!(
                    "kept {}'s {}, changed since",
                    name(),
                    join_words(&conflicts)
                ));
            }
            (&target != now).then(|| Transition { before: Some(now.clone()), after: Some(target) })
        }
        (Some(_), Some(_), None) => {
            kept.push(format!("{} no longer exists", name()));
            None
        }

        // Neither side exists, or there was nothing to remove.
        _ => None,
    }
}

/// `now` with every field the step changes put to its `to` value, wherever it still holds
/// its `from` value. Returns the fields that had changed to something else.
fn merge_fields<T>(from: &T, to: &T, now: &T) -> Option<(T, Vec<String>)>
where
    T: Serialize + DeserializeOwned,
{
    let as_map = |value: &T| match serde_json::to_value(value) {
        Ok(Value::Object(map)) => Some(map),
        _ => None,
    };
    let (from, to, mut target) = (as_map(from)?, as_map(to)?, as_map(now)?);
    let mut conflicts = Vec::new();
    let keys: std::collections::BTreeSet<&String> = from.keys().chain(to.keys()).collect();
    for key in keys {
        let (was, will) = (from.get(key), to.get(key));
        if was == will {
            continue;
        }
        let is = target.get(key);
        if is == was {
            match will {
                Some(value) => {
                    target.insert(key.clone(), value.clone());
                }
                None => {
                    target.remove(key);
                }
            }
        } else if is != will {
            conflicts.push(field_words(key));
        }
    }
    let target = serde_json::from_value(Value::Object(target)).ok()?;
    Some((target, conflicts))
}

/// "task Review PR", or the noun alone when the record has no title or name.
fn describe<T: Serialize>(noun: &str, record: Option<&T>) -> String {
    let title = record
        .and_then(|r| serde_json::to_value(r).ok())
        .and_then(|value| match value {
            Value::Object(map) => title_of(&map),
            _ => None,
        });
    match title {
        Some(title) => format!("{noun} {title}"),
        None => format!("the {noun}"),
    }
}

fn title_of(map: &Map<String, Value>) -> Option<String> {
    ["title", "name"]
        .iter()
        .find_map(|key| map.get(*key).and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

/// A field name as words a person would say.
fn field_words(key: &str) -> String {
    match key {
        "project_id" => "project".to_owned(),
        "parent_id" => "parent".to_owned(),
        "deleted_at" => "trash state".to_owned(),
        "due" => "due date".to_owned(),
        "estimate_mins" => "estimate".to_owned(),
        "accumulated_mins" => "logged time".to_owned(),
        "running_since" => "timer".to_owned(),
        "planned_mins" => "planned time".to_owned(),
        "depends" => "dependencies".to_owned(),
        "order" => "position".to_owned(),
        other => other.replace('_', " "),
    }
}

fn join_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumenna_core::model::{Priority, Task};
    use lumenna_core::order::OrderKey;

    fn task() -> Task {
        Task::new(lumenna_core::ProjectId::INBOX, "Review PR", OrderKey::middle())
    }

    #[test]
    fn an_untouched_record_goes_back_exactly() {
        let before = task();
        let mut after = before.clone();
        after.priority = Priority::P1;
        let t = rebase_record(Some(&after), Some(&before), Some(&after), "task", &mut Vec::new())
            .unwrap();
        assert_eq!(t.after, Some(before));
    }

    #[test]
    fn a_field_changed_since_is_kept_and_the_rest_goes_back() {
        let before = task();
        let mut after = before.clone();
        after.priority = Priority::P1;
        after.title = "Review the PR".to_owned();
        // Since then, someone retitled it again.
        let mut now = after.clone();
        now.title = "Review PR 42".to_owned();

        let mut kept = Vec::new();
        let t = rebase_record(Some(&after), Some(&before), Some(&now), "task", &mut kept).unwrap();
        let result = t.after.unwrap();
        assert_eq!(result.priority, Priority::P4, "the priority went back");
        assert_eq!(result.title, "Review PR 42", "the newer title stayed");
        assert_eq!(kept, vec!["kept task Review PR 42's title, changed since".to_owned()]);
    }

    #[test]
    fn a_change_to_a_field_the_edit_did_not_touch_survives() {
        let before = task();
        let mut after = before.clone();
        after.priority = Priority::P1;
        let mut now = after.clone();
        now.notes = "added later".to_owned();

        let mut kept = Vec::new();
        let t = rebase_record(Some(&after), Some(&before), Some(&now), "task", &mut kept).unwrap();
        let result = t.after.unwrap();
        assert_eq!(result.priority, Priority::P4);
        assert_eq!(result.notes, "added later");
        assert!(kept.is_empty());
    }

    #[test]
    fn a_created_record_edited_since_is_not_removed() {
        let created = task();
        let mut now = created.clone();
        now.title = "Renamed since".to_owned();
        let mut kept = Vec::new();
        assert!(rebase_record(Some(&created), None, Some(&now), "task", &mut kept).is_none());
        assert_eq!(kept, vec!["kept task Renamed since, which has changed since".to_owned()]);
    }

    #[test]
    fn something_already_where_it_is_going_needs_nothing() {
        let t = task();
        assert!(rebase_record(Some(&t), None, None, "task", &mut Vec::new()).is_none());
        assert!(rebase_record(None, Some(&t), Some(&t), "task", &mut Vec::new()).is_none());
    }
}
