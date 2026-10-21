//! Snapshots of the row projection: what a task list and a day are, row by row, before any app
//! words them. Each line is a row's depth, position, folding and checked state, then its speech
//! and its braille, so a change to any of them shows here as a diff to read. Rewrite the
//! snapshots after a change meant to alter them with `UPDATE_SNAPSHOTS=1`.

use std::path::Path;

use jiff::Zoned;
use jiff::civil::{date, time};
use lumenna_core::filter::{Context, Expr};
use lumenna_core::id::TaskId;
use lumenna_core::model::{
    BlockAssignment, BlockKind, BlockRef, BlockSeries, Due, Priority, Project, Recurrence, Task, TaskCompletion,
};
use lumenna_core::order::OrderKey;
use lumenna_core::row::Row;
use lumenna_core::snapshot::Snapshot;

/// Wednesday 2026-05-06, mid-afternoon.
fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

/// One row as a line: its place in the tree and its state, then what speech and braille make of it.
fn line(row: &Row) -> String {
    let fold = match row.expanded {
        Some(true) => " expanded",
        Some(false) => " collapsed",
        None => "",
    };
    let checked = match row.checked {
        Some(true) => " checked",
        Some(false) => " unchecked",
        None => "",
    };
    format!("{}[{}/{}]{fold}{checked} | {} | {}", "  ".repeat(row.depth as usize), row.index, row.count, row.speech(), row.braille())
}

/// Compares `rows` with the snapshot `name`, or writes it when asked.
fn snapshot(name: &str, rows: &[Row]) {
    let shown = rows.iter().map(line).collect::<Vec<_>>().join("\n") + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots").join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &shown).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("no snapshot {}; run with UPDATE_SNAPSHOTS=1", path.display()));
    assert_eq!(shown, expected, "the {name} rows changed; if that is meant, run with UPDATE_SNAPSHOTS=1 and read the diff");
}

struct World {
    snapshot: Snapshot,
}

impl World {
    fn new() -> Self {
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        snapshot.projects.insert(inbox.id, inbox);
        Self { snapshot }
    }

    /// A task made at one fixed moment, so the urgency that orders the list cannot drift.
    fn add(&mut self, title: &str, change: impl FnOnce(&mut Task)) -> TaskId {
        let inbox = self.snapshot.projects.values().next().unwrap().id;
        let mut task = Task::new(inbox, title, OrderKey::middle());
        task.created_at = now().timestamp();
        change(&mut task);
        let id = task.id;
        self.snapshot.tasks.insert(id, task);
        id
    }

    fn complete(&mut self, task: TaskId) {
        let mut completion = TaskCompletion::new(task, None);
        completion.completed_at = now().timestamp();
        self.snapshot.completions.insert(completion.id, completion);
    }
}

fn world() -> World {
    let mut world = World::new();
    let trip = world.add("Plan the trip", |t| t.priority = Priority::P1);
    world.add("Find a hotel", |t| t.parent_id = Some(trip));
    let train = world.add("Book the train", |t| t.parent_id = Some(trip));
    world.complete(train);
    world.add("Pay the rent", |t| t.due = Some(Due::on(date(2026, 5, 5))));
    world.add("Water the plants", |t| {
        t.due = Some(Due {
            time: Some(time(18, 0, 0, 0)),
            recurrence: Some(Recurrence { rrule: "FREQ=DAILY".to_owned(), from_completion: false }),
            ..Due::on(date(2026, 5, 6))
        });
    });
    let data = world.add("Get the data", |t| t.priority = Priority::P2);
    world.add("Write the report", |t| {
        t.depends.insert(data);
    });
    world
}

#[test]
fn the_task_list_reads_as_its_snapshot() {
    let world = world();
    let now = now();
    let cx = Context::new(&world.snapshot, &now);
    snapshot("task_list", &world.snapshot.task_rows(&Expr::All, &cx));
}

#[test]
fn the_task_list_with_completed_subtasks_shown_reads_as_its_snapshot() {
    let mut world = world();
    world.snapshot.settings.show_completed_subtasks = true;
    let now = now();
    let cx = Context::new(&world.snapshot, &now);
    snapshot("task_list_with_completed_subtasks", &world.snapshot.task_rows(&Expr::All, &cx));
}

#[test]
fn the_day_reads_as_its_snapshot() {
    let mut world = world();
    let day = date(2026, 5, 6);
    let deep = BlockSeries::one_off("Deep work", BlockKind::Work, day, time(9, 0, 0, 0), 120).unwrap();
    let lunch = BlockSeries::one_off("Lunch", BlockKind::Break, day, time(12, 0, 0, 0), 45).unwrap();
    let review = BlockSeries::one_off("Review", BlockKind::Work, day, time(15, 0, 0, 0), 60).unwrap();
    let report = world.snapshot.tasks.values().find(|t| t.title == "Write the report").unwrap().id;
    let sitting = BlockAssignment::new(BlockRef::OneOff(review.id), report, OrderKey::middle());
    world.snapshot.assignments.insert(sitting.id, sitting);
    for series in [deep, lunch, review] {
        world.snapshot.series.insert(series.id, series);
    }
    snapshot("day", &world.snapshot.day_rows(day, &now()).unwrap());
}
