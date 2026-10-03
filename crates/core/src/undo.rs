//! The undo stack (§9).
//!
//! **Automerge does not provide undo.** It provides history, and rewinding a document is not
//! undo — rewinding would discard concurrent remote changes along with your own. Undo means
//! computing and applying an *inverse*, which [`Edit::inverse`](crate::edit::Edit::inverse)
//! already does. This is only the bookkeeping.
//!
//! Three properties, all from §9:
//!
//! - **Local-only and per-session** (§3.12). Undo is not a synced concept: the *change* an
//!   undo produces syncs like any other edit, but the stack itself never leaves the process.
//!   Two devices each have their own history of what they did.
//! - **Multi-level**, and bounded, so a long session cannot grow without limit.
//! - **Announceable.** Never a silent state change.
//!
//! That last one matters more here than in a sighted application. A sighted user notices
//! *"wait, that moved"* immediately; without that channel a mis-keystroke can go unnoticed
//! for minutes, by which point the context for recovering it is gone.

use std::collections::VecDeque;

use crate::edit::Edit;

/// How many edits a stack keeps by default.
///
/// Deep enough that reaching the bottom means something went wrong several minutes ago, and
/// shallow enough that a day's work does not accumulate in memory on a watch.
pub const DEFAULT_DEPTH: usize = 100;

/// A bounded, per-session history of what was done.
#[derive(Debug, Clone)]
pub struct UndoStack {
    done: VecDeque<Edit>,
    undone: Vec<Edit>,
    depth: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(DEFAULT_DEPTH)
    }
}

impl UndoStack {
    /// A stack holding at most `depth` edits.
    #[must_use]
    pub fn new(depth: usize) -> Self {
        Self { done: VecDeque::new(), undone: Vec::new(), depth: depth.max(1) }
    }

    /// Records an edit that has just been applied.
    ///
    /// Discards the redo history, as every editor does: once you do something new, the
    /// branch you had undone is no longer reachable and pretending otherwise would let a
    /// redo apply an inverse against a state it was never computed for.
    pub fn record(&mut self, edit: Edit) {
        if edit.is_empty() {
            return;
        }
        self.undone.clear();
        self.done.push_back(edit);
        while self.done.len() > self.depth {
            self.done.pop_front();
        }
    }

    /// The edit to apply to undo the last action, if there is one.
    ///
    /// The caller applies what comes back and announces
    /// [`UndoStack::last_description`] — which it must read *before* calling this, since
    /// this moves the entry.
    pub fn undo(&mut self) -> Option<Edit> {
        let edit = self.done.pop_back()?;
        let inverse = edit.clone().inverse();
        self.undone.push(edit);
        Some(inverse)
    }

    /// The edit to apply to redo what was last undone.
    pub fn redo(&mut self) -> Option<Edit> {
        let edit = self.undone.pop()?;
        let forward = edit.clone();
        self.done.push_back(edit);
        Some(forward)
    }

    /// What undoing would reverse, for announcing before it happens.
    #[must_use]
    pub fn last_description(&self) -> Option<&str> {
        self.done.back().map(|edit| edit.description.as_str())
    }

    /// What redoing would repeat.
    #[must_use]
    pub fn next_description(&self) -> Option<&str> {
        self.undone.last().map(|edit| edit.description.as_str())
    }

    /// The sentence to speak after undoing — *"Undid: Completed Review PR"*.
    ///
    /// Phrased in the core so all eleven targets say the same thing (§13).
    #[must_use]
    pub fn undo_announcement(&self) -> Option<String> {
        self.last_description().map(|what| format!("Undid: {what}"))
    }

    /// The sentence to speak after redoing.
    #[must_use]
    pub fn redo_announcement(&self) -> Option<String> {
        self.next_description().map(|what| format!("Redid: {what}"))
    }

    /// Whether there is anything to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    /// Whether there is anything to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// How many edits are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.done.len()
    }

    /// Whether the stack holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{Change, Transition};
    use crate::model::{Project, Task};
    use crate::order::OrderKey;
    use crate::ProjectId;

    fn edit(description: &str) -> Edit {
        let task = Task::new(ProjectId::new(), description, OrderKey::middle());
        Edit {
            description: description.to_owned(),
            changes: vec![Change::Task(Box::new(Transition {
                before: None,
                after: Some(task),
            }))],
        }
    }

    #[test]
    fn an_empty_stack_offers_nothing() {
        let mut stack = UndoStack::default();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo(), None);
        assert_eq!(stack.redo(), None);
        assert_eq!(stack.undo_announcement(), None);
    }

    #[test]
    fn undoing_returns_the_inverse_and_redoing_returns_the_original() {
        let mut stack = UndoStack::default();
        let original = edit("Completed Review PR");
        stack.record(original.clone());

        assert_eq!(stack.undo_announcement().as_deref(), Some("Undid: Completed Review PR"));
        let undo = stack.undo().unwrap();
        assert_eq!(undo, original.clone().inverse());
        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        assert_eq!(stack.redo_announcement().as_deref(), Some("Redid: Completed Review PR"));
        assert_eq!(stack.redo(), Some(original));
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_is_multi_level_and_goes_backwards_in_order() {
        let mut stack = UndoStack::default();
        for name in ["first", "second", "third"] {
            stack.record(edit(name));
        }
        assert_eq!(stack.last_description(), Some("third"));
        stack.undo();
        assert_eq!(stack.last_description(), Some("second"));
        stack.undo();
        assert_eq!(stack.last_description(), Some("first"));
    }

    #[test]
    fn doing_something_new_discards_the_redo_branch() {
        // Redoing into a branch that a later edit has diverged from would apply an inverse
        // against a state it was never computed for.
        let mut stack = UndoStack::default();
        stack.record(edit("first"));
        stack.undo();
        assert!(stack.can_redo());

        stack.record(edit("second"));
        assert!(!stack.can_redo());
    }

    #[test]
    fn the_stack_is_bounded() {
        let mut stack = UndoStack::new(3);
        for name in ["a", "b", "c", "d", "e"] {
            stack.record(edit(name));
        }
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.last_description(), Some("e"));
        stack.undo();
        stack.undo();
        stack.undo();
        assert!(!stack.can_undo(), "the oldest two fell off the bottom");
    }

    #[test]
    fn an_edit_that_changed_nothing_is_not_recorded() {
        // Otherwise undo would appear to do nothing, twice, and the user would have no way
        // to tell that from a broken keystroke.
        let mut stack = UndoStack::default();
        stack.record(Edit::nothing());
        assert!(!stack.can_undo());

        let unchanged = Project::inbox();
        let inert = crate::edit::update_project(unchanged.clone(), unchanged);
        assert!(inert.is_empty());
        stack.record(inert);
        assert!(!stack.can_undo());
    }

    #[test]
    fn a_depth_of_zero_still_holds_one_edit() {
        let mut stack = UndoStack::new(0);
        stack.record(edit("only"));
        assert_eq!(stack.len(), 1);
    }
}
