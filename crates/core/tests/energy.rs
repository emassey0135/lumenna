//! Energy as the person rates it: weighed by Plan the Day, the suggestions and Replan from Now,
//! and never anything when nothing is rated.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::{date, time};
use lumenna_core::SeriesId;
use lumenna_core::dayplan::propose_day;
use lumenna_core::energy::{CheckIn, Energy, Fit, Hours, TimeRange};
use lumenna_core::id::{ProjectId, TaskId};
use lumenna_core::model::{BlockAssignment, BlockKind, BlockRef, BlockSeries, Priority, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_core::reflow::{Proposed, propose_reflow};
use lumenna_core::snapshot::Snapshot;
use lumenna_core::suggestions::{Reason, suggest};

const DAY: jiff::civil::Date = date(2026, 5, 6);

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

    fn block(&mut self, title: &str, h: i8, minutes: u32, hours: Option<Hours>) -> SeriesId {
        let mut series = BlockSeries::one_off(title, BlockKind::Work, DAY, time(h, 0, 0, 0), minutes).unwrap();
        series.hours = hours;
        let id = series.id;
        self.snapshot.series.insert(id, series);
        id
    }

    fn task(&mut self, title: &str, priority: u8, estimate: u32, energy: Option<Energy>) -> TaskId {
        let mut task = Task::new(self.project, title, OrderKey::middle());
        task.priority = Priority::from_u8(priority);
        task.estimate_mins = Some(estimate);
        task.energy = energy;
        let id = task.id;
        self.snapshot.tasks.insert(id, task);
        id
    }

    fn assign(&mut self, task: TaskId, block: SeriesId) {
        let mut sitting = BlockAssignment::new(BlockRef::OneOff(block), task, OrderKey::middle());
        sitting.planned_mins = self.snapshot.tasks[&task].estimate_mins;
        self.snapshot.assignments.insert(sitting.id, sitting);
    }

    fn low_today(&mut self) {
        self.snapshot.settings.energy_today = Some(CheckIn { date: DAY, level: Energy::Low });
    }

    fn now(at: &str) -> jiff::Zoned {
        at.parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::UTC).unwrap()
    }

    fn plan(&self) -> lumenna_core::dayplan::DayProposal {
        propose_day(&self.snapshot, DAY, &Self::now("2026-05-06T08:00"), &BTreeMap::new(), &BTreeSet::new(), &BTreeSet::new())
            .unwrap()
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
fn rating_nothing_changes_nothing() {
    let mut world = World::new();
    let morning = world.block("Morning", 9, 60, None);
    let afternoon = world.block("Afternoon", 14, 60, Some(Hours::Best));
    world.task("Report", 1, 60, None);
    world.task("Email", 2, 60, None);
    world.snapshot.settings.best_hours = Some(TimeRange { start: time(14, 0, 0, 0), end: time(17, 0, 0, 0) });
    let proposal = world.plan();
    // The best hours fill first, but with nothing rated the most urgent still goes first in the day.
    assert_eq!(world.titles(&proposal, morning).len() + world.titles(&proposal, afternoon).len(), 2);
    assert_eq!(proposal.capacity, None);
    assert!(proposal.sittings.iter().all(|s| !s.reasons.iter().any(|r| matches!(r, Reason::Energy(_)))));
}

#[test]
fn a_demanding_task_goes_to_the_best_hours_and_a_light_one_to_the_worst() {
    let mut world = World::new();
    let morning = world.block("Morning", 9, 60, Some(Hours::Worst));
    let afternoon = world.block("Afternoon", 14, 60, Some(Hours::Best));
    world.task("Essay", 2, 60, Some(Energy::High));
    world.task("Filing", 1, 60, Some(Energy::Low));
    let proposal = world.plan();
    assert_eq!(world.titles(&proposal, afternoon), ["Essay"]);
    assert_eq!(world.titles(&proposal, morning), ["Filing"]);
    // Said in the order of the day.
    assert_eq!(proposal.sittings[0].block, BlockRef::OneOff(morning));
    let essay = &proposal.sittings[1];
    assert!(essay.reasons.contains(&Reason::Energy(Fit::DemandingInBest)));
}

#[test]
fn a_time_of_day_marks_the_blocks_that_start_in_it() {
    let mut world = World::new();
    let morning = world.block("Morning", 9, 60, None);
    world.block("Afternoon", 14, 60, None);
    world.snapshot.settings.best_hours = Some(TimeRange { start: time(8, 30, 0, 0), end: time(11, 0, 0, 0) });
    world.task("Filing", 1, 60, None);
    world.task("Essay", 2, 60, Some(Energy::High));
    assert_eq!(world.titles(&world.plan(), morning), ["Essay"]);
}

#[test]
fn plan_the_day_stops_before_the_days_capacity_is_spent() {
    let mut world = World::new();
    world.block("Morning", 9, 180, None);
    world.snapshot.settings.energy_capacity = Some(120);
    world.task("Essay", 1, 60, Some(Energy::High));
    world.task("Email", 2, 30, None);
    world.task("Filing", 3, 30, Some(Energy::Low));
    let proposal = world.plan();
    // Essay spends 120 of the 120 alone: the rest are left for another day.
    let planned: Vec<String> = proposal.sittings.iter().map(|s| world.snapshot.tasks[&s.task].title.clone()).collect();
    assert_eq!(planned, ["Essay"]);
    assert_eq!((proposal.planned, proposal.capacity.map(|c| c.minutes)), (120, Some(120)));
    assert_eq!(proposal.spared.len(), 2);
}

#[test]
fn a_low_day_has_half_its_usual_room() {
    let mut world = World::new();
    world.block("Morning", 9, 180, None);
    world.snapshot.settings.energy_capacity = Some(120);
    world.low_today();
    world.task("Email", 1, 30, None);
    world.task("Calls", 2, 30, None);
    world.task("Filing", 3, 30, Some(Energy::Low));
    let proposal = world.plan();
    let planned: Vec<String> = proposal.sittings.iter().map(|s| world.snapshot.tasks[&s.task].title.clone()).collect();
    // 60 of room: two ordinary half hours.
    assert_eq!(planned, ["Email", "Calls"]);
}

#[test]
fn on_a_low_day_a_light_task_is_raised_and_a_demanding_one_lowered() {
    let mut world = World::new();
    let morning = world.block("Morning", 9, 60, None);
    world.low_today();
    let essay = world.task("Essay", 2, 30, Some(Energy::High));
    let filing = world.task("Filing", 2, 30, Some(Energy::Low));
    let found = suggest(&world.snapshot, None, BlockRef::OneOff(morning), 60, &World::now("2026-05-06T08:00"), 5);
    assert_eq!(found[0].task, filing);
    assert!(found[0].reasons.contains(&Reason::Energy(Fit::LightOnLowDay)));
    assert!(found.iter().find(|s| s.task == essay).unwrap().reasons.contains(&Reason::Energy(Fit::DemandingOnLowDay)));
}

#[test]
fn replan_from_now_lightens_a_low_day_the_most_demanding_first() {
    let mut world = World::new();
    let morning = world.block("Morning", 9, 60, None);
    let afternoon = world.block("Afternoon", 14, 60, None);
    world.snapshot.settings.energy_capacity = Some(120);
    let essay = world.task("Essay", 1, 30, Some(Energy::High));
    let email = world.task("Email", 2, 30, None);
    let calls = world.task("Calls", 2, 30, None);
    world.assign(essay, morning);
    world.assign(email, morning);
    world.assign(calls, afternoon);
    let reflow = |world: &World| propose_reflow(&world.snapshot, DAY, 8 * 60, 22 * 60, &[]).unwrap();
    // Not low: nothing leaves, though the day spends more than its usual 120.
    assert!(!reflow(&world).changes.iter().any(|c| matches!(c, Proposed::Lighten { .. })));
    world.low_today();
    let proposal = reflow(&world);
    let gone: Vec<TaskId> = proposal
        .changes
        .iter()
        .filter_map(|c| if let Proposed::Lighten { task, .. } = c { Some(*task) } else { None })
        .collect();
    // 120 planned, 60 of it Essay's, against 60 of room: Essay leaves, and the rest fits.
    assert_eq!(gone, [essay]);
    assert_eq!(proposal.capacity.map(|c| c.minutes), Some(60));
}
