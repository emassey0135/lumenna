//! Snapshots of the row projection: what a task list is, row by row, before any app words it.
//! Each line is a row's depth, position, folding and checked state, then the parts it carries
//! (title, role, states, due words and time, value), so a change to any shows here as a diff to
//! read. How a line is joined is the app's (`lumenna_app::speech`), tested there: a sentence
//! composed here would be one no app says. Rewrite the snapshots after a change meant to alter
//! them with `UPDATE_SNAPSHOTS=1`.

use std::path::Path;

use jiff::Zoned;
use jiff::civil::{date, time};
use lumenna_core::filter::{Context, Expr};
use lumenna_core::id::TaskId;
use lumenna_core::model::{
    Due, Priority, Project, Recurrence, Task, TaskCompletion,
};
use lumenna_core::order::OrderKey;
use lumenna_core::row::Row;
use lumenna_core::snapshot::Snapshot;

/// Wednesday 2026-05-06, mid-afternoon.
fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

/// One row as a line: its place in the tree and its state, then its parts as it carries them.
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
    let states: Vec<&str> = row.state.iter().map(|s| s.speech()).collect();
    let due_time = row.due_time.map(|t| format!("{:02}:{:02}", t.hour(), t.minute()));
    let parts = [
        Some(row.title.clone()),
        Some(format!("{:?}", row.role)),
        (!states.is_empty()).then(|| states.join(" ")),
        row.due.clone(),
        due_time,
        row.value.clone(),
    ];
    let parts: Vec<String> = parts.into_iter().flatten().collect();
    format!("{}[{}/{}]{fold}{checked} | {}", "  ".repeat(row.depth as usize), row.index, row.count, parts.join(" | "))
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
    // A Windows checkout may give the file CRLF line endings; the rows are the same either way.
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no snapshot {}; run with UPDATE_SNAPSHOTS=1", path.display()))
        .replace("\r\n", "\n");
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
