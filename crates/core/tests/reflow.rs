//! Re-flow when the day slips: what is proposed, never done by itself.

use jiff::civil::{date, time};
use lumenna_core::SeriesId;
use lumenna_core::id::ProjectId;
use lumenna_core::model::{BlockAssignment, BlockKind, BlockRef, BlockSeries, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_core::reflow::{Pin, Proposed, propose_reflow};
use lumenna_core::snapshot::Snapshot;

const DAY_END: i64 = 22 * 60;

struct World {
    snapshot: Snapshot,
    project: ProjectId,
}

fn at(h: i64, m: i64) -> i64 {
    h * 60 + m
}

impl World {
    fn new() -> Self {
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let project = inbox.id;
        snapshot.projects.insert(inbox.id, inbox);
        Self { snapshot, project }
    }

    fn block(&mut self, title: &str, kind: BlockKind, h: i8, m: i8, minutes: u32, anchored: bool) -> SeriesId {
        let mut series = BlockSeries::one_off(title, kind, date(2026, 5, 6), time(h, m, 0, 0), minutes).unwrap();
        series.flags.anchored = anchored;
        let id = series.id;
        self.snapshot.series.insert(id, series);
        id
    }

    fn sitting(&mut self, block: SeriesId, title: &str, planned: u32) {
        let task = Task::new(self.project, title, OrderKey::middle());
        let mut sitting = BlockAssignment::new(BlockRef::OneOff(block), task.id, OrderKey::after(&OrderKey::middle()));
        sitting.order = OrderKey::between(
            self.snapshot.assignments.values().map(|a| a.order.clone()).max().as_ref(),
            None,
        )
        .unwrap();
        sitting.planned_mins = Some(planned);
        self.snapshot.tasks.insert(task.id, task);
        self.snapshot.assignments.insert(sitting.id, sitting);
    }

    fn reflow(&self, now: i64, pins: &[Pin]) -> lumenna_core::reflow::Proposal {
        propose_reflow(&self.snapshot, date(2026, 5, 6), now, DAY_END, pins).unwrap()
    }
}

fn block_change(change: &Proposed) -> Option<(SeriesId, i64, u32)> {
    match change {
        Proposed::Block { series, to_start, to_mins, .. } => Some((*series, *to_start, *to_mins)),
        _ => None,
    }
}

#[test]
fn a_block_pushed_later_shrinks_keeping_its_end_while_it_has_time_above_its_minimum() {
    let mut world = World::new();
    world.block("Deep work", BlockKind::Work, 9, 0, 90, false);
    let email = world.block("Email", BlockKind::Work, 10, 30, 30, false);
    // Deep work has run over by 15 minutes: it is 10:45 and it is still going, as extended.
    world.snapshot.series.values_mut().find(|s| s.title == "Deep work").unwrap().duration_mins = 105;
    let proposal = world.reflow(at(10, 40), &[]);
    assert_eq!(proposal.changes.iter().filter_map(block_change).collect::<Vec<_>>(), [(email, at(10, 45), 15)]);
    assert!(proposal.unplaceable.is_empty());
}

#[test]
fn a_block_at_its_minimum_moves_later_and_carries_the_slip_on() {
    let mut world = World::new();
    world.block("Deep work", BlockKind::Work, 9, 0, 120, false);
    let lunch = world.block("Lunch", BlockKind::Break, 10, 30, 30, false);
    let review = world.block("Review", BlockKind::Work, 11, 0, 60, false);
    // Deep work runs to 11:00: a break cannot shrink, so it moves, and Review shrinks after it.
    let proposal = world.reflow(at(10, 50), &[]);
    assert_eq!(
        proposal.changes.iter().filter_map(block_change).collect::<Vec<_>>(),
        [(lunch, at(11, 0), 30), (review, at(11, 30), 30)]
    );
}

#[test]
fn an_anchored_block_stays_and_what_crowds_it_goes_after_it() {
    let mut world = World::new();
    world.block("Deep work", BlockKind::Work, 9, 0, 120, false);
    let call = world.block("Call", BlockKind::Event, 10, 0, 30, false);
    let class = world.block("Class", BlockKind::Event, 10, 30, 60, true);
    // Deep work runs to 11:00, so Call cannot fit before Class, which does not move.
    let proposal = world.reflow(at(10, 40), &[]);
    let changes: Vec<_> = proposal.changes.iter().filter_map(block_change).collect();
    assert!(!changes.iter().any(|(s, ..)| *s == class), "anchored: {changes:?}");
    assert!(proposal.unplaceable.contains(&class), "the block under way runs into it: a conflict to say");
    assert_eq!(changes, [(call, at(11, 0), 30)]);
}

#[test]
fn the_work_moves_before_the_day_does() {
    let mut world = World::new();
    let deep = world.block("Deep work", BlockKind::Work, 9, 0, 60, false);
    world.sitting(deep, "Essay", 40);
    world.sitting(deep, "Email", 30);
    // At 9:20 there are 40 minutes left and 70 planned: the last sitting goes, not the block.
    let proposal = world.reflow(at(9, 20), &[]);
    let moved: Vec<_> = proposal
        .changes
        .iter()
        .filter_map(|c| match c {
            Proposed::MoveSitting { task, .. } => Some(world.snapshot.tasks[task].title.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(moved, ["Email"]);
    assert!(proposal.changes.iter().filter_map(block_change).next().is_none(), "no block moved");
}

#[test]
fn a_sitting_the_person_keeps_stays_and_the_one_before_it_goes() {
    let mut world = World::new();
    let deep = world.block("Deep work", BlockKind::Work, 9, 0, 60, false);
    world.sitting(deep, "Essay", 30);
    world.sitting(deep, "Email", 30);
    let email = *world.snapshot.assignments.iter().find(|(_, a)| world.snapshot.tasks[&a.task_id].title == "Email").unwrap().0;
    let proposal = world.reflow(at(9, 20), &[Pin::KeepSitting(email)]);
    let moved: Vec<_> = proposal
        .changes
        .iter()
        .filter_map(|c| match c {
            Proposed::MoveSitting { task, .. } => Some(world.snapshot.tasks[task].title.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(moved, ["Essay"]);
}

#[test]
fn what_the_person_pins_is_kept_or_skipped_and_the_proposal_follows() {
    let mut world = World::new();
    world.block("Deep work", BlockKind::Work, 9, 0, 120, false);
    let lunch = world.block("Lunch", BlockKind::Break, 10, 30, 30, false);
    let review = world.block("Review", BlockKind::Work, 11, 0, 60, false);
    let proposal = world.reflow(at(10, 50), &[Pin::Skip(lunch)]);
    assert!(proposal.changes.contains(&Proposed::Skip { series: lunch }));
    assert!(proposal.changes.iter().filter_map(block_change).next().is_none(), "with lunch skipped, Review keeps its time");
    let proposal = world.reflow(at(10, 50), &[Pin::Keep(review)]);
    assert!(!proposal.changes.iter().filter_map(block_change).any(|(s, ..)| s == review), "kept where it is");
}

#[test]
fn a_block_pushed_past_midnight_does_not_fit_even_when_the_day_runs_late() {
    let mut world = World::new();
    world.block("Deep work", BlockKind::Work, 22, 0, 130, false);
    let review = world.block("Review", BlockKind::Work, 23, 30, 30, false);
    // Deep work runs to 00:10, so Review, even shrunk, would start on the next day.
    let proposal = propose_reflow(&world.snapshot, date(2026, 5, 6), at(23, 40), 26 * 60, &[]).unwrap();
    assert_eq!(proposal.unplaceable, [review]);
}

#[test]
fn what_does_not_fit_before_the_days_end_is_said_not_squeezed_in() {
    let mut world = World::new();
    world.block("Deep work", BlockKind::Work, 19, 0, 150, false);
    let wind_down = world.block("Wind down", BlockKind::Break, 21, 30, 60, false);
    let proposal = world.reflow(at(21, 20), &[]);
    assert_eq!(proposal.unplaceable, [wind_down], "a break cannot shrink, and 21:30 + 60 is past 22:00");
}
