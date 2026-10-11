//! When a block's planned work outruns its time.

use jiff::civil::{date, time};
use jiff::{SignedDuration, Timestamp};
use lumenna_core::id::ProjectId;
use lumenna_core::model::{BlockAssignment, BlockKind, BlockRef, BlockSeries, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_core::remind::overrun_at;
use lumenna_core::snapshot::Snapshot;

const START: &str = "2026-05-06T09:00:00Z";
const END: &str = "2026-05-06T10:00:00Z";

fn world(planned: &[u32]) -> (Snapshot, BlockRef, Vec<lumenna_core::id::AssignmentId>) {
    let mut snapshot = Snapshot::default();
    let inbox = Project::inbox();
    let project: ProjectId = inbox.id;
    snapshot.projects.insert(inbox.id, inbox);
    let series = BlockSeries::one_off("Deep work", BlockKind::Work, date(2026, 5, 6), time(9, 0, 0, 0), 60).unwrap();
    let block = BlockRef::OneOff(series.id);
    snapshot.series.insert(series.id, series);
    let mut ids = Vec::new();
    for minutes in planned {
        let task = Task::new(project, "Work", OrderKey::middle());
        let mut sitting = BlockAssignment::new(block, task.id, OrderKey::middle());
        sitting.planned_mins = Some(*minutes);
        ids.push(sitting.id);
        snapshot.tasks.insert(task.id, task);
        snapshot.assignments.insert(sitting.id, sitting);
    }
    (snapshot, block, ids)
}

fn when(snapshot: &Snapshot, block: BlockRef) -> Option<(Timestamp, lumenna_core::remind::Overrun)> {
    let facts = snapshot.facts();
    overrun_at(snapshot, &facts, block, START.parse().unwrap(), END.parse().unwrap(), 5)
}

#[test]
fn idle_the_shortfall_grows_a_minute_a_minute_from_the_records_alone() {
    let (snapshot, block, _) = world(&[30, 20]);
    let (at, overrun) = when(&snapshot, block).unwrap();
    assert_eq!(at, "2026-05-06T09:15:00Z".parse::<Timestamp>().unwrap());
    assert_eq!((overrun.left, overrun.work), (45, 50));
}

#[test]
fn with_a_timer_running_work_keeps_pace_with_the_clock() {
    let (mut snapshot, block, ids) = world(&[30, 20]);
    snapshot.assignments.get_mut(&ids[0]).unwrap().running_since = Some(START.parse().unwrap());
    assert_eq!(when(&snapshot, block), None, "50 minutes of work in 60, being done");
    // Started late, at 9:20: 50 minutes of work and 40 left, 10 short from then.
    let late: Timestamp = "2026-05-06T09:20:00Z".parse().unwrap();
    snapshot.assignments.get_mut(&ids[0]).unwrap().running_since = Some(late);
    let (at, overrun) = when(&snapshot, block).unwrap();
    assert_eq!(at, late);
    assert_eq!((overrun.left, overrun.work), (40, 50));
}

#[test]
fn logged_time_counts_against_the_work_left() {
    let (mut snapshot, block, ids) = world(&[30, 40]);
    snapshot.assignments.get_mut(&ids[0]).unwrap().accumulated_mins = 30;
    // 40 left of 70: it outruns by 5 only at 9:25.
    let (at, _) = when(&snapshot, block).unwrap();
    assert_eq!(at, "2026-05-06T09:25:00Z".parse::<Timestamp>().unwrap() + SignedDuration::ZERO);
}
