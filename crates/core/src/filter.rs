//! The filter query language: its shape, its meaning, and its readback (§6.2).
//!
//! A filter is a **boolean expression over predicates** — `&`, `|`, `!`, parentheses. That
//! is Todoist's level, it is what people arriving from Todoist expect, and it covers
//! essentially every real query:
//!
//! ```text
//! #work & (p1 | overdue) & !@waiting
//! ```
//!
//! One implementation serves two places: saved filters, and
//! [`BlockSeries::task_filter`](crate::model::BlockSeries::task_filter), which scopes what
//! auto-suggestion will offer for a block — *"this is my `#work` block, don't offer me
//! personal errands"*. That second use is what promotes this from a nice-to-have to a
//! load-bearing component.
//!
//! Parsing lives in `parse`. What lives here is the AST, which §6.2 calls **the stable
//! interface** — evaluation strategy can change behind it with no user-visible effect —
//! along with evaluation and the human-readable rendering.
//!
//! # Why there is a readback at all
//!
//! [`Expr::describe`] turns the query above into *"tasks in Work, and either priority 1 or
//! overdue, and not labelled waiting"*. This is not a debugging convenience. **A mis-parsed
//! filter shows wrong results silently**, and wrong results are invisible — there is no
//! squiggle, no highlighting, nothing to notice. Without a readback there is no way to
//! discover that the query did not mean what you thought.
//!
//! # Names, not identifiers
//!
//! Predicates hold the text the user typed. Resolution needs a [`Snapshot`] and is a
//! separate step ([`Expr::unresolved`]) precisely because §6.1 has to *report* what it could
//! not resolve — an unknown `#project` is an error, an unknown `@label` is a new label —
//! rather than silently matching nothing.

use std::collections::BTreeSet;

use jiff::{Span, Zoned};

use crate::model::{Priority, Task};
use crate::snapshot::Snapshot;
use crate::state::{Facts, State};
use crate::suggest;
use crate::time::DateSpec;

/// A boolean expression over predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// Matches everything. What an empty query means.
    All,
    /// A single condition.
    Predicate(Predicate),
    /// Negation.
    Not(Box<Expr>),
    /// All of them.
    And(Vec<Expr>),
    /// Any of them.
    Or(Vec<Expr>),
}

/// One condition a task can satisfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    /// `#work`, or `##work` for the project and everything under it.
    Project {
        /// The name as typed.
        name: String,
        /// Whether to include the full transitive closure of sub-projects.
        ///
        /// Unlimited depth, never depth-limited — there is no `###`. Evaluating it walks
        /// the project tree, so it depends on §3.13's cycle repair having run; an unguarded
        /// closure walk over a cyclic tree never terminates.
        include_descendants: bool,
    },
    /// `@laptop`.
    Label(String),
    /// `p1` through `p4`.
    Priority(Priority),
    /// A bare word from [`State`], shared with the accessibility layer (§13).
    State(State),
    /// A condition on the due date.
    Due(DueFilter),
    /// `assigned: today` — placed into a block on that day (§6.2).
    Assigned(DateSpec),
    /// `search: invoice` — free text over titles and notes.
    Search(String),
}

/// A condition on a task's due date.
///
/// `overdue` and `no date` are deliberately absent: they are [`State`] variants, because
/// §6.2 requires that anything a filter can select on is something a screen reader can
/// announce, and "overdue" is exactly the kind of thing it must say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DueFilter {
    /// Due on the anchor's own day.
    Today,
    /// Due at any point between today and `days` from now, inclusive. `7 days`.
    Within {
        /// How far ahead to look.
        days: i64,
    },
    /// Due on exactly this day.
    On(DateSpec),
    /// Due strictly before this day.
    Before(DateSpec),
    /// Due strictly after this day.
    After(DateSpec),
}

/// What evaluation needs besides the expression.
///
/// Built once per query with [`Context::new`], which indexes the snapshot so that matching
/// every task in it does not rescan every completion and assignment per task (see
/// [`Facts`]).
#[derive(Debug, Clone)]
pub struct Context<'a> {
    /// The data to match against.
    pub snapshot: &'a Snapshot,
    /// The user's current zoned datetime. Never UTC — §4 anchors every relative date to
    /// where the user actually is, and a filter is evaluated fresh every time precisely so
    /// that `today` keeps meaning today (§6.2).
    pub now: &'a Zoned,
    facts: Facts<'a>,
}

impl<'a> Context<'a> {
    /// A context for evaluating against `snapshot` as of `now`.
    #[must_use]
    pub fn new(snapshot: &'a Snapshot, now: &'a Zoned) -> Self {
        Self { snapshot, now, facts: Facts::new(snapshot) }
    }

    /// The snapshot's per-task index.
    #[must_use]
    pub const fn facts(&self) -> &Facts<'a> {
        &self.facts
    }
}

/// A name in a query that does not match anything in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    /// What kind of name it was.
    pub kind: NameKind,
    /// The name as typed.
    pub name: String,
    /// The closest thing that does exist, if there is one worth offering.
    pub suggestion: Option<String>,
}

/// What sort of name failed to resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    /// A `#project`.
    Project,
    /// An `@label`.
    Label,
}

impl NameKind {
    /// The word to use when announcing it — *"project Work"*, *"label laptop"* (§6.3).
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Label => "label",
        }
    }
}

impl Expr {
    /// Whether a task satisfies the expression.
    ///
    /// This says nothing about whether the task should be *shown*; see [`Expr::select`] for
    /// the defaults that apply to a list.
    #[must_use]
    pub fn matches(&self, task: &Task, cx: &Context<'_>) -> bool {
        match self {
            Self::All => true,
            Self::Predicate(p) => p.matches(task, cx),
            Self::Not(inner) => !inner.matches(task, cx),
            Self::And(parts) => parts.iter().all(|p| p.matches(task, cx)),
            Self::Or(parts) => parts.iter().any(|p| p.matches(task, cx)),
        }
    }

    /// The tasks a list should show for this expression.
    ///
    /// **Completed and trashed tasks are excluded unless the query mentions them.** A filter
    /// that returned every task ever finished would be useless, and one that quietly
    /// included the trash would be alarming. Asking for them explicitly — `completed`,
    /// `deleted` — opts back in, which is what makes a review view or a Trash view
    /// expressible in the same language rather than needing a special case.
    ///
    /// Results come back in the order §3.13 defines: position, then identifier to break
    /// ties, so two devices show the same list.
    #[must_use]
    pub fn select<'a>(&self, cx: &Context<'a>) -> Vec<&'a Task> {
        let wants_completed = self.mentions(State::Completed);
        let wants_deleted = self.mentions(State::Deleted);
        let mut found: Vec<&Task> = cx
            .snapshot
            .tasks
            .values()
            .filter(|task| wants_deleted || !task.is_deleted())
            .filter(|task| wants_completed || !cx.facts.is_completed(task))
            .filter(|task| self.matches(task, cx))
            .collect();
        found.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
        found
    }

    /// Whether the expression refers to a state anywhere inside it, at any nesting depth.
    #[must_use]
    pub fn mentions(&self, state: State) -> bool {
        match self {
            Self::All => false,
            Self::Predicate(Predicate::State(s)) => *s == state,
            Self::Predicate(_) => false,
            Self::Not(inner) => inner.mentions(state),
            Self::And(parts) | Self::Or(parts) => parts.iter().any(|p| p.mentions(state)),
        }
    }

    /// Every name in the query that does not exist in the store.
    ///
    /// The order is the order the names appear, so a message can walk them left to right.
    #[must_use]
    pub fn unresolved(&self, snapshot: &Snapshot) -> Vec<Unresolved> {
        let mut found = Vec::new();
        self.collect_unresolved(snapshot, &mut found);
        found
    }

    fn collect_unresolved(&self, snapshot: &Snapshot, out: &mut Vec<Unresolved>) {
        match self {
            Self::All => {}
            Self::Predicate(Predicate::Project { name, .. }) => {
                if snapshot.project_by_name(name).is_none() {
                    let names: Vec<&str> = snapshot
                        .projects
                        .values()
                        .filter(|p| p.deleted_at.is_none())
                        .map(|p| p.name.as_str())
                        .collect();
                    out.push(Unresolved {
                        kind: NameKind::Project,
                        name: name.clone(),
                        suggestion: suggest::nearest(name, names).map(ToOwned::to_owned),
                    });
                }
            }
            Self::Predicate(Predicate::Label(name)) => {
                if snapshot.label_by_name(name).is_none() {
                    let names: Vec<&str> = snapshot
                        .labels
                        .values()
                        .filter(|l| l.deleted_at.is_none())
                        .map(|l| l.name.as_str())
                        .collect();
                    out.push(Unresolved {
                        kind: NameKind::Label,
                        name: name.clone(),
                        suggestion: suggest::nearest(name, names).map(ToOwned::to_owned),
                    });
                }
            }
            Self::Predicate(_) => {}
            Self::Not(inner) => inner.collect_unresolved(snapshot, out),
            Self::And(parts) | Self::Or(parts) => {
                for part in parts {
                    part.collect_unresolved(snapshot, out);
                }
            }
        }
    }

    /// The query in English, for the readback §6.3 requires.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::All => "all tasks".to_owned(),
            other => format!("tasks {}", other.describe_body()),
        }
    }

    fn describe_body(&self) -> String {
        match self {
            Self::All => "of any kind".to_owned(),
            Self::Predicate(p) => p.describe(),
            Self::Not(inner) => format!("not {}", inner.describe_body()),
            Self::And(parts) => join(parts, ", and "),
            Self::Or(parts) => match parts.len() {
                0 => "of any kind".to_owned(),
                1 => parts[0].describe_body(),
                _ => format!("either {}", join(parts, " or ")),
            },
        }
    }
}

fn join(parts: &[Expr], separator: &str) -> String {
    parts.iter().map(Expr::describe_body).collect::<Vec<_>>().join(separator)
}

impl Predicate {
    /// Whether a task satisfies this one condition.
    #[must_use]
    pub fn matches(&self, task: &Task, cx: &Context<'_>) -> bool {
        let snapshot = cx.snapshot;
        match self {
            Self::Project { name, include_descendants } => {
                let Some(project) = snapshot.project_by_name(name) else {
                    // An unknown project matches nothing. Reporting it is
                    // `Expr::unresolved`'s job; silently matching everything would be worse
                    // than showing an empty list.
                    return false;
                };
                if *include_descendants {
                    snapshot.is_within(task.project_id, project.id)
                } else {
                    task.project_id == project.id
                }
            }
            Self::Label(name) => snapshot
                .label_by_name(name)
                .is_some_and(|label| task.labels.contains(&label.id)),
            Self::Priority(p) => task.priority == *p,
            Self::State(s) => cx.facts.has_state(task, *s, cx.now),
            Self::Due(filter) => filter.matches(task, cx),
            Self::Assigned(spec) => spec
                .resolve(cx.now)
                .is_some_and(|date| cx.facts.is_assigned_on(task.id, date)),
            Self::Search(needle) => {
                let needle = needle.to_lowercase();
                task.title.to_lowercase().contains(&needle)
                    || task.notes.to_lowercase().contains(&needle)
            }
        }
    }

    /// This condition in English.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Project { name, include_descendants: false } => format!("in {name}"),
            Self::Project { name, include_descendants: true } => {
                format!("in {name} or anything under it")
            }
            Self::Label(name) => format!("labelled {name}"),
            Self::Priority(p) => format!("priority {}", p.as_u8()),
            Self::State(s) => s.speech().to_owned(),
            Self::Due(filter) => filter.describe(),
            Self::Assigned(spec) => format!("assigned {spec}"),
            Self::Search(text) => format!("matching {text:?}"),
        }
    }
}

impl DueFilter {
    fn matches(&self, task: &Task, cx: &Context<'_>) -> bool {
        let Some(due) = task.due.as_ref().map(|d| d.date) else {
            // A task with no due date satisfies no date condition. Selecting for its absence
            // is `no date`, which is a State.
            return false;
        };
        let today = cx.now.date();
        match self {
            Self::Today => due == today,
            Self::Within { days } => match today.checked_add(Span::new().days(*days)) {
                Ok(limit) if *days >= 0 => due >= today && due <= limit,
                Ok(limit) => due <= today && due >= limit,
                Err(_) => false,
            },
            Self::On(spec) => spec.resolve(cx.now) == Some(due),
            Self::Before(spec) => spec.resolve(cx.now).is_some_and(|date| due < date),
            Self::After(spec) => spec.resolve(cx.now).is_some_and(|date| due > date),
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Today => "due today".to_owned(),
            Self::Within { days } => {
                let plural = if *days == 1 { "" } else { "s" };
                format!("due within {days} day{plural}")
            }
            Self::On(spec) => format!("due {spec}"),
            Self::Before(spec) => format!("due before {spec}"),
            Self::After(spec) => format!("due after {spec}"),
        }
    }
}

impl Snapshot {
    /// A project by name, case-insensitively, skipping deleted ones.
    #[must_use]
    pub fn project_by_name(&self, name: &str) -> Option<&crate::model::Project> {
        self.projects
            .values()
            .find(|p| p.deleted_at.is_none() && p.name.eq_ignore_ascii_case(name))
    }

    /// A label by name, case-insensitively, skipping deleted ones.
    #[must_use]
    pub fn label_by_name(&self, name: &str) -> Option<&crate::model::Label> {
        self.labels
            .values()
            .find(|l| l.deleted_at.is_none() && l.name.eq_ignore_ascii_case(name))
    }

    /// Whether `project` is `ancestor` or sits anywhere beneath it.
    ///
    /// Walks with a visited set. §3.13's repair should have run first, but a closure walk
    /// that hangs on a cyclic tree would take the whole UI with it, and this is cheap
    /// insurance against being called on an unrepaired snapshot.
    #[must_use]
    pub fn is_within(
        &self,
        project: crate::id::ProjectId,
        ancestor: crate::id::ProjectId,
    ) -> bool {
        let mut seen = BTreeSet::new();
        let mut current = Some(project);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            if !seen.insert(id) {
                return false;
            }
            current = self.projects.get(&id).and_then(|p| p.parent_id);
        }
        false
    }

    /// Whether a task is placed into any block occurring on `date`. See
    /// [`Facts::is_assigned_on`].
    #[must_use]
    pub fn is_assigned_on(&self, task: crate::id::TaskId, date: jiff::civil::Date) -> bool {
        self.facts().is_assigned_on(task, date)
    }
}
