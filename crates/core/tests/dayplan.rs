//! The whole-day plan: each work block still to come given its best tasks until full.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::{date, time};
use lumenna_core::SeriesId;
use lumenna_core::dayplan::{UNESTIMATED_MINS, propose_day};
use lumenna_core::id::{ProjectId, TaskId};
use lumenna_core::model::{AssignmentStatus, BlockAssignment, BlockKind, BlockRef, BlockSeries, Priority, Project, Task};
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
    // Too long for any block, Big project takes the whole of the afternoon, not the quarter
    // hour the morning has left after Report.
    assert_eq!(world.titles(&proposal, afternoon), ["Big project"]);
    let part = proposal.sittings.iter().find(|s| s.block == BlockRef::OneOff(afternoon)).unwrap();
    assert_eq!((part.minutes, part.rest), (60, 120));
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
    // At 9:40, twenty minutes are left of Morning: Report gets them, as the part that fits.
    let proposal = world.plan("2026-05-06T09:40");
    assert_eq!(world.titles(&proposal, morning), ["Report"]);
    assert_eq!((proposal.sittings[0].minutes, proposal.sittings[0].rest), (20, 45));
}

#[test]
fn what_is_due_and_fits_in_part_is_named_with_what_is_short() {
    let mut world = World::new();
    world.block("Morning", BlockKind::Work, 9, 30);
    let due = world.task("Tax return", 1, Some(90));
    world.snapshot.tasks.get_mut(&due).unwrap().due = Some(lumenna_core::model::Due::on(date(2026, 5, 6)));
    let proposal = world.plan("2026-05-06T08:00");
    assert_eq!(proposal.sittings[0].minutes, 30);
    assert!(proposal.unfit.is_empty());
    assert_eq!(proposal.short, [(due, 60)]);
}

#[test]
fn what_is_due_and_fits_nowhere_is_named() {
    let mut world = World::new();
    world.block("Morning", BlockKind::Work, 9, 30);
    let first = world.task("Report", 1, Some(30));
    world.snapshot.tasks.get_mut(&first).unwrap().due = Some(lumenna_core::model::Due::on(date(2026, 5, 6)));
    let due = world.task("Tax return", 2, Some(90));
    world.snapshot.tasks.get_mut(&due).unwrap().due = Some(lumenna_core::model::Due::on(date(2026, 5, 6)));
    let proposal = world.plan("2026-05-06T08:00");
    assert_eq!(proposal.unfit, [due]);
}

#[test]
fn a_task_in_sittings_gets_one_sitting_a_day_of_its_length_even_where_more_would_fit() {
    let mut world = World::new();
    let morning = world.block("Morning", BlockKind::Work, 9, 120);
    let afternoon = world.block("Afternoon", BlockKind::Work, 14, 120);
    let essay = world.task("Essay", 1, Some(180));
    world.snapshot.tasks.get_mut(&essay).unwrap().sitting_mins = Some(45);
    let proposal = world.plan("2026-05-06T08:00");
    assert_eq!(world.titles(&proposal, morning), ["Essay"]);
    assert!(world.titles(&proposal, afternoon).is_empty());
    assert_eq!((proposal.sittings[0].minutes, proposal.sittings[0].rest), (45, 180));
}

#[test]
fn a_task_in_sittings_due_too_soon_for_one_a_day_gets_more_today() {
    let mut world = World::new();
    world.block("Morning", BlockKind::Work, 9, 60);
    world.block("Afternoon", BlockKind::Work, 14, 60);
    world.block("Evening", BlockKind::Work, 19, 60);
    let essay = world.task("Essay", 1, Some(180));
    world.snapshot.tasks.get_mut(&essay).unwrap().sitting_mins = Some(45);
    // Four sittings to go and due tomorrow: three today, one tomorrow.
    world.snapshot.tasks.get_mut(&essay).unwrap().due = Some(lumenna_core::model::Due::on(date(2026, 5, 7)));
    let proposal = world.plan("2026-05-06T08:00");
    let lengths: Vec<(u32, u32)> = proposal.sittings.iter().map(|s| (s.minutes, s.sitting)).collect();
    assert_eq!(lengths, [(45, 1), (45, 2), (45, 3)]);
}

impl World {
    /// A sitting of `task` in the one-off `block`, as it stands: `status`, `worked` minutes in.
    fn sitting(&mut self, task: TaskId, block: SeriesId, status: AssignmentStatus, worked: u32) {
        let mut sitting = BlockAssignment::new(BlockRef::OneOff(block), task, OrderKey::middle());
        sitting.status = status;
        sitting.accumulated_mins = worked;
        self.snapshot.assignments.insert(sitting.id, sitting);
    }
}

#[test]
fn a_task_worked_earlier_today_and_still_open_is_planned_again_for_what_is_left_of_its_estimate() {
    let mut world = World::new();
    let morning = world.block("Morning", BlockKind::Work, 9, 60);
    let afternoon = world.block("Afternoon", BlockKind::Work, 14, 60);
    let essay = world.task("Essay", 1, Some(90));
    world.sitting(essay, morning, AssignmentStatus::Worked, 50);
    let proposal = world.plan("2026-05-06T12:00");
    assert_eq!(world.titles(&proposal, afternoon), ["Essay"]);
    let sitting = &proposal.sittings[0];
    assert_eq!((sitting.minutes, sitting.spent, sitting.assumed), (40, 50, false));
}

#[test]
fn a_task_with_a_sitting_still_to_come_that_day_is_not_planned_twice() {
    let mut world = World::new();
    world.block("Morning", BlockKind::Work, 9, 60);
    let afternoon = world.block("Afternoon", BlockKind::Work, 14, 60);
    let essay = world.task("Essay", 1, Some(30));
    world.sitting(essay, afternoon, AssignmentStatus::Planned, 0);
    assert!(world.plan("2026-05-06T08:00").sittings.is_empty());
}

#[test]
fn a_task_past_its_estimate_is_planned_at_the_assumed_length_and_says_so() {
    let mut world = World::new();
    let morning = world.block("Morning", BlockKind::Work, 9, 60);
    world.block("Afternoon", BlockKind::Work, 14, 60);
    let essay = world.task("Essay", 1, Some(30));
    world.sitting(essay, morning, AssignmentStatus::Worked, 45);
    let proposal = world.plan("2026-05-06T12:00");
    assert_eq!(proposal.sittings.len(), 1);
    assert!(proposal.sittings[0].assumed);
    assert_eq!(proposal.sittings[0].minutes, UNESTIMATED_MINS);
}
