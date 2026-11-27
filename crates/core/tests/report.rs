//! Time reports: what the sittings logged, summed by week, project and label.

use jiff::Timestamp;
use jiff::civil::{date, time};
use lumenna_core::model::{BlockAssignment, BlockKind, BlockRef, BlockSeries, Label, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_core::report::{report, week_of};
use lumenna_core::snapshot::Snapshot;

fn now() -> Timestamp {
    "2026-11-27T18:00:00Z".parse().unwrap()
}

/// A one-off block on `day` with a sitting of `task` that logged `mins`.
fn worked(snapshot: &mut Snapshot, task: &Task, day: jiff::civil::Date, mins: u32) {
    let block = BlockSeries::one_off("Work", BlockKind::Work, day, time(9, 0, 0, 0), 120).unwrap();
    let mut sitting = BlockAssignment::new(BlockRef::OneOff(block.id), task.id, OrderKey::middle());
    sitting.set_logged_mins(mins);
    snapshot.series.insert(block.id, block);
    snapshot.assignments.insert(sitting.id, sitting);
}

#[test]
fn a_week_starts_on_the_day_the_settings_say() {
    assert_eq!(week_of(date(2026, 11, 27), jiff::civil::Weekday::Monday), date(2026, 11, 23));
    assert_eq!(week_of(date(2026, 11, 27), jiff::civil::Weekday::Sunday), date(2026, 11, 22));
    assert_eq!(week_of(date(2026, 11, 23), jiff::civil::Weekday::Monday), date(2026, 11, 23));
}

#[test]
fn every_sitting_in_the_weeks_is_summed_by_week_project_and_label_with_each_tasks_estimate_once() {
    let mut snapshot = Snapshot::default();
    let inbox = Project::inbox();
    let work = Project::new("Work", OrderKey::middle());
    let calls = Label::new("calls", OrderKey::middle());
    let mut essay = Task::new(work.id, "Essay", OrderKey::middle());
    essay.estimate_mins = Some(120);
    essay.labels.insert(calls.id);
    let mut tidy = Task::new(inbox.id, "Tidy", OrderKey::middle());
    tidy.estimate_mins = None;
    worked(&mut snapshot, &essay, date(2026, 11, 24), 50);
    worked(&mut snapshot, &essay, date(2026, 11, 26), 40);
    worked(&mut snapshot, &tidy, date(2026, 11, 17), 15);
    worked(&mut snapshot, &tidy, date(2026, 9, 1), 600);
    for project in [inbox, work.clone()] {
        snapshot.projects.insert(project.id, project);
    }
    snapshot.labels.insert(calls.id, calls.clone());
    snapshot.tasks.insert(essay.id, essay.clone());
    snapshot.tasks.insert(tidy.id, tidy);

    let report = report(&snapshot, date(2026, 11, 27), 4, now());
    let weeks: Vec<_> = report.weeks.iter().map(|(start, total)| (*start, total.logged_secs / 60)).collect();
    assert_eq!(
        weeks,
        [(date(2026, 11, 23), 90), (date(2026, 11, 16), 15), (date(2026, 11, 9), 0), (date(2026, 11, 2), 0)],
        "newest first, every week, September's ten hours outside them"
    );
    assert_eq!(report.first, date(2026, 11, 2));
    let (project, total) = &report.projects[0];
    assert_eq!((*project, total.logged_secs / 60, total.estimate_mins, total.tasks, total.sittings), (work.id, 90, 120, 1, 2));
    assert_eq!(report.projects[1].1.unestimated, 1);
    assert_eq!(report.labels.iter().map(|(id, t)| (*id, t.logged_secs / 60)).collect::<Vec<_>>(), [(calls.id, 90)]);
    assert_eq!((report.total.logged_secs / 60, report.total.tasks, report.total.estimate_mins), (105, 2, 120));
}
