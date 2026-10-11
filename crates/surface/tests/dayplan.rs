//! Planning a whole day: said in full before anything changes, then applied as accepted.

use lumenna_surface::{DayPins, Lumenna, NewBlock, TimeStyle};

const NOW: &str = "2026-05-06T08:00";

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(lumenna: &Lumenna, title: &str, kind: &str, at: &str, minutes: u32, filter: Option<&str>) {
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes,
            date: Some("2026-05-06".to_owned()),
            kind: kind.to_owned(),
            task_filter: filter.map(ToOwned::to_owned),
            ..NewBlock::default()
        })
        .unwrap();
}

fn plan(lumenna: &Lumenna, style: TimeStyle) -> lumenna_surface::DayPlan {
    lumenna.propose_day_plan(Some("2026-05-06".to_owned()), style, DayPins::default(), Some(NOW.to_owned())).unwrap()
}

#[test]
fn a_day_plan_is_said_block_by_block_and_each_sitting_says_why() {
    let (_directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    block(&lumenna, "Morning", "work", "09:00", 60, Some("#Work"));
    block(&lumenna, "Afternoon", "work", "14:00", 60, None);
    lumenna.add_task("Report #Work p1").unwrap();
    lumenna.add_task("Groceries p2").unwrap();
    let proposal = plan(&lumenna, TimeStyle::TwelveHour);
    assert_eq!(proposal.announcement, "Plans 2 tasks: Report in Morning and Groceries in Afternoon.");
    assert_eq!(proposal.sittings[0].sentence, "Report goes in Morning at 9:00 AM, for 30 minutes, since it has no estimate");
    assert_eq!(proposal.sittings[0].reason.as_deref(), Some("Suggested because priority 1."));
    assert_eq!(proposal.sittings[1].sentence, "Groceries goes in Afternoon at 2:00 PM, for 30 minutes, since it has no estimate");
}

#[test]
fn an_accepted_plan_is_one_change_and_planning_again_offers_nothing_twice() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Morning", "work", "09:00", 60, None);
    lumenna.add_task("Report p1").unwrap();
    lumenna.add_task("Email p2").unwrap();
    let proposal = plan(&lumenna, TimeStyle::TwentyFourHour);
    assert_eq!(proposal.sittings.len(), 2);
    let done = lumenna.apply_day_plan(proposal.clone(), proposal.sittings.clone(), Some(NOW.to_owned())).unwrap();
    assert_eq!(done.announcement, "Planned 2 tasks.");
    assert!(plan(&lumenna, TimeStyle::TwentyFourHour).sittings.is_empty(), "both are planned already");
    assert!(lumenna.apply_day_plan(proposal.clone(), proposal.sittings, Some(NOW.to_owned())).is_err(), "applying it again is stale");
    lumenna.undo().unwrap();
    assert_eq!(plan(&lumenna, TimeStyle::TwentyFourHour).sittings.len(), 2, "one undo takes the plan back");
}

#[test]
fn what_is_due_and_fits_nowhere_is_named() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Morning", "work", "09:00", 30, None);
    lumenna.add_task("Tax return p1").unwrap();
    let id = lumenna.list_tasks("").unwrap().rows[0].id.clone();
    lumenna
        .edit_task(&id, lumenna_surface::TaskEdit { due: Some("2026-05-06".to_owned()), estimate: Some("2h".to_owned()), ..Default::default() })
        .unwrap();
    let proposal = plan(&lumenna, TimeStyle::TwentyFourHour);
    assert!(proposal.announcement.ends_with("Tax return is due and fits in no block."), "{}", proposal.announcement);
}

#[test]
fn a_sitting_turned_down_frees_its_time_for_another_task_and_its_task_for_another_block() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Morning", "work", "09:00", 30, None);
    block(&lumenna, "Afternoon", "work", "14:00", 30, None);
    lumenna.add_task("Report p1").unwrap();
    lumenna.add_task("Email p2").unwrap();
    lumenna.add_task("Filing p3").unwrap();
    let proposal = plan(&lumenna, TimeStyle::TwentyFourHour);
    assert_eq!(proposal.announcement, "Plans 2 tasks: Report in Morning and Email in Afternoon.");
    // Report is turned down for Morning, Email taken: Morning goes to Filing next, and Report
    // has no other block left with room.
    let email = proposal.sittings[1].clone();
    let done = lumenna.apply_day_plan(proposal, vec![email], Some(NOW.to_owned())).unwrap();
    assert_eq!(done.announcement, "Planned 1 task. Now: Plans 1 task: Filing in Morning.");
}
