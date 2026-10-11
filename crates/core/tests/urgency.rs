//! How urgent a task is, and what has happened to it.

use jiff::civil::{date, time};
use jiff::Zoned;
use lumenna_core::history::history;
use lumenna_core::id::{ProjectId, TaskId};
use lumenna_core::model::{
    AssignmentStatus, BlockAssignment, BlockKind, BlockRef, BlockSeries, Due, Priority, Project, Task,
};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_core::urgency::{Coefficients, Term, urgency};

fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

struct World {
    snapshot: Snapshot,
    project: ProjectId,
}

impl World {
    fn new() -> Self {
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let project = inbox.id;
        snapshot.projects.insert(inbox.id, inbox);
        Self { snapshot, project }
    }

    fn task(&mut self, title: &str) -> TaskId {
        let mut task = Task::new(self.project, title, OrderKey::middle());
        task.created_at = now().timestamp();
        let id = task.id;
        self.snapshot.tasks.insert(id, task);
        id
    }

    fn due(&mut self, id: TaskId, days: i32) {
        let day = now().date().checked_add(jiff::Span::new().days(days)).unwrap();
        self.snapshot.tasks.get_mut(&id).unwrap().due = Some(Due::at(day, time(17, 0, 0, 0)));
    }

    fn score(&self, id: TaskId) -> f32 {
        let facts = self.snapshot.facts();
        urgency(&self.snapshot, &facts, &self.snapshot.tasks[&id], &now(), &Coefficients::default()).score
    }
}

#[test]
fn overdue_outranks_due_tomorrow_which_outranks_due_next_month_which_outranks_no_date() {
    let mut world = World::new();
    let (late, soon, later, none) = (world.task("late"), world.task("soon"), world.task("later"), world.task("none"));
    world.due(late, -3);
    world.due(soon, 1);
    world.due(later, 30);
    assert!(world.score(late) > world.score(soon));
    assert!(world.score(soon) > world.score(later));
    assert!(world.score(later) > world.score(none));
}

#[test]
fn a_higher_priority_counts_for_more_and_waiting_on_an_open_task_counts_against() {
    let mut world = World::new();
    let (top, plain) = (world.task("top"), world.task("plain"));
    world.snapshot.tasks.get_mut(&top).unwrap().priority = Priority::P1;
    assert!(world.score(top) > world.score(plain));
    let first = world.task("first");
    world.snapshot.tasks.get_mut(&plain).unwrap().depends.insert(first);
    let facts = world.snapshot.facts();
    let blocked = urgency(&world.snapshot, &facts, &world.snapshot.tasks[&plain], &now(), &Coefficients::default());
    assert!(blocked.terms.iter().any(|(t, v)| *t == Term::Blocked && *v < 0.0));
    let blocking = urgency(&world.snapshot, &facts, &world.snapshot.tasks[&first], &now(), &Coefficients::default());
    assert!(blocking.terms.iter().any(|(t, _)| *t == Term::Blocking), "others wait on it");
}

#[test]
fn a_projects_weight_scales_its_tasks_and_keeps_their_order_among_themselves() {
    let mut world = World::new();
    let mut heavy = Project::new("Heavy", OrderKey::middle());
    heavy.weight = Some(2.0);
    let heavy_id = heavy.id;
    world.snapshot.projects.insert(heavy.id, heavy);
    let (a, b) = (world.task("a"), world.task("b"));
    world.due(a, 1);
    world.due(b, 10);
    let before = (world.score(a), world.score(b));
    for id in [a, b] {
        world.snapshot.tasks.get_mut(&id).unwrap().project_id = heavy_id;
    }
    let after = (world.score(a), world.score(b));
    assert!((after.0 - 2.0 * before.0).abs() < 1e-4 && (after.1 - 2.0 * before.1).abs() < 1e-4);
    assert!(after.0 > after.1, "the order within the project holds");
}

#[test]
fn history_counts_sittings_time_and_the_days_it_was_put_in_a_block_and_left() {
    let mut world = World::new();
    let task = world.task("Essay draft");
    world.snapshot.tasks.get_mut(&task).unwrap().estimate_mins = Some(60);
    world.snapshot.tasks.get_mut(&task).unwrap().created_at =
        now().timestamp().checked_sub(jiff::SignedDuration::from_hours(24 * 120)).unwrap();
    let mut series = BlockSeries::one_off("Deep work", BlockKind::Work, date(2026, 5, 1), time(9, 0, 0, 0), 90).unwrap();
    series.rrule = Some("FREQ=DAILY".to_owned());
    series.end_date = None;
    let series_id = series.id;
    world.snapshot.series.insert(series_id, series);
    let mut sit = |day: i8, minutes: u32, status: AssignmentStatus| {
        let mut sitting = BlockAssignment::new(BlockRef::Occurrence(series_id, date(2026, 5, day)), task, OrderKey::middle());
        sitting.accumulated_mins = minutes;
        sitting.status = status;
        world.snapshot.assignments.insert(sitting.id, sitting);
    };
    sit(2, 40, AssignmentStatus::Worked);
    sit(3, 0, AssignmentStatus::Planned);
    sit(4, 0, AssignmentStatus::Deferred);
    sit(5, 50, AssignmentStatus::Worked);
    sit(7, 0, AssignmentStatus::Planned);
    let found = history(&world.snapshot, &world.snapshot.tasks[&task], &now());
    assert_eq!(found.sittings, 5);
    assert_eq!(found.worked_mins, 90);
    assert_eq!(found.put_off, 2, "the 3rd and 4th; tomorrow's is still to come");
    assert_eq!(found.age_days, 120);
    assert_eq!(found.last_worked, Some(date(2026, 5, 5)));
}
