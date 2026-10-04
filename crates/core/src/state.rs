//! Computed states: one vocabulary, two surfaces.
//!
//! A [`State`] is something true about a task that nobody typed — overdue, blocked, running.
//! §6.2 and §13 both need this list, and the whole point of this module is that they get the
//! *same* list:
//!
//! - The **filter language** selects on them as bare words: `blocked & !recurring`.
//! - The **accessibility row projection** announces them after the title: *"Review PR,
//!   overdue"*.
//!
//! §6.2 states the consequence as a rule: **adding a computed state means adding a `State`
//! variant, and anything a filter can select on is something a screen reader can announce.**
//! Two lists would drift, and the drift would be invisible — a state you can filter by but
//! never hear, or hear but never filter by.
//!
//! # Why these are not labels
//!
//! Taskwarrior spells virtual tags exactly like real ones — `+OVERDUE` beside `+home` —
//! distinguished only by a capitalisation convention it does not enforce. That is elegant
//! when tags are your only such axis, and wrong here, for a reason that is specifically an
//! accessibility reason: `@` completion must offer the user's *own* labels, and in a screen
//! reader you cannot separate two kinds of candidate by styling, only by where they appear.
//! So labels are `@laptop`, states are bare words, and they complete from different lists on
//! different triggers.
//!
//! A user who saw `blocked` sitting where `@waiting` sits would also try to remove it, and
//! could not.

use std::collections::BTreeMap;

use jiff::{Zoned, civil};

use crate::id::TaskId;
use crate::model::{AssignmentStatus, BlockAssignment, Due, Task, TaskCompletion};
use crate::snapshot::Snapshot;

/// Something true about a task that was computed rather than entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    /// Finished. For a recurring task, and for a subtask of one, this means the current
    /// occurrence, since completing one advances the due date rather than ending the task
    /// (§3.3) — see [`Snapshot::occurrence_of`].
    Completed,
    /// In the trash, recoverable (§3.2).
    Deleted,
    /// Due in the past and not finished.
    Overdue,
    /// Repeats.
    Recurring,
    /// Has a parent.
    Subtask,
    /// Something it depends on is unfinished.
    Blocked,
    /// Nothing blocks it and it is not finished — the state worth acting on.
    Ready,
    /// A timer is running against it right now.
    Running,
    /// Time has been logged against it, but it is not finished. What multi-sitting work
    /// looks like between sittings (§3.7).
    Started,
    /// No sitting ahead: not planned into any block today or later, and no timer running.
    /// What auto-suggestion looks for — including a task worked on before and not finished,
    /// and one whose planned sitting was missed (see [`Facts::has_sitting_ahead`]).
    Unassigned,
    /// No due date.
    NoDate,
    /// Wears no label.
    NoLabel,
    /// Still in the Inbox, which is where captured-but-not-filed lives (§3.4).
    NoProject,
    /// Unsized, so auto-suggestion cannot rank it (§6.2).
    NoEstimate,
}

impl State {
    /// Every state, in the order a reader should hear them when several apply.
    ///
    /// Ordered by how much it changes what you would do: whether the thing is still live
    /// first, then whether it is late, then how it relates to other work, then what it is
    /// missing.
    pub const ALL: &'static [Self] = &[
        Self::Completed,
        Self::Deleted,
        Self::Overdue,
        Self::Running,
        Self::Blocked,
        Self::Ready,
        Self::Started,
        Self::Recurring,
        Self::Subtask,
        Self::Unassigned,
        Self::NoDate,
        Self::NoProject,
        Self::NoLabel,
        Self::NoEstimate,
    ];

    /// The bare word that selects this state in a filter (§6.2).
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Deleted => "deleted",
            Self::Overdue => "overdue",
            Self::Recurring => "recurring",
            Self::Subtask => "subtask",
            Self::Blocked => "blocked",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Started => "started",
            Self::Unassigned => "unassigned",
            Self::NoDate => "no date",
            Self::NoLabel => "no label",
            Self::NoProject => "no project",
            Self::NoEstimate => "no estimate",
        }
    }

    /// What a screen reader says. Identical to the keyword today, and deliberately a
    /// separate function: the filter word is a stable interface people write into saved
    /// queries, while the spoken form should be free to read better without breaking them.
    #[must_use]
    pub const fn speech(self) -> &'static str {
        self.keyword()
    }

    /// The braille short form.
    ///
    /// §13 permits abbreviating roles and states — and only those, never titles — because
    /// the reader already knows the convention. **These need checking against the
    /// convention BTBraille's tree view actually uses** before they ship; they are chosen to
    /// be unambiguous among themselves, which is not the same as being what a reader
    /// expects.
    #[must_use]
    pub const fn abbreviation(self) -> &'static str {
        match self {
            Self::Completed => "done",
            Self::Deleted => "del",
            Self::Overdue => "ovd",
            Self::Recurring => "rec",
            Self::Subtask => "sub",
            Self::Blocked => "blk",
            Self::Ready => "rdy",
            Self::Running => "run",
            Self::Started => "strt",
            Self::Unassigned => "unas",
            Self::NoDate => "nodt",
            Self::NoLabel => "nolb",
            Self::NoProject => "nopj",
            Self::NoEstimate => "noes",
        }
    }

    /// Whether this state is worth announcing unprompted.
    ///
    /// Every state is worth **filtering** on — that is what they are for. Only some are
    /// worth *saying* on every row. "Review PR, task, ready, unassigned, no estimate, 1 of
    /// 12" buries the one word that mattered under four that are true of almost everything,
    /// and a screen reader user cannot skim past them the way a sighted reader skips a
    /// column.
    ///
    /// So the absence family — `no date`, `no label`, `no project`, `no estimate` — and the
    /// two near-universal states are notable only when asked for. A detail view (`lum task show`)
    /// still lists everything, because there the user asked.
    #[must_use]
    pub const fn is_notable(self) -> bool {
        !matches!(
            self,
            Self::Ready
                | Self::Unassigned
                | Self::NoDate
                | Self::NoLabel
                | Self::NoProject
                | Self::NoEstimate
        )
    }

    /// Looks a state up by its filter keyword.
    #[must_use]
    pub fn from_keyword(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|state| state.keyword() == word)
    }
}

/// Whether a task's due date has passed.
///
/// A date with no time is overdue the day after; §3.5 keeps "Tuesday" and "Tuesday at 3pm"
/// genuinely different states, and treating the first as midnight would make everything
/// overdue a day early.
///
/// A due time with no zone **floats** — 3pm wherever you are — so it is compared against the
/// local wall clock, not against an instant. Only a due date anchored to a real place
/// resolves to an instant first.
#[must_use]
pub fn is_overdue(due: &Due, now: &Zoned) -> bool {
    let Some(time) = due.time else {
        return due.date < now.date();
    };
    let civil = due.date.to_datetime(time);
    match due.timezone.as_ref().and_then(|tz| tz.get().ok()) {
        Some(tz) => match civil.to_zoned(tz) {
            Ok(zoned) => zoned.timestamp() < now.timestamp(),
            // A zone this device's tzdb has never heard of: fall back to floating rather
            // than declaring the task not-due forever.
            Err(_) => civil < now.datetime(),
        },
        None => civil < now.datetime(),
    }
}

/// A task's completions and assignments, gathered once.
///
/// Every state question is about one task's completions or assignments, and both live in
/// flat maps keyed by their own identifiers. Asking [`Snapshot::has_state`] of every task in a
/// list therefore scans every completion and every assignment once per task per state —
/// harmless on a laptop with a few hundred records, and the wrong shape for a watch holding a
/// recurring task's whole history (§16.9). A query that asks about many tasks builds one of
/// these first, through [`Snapshot::facts`] or [`crate::filter::Context::new`], and each
/// question is then a lookup.
///
/// The methods here are the definitions; `Snapshot`'s methods of the same names build a
/// `Facts` and ask it, which is the right cost for a single question.
#[derive(Debug, Clone)]
pub struct Facts<'a> {
    snapshot: &'a Snapshot,
    completions: BTreeMap<TaskId, Vec<&'a TaskCompletion>>,
    assignments: BTreeMap<TaskId, Vec<&'a BlockAssignment>>,
}

impl<'a> Facts<'a> {
    /// Indexes a snapshot.
    #[must_use]
    pub fn new(snapshot: &'a Snapshot) -> Self {
        let mut completions: BTreeMap<TaskId, Vec<&TaskCompletion>> = BTreeMap::new();
        for completion in snapshot.completions.values() {
            completions.entry(completion.task_id).or_default().push(completion);
        }
        let mut assignments: BTreeMap<TaskId, Vec<&BlockAssignment>> = BTreeMap::new();
        for assignment in snapshot.assignments.values() {
            assignments.entry(assignment.task_id).or_default().push(assignment);
        }
        Self { snapshot, completions, assignments }
    }

    /// The snapshot this indexes.
    #[must_use]
    pub const fn snapshot(&self) -> &'a Snapshot {
        self.snapshot
    }

    /// A task's completions, in identifier order.
    #[must_use]
    pub fn completions_of(&self, task: TaskId) -> &[&'a TaskCompletion] {
        self.completions.get(&task).map_or(&[], Vec::as_slice)
    }

    /// The assignments placing a task into blocks, in creation order.
    #[must_use]
    pub fn assignments_of(&self, task: TaskId) -> &[&'a BlockAssignment] {
        self.assignments.get(&task).map_or(&[], Vec::as_slice)
    }

    /// Whether a task's current occurrence is finished.
    ///
    /// For a task with no recurrence above or on it, any completion. Otherwise, a completion
    /// recorded against the occurrence now current — see [`Snapshot::occurrence_of`].
    #[must_use]
    pub fn is_completed(&self, task: &Task) -> bool {
        let completions = self.completions_of(task.id);
        match self.snapshot.occurrence_of(task) {
            None => !completions.is_empty(),
            Some(date) => completions.iter().any(|c| c.occurrence_date == Some(date)),
        }
    }

    /// Whether anything a task depends on is still outstanding.
    ///
    /// A dependency that is not in this snapshot does not block: §3.1 forbids referential
    /// integrity across documents, so a reference to a task this device has not merged is
    /// expected. Treating it as blocking would make a task on a partially synced device
    /// look unstartable for no reason the user could see.
    #[must_use]
    pub fn is_blocked(&self, task: &Task) -> bool {
        task.depends.iter().any(|id| {
            self.snapshot
                .tasks
                .get(id)
                .is_some_and(|dep| !dep.is_deleted() && !self.is_completed(dep))
        })
    }

    /// Whether a task is placed into any block occurring on `date`.
    ///
    /// A one-off block's assignment names only the series (§3.7), so its date comes from
    /// the series — which may live in a document that is not loaded, in which case it
    /// simply does not match rather than being an error.
    #[must_use]
    pub fn is_assigned_on(&self, task: TaskId, date: civil::Date) -> bool {
        self.assignments_of(task)
            .iter()
            .any(|assignment| self.snapshot.assignment_date(assignment) == Some(date))
    }

    /// Whether a task has a sitting still ahead of it: planned or under way, in a block
    /// today or later.
    ///
    /// This, not "was ever assigned", is what makes a task *unassigned*. A task worked on
    /// last Tuesday and not finished needs another sitting, and a sitting planned for
    /// yesterday that never happened was missed rather than scheduled — both are exactly
    /// what auto-suggestion (§10.2) has to find. A timer still running counts whatever day
    /// its block was, and so does a sitting whose day cannot be told because its block is
    /// not loaded: wrongly calling a task unscheduled invites planning it twice.
    #[must_use]
    pub fn has_sitting_ahead(&self, task: TaskId, now: &Zoned) -> bool {
        let today = now.date();
        self.assignments_of(task).iter().any(|assignment| {
            assignment.is_running()
                || (matches!(
                    assignment.status,
                    AssignmentStatus::Planned | AssignmentStatus::InProgress
                ) && self.snapshot.assignment_date(assignment).is_none_or(|date| date >= today))
        })
    }

    /// Whether one state holds for a task.
    #[must_use]
    pub fn has_state(&self, task: &Task, state: State, now: &Zoned) -> bool {
        let completed = || self.is_completed(task);
        match state {
            State::Completed => completed(),
            State::Deleted => task.is_deleted(),
            State::Overdue => {
                task.due.as_ref().is_some_and(|due| is_overdue(due, now)) && !completed()
            }
            State::Recurring => task.due.as_ref().is_some_and(|d| d.recurrence.is_some()),
            State::Subtask => task.parent_id.is_some(),
            State::Blocked => self.is_blocked(task),
            // Ready is not simply "not blocked": a finished or trashed task is not something
            // to pick up, and offering it would make the most useful state the noisiest.
            State::Ready => !self.is_blocked(task) && !completed() && !task.is_deleted(),
            State::Running => self.assignments_of(task.id).iter().any(|a| a.is_running()),
            State::Started => {
                !completed()
                    && self
                        .assignments_of(task.id)
                        .iter()
                        .any(|a| a.accumulated_mins > 0 || a.is_running())
            }
            State::Unassigned => !self.has_sitting_ahead(task.id, now),
            State::NoDate => task.due.is_none(),
            State::NoLabel => self.snapshot.labels_of(task).is_empty(),
            State::NoProject => {
                self.snapshot.projects.get(&task.project_id).is_none_or(|project| project.is_inbox)
            }
            State::NoEstimate => task.estimate_mins.is_none(),
        }
    }

    /// Every state that holds for a task, in announcement order.
    #[must_use]
    pub fn states_of(&self, task: &Task, now: &Zoned) -> Vec<State> {
        State::ALL.iter().copied().filter(|s| self.has_state(task, *s, now)).collect()
    }

    /// The states worth saying on a list row — see [`State::is_notable`].
    #[must_use]
    pub fn notable_states_of(&self, task: &Task, now: &Zoned) -> Vec<State> {
        State::ALL
            .iter()
            .copied()
            .filter(|s| s.is_notable() && self.has_state(task, *s, now))
            .collect()
    }
}

impl Snapshot {
    /// Indexes the snapshot for asking about many tasks. See [`Facts`].
    #[must_use]
    pub fn facts(&self) -> Facts<'_> {
        Facts::new(self)
    }

    /// The occurrence a task's completion is recorded against, if completions are scoped to
    /// one.
    ///
    /// A recurring task's own due date, since completing it advances the date rather than
    /// ending it (§3.3). A subtask **of** a recurring task recurs with it: "clear the inbox"
    /// under a weekly review is done for this week's review, not forever, so its completion
    /// is scoped to its nearest recurring ancestor's current occurrence. When that ancestor
    /// advances, the subtask is open again — which is what a checklist under a repeating
    /// task is for. A task with no recurrence on or above it is `None`: one completion
    /// finishes it.
    ///
    /// Walks with a visited set, so an unrepaired parent cycle ends the walk.
    #[must_use]
    pub fn occurrence_of(&self, task: &Task) -> Option<civil::Date> {
        let mut seen = std::collections::BTreeSet::new();
        let mut current = Some(task);
        while let Some(t) = current {
            if !seen.insert(t.id) {
                return None;
            }
            if let Some(due) = &t.due
                && due.recurrence.is_some()
            {
                return Some(due.date);
            }
            current = t.parent_id.and_then(|id| self.tasks.get(&id));
        }
        None
    }

    /// The day an assignment's block falls on: its own date for a recurring block's
    /// occurrence, the series' for a one-off — unknown if that series is not loaded.
    #[must_use]
    pub fn assignment_date(&self, assignment: &BlockAssignment) -> Option<civil::Date> {
        assignment.block_ref.date().or_else(|| {
            self.series.get(&assignment.block_ref.series_id()).map(|series| series.start_date)
        })
    }

    /// Whether a task's current occurrence is finished. See [`Facts::is_completed`].
    #[must_use]
    pub fn is_completed(&self, task: &Task) -> bool {
        let occurrence = self.occurrence_of(task);
        self.completions.values().any(|c| {
            c.task_id == task.id && (occurrence.is_none() || c.occurrence_date == occurrence)
        })
    }

    /// The assignments placing a task into blocks, in creation order.
    pub fn assignments_of(&self, task: TaskId) -> impl Iterator<Item = &BlockAssignment> {
        self.assignments.values().filter(move |a| a.task_id == task)
    }

    /// Whether one state holds for a task. See [`Facts::has_state`].
    #[must_use]
    pub fn has_state(&self, task: &Task, state: State, now: &Zoned) -> bool {
        self.facts().has_state(task, state, now)
    }

    /// Whether anything a task depends on is still outstanding. See [`Facts::is_blocked`].
    #[must_use]
    pub fn is_blocked(&self, task: &Task) -> bool {
        self.facts().is_blocked(task)
    }

    /// Whether `from` waits for `to`, directly or through any chain of other tasks.
    ///
    /// Walks with a visited set, so it terminates on a dependency cycle merge produced and
    /// repair has not yet seen.
    #[must_use]
    pub fn waits_for(&self, from: TaskId, to: TaskId) -> bool {
        let mut seen = std::collections::BTreeSet::from([from]);
        let mut queue = vec![from];
        while let Some(id) = queue.pop() {
            let Some(task) = self.tasks.get(&id) else { continue };
            for &next in &task.depends {
                if next == to {
                    return true;
                }
                if seen.insert(next) {
                    queue.push(next);
                }
            }
        }
        false
    }

    /// Every state that holds for a task, in announcement order.
    #[must_use]
    pub fn states_of(&self, task: &Task, now: &Zoned) -> Vec<State> {
        self.facts().states_of(task, now)
    }

    /// The states worth saying on a list row — see [`State::is_notable`].
    #[must_use]
    pub fn notable_states_of(&self, task: &Task, now: &Zoned) -> Vec<State> {
        self.facts().notable_states_of(task, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AssignmentStatus, BlockAssignment, BlockRef, Due, Project, Recurrence, TaskCompletion,
    };
    use crate::order::OrderKey;
    use crate::{SeriesId, TaskId};
    use jiff::civil::{date, time};

    fn now() -> Zoned {
        date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
    }

    fn snapshot() -> (Snapshot, crate::ProjectId, crate::ProjectId) {
        let mut snap = Snapshot::default();
        let inbox = Project::inbox();
        let work = Project::new("Work", OrderKey::middle());
        let (inbox_id, work_id) = (inbox.id, work.id);
        snap.projects.insert(inbox.id, inbox);
        snap.projects.insert(work.id, work);
        (snap, inbox_id, work_id)
    }

    fn task(snap: &mut Snapshot, project: crate::ProjectId, title: &str) -> TaskId {
        let t = Task::new(project, title, OrderKey::middle());
        let id = t.id;
        snap.tasks.insert(id, t);
        id
    }

    #[test]
    fn every_state_has_a_distinct_keyword_that_round_trips() {
        let mut seen = std::collections::BTreeSet::new();
        for state in State::ALL {
            assert!(seen.insert(state.keyword()), "duplicate keyword {}", state.keyword());
            assert_eq!(State::from_keyword(state.keyword()), Some(*state));
        }
        assert_eq!(seen.len(), State::ALL.len());
        // Abbreviations must be distinguishable too, or braille loses information speech has.
        let abbrevs: std::collections::BTreeSet<_> =
            State::ALL.iter().map(|s| s.abbreviation()).collect();
        assert_eq!(abbrevs.len(), State::ALL.len());
    }

    #[test]
    fn a_bare_new_task_is_ready_and_missing_everything() {
        let (mut snap, inbox, _) = snapshot();
        let id = task(&mut snap, inbox, "t");
        let states = snap.states_of(&snap.tasks[&id], &now());
        assert_eq!(
            states,
            vec![
                State::Ready,
                State::Unassigned,
                State::NoDate,
                State::NoProject,
                State::NoLabel,
                State::NoEstimate,
            ]
        );
    }

    #[test]
    fn a_date_only_due_is_overdue_the_day_after_not_at_midnight() {
        let due_today = Due::on(date(2026, 5, 6));
        assert!(!is_overdue(&due_today, &now()), "today is not late");
        assert!(is_overdue(&Due::on(date(2026, 5, 5)), &now()));
    }

    #[test]
    fn a_floating_due_time_is_compared_against_the_local_wall_clock() {
        // 14:30 local. A 15:00 due time has not passed; 14:00 has.
        assert!(!is_overdue(&Due::at(date(2026, 5, 6), time(15, 0, 0, 0)), &now()));
        assert!(is_overdue(&Due::at(date(2026, 5, 6), time(14, 0, 0, 0)), &now()));
    }

    #[test]
    fn an_anchored_due_time_resolves_before_comparing() {
        // 14:30 in New York is 19:30 UTC, so a 15:00 London deadline has already passed
        // even though the wall clock says otherwise.
        let mut due = Due::at(date(2026, 5, 6), time(15, 0, 0, 0));
        due.timezone = Some(crate::model::TzName::new("Europe/London"));
        assert!(is_overdue(&due, &now()));
    }

    #[test]
    fn completing_a_task_removes_ready_and_overdue() {
        let (mut snap, inbox, _) = snapshot();
        let id = task(&mut snap, inbox, "t");
        snap.tasks.get_mut(&id).unwrap().due = Some(Due::on(date(2026, 1, 1)));
        assert!(snap.has_state(&snap.tasks[&id], State::Overdue, &now()));

        let completion = TaskCompletion::new(id, None);
        snap.completions.insert(completion.id, completion);
        let states = snap.states_of(&snap.tasks[&id], &now());
        assert!(states.contains(&State::Completed));
        assert!(!states.contains(&State::Overdue), "a finished task is not late");
        assert!(!states.contains(&State::Ready));
    }

    #[test]
    fn a_recurring_task_is_only_complete_for_the_occurrence_it_is_due_on() {
        let (mut snap, inbox, _) = snapshot();
        let id = task(&mut snap, inbox, "water plants");
        snap.tasks.get_mut(&id).unwrap().due = Some(Due {
            recurrence: Some(Recurrence {
                rrule: "FREQ=DAILY".to_owned(),
                from_completion: false,
            }),
            ..Due::on(date(2026, 5, 6))
        });

        // Yesterday's completion says nothing about today.
        let old = TaskCompletion::new(id, Some(date(2026, 5, 5)));
        snap.completions.insert(old.id, old);
        assert!(!snap.is_completed(&snap.tasks[&id]));

        let today = TaskCompletion::new(id, Some(date(2026, 5, 6)));
        snap.completions.insert(today.id, today);
        assert!(snap.is_completed(&snap.tasks[&id]));
    }

    #[test]
    fn dependencies_decide_blocked_and_ready() {
        let (mut snap, inbox, _) = snapshot();
        let first = task(&mut snap, inbox, "draft");
        let second = task(&mut snap, inbox, "review");
        snap.tasks.get_mut(&second).unwrap().depends.insert(first);

        assert!(snap.has_state(&snap.tasks[&second], State::Blocked, &now()));
        assert!(!snap.has_state(&snap.tasks[&second], State::Ready, &now()));
        assert!(snap.has_state(&snap.tasks[&first], State::Ready, &now()));

        let completion = TaskCompletion::new(first, None);
        snap.completions.insert(completion.id, completion);
        assert!(!snap.has_state(&snap.tasks[&second], State::Blocked, &now()));
        assert!(snap.has_state(&snap.tasks[&second], State::Ready, &now()));
    }

    #[test]
    fn a_dependency_this_device_has_not_merged_does_not_block() {
        // §3.1: a reference into a document that is not loaded is expected, not corrupt.
        // Blocking on it would make a task unstartable for a reason nobody could see.
        let (mut snap, inbox, _) = snapshot();
        let id = task(&mut snap, inbox, "t");
        snap.tasks.get_mut(&id).unwrap().depends.insert(TaskId::new());
        assert!(!snap.has_state(&snap.tasks[&id], State::Blocked, &now()));
    }

    #[test]
    fn a_deleted_dependency_stops_blocking() {
        let (mut snap, inbox, _) = snapshot();
        let first = task(&mut snap, inbox, "draft");
        let second = task(&mut snap, inbox, "review");
        snap.tasks.get_mut(&second).unwrap().depends.insert(first);
        snap.tasks.get_mut(&first).unwrap().deleted_at = Some(crate::time::now());
        assert!(!snap.has_state(&snap.tasks[&second], State::Blocked, &now()));
    }

    #[test]
    fn timers_and_assignments_drive_running_started_and_unassigned() {
        let (mut snap, inbox, _) = snapshot();
        let id = task(&mut snap, inbox, "essay");
        assert!(snap.has_state(&snap.tasks[&id], State::Unassigned, &now()));

        let mut assignment = BlockAssignment::new(
            BlockRef::Occurrence(SeriesId::new(), date(2026, 5, 6)),
            id,
            OrderKey::middle(),
        );
        snap.assignments.insert(assignment.id, assignment.clone());
        assert!(!snap.has_state(&snap.tasks[&id], State::Unassigned, &now()));
        assert!(!snap.has_state(&snap.tasks[&id], State::Started, &now()));

        assignment.accumulated_mins = 25;
        snap.assignments.insert(assignment.id, assignment.clone());
        assert!(snap.has_state(&snap.tasks[&id], State::Started, &now()));
        assert!(!snap.has_state(&snap.tasks[&id], State::Running, &now()));

        assignment.running_since = Some(crate::time::now());
        snap.assignments.insert(assignment.id, assignment);
        assert!(snap.has_state(&snap.tasks[&id], State::Running, &now()));
    }

    #[test]
    fn unassigned_means_no_sitting_ahead_not_never_scheduled() {
        // Today is 2026-05-06. A task worked last week and not finished, or one whose sitting
        // yesterday never happened, is exactly what auto-suggestion has to find again.
        let (mut snap, inbox, _) = snapshot();
        let id = task(&mut snap, inbox, "essay");
        let sitting = |snap: &mut Snapshot, day: i8, status: AssignmentStatus| {
            let mut a = BlockAssignment::new(
                BlockRef::Occurrence(SeriesId::new(), date(2026, 5, day)),
                id,
                OrderKey::middle(),
            );
            a.status = status;
            snap.assignments.insert(a.id, a.clone());
            a
        };
        sitting(&mut snap, 1, AssignmentStatus::Worked);
        sitting(&mut snap, 5, AssignmentStatus::Planned);
        assert!(snap.has_state(&snap.tasks[&id], State::Unassigned, &now()), "both are behind it");

        let today = sitting(&mut snap, 6, AssignmentStatus::Planned);
        assert!(!snap.has_state(&snap.tasks[&id], State::Unassigned, &now()), "today counts");
        snap.assignments.remove(&today.id);

        let mut forgotten = sitting(&mut snap, 4, AssignmentStatus::InProgress);
        forgotten.running_since = Some(crate::time::now());
        snap.assignments.insert(forgotten.id, forgotten);
        assert!(
            !snap.has_state(&snap.tasks[&id], State::Unassigned, &now()),
            "a running timer is a sitting under way, whatever day its block was"
        );
    }

    #[test]
    fn a_subtask_of_a_recurring_task_is_complete_only_for_the_current_occurrence() {
        let (mut snap, inbox, _) = snapshot();
        let review = task(&mut snap, inbox, "weekly review");
        snap.tasks.get_mut(&review).unwrap().due = Some(Due {
            recurrence: Some(Recurrence { rrule: "FREQ=WEEKLY".to_owned(), from_completion: false }),
            ..Due::on(date(2026, 5, 6))
        });
        let step = task(&mut snap, inbox, "clear the inbox");
        snap.tasks.get_mut(&step).unwrap().parent_id = Some(review);

        let done = TaskCompletion::new(step, Some(date(2026, 5, 6)));
        snap.completions.insert(done.id, done);
        assert!(snap.is_completed(&snap.tasks[&step]));

        // The review comes round again; this week's step is open.
        snap.tasks.get_mut(&review).unwrap().due.as_mut().unwrap().date = date(2026, 5, 13);
        assert!(!snap.is_completed(&snap.tasks[&step]));
    }

    #[test]
    fn the_index_and_the_direct_answer_agree() {
        let (mut snap, inbox, _) = snapshot();
        let a = task(&mut snap, inbox, "a");
        let b = task(&mut snap, inbox, "b");
        snap.tasks.get_mut(&b).unwrap().depends.insert(a);
        let completion = TaskCompletion::new(a, None);
        snap.completions.insert(completion.id, completion);

        let facts = snap.facts();
        for id in [a, b] {
            let t = &snap.tasks[&id];
            assert_eq!(facts.is_completed(t), snap.is_completed(t));
            assert_eq!(facts.states_of(t, &now()), snap.states_of(t, &now()));
            assert_eq!(facts.assignments_of(id).len(), snap.assignments_of(id).count());
        }
    }

    #[test]
    fn the_inbox_is_what_no_project_means() {
        let (mut snap, inbox, work) = snapshot();
        let captured = task(&mut snap, inbox, "captured");
        let filed = task(&mut snap, work, "filed");
        assert!(snap.has_state(&snap.tasks[&captured], State::NoProject, &now()));
        assert!(!snap.has_state(&snap.tasks[&filed], State::NoProject, &now()));
    }
}
