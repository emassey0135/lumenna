//! Replanning the rest of today: said in full before anything changes, then applied as
//! accepted, and refused if the day has moved since.

use lumenna_surface::{Lumenna, NewBlock, ReplanPins, TimeStyle};

const NOW: &str = "2026-05-06T10:40";

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(lumenna: &Lumenna, title: &str, kind: &str, at: &str, minutes: u32) {
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes,
            date: Some("2026-05-06".to_owned()),
            kind: kind.to_owned(),
            ..NewBlock::default()
        })
        .unwrap();
}

#[test]
fn a_replan_says_every_change_in_the_clock_asked_for_and_changes_nothing_by_itself() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Deep work", "work", "09:00", 120);
    block(&lumenna, "Lunch", "break", "10:30", 30);
    let proposal = lumenna.replan(TimeStyle::TwelveHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    assert_eq!(proposal.announcement, "Lunch moves from 10:30 AM to 11:00 AM.");
    let again = lumenna.replan(TimeStyle::TwentyFourHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    assert_eq!(again.announcement, "Lunch moves from 10:30 to 11:00.", "nothing was applied in between");
}

#[test]
fn an_accepted_replan_is_applied_once_and_a_stale_one_is_refused() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Deep work", "work", "09:00", 120);
    block(&lumenna, "Lunch", "break", "10:30", 30);
    let proposal = lumenna.replan(TimeStyle::TwentyFourHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    let applied = lumenna.apply_replan(proposal.clone(), proposal.changes.clone(), Some(NOW.to_owned())).unwrap();
    assert!(applied.announcement.starts_with("Replanned"), "{}", applied.announcement);
    assert!(applied.next.changes.is_empty(), "taken whole, nothing is left to change");
    let after = lumenna.replan(TimeStyle::TwentyFourHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    assert!(after.changes.is_empty(), "{}", after.announcement);
    assert!(
        lumenna.apply_replan(proposal.clone(), proposal.changes, Some(NOW.to_owned())).is_err(),
        "Lunch no longer starts at 10:30"
    );
    lumenna.undo().unwrap();
    let undone = lumenna.replan(TimeStyle::TwentyFourHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    assert_eq!(undone.changes.len(), 1, "one undo takes the whole replan back");
}

#[test]
fn a_skipped_block_is_said_and_frees_its_time() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Deep work", "work", "09:00", 120);
    block(&lumenna, "Lunch", "break", "10:30", 30);
    let lunch = lumenna.list_blocks().unwrap().rows.iter().find(|r| r.title == "Lunch").unwrap().id.clone();
    let lunch = lunch.split('@').next().unwrap().to_owned();
    let proposal = lumenna.replan(TimeStyle::TwentyFourHour, ReplanPins { skip: vec![lunch], ..ReplanPins::default() }, Some(NOW.to_owned())).unwrap();
    assert_eq!(proposal.announcement, "Lunch is skipped today.");
    lumenna.apply_replan(proposal.clone(), proposal.changes, Some(NOW.to_owned())).unwrap();
    let plan = lumenna.plan(Some("2026-05-06".to_owned())).unwrap();
    assert!(!plan.blocks.iter().any(|b| b.title == "Lunch"), "{:?}", plan.blocks.iter().map(|b| &b.title).collect::<Vec<_>>());
}

#[test]
fn work_that_no_longer_fits_leaves_its_block_before_any_block_moves() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Deep work", "work", "09:00", 60);
    let deep = lumenna.list_blocks().unwrap().rows[0].id.clone();
    for (title, minutes) in [("Essay", 40), ("Email", 30)] {
        let task = lumenna.add_task(title).unwrap().task.unwrap().id;
        lumenna.assign(&task, &deep, Some("2026-05-06".to_owned()), Some(minutes)).unwrap();
    }
    let proposal = lumenna.replan(TimeStyle::TwentyFourHour, ReplanPins::default(), Some("2026-05-06T09:20".to_owned())).unwrap();
    // The day is long past, so no block of its week is still to come to take it.
    assert_eq!(
        proposal.announcement,
        "Email does not fit in Deep work and goes back to the list, since no block this week takes it."
    );
    lumenna.apply_replan(proposal.clone(), proposal.changes, Some("2026-05-06T09:20".to_owned())).unwrap();
    let plan = lumenna.plan(Some("2026-05-06".to_owned())).unwrap();
    let left: Vec<_> = plan.blocks[0].assignments.iter().map(|a| a.title.clone()).collect();
    assert_eq!(left, ["Essay"]);
}

#[test]
fn a_change_taken_alone_is_made_and_the_rest_of_the_day_is_proposed_again_around_what_was_turned_down() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Deep work", "work", "09:00", 120);
    block(&lumenna, "Lunch", "break", "10:30", 30);
    block(&lumenna, "Review", "work", "11:00", 60);
    let proposal = lumenna.replan(TimeStyle::TwentyFourHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    assert_eq!(proposal.announcement, "Lunch moves from 10:30 to 11:00; Review moves from 11:00 to 11:30 and shortens from 1 hour to 30 minutes.");
    // Only Review's change is taken: Lunch, turned down, is kept where it is, so the next
    // proposal says what that leaves, rather than the day being left half-moved in silence.
    let review = proposal.changes[1].clone();
    let done = lumenna.apply_replan(proposal, vec![review], Some(NOW.to_owned())).unwrap();
    assert_eq!(done.next.pins.keep.len(), 1);
    assert_eq!(
        done.announcement,
        "Replanned: Review moves from 11:00 to 11:30 and shortens from 1 hour to 30 minutes. \
         What is under way runs into Lunch at 10:30: stop now to make it, or miss it."
    );
}
