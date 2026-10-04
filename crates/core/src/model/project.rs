//! Projects, labels, and saved filters (§3.4).

use jiff::Timestamp;

use crate::id::{FilterId, LabelId, ProjectId};
use crate::order::OrderKey;

/// An area of work. Tasks belong to exactly one.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Project {
    /// Identity.
    pub id: ProjectId,
    /// Display name.
    pub name: String,
    /// Parent project. May dangle or form a cycle after merge; see [`crate::repair`].
    pub parent_id: Option<ProjectId>,
    /// Presentation only. Never the sole carrier of meaning (§13).
    pub color: Option<String>,
    /// Position among siblings.
    pub order: OrderKey,
    /// Archived projects keep their tasks but leave the active views.
    pub archived: bool,
    /// Exactly one project has this. Inbox is a real record rather than a null
    /// `project_id`, which keeps ordering, view settings, and queries uniform.
    pub is_inbox: bool,
    /// How much this whole area matters right now — a **multiplier on urgency** (§10.3).
    ///
    /// **This is not a second priority and must never be called one.** Task priority
    /// answers "how much does this one item matter"; weight answers "how much does this
    /// whole area matter", the state where everything in `#thesis` outranks everything in
    /// `#chores` for a fortnight. Users who meet two fields both named *priority*
    /// immediately ask what a P1 task in a low-priority project means, and there is no good
    /// answer.
    ///
    /// Multiplicative rather than additive, so a heavy project lifts its contents
    /// proportionally instead of swamping due dates. Keep the UI range near
    /// [`Project::WEIGHT_RANGE`]: anything wider lets one project dominate every ranking,
    /// and the user then distrusts the whole feature.
    ///
    /// It **inherits down the tree** unless a sub-project sets its own — see
    /// [`crate::snapshot::Snapshot::effective_weight`]. Making `#thesis` heavy should not
    /// require touching each of its chapters.
    ///
    /// `None` inherits. `Some(1.0)` is a real setting, not a synonym for `None`: it is how a
    /// chapter opts back to neutral under a heavy thesis.
    pub weight: Option<f32>,
    /// Trash, for undo.
    pub deleted_at: Option<Timestamp>,
}

impl Project {
    /// Neutral weight: contributes nothing to urgency either way.
    pub const NEUTRAL_WEIGHT: f32 = 1.0;

    /// The range the UI should offer, inclusive.
    pub const WEIGHT_RANGE: (f32, f32) = (0.5, 2.0);

    /// A new project that inherits its weight.
    #[must_use]
    pub fn new(name: impl Into<String>, order: OrderKey) -> Self {
        Self {
            id: ProjectId::new(),
            name: name.into(),
            parent_id: None,
            color: None,
            order,
            archived: false,
            is_inbox: false,
            weight: None,
            deleted_at: None,
        }
    }

    /// The Inbox, which every store has exactly one of, under [`ProjectId::INBOX`].
    #[must_use]
    pub fn inbox() -> Self {
        Self {
            id: ProjectId::INBOX,
            is_inbox: true,
            ..Self::new("Inbox", OrderKey::middle())
        }
    }

    /// Whether the project sets a weight of its own, or inherits one.
    ///
    /// A weight that is not finite, or is not positive, is treated as unset: it can only
    /// have come from a bad import or a future version, and a zero or negative multiplier
    /// would silently erase or invert the urgency of everything beneath it.
    #[must_use]
    pub fn declared_weight(&self) -> Option<f32> {
        self.weight.filter(|w| w.is_finite() && *w > 0.0)
    }
}

/// Cross-cutting context: `@laptop`, `@errand`.
///
/// # Why labels are records rather than strings
///
/// Taskwarrior and `todo.txt` treat tags as plain strings on the task — `+home` exists
/// because something wears it and vanishes when nothing does. This model is the other kind,
/// and [`Task::labels`](super::Task::labels) is a set of identifiers, for three reasons
/// (§3.4):
///
/// 1. **Rename works.** Renaming `@work` to `@office` updates one record. With strings it
///    is a rewrite of every task carrying it, which in a CRDT is a large multi-object change
///    where a concurrent edit can leave the rename half-applied.
/// 2. **A label list can exist at all.** A set derived from whatever tasks happen to mention
///    it cannot carry colour or ordering, and cannot be curated.
/// 3. **Typo detection needs a closed set.** §6.3 promises *"unknown label 'lapto' — did you
///    mean 'laptop'?"*. With free strings there is no such thing as an unknown label, and
///    every typo silently becomes a new one that quietly splits a filter's results.
///
/// **Creation is still implicit.** Typing `@errand` for a label that does not exist creates
/// it — no dialog, no management screen. Being a record is an implementation fact the user
/// should never have to think about during capture. The two pull against each other and the
/// resolution is *confirm-on-new, never prompt-on-known*.
///
/// **Deletion touches no tasks.** It soft-deletes this record; identifiers left pointing at
/// it project as absent, which §3.1's tolerate-dangling-references rule already requires.
/// Undo is free and a large multi-task write is avoided. The consequence is worth stating:
/// a later label with the same *name* is a different record, and old tasks do not acquire it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Label {
    /// Identity.
    pub id: LabelId,
    /// Display name, without the leading `@`.
    pub name: String,
    /// Presentation only.
    pub color: Option<String>,
    /// Position in the label list.
    pub order: OrderKey,
    /// Trash, for undo.
    pub deleted_at: Option<Timestamp>,
}

impl Label {
    /// A new label.
    #[must_use]
    pub fn new(name: impl Into<String>, order: OrderKey) -> Self {
        Self {
            id: LabelId::new(),
            name: name.into(),
            color: None,
            order,
            deleted_at: None,
        }
    }
}

/// A named query (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SavedFilter {
    /// Identity.
    pub id: FilterId,
    /// Display name.
    pub name: String,
    /// Stored as **text, never as a resolved date range** (§6.2). A filter saved as
    /// "due before next Friday" must still mean that next month. The same language appears
    /// inline in [`BlockSeries::task_filter`](super::BlockSeries::task_filter).
    pub query: String,
    /// Position in the filter list.
    pub order: OrderKey,
    /// Presentation only.
    pub color: Option<String>,
    /// Trash, for undo.
    pub deleted_at: Option<Timestamp>,
}
