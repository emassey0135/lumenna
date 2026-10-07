//! Mutations: the rules that no caller should have to remember.

use jiff::Zoned;
use jiff::civil::date;
use lumenna_core::edit::{
    Change, Edit, EditError, MoveTo, ProjectDeletion, add_dependency, adopt_inbox, assign_task,
    complete_task, create_task, except_occurrence, log_minutes, merge_labels, move_task, plan_minutes,
    purge_task, remove_dependency, restore_occurrence, restore_task, start_timer, stop_timer,
    trash_project, trash_task, uncomplete_task, unassign, update_series, update_task,
};
use lumenna_core::id::{LabelId, ProjectId, TaskId};
use lumenna_core::model::{
    BlockKind, BlockRef, BlockSeries, Due, ExceptionAction, Label, Priority, Project, Recurrence,
    Task, TaskCompletion,
};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_core::{SeriesId, State};

/// One mutation, deferred so the same list can be replayed against a changing snapshot.
type Operation = Box<dyn Fn(&Snapshot) -> Edit>;

fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

struct World {
    snapshot: Snapshot,
    inbox: ProjectId,
    work: ProjectId,
}

impl World {
    fn new() -> Self {
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let work = Project::new("Work", OrderKey::middle());
        let (inbox_id, work_id) = (inbox.id, work.id);
        snapshot.projects.insert(inbox.id, inbox);
        snapshot.projects.insert(work.id, work);
        Self { snapshot, inbox: inbox_id, work: work_id }
    }

    fn add(&mut self, title: &str) -> TaskId {
        let task = Task::new(self.work, title, OrderKey::middle());
        let id = task.id;
        self.snapshot.tasks.insert(id, task);
        id
    }

    fn child_of(&mut self, parent: TaskId, title: &str) -> TaskId {
        let id = self.add(title);
        self.snapshot.tasks.get_mut(&id).unwrap().parent_id = Some(parent);
        id
    }

    /// A block every day from the first of May, which is what most assignments here need.
    fn daily_block(&mut self) -> SeriesId {
        self.block(BlockKind::Work, Some("FREQ=DAILY"))
    }

    fn block(&mut self, kind: BlockKind, rrule: Option<&str>) -> SeriesId {
        let mut series = BlockSeries::one_off(
            "Focus",
            kind,
            date(2026, 5, 1),
            jiff::civil::time(9, 0, 0, 0),
            90,
        )
        .unwrap();
        if let Some(rule) = rrule {
            series.rrule = Some(rule.to_owned());
            series.end_date = None;
        }
        let id = series.id;
        self.snapshot.series.insert(id, series);
        id
    }

    fn label(&mut self, name: &str) -> LabelId {
        let label = Label::new(name, OrderKey::middle());
        let id = label.id;
        self.snapshot.labels.insert(id, label);
        id
    }

    /// Applies an edit to the snapshot directly, which is what `store` does for real.
    fn apply(&mut self, edit: &Edit) {
        for change in &edit.changes {
            match change {
                Change::Task(t) => match (&t.before, &t.after) {
                    (_, Some(after)) => {
                        self.snapshot.tasks.insert(after.id, after.clone());
                    }
                    (Some(before), None) => {
                        self.snapshot.tasks.remove(&before.id);
                    }
                    (None, None) => {}
                },
                Change::Completion(t) => match (&t.before, &t.after) {
                    (_, Some(after)) => {
                        self.snapshot.completions.insert(after.id, after.clone());
                    }
                    (Some(before), None) => {
                        self.snapshot.completions.remove(&before.id);
                    }
                    (None, None) => {}
                },
                Change::Project(t) => match (&t.before, &t.after) {
                    (_, Some(after)) => {
                        self.snapshot.projects.insert(after.id, after.clone());
                    }
                    (Some(before), None) => {
                        self.snapshot.projects.remove(&before.id);
                    }
                    (None, None) => {}
                },
                Change::Label(t) => match (&t.before, &t.after) {
                    (_, Some(after)) => {
                        self.snapshot.labels.insert(after.id, after.clone());
                    }
                    (Some(before), None) => {
                        self.snapshot.labels.remove(&before.id);
                    }
                    (None, None) => {}
                },
                Change::Assignment { transition, .. } => {
                    match (&transition.before, &transition.after) {
                        (_, Some(after)) => {
                            self.snapshot.assignments.insert(after.id, after.clone());
                        }
                        (Some(before), None) => {
                            self.snapshot.assignments.remove(&before.id);
                        }
                        (None, None) => {}
                    }
                }
                Change::Settings { after, .. } => self.snapshot.settings = (**after).clone(),
                Change::Exception(t) => match (&t.before, &t.after) {
                    (_, Some(after)) => {
                        self.snapshot
                            .exceptions
                            .insert((after.series_id, after.original_date), after.clone());
                    }
                    (Some(before), None) => {
                        self.snapshot.exceptions.remove(&(before.series_id, before.original_date));
                    }
                    (None, None) => {}
                },
                Change::Series(t) => {
                    if let Some(after) = &t.after {
                        self.snapshot.series.insert(after.id, after.clone());
                    }
                }
                _ => unreachable!("not used by these tests"),
            }
        }
    }

    fn task(&self, id: TaskId) -> &Task {
        &self.snapshot.tasks[&id]
    }

    fn is_done(&self, id: TaskId) -> bool {
        self.snapshot.is_completed(self.task(id))
    }
}

fn recurring(rrule: &str, on: jiff::civil::Date, from_completion: bool) -> Due {
    Due {
        recurrence: Some(Recurrence { rrule: rrule.to_owned(), from_completion }),
        ..Due::on(on)
    }
}

// ---------------------------------------------------------------------------------------
// Completing
// ---------------------------------------------------------------------------------------

#[test]
fn completing_a_plain_task_records_one_completion() {
    let mut world = World::new();
    let id = world.add("Review PR");
    let edit = complete_task(&world.snapshot, id, &now()).unwrap();
    assert_eq!(edit.description, "Completed Review PR");
    assert_eq!(edit.changes.len(), 1);

    world.apply(&edit);
    assert!(world.is_done(id));
}

#[test]
fn completing_twice_is_refused_rather_than_recorded_twice() {
    let mut world = World::new();
    let id = world.add("Review PR");
    let edit = complete_task(&world.snapshot, id, &now()).unwrap();
    world.apply(&edit);
    assert_eq!(complete_task(&world.snapshot, id, &now()), Err(EditError::AlreadyComplete));
}

#[test]
fn subtasks_cascade_when_the_setting_says_so() {
    let mut world = World::new();
    let parent = world.add("Ship release");
    let a = world.child_of(parent, "Write notes");
    let b = world.child_of(parent, "Tag version");
    let deep = world.child_of(a, "Proofread");

    let edit = complete_task(&world.snapshot, parent, &now()).unwrap();
    world.apply(&edit);

    assert!(world.is_done(parent));
    assert!(world.is_done(a));
    assert!(world.is_done(b));
    assert!(world.is_done(deep), "the cascade reaches any depth");
}

#[test]
fn the_cascade_can_be_turned_off() {
    let mut world = World::new();
    world.snapshot.settings.cascade_complete_subtasks = false;
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Write notes");

    let edit = complete_task(&world.snapshot, parent, &now()).unwrap();
    world.apply(&edit);
    assert!(world.is_done(parent));
    assert!(!world.is_done(child));
}

#[test]
fn uncompleting_reverses_only_the_completions_the_cascade_caused() {
    // A subtask you independently finished last week must not be uncompleted because you
    // changed your mind about the parent.
    let mut world = World::new();
    let parent = world.add("Ship release");
    let independent = world.child_of(parent, "Write notes");
    let cascaded = world.child_of(parent, "Tag version");

    // Finished on its own, before the parent was touched.
    let own = TaskCompletion::new(independent, None);
    world.snapshot.completions.insert(own.id, own);

    let edit = complete_task(&world.snapshot, parent, &now()).unwrap();
    world.apply(&edit);
    assert!(world.is_done(cascaded));

    let edit = uncomplete_task(&world.snapshot, parent).unwrap();
    world.apply(&edit);

    assert!(!world.is_done(parent));
    assert!(!world.is_done(cascaded), "this one was only done because the parent was");
    assert!(world.is_done(independent), "this one was finished on its own");
}

#[test]
fn uncompleting_a_recurring_parent_leaves_earlier_occurrences_cascades_alone() {
    // Each occurrence of a recurring parent is its own completion, and its cascade belongs
    // to it. Reversing this week's must not reach back and reopen last week's subtasks.
    let mut world = World::new();
    let parent = world.add("Weekly review");
    world.snapshot.tasks.get_mut(&parent).unwrap().due =
        Some(recurring("FREQ=WEEKLY", date(2026, 4, 29), false));
    let child = world.child_of(parent, "Clear the inbox");

    // Last week's completion, and the cascade it caused, recorded a week ago.
    let week_ago = jiff::Timestamp::from_second(1_777_000_000).unwrap();
    let mut old = TaskCompletion::new(parent, Some(date(2026, 4, 29)));
    old.completed_at = week_ago;
    let mut old_cascade = TaskCompletion::cascaded(child, None, parent);
    old_cascade.completed_at = week_ago;
    world.snapshot.completions.insert(old.id, old);
    world.snapshot.completions.insert(old_cascade.id, old_cascade.clone());
    world.snapshot.tasks.get_mut(&parent).unwrap().due =
        Some(recurring("FREQ=WEEKLY", date(2026, 5, 6), false));

    // Uncompleting the child's cascade lets this week's completion cascade afresh.
    world.snapshot.completions.remove(&old_cascade.id);
    world.apply(&complete_task(&world.snapshot, parent, &now()).unwrap());
    world.snapshot.completions.insert(old_cascade.id, old_cascade.clone());
    let cascades = |w: &World| {
        w.snapshot.completions.values().filter(|c| c.cascaded_from == Some(parent)).count()
    };
    assert_eq!(cascades(&world), 2);

    world.apply(&uncomplete_task(&world.snapshot, parent).unwrap());
    assert_eq!(cascades(&world), 1, "only this week's cascade goes");
    assert!(world.snapshot.completions.contains_key(&old_cascade.id));
}

#[test]
fn a_recurring_tasks_checklist_reopens_when_it_comes_round_again() {
    // Completing a weekly review cascades to its steps for *this* week. Next week they are
    // open again — otherwise the checklist would be empty from the second week on.
    let mut world = World::new();
    let review = world.add("Weekly review");
    world.snapshot.tasks.get_mut(&review).unwrap().due =
        Some(recurring("FREQ=WEEKLY", date(2026, 5, 6), false));
    let step = world.child_of(review, "Clear the inbox");

    world.apply(&complete_task(&world.snapshot, review, &now()).unwrap());
    assert_eq!(world.task(review).due.as_ref().unwrap().date, date(2026, 5, 13));
    assert!(!world.is_done(step), "this week's step is open");

    let history = world.snapshot.completions.values().filter(|c| c.task_id == step).count();
    assert_eq!(history, 1, "last week's step is still on record");

    // Undoing the review takes it back to last week, and the step with it.
    world.apply(&uncomplete_task(&world.snapshot, review).unwrap());
    assert_eq!(world.task(review).due.as_ref().unwrap().date, date(2026, 5, 6));
    assert!(!world.is_done(step));
    assert!(world.task(step).due.is_none(), "the subtask's own date is untouched");
}

#[test]
fn a_subtask_completed_by_its_parent_can_be_uncompleted_on_its_own() {
    let mut world = World::new();
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Tag version");
    world.apply(&complete_task(&world.snapshot, parent, &now()).unwrap());
    assert!(world.is_done(child));

    world.apply(&uncomplete_task(&world.snapshot, child).unwrap());
    assert!(!world.is_done(child));
    assert!(world.is_done(parent), "the parent's own completion is untouched");
}

#[test]
fn uncompleting_something_that_was_never_done_is_refused() {
    let mut world = World::new();
    let id = world.add("Review PR");
    assert_eq!(uncomplete_task(&world.snapshot, id), Err(EditError::NotComplete));
}

// ---------------------------------------------------------------------------------------
// Recurrence
// ---------------------------------------------------------------------------------------

#[test]
fn completing_a_recurring_task_advances_it_instead_of_ending_it() {
    let mut world = World::new();
    let id = world.add("Water plants");
    world.snapshot.tasks.get_mut(&id).unwrap().due =
        Some(recurring("FREQ=DAILY;INTERVAL=3", date(2026, 5, 6), false));

    let edit = complete_task(&world.snapshot, id, &now()).unwrap();
    world.apply(&edit);

    assert_eq!(world.task(id).due.as_ref().unwrap().date, date(2026, 5, 9));
    assert!(!world.is_done(id), "it is due again, so it is not finished");
    assert_eq!(world.snapshot.completion_count(id), 1, "but the sitting was recorded");
}

#[test]
fn the_completion_names_the_occurrence_it_closed() {
    let mut world = World::new();
    let id = world.add("Water plants");
    world.snapshot.tasks.get_mut(&id).unwrap().due =
        Some(recurring("FREQ=DAILY", date(2026, 5, 6), false));

    let edit = complete_task(&world.snapshot, id, &now()).unwrap();
    world.apply(&edit);
    let completion = world.snapshot.completions.values().next().unwrap();
    assert_eq!(completion.occurrence_date, Some(date(2026, 5, 6)));
}

#[test]
fn uncompleting_a_recurring_task_rolls_the_date_back_to_the_occurrence() {
    let mut world = World::new();
    let id = world.add("Water plants");
    world.snapshot.tasks.get_mut(&id).unwrap().due =
        Some(recurring("FREQ=DAILY;INTERVAL=3", date(2026, 5, 6), false));

    world.apply(&complete_task(&world.snapshot, id, &now()).unwrap());
    assert_eq!(world.task(id).due.as_ref().unwrap().date, date(2026, 5, 9));

    world.apply(&uncomplete_task(&world.snapshot, id).unwrap());
    assert_eq!(world.task(id).due.as_ref().unwrap().date, date(2026, 5, 6));
    assert_eq!(world.snapshot.completion_count(id), 0);
}

#[test]
fn a_count_bounded_task_stops_when_it_runs_out() {
    let mut world = World::new();
    let id = world.add("Take the course");
    world.snapshot.tasks.get_mut(&id).unwrap().due =
        Some(recurring("FREQ=DAILY;COUNT=3", date(2026, 5, 6), false));

    for _ in 0..3 {
        let edit = complete_task(&world.snapshot, id, &now()).unwrap();
        world.apply(&edit);
    }
    assert_eq!(world.snapshot.completion_count(id), 3);
    assert!(world.is_done(id), "the third completion is the last, so it stays done");
}

#[test]
fn a_cascaded_recurring_subtask_does_not_come_back() {
    // The cascade means "the parent is finished, so these are too". Advancing a subtask
    // would resurrect the very thing that was just closed out.
    let mut world = World::new();
    let parent = world.add("Close the sprint");
    let child = world.child_of(parent, "Daily standup note");
    world.snapshot.tasks.get_mut(&child).unwrap().due =
        Some(recurring("FREQ=DAILY", date(2026, 5, 6), false));

    world.apply(&complete_task(&world.snapshot, parent, &now()).unwrap());
    assert_eq!(world.task(child).due.as_ref().unwrap().date, date(2026, 5, 6));
    assert!(world.is_done(child));
}

// ---------------------------------------------------------------------------------------
// Trash, restore, purge
// ---------------------------------------------------------------------------------------

#[test]
fn trashing_takes_the_subtree_and_restoring_brings_it_back() {
    let mut world = World::new();
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Write notes");

    world.apply(&trash_task(&world.snapshot, parent).unwrap());
    assert!(world.task(parent).is_deleted());
    assert!(world.task(child).is_deleted(), "a subtask left behind is unreachable");

    world.apply(&restore_task(&world.snapshot, parent).unwrap());
    assert!(!world.task(parent).is_deleted());
    assert!(!world.task(child).is_deleted());
}

#[test]
fn restoring_leaves_alone_what_was_already_in_the_trash() {
    let mut world = World::new();
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Abandoned earlier");

    world.apply(&trash_task(&world.snapshot, child).unwrap());
    world.apply(&trash_task(&world.snapshot, parent).unwrap());
    world.apply(&restore_task(&world.snapshot, parent).unwrap());

    assert!(!world.task(parent).is_deleted());
    // The child came back too, because trashing the parent trashed it a second time. That
    // is the honest outcome of a subtree operation, and undo is what recovers the nuance.
    assert!(!world.task(child).is_deleted());
}

#[test]
fn purging_takes_the_completions_with_it() {
    let mut world = World::new();
    let id = world.add("Review PR");
    world.apply(&complete_task(&world.snapshot, id, &now()).unwrap());
    assert_eq!(world.snapshot.completions.len(), 1);

    world.apply(&purge_task(&world.snapshot, id).unwrap());
    assert!(world.snapshot.tasks.is_empty());
    assert!(world.snapshot.completions.is_empty(), "orphaned history is not history");
}

// ---------------------------------------------------------------------------------------
// Moving
// ---------------------------------------------------------------------------------------

#[test]
fn reparenting_into_a_descendant_is_refused() {
    // Merge can still produce a cycle from two concurrent moves, and repair exists for that.
    // A local move that closes one is a mistake this device can see.
    let mut world = World::new();
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Write notes");
    let grandchild = world.child_of(child, "Proofread");

    assert_eq!(
        move_task(&world.snapshot, parent, MoveTo::Parent(Some(grandchild))),
        Err(EditError::WouldCycle)
    );
    assert_eq!(
        move_task(&world.snapshot, parent, MoveTo::Parent(Some(parent))),
        Err(EditError::WouldCycle)
    );
    assert!(move_task(&world.snapshot, grandchild, MoveTo::Parent(None)).is_ok());
}

#[test]
fn changing_project_takes_the_subtree() {
    let mut world = World::new();
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Write notes");
    let inbox = world.inbox;

    world.apply(&move_task(&world.snapshot, parent, MoveTo::Project(inbox)).unwrap());
    assert_eq!(world.task(parent).project_id, inbox);
    assert_eq!(world.task(child).project_id, inbox, "a split subtree has no view");
}

#[test]
fn a_subtask_moved_to_another_project_leaves_its_parent_behind() {
    let mut world = World::new();
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Write notes");
    let grandchild = world.child_of(child, "Proofread");
    let inbox = world.inbox;

    world.apply(&move_task(&world.snapshot, child, MoveTo::Project(inbox)).unwrap());
    assert_eq!(world.task(child).parent_id, None, "its parent stayed in Work");
    assert_eq!(world.task(grandchild).parent_id, Some(child), "its own subtree came along");
    assert_eq!(world.task(grandchild).project_id, inbox);
    assert_eq!(world.task(parent).project_id, world.work);
}

#[test]
fn a_task_put_under_a_parent_joins_the_parents_project() {
    let mut world = World::new();
    let parent = world.add("Ship release");
    let stray = world.add("Write notes");
    let under = world.child_of(stray, "Proofread");
    let inbox = world.inbox;
    for id in [stray, under] {
        world.snapshot.tasks.get_mut(&id).unwrap().project_id = inbox;
    }

    world.apply(&move_task(&world.snapshot, stray, MoveTo::Parent(Some(parent))).unwrap());
    assert_eq!(world.task(stray).parent_id, Some(parent));
    assert_eq!(world.task(stray).project_id, world.work);
    assert_eq!(world.task(under).project_id, world.work);
}

#[test]
fn reordering_puts_a_task_between_its_neighbours() {
    let mut world = World::new();
    let mut order = OrderKey::middle();
    let mut ids = Vec::new();
    for title in ["first", "second", "third"] {
        let mut task = Task::new(world.work, title, order.clone());
        order = OrderKey::after(&order);
        ids.push(task.id);
        task.project_id = world.work;
        world.snapshot.tasks.insert(task.id, task);
    }

    let edit =
        move_task(&world.snapshot, ids[2], MoveTo::Between { after: Some(ids[0]), before: Some(ids[1]) })
            .unwrap();
    world.apply(&edit);

    assert!(world.task(ids[0]).order < world.task(ids[2]).order);
    assert!(world.task(ids[2]).order < world.task(ids[1]).order);
}

#[test]
fn a_move_that_changes_nothing_produces_no_edit() {
    let mut world = World::new();
    let id = world.add("Review PR");
    let work = world.work;
    assert!(move_task(&world.snapshot, id, MoveTo::Project(work)).unwrap().is_empty());
    assert!(move_task(&world.snapshot, id, MoveTo::Parent(None)).unwrap().is_empty());
    assert!(update_task(world.task(id).clone(), world.task(id).clone()).is_empty());
}

// ---------------------------------------------------------------------------------------
// Projects and labels
// ---------------------------------------------------------------------------------------

#[test]
fn deleting_a_project_offers_both_intentions() {
    let mut world = World::new();
    let id = world.add("Review PR");
    let work = world.work;

    let mut moved = World { snapshot: world.snapshot.clone(), ..World::new() };
    moved.inbox = world.inbox;
    moved.apply(
        &trash_project(&moved.snapshot, work, ProjectDeletion::MoveTasksToInbox).unwrap(),
    );
    assert_eq!(moved.task(id).project_id, moved.inbox);
    assert!(!moved.task(id).is_deleted());

    world.apply(&trash_project(&world.snapshot, work, ProjectDeletion::TrashTasks).unwrap());
    assert!(world.task(id).is_deleted());
    assert!(world.snapshot.projects[&work].deleted_at.is_some());
}

#[test]
fn deleting_a_project_takes_its_sub_projects_too() {
    let mut world = World::new();
    let mut backend = Project::new("Backend", OrderKey::middle());
    backend.parent_id = Some(world.work);
    let backend_id = backend.id;
    world.snapshot.projects.insert(backend.id, backend);

    let work = world.work;
    world.apply(&trash_project(&world.snapshot, work, ProjectDeletion::TrashTasks).unwrap());
    assert!(world.snapshot.projects[&backend_id].deleted_at.is_some());
}

#[test]
fn merging_labels_rewrites_the_tasks_and_retires_the_loser() {
    // Cheap with records; impossible with plain strings, where the two tags were never
    // distinguishable from intent in the first place.
    let mut world = World::new();
    let laptop = world.label("laptop");
    let lapto = world.label("lapto");
    let a = world.add("Write");
    let b = world.add("Edit");
    world.snapshot.tasks.get_mut(&a).unwrap().labels.insert(lapto);
    world.snapshot.tasks.get_mut(&b).unwrap().labels.insert(laptop);

    let edit = merge_labels(&world.snapshot, lapto, laptop).unwrap();
    assert_eq!(edit.description, "Merged label lapto into laptop");
    world.apply(&edit);

    assert!(world.task(a).labels.contains(&laptop));
    assert!(!world.task(a).labels.contains(&lapto));
    assert!(world.task(b).labels.contains(&laptop));
    assert!(world.snapshot.labels[&lapto].deleted_at.is_some());
    assert_eq!(world.snapshot.labels_of(world.task(a)).len(), 1);
}

// ---------------------------------------------------------------------------------------
// Assignments and timers
// ---------------------------------------------------------------------------------------

#[test]
fn a_task_can_be_scheduled_into_several_sittings() {
    // Planning three sittings for a long essay up front is a first-class use case, not an
    // accident to prevent.
    let mut world = World::new();
    let id = world.add("Write the essay");
    let series = world.daily_block();

    for day in 6..9 {
        let block = BlockRef::Occurrence(series, date(2026, 5, day));
        world.apply(&assign_task(&world.snapshot, id, block, 2026).unwrap());
    }
    assert_eq!(world.snapshot.assignments_of(id).count(), 3);
    assert!(!world.snapshot.has_state(world.task(id), State::Unassigned, &now()));
}

#[test]
fn timers_record_a_fact_and_stopping_logs_the_minutes() {
    let mut world = World::new();
    let id = world.add("Write the essay");
    let block = BlockRef::Occurrence(world.daily_block(), date(2026, 5, 6));
    world.apply(&assign_task(&world.snapshot, id, block, 2026).unwrap());
    let assignment_id = *world.snapshot.assignments.keys().next().unwrap();

    world.apply(&start_timer(&world.snapshot, assignment_id, 2026, &now()).unwrap());
    assert!(world.snapshot.assignments[&assignment_id].is_running());
    assert!(world.snapshot.has_state(world.task(id), State::Running, &now()));
    assert_eq!(world.snapshot.assignments[&assignment_id].accumulated_mins, 0);

    let later = now().checked_add(jiff::Span::new().minutes(43)).unwrap();
    let (edit, elapsed) =
        stop_timer(&world.snapshot, assignment_id, 2026, Some(90), &later).unwrap();
    assert_eq!(elapsed.mins, 43);
    assert!(!elapsed.capped);
    world.apply(&edit);

    assert_eq!(world.snapshot.assignments[&assignment_id].accumulated_mins, 43);
    assert!(!world.snapshot.assignments[&assignment_id].is_running());
    assert!(world.snapshot.has_state(world.task(id), State::Started, &now()));
}

#[test]
fn an_orphaned_timer_is_capped_and_the_caller_is_told() {
    let mut world = World::new();
    let id = world.add("Write the essay");
    let block = BlockRef::Occurrence(world.daily_block(), date(2026, 5, 6));
    world.apply(&assign_task(&world.snapshot, id, block, 2026).unwrap());
    let assignment_id = *world.snapshot.assignments.keys().next().unwrap();
    world.apply(&start_timer(&world.snapshot, assignment_id, 2026, &now()).unwrap());

    let next_day = now().checked_add(jiff::Span::new().hours(19)).unwrap();
    let (_, elapsed) =
        stop_timer(&world.snapshot, assignment_id, 2026, Some(90), &next_day).unwrap();
    assert_eq!(elapsed.mins, 90);
    assert!(elapsed.capped, "confirm rather than record nineteen hours silently");
}

#[test]
fn a_block_has_to_happen_and_take_tasks_to_be_scheduled_into() {
    let mut world = World::new();
    let id = world.add("Write the essay");

    let weekly = world.block(BlockKind::Work, Some("FREQ=WEEKLY"));
    // The first of May 2026 is a Friday; the sixth is a Wednesday it skips.
    assert_eq!(
        assign_task(&world.snapshot, id, BlockRef::Occurrence(weekly, date(2026, 5, 6)), 2026),
        Err(EditError::NoOccurrence { date: date(2026, 5, 6) })
    );
    assert!(
        assign_task(&world.snapshot, id, BlockRef::Occurrence(weekly, date(2026, 5, 8)), 2026)
            .is_ok()
    );

    let lunch = world.block(BlockKind::Break, None);
    assert_eq!(
        assign_task(&world.snapshot, id, BlockRef::OneOff(lunch), 2026),
        Err(EditError::RefusesTasks)
    );
    assert_eq!(
        assign_task(&world.snapshot, id, BlockRef::OneOff(SeriesId::new()), 2026),
        Err(EditError::NotFound { kind: "block" })
    );

    world.apply(&trash_task(&world.snapshot, id).unwrap());
    let focus = world.daily_block();
    assert_eq!(
        assign_task(&world.snapshot, id, BlockRef::Occurrence(focus, date(2026, 5, 6)), 2026),
        Err(EditError::Trashed)
    );
}

#[test]
fn stopping_a_timer_that_is_not_running_changes_nothing() {
    let mut world = World::new();
    let id = world.add("Write the essay");
    let block = BlockRef::Occurrence(world.daily_block(), date(2026, 5, 6));
    world.apply(&assign_task(&world.snapshot, id, block, 2026).unwrap());
    let assignment_id = *world.snapshot.assignments.keys().next().unwrap();

    let (edit, elapsed) =
        stop_timer(&world.snapshot, assignment_id, 2026, Some(90), &now()).unwrap();
    assert!(edit.is_empty());
    assert_eq!(elapsed.mins, 0);
}

#[test]
fn logged_minutes_replace_a_capped_figure() {
    let mut world = World::new();
    let id = world.add("Write the essay");
    let block = BlockRef::Occurrence(world.daily_block(), date(2026, 5, 6));
    world.apply(&assign_task(&world.snapshot, id, block, 2026).unwrap());
    let assignment_id = *world.snapshot.assignments.keys().next().unwrap();
    world.apply(&start_timer(&world.snapshot, assignment_id, 2026, &now()).unwrap());

    world.apply(&log_minutes(&world.snapshot, assignment_id, 2026, 50).unwrap());
    let assignment = &world.snapshot.assignments[&assignment_id];
    assert_eq!(assignment.accumulated_mins, 50);
    assert!(!assignment.is_running());
    assert!(log_minutes(&world.snapshot, assignment_id, 2026, 50).unwrap().is_empty());
}

#[test]
fn planning_a_sittings_length_leaves_what_was_logged_alone() {
    let mut world = World::new();
    let id = world.add("Write the essay");
    let block = BlockRef::Occurrence(world.daily_block(), date(2026, 5, 6));
    world.apply(&assign_task(&world.snapshot, id, block, 2026).unwrap());
    let assignment_id = *world.snapshot.assignments.keys().next().unwrap();
    world.apply(&log_minutes(&world.snapshot, assignment_id, 2026, 20).unwrap());

    world.apply(&plan_minutes(&world.snapshot, assignment_id, 2026, Some(45)).unwrap());
    let assignment = &world.snapshot.assignments[&assignment_id];
    assert_eq!((assignment.planned_mins, assignment.accumulated_mins), (Some(45), 20));
    assert!(plan_minutes(&world.snapshot, assignment_id, 2026, Some(45)).unwrap().is_empty());

    world.apply(&plan_minutes(&world.snapshot, assignment_id, 2026, None).unwrap());
    assert_eq!(world.snapshot.assignments[&assignment_id].planned_mins, None);
}

#[test]
fn unassigning_removes_the_sitting() {
    let mut world = World::new();
    let id = world.add("Write the essay");
    let block = BlockRef::Occurrence(world.daily_block(), date(2026, 5, 6));
    world.apply(&assign_task(&world.snapshot, id, block, 2026).unwrap());
    let assignment_id = *world.snapshot.assignments.keys().next().unwrap();

    world.apply(&unassign(&world.snapshot, assignment_id, 2026).unwrap());
    assert!(world.snapshot.assignments.is_empty());
}

// ---------------------------------------------------------------------------------------
// Dependencies and the Inbox
// ---------------------------------------------------------------------------------------

#[test]
fn a_dependency_that_closes_a_cycle_of_any_length_is_refused() {
    let mut world = World::new();
    let a = world.add("Draft");
    let b = world.add("Review");
    let c = world.add("Publish");
    world.apply(&add_dependency(&world.snapshot, b, a).unwrap());
    world.apply(&add_dependency(&world.snapshot, c, b).unwrap());

    assert_eq!(add_dependency(&world.snapshot, a, c), Err(EditError::DependencyCycle));
    assert_eq!(add_dependency(&world.snapshot, a, a), Err(EditError::DependencyCycle));
    assert!(add_dependency(&world.snapshot, c, a).is_ok(), "a shortcut is not a cycle");

    world.apply(&remove_dependency(&world.snapshot, b, a).unwrap());
    assert!(add_dependency(&world.snapshot, a, c).is_ok(), "nothing closes it any more");
}

#[test]
fn a_legacy_inbox_is_folded_into_the_canonical_one() {
    let mut world = World::new();
    let legacy = Project { id: ProjectId::new(), ..Project::inbox() };
    let legacy_id = legacy.id;
    world.snapshot.projects.insert(legacy_id, legacy);
    let mut task = Task::new(legacy_id, "captured on the old Inbox", OrderKey::middle());
    task.id = TaskId::new();
    let task_id = task.id;
    world.snapshot.tasks.insert(task_id, task);

    let edit = adopt_inbox(&world.snapshot);
    world.apply(&edit);
    assert_eq!(world.task(task_id).project_id, ProjectId::INBOX);
    assert!(!world.snapshot.projects[&legacy_id].is_inbox);
    assert!(world.snapshot.projects[&legacy_id].deleted_at.is_some());
    assert_eq!(world.snapshot.inbox().map(|p| p.id), Some(ProjectId::INBOX));
    assert!(adopt_inbox(&world.snapshot).is_empty(), "and once is enough");
}

// ---------------------------------------------------------------------------------------
// Undo
// ---------------------------------------------------------------------------------------

#[test]
fn every_operation_inverts_back_to_where_it_started() {
    // Automerge provides history, not undo. Undo means computing an inverse, and an edit
    // that carries both sides of every record is its own inverse when swapped.
    let mut world = World::new();
    let parent = world.add("Ship release");
    let child = world.child_of(parent, "Write notes");
    let laptop = world.label("laptop");
    let lapto = world.label("lapto");
    world.snapshot.tasks.get_mut(&child).unwrap().labels.insert(lapto);
    world.snapshot.tasks.get_mut(&parent).unwrap().due =
        Some(recurring("FREQ=WEEKLY", date(2026, 5, 6), false));
    let work = world.work;
    let inbox = world.inbox;
    let focus = world.daily_block();

    let operations: Vec<Operation> = vec![
        Box::new(move |s| complete_task(s, parent, &now()).unwrap()),
        Box::new(move |s| trash_task(s, child).unwrap()),
        Box::new(move |s| move_task(s, child, MoveTo::Project(inbox)).unwrap()),
        Box::new(move |s| move_task(s, child, MoveTo::Parent(None)).unwrap()),
        Box::new(move |s| merge_labels(s, lapto, laptop).unwrap()),
        Box::new(move |s| trash_project(s, work, ProjectDeletion::TrashTasks).unwrap()),
        Box::new(move |s| {
            let mut edited = s.tasks[&parent].clone();
            edited.priority = Priority::P1;
            edited.title = "Renamed".to_owned();
            update_task(s.tasks[&parent].clone(), edited)
        }),
        Box::new(move |s| {
            assign_task(s, child, BlockRef::Occurrence(focus, date(2026, 5, 6)), 2026).unwrap()
        }),
    ];

    for operation in operations {
        let before = world.snapshot.clone();
        let edit = operation(&world.snapshot);
        let description = edit.description.clone();
        world.apply(&edit);
        world.apply(&edit.inverse());

        assert_eq!(world.snapshot.tasks, before.tasks, "tasks after undoing: {description}");
        assert_eq!(world.snapshot.projects, before.projects, "projects: {description}");
        assert_eq!(world.snapshot.labels, before.labels, "labels: {description}");
        assert_eq!(
            world.snapshot.completions, before.completions,
            "completions: {description}"
        );
        assert_eq!(
            world.snapshot.assignments, before.assignments,
            "assignments: {description}"
        );
    }
}

#[test]
fn an_inverse_can_itself_be_inverted_for_redo() {
    let mut world = World::new();
    let id = world.add("Review PR");
    let edit = complete_task(&world.snapshot, id, &now()).unwrap();

    world.apply(&edit);
    assert!(world.is_done(id));
    world.apply(&edit.clone().inverse());
    assert!(!world.is_done(id));
    world.apply(&edit.clone().inverse().inverse());
    assert!(world.is_done(id), "redo is the inverse of the inverse");
}

#[test]
fn the_description_survives_inversion_so_undo_can_be_announced() {
    // Never a silent state change. Without a visual channel a mis-keystroke goes
    // unnoticed for minutes, by which point the context for recovering it is gone.
    let mut world = World::new();
    let id = world.add("Review PR");
    let edit = complete_task(&world.snapshot, id, &now()).unwrap();
    assert_eq!(edit.description, "Completed Review PR");
    assert_eq!(edit.clone().inverse().description, "Completed Review PR");
    assert_eq!(format!("Undid: {}", edit.description), "Undid: Completed Review PR");
}

#[test]
fn creating_a_task_inverts_into_removing_it() {
    let mut world = World::new();
    let task = Task::new(world.work, "New", OrderKey::middle());
    let id = task.id;
    let edit = create_task(task);

    world.apply(&edit);
    assert!(world.snapshot.tasks.contains_key(&id));
    world.apply(&edit.inverse());
    assert!(!world.snapshot.tasks.contains_key(&id));
}

// ---------------------------------------------------------------------------------------
// Changing blocks: the series, or one occurrence
// ---------------------------------------------------------------------------------------

fn moved_to(hour: i8) -> ExceptionAction {
    ExceptionAction::Modified {
        start_time: Some(jiff::civil::time(hour, 0, 0, 0)),
        duration_mins: None,
        title: None,
        kind: None,
        flags: None,
    }
}

#[test]
fn changing_one_occurrence_moves_that_day_and_leaves_the_rest() {
    let mut world = World::new();
    let block = world.daily_block();
    let edit = except_occurrence(&world.snapshot, block, date(2026, 5, 6), moved_to(14)).unwrap();
    world.apply(&edit);

    let start = |day| world.snapshot.day(day).unwrap()[0].start_time;
    assert_eq!(start(date(2026, 5, 6)), jiff::civil::time(14, 0, 0, 0));
    assert_eq!(start(date(2026, 5, 7)), jiff::civil::time(9, 0, 0, 0));
}

#[test]
fn a_cancelled_occurrence_is_gone_from_its_day_and_can_be_put_back() {
    let mut world = World::new();
    let block = world.daily_block();
    let day = date(2026, 5, 6);
    world.apply(&except_occurrence(&world.snapshot, block, day, ExceptionAction::Cancelled).unwrap());
    assert!(world.snapshot.day(day).unwrap().is_empty());

    // A cancelled occurrence is still one the rule placed there, so it can be changed again.
    assert!(except_occurrence(&world.snapshot, block, day, moved_to(10)).is_ok());

    world.apply(&restore_occurrence(&world.snapshot, block, day).unwrap());
    assert_eq!(world.snapshot.day(day).unwrap().len(), 1);
}

#[test]
fn a_day_the_rule_never_reaches_cannot_be_singled_out() {
    let mut world = World::new();
    let block = world.block(BlockKind::Work, Some("FREQ=DAILY;INTERVAL=2"));
    // From 1 May every other day: the 2nd is skipped.
    let refused = except_occurrence(&world.snapshot, block, date(2026, 5, 2), moved_to(10));
    assert_eq!(refused, Err(EditError::NoOccurrence { date: date(2026, 5, 2) }));
}

#[test]
fn a_block_that_happens_once_is_changed_through_the_block_not_an_occurrence() {
    let mut world = World::new();
    let block = world.block(BlockKind::Work, None);
    let refused = except_occurrence(&world.snapshot, block, date(2026, 5, 1), moved_to(10));
    assert_eq!(refused, Err(EditError::NotRepeating));

    let before = world.snapshot.series[&block].clone();
    let mut after = before.clone();
    after.title = "Writing".to_owned();
    world.apply(&update_series(before, after));
    assert_eq!(world.snapshot.day(date(2026, 5, 1)).unwrap()[0].title, "Writing");
}

#[test]
fn a_series_change_keeps_what_one_occurrence_overrode() {
    let mut world = World::new();
    let block = world.daily_block();
    let day = date(2026, 5, 6);
    world.apply(&except_occurrence(&world.snapshot, block, day, moved_to(14)).unwrap());

    let before = world.snapshot.series[&block].clone();
    let mut after = before.clone();
    after.title = "Writing".to_owned();
    world.apply(&update_series(before, after));

    let moved = &world.snapshot.day(day).unwrap()[0];
    assert_eq!(moved.title, "Writing", "the title follows the series");
    assert_eq!(moved.start_time, jiff::civil::time(14, 0, 0, 0), "the time stays overridden");
}

#[test]
fn changing_a_block_to_what_it_already_is_writes_nothing() {
    let mut world = World::new();
    let block = world.daily_block();
    let series = world.snapshot.series[&block].clone();
    assert!(update_series(series.clone(), series).is_empty());
}

#[test]
fn the_format_changes_only_once_every_listed_device_has_said_it_understands() {
    use lumenna_core::model::{Device, SCHEMA_VERSION};
    let mut snapshot = lumenna_core::snapshot::Snapshot::default();
    let device = |byte: u8, schema: u32| Device {
        node_id: lumenna_core::id::NodeId::from_bytes([byte; 32]),
        name: format!("device {byte}"),
        platform: "linux".to_owned(),
        paired_at: lumenna_core::time::now(),
        last_seen: lumenna_core::time::now(),
        schema,
    };
    for d in [device(1, SCHEMA_VERSION), device(2, 0)] {
        snapshot.devices.insert(d.node_id, d);
    }
    assert!(!snapshot.all_devices_at_least(SCHEMA_VERSION), "one has not said: it counts as older");
    let late = device(2, SCHEMA_VERSION);
    snapshot.devices.insert(late.node_id, late);
    assert!(snapshot.all_devices_at_least(SCHEMA_VERSION));
    assert!(!snapshot.all_devices_at_least(SCHEMA_VERSION + 1));
}
