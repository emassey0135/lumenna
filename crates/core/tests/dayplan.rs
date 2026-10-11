//! The whole-day plan: each work block still to come given its best tasks until full.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::{date, time};
use lumenna_core::SeriesId;
use lumenna_core::dayplan::{UNESTIMATED_MINS, propose_day};
use lumenna_core::id::{ProjectId, TaskId};
use lumenna_core::model::{BlockKind, BlockRef, BlockSeries, Priority, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;

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

    fn block(&mut self, title: &str, kind: BlockKind, h: i8, minutes: u32) -> SeriesId {
        let series = BlockSeries::one_off(title, kind, date(2026, 5, 6), time(h, 0, 0, 0), minutes).unwrap();
        let id = series.id;
        self.snapshot.series.insert(id, series);
        id
    }

    fn task(&mut self, title: &str, priority: u8, estimate: Option<u32>) -> TaskId {
        let mut task = Task::new(self.project, title, OrderKey::middle());
        task.priority = Priority::from_u8(priority);
        task.estimate_mins = estimate;
        let id = task.id;
        self.snapshot.tasks.insert(id, task);
        id
    }

    fn plan(&self, now: &str) -> lumenna_core::dayplan::DayProposal {
        let now = now.parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap();
        propose_day(&self.snapshot, date(2026, 5, 6), &now, &BTreeMap::new(), &BTreeSet::new(), &BTreeSet::new()).unwrap()
    }

    fn titles(&self, proposal: &lumenna_core::dayplan::DayProposal, block: SeriesId) -> Vec<String> {
        proposal
            .sittings
            .iter()
            .filter(|s| s.block == BlockRef::OneOff(block))
            .map(|s| self.snapshot.tasks[&s.task].title.clone())
            .collect()
    }
}

#[test]
fn each_block_takes_the_most_urgent_tasks_that_fit_and_a_task_goes_in_one_block() {
    let mut world = World::new();
    let morning = world.block("Morning", BlockKind::Work, 9, 60);
    let afternoon = world.block("Afternoon", BlockKind::Work, 14, 60);
    world.block("Lunch", BlockKind::Break, 12, 60);
    world.task("Report", 1, Some(45));
    world.task("Email", 2, Some(15));
    world.task("Filing", 4, Some(30));
    world.task("Big project", 1, Some(120));
    let proposal = world.plan("2026-05-06T08:00");
    assert_eq!(world.titles(&proposal, morning), ["Report", "Email"]);
    assert_eq!(world.titles(&proposal, afternoon), ["Filing"], "too long for any block, Big project is left out");
}

#[test]
fn a_task_without_an_estimate_is_planned_at_the_assumed_length_and_says_so() {
    let mut world = World::new();
    world.block("Morning", BlockKind::Work, 9, 60);
    world.task("Call", 1, None);
    let proposal = world.plan("2026-05-06T08:00");
    assert_eq!(proposal.sittings.len(), 1);
    assert!(proposal.sittings[0].assumed);
    assert_eq!(proposal.sittings[0].minutes, UNESTIMATED_MINS);
}

#[test]
fn only_the_time_still_to_come_is_planned() {
    let mut world = World::new();
    let morning = world.block("Morning", BlockKind::Work, 9, 60);
    world.task("Report", 1, Some(45));
    world.task("Email", 2, Some(15));
    // At 9:40, twenty minutes are left of Morning.
    let proposal = world.plan("2026-05-06T09:40");
    assert_eq!(world.titles(&proposal, morning), ["Email"]);
}

#[test]
fn what_is_due_and_does_not_fit_is_named() {
    let mut world = World::new();
    world.block("Morning", BlockKind::Work, 9, 30);
    let due = world.task("Tax return", 1, Some(90));
    world.snapshot.tasks.get_mut(&due).unwrap().due = Some(lumenna_core::model::Due::on(date(2026, 5, 6)));
    let proposal = world.plan("2026-05-06T08:00");
    assert_eq!(proposal.unfit, [due]);
}
