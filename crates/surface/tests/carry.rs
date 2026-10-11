//! A block's end offers to carry on what it left short.

use lumenna_surface::{Lumenna, NewBlock};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(lumenna: &Lumenna, title: &str, date: &str, at: &str) -> String {
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes: 60,
            date: Some(date.to_owned()),
            kind: "work".to_owned(),
            ..NewBlock::default()
        })
        .unwrap();
    lumenna.list_blocks().unwrap().rows.into_iter().find(|r| r.title == title).unwrap().id
}

/// The local instant of a civil time, as reminders take it.
fn at(civil: &str) -> String {
    civil.parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::system()).unwrap().timestamp().to_string()
}

#[test]
fn a_block_that_ends_with_work_short_offers_to_carry_the_rest_to_the_next_block_that_takes_it() {
    let (_directory, lumenna) = open();
    // No reminder of its own at the end: the offer fires by itself.
    lumenna.set_setting("default-block-reminders", "").unwrap();
    let deep = block(&lumenna, "Deep work", "2026-05-06", "09:00");
    block(&lumenna, "Admin", "2026-05-07", "09:00");
    let essay = lumenna.add_task("Essay").unwrap().task.unwrap().id;
    lumenna.assign(&essay, &deep, Some("2026-05-06".to_owned()), Some(90)).unwrap();
    let sitting = lumenna.plan(Some("2026-05-06".to_owned())).unwrap().blocks[0].assignments[0].id.clone();
    lumenna.stop_timer(&sitting, Some(40)).unwrap();

    let due = lumenna.reminders_due(at("2026-05-06T09:59"), Some(at("2026-05-06T10:01"))).unwrap();
    let end = &due.due[0];
    assert_eq!(
        end.text,
        "Deep work is ending. Essay: 40 minutes of 1 hour 30 minutes. Carry the remaining 50 minutes to Admin tomorrow?"
    );
    assert_eq!(end.carry, "Carry Forward");

    let carried = lumenna.carry_rest_forward(&format!("{}@{}", end.target, end.occurrence)).unwrap();
    assert_eq!(carried.announcement, "Carried forward Essay to Admin");
    let next = lumenna.plan(Some("2026-05-07".to_owned())).unwrap();
    assert_eq!(next.blocks[0].assignments[0].planned_mins, Some(50));
    // Carried, nothing is left short: the end offers nothing more.
    let again = lumenna.reminders_due(at("2026-05-06T09:59"), Some(at("2026-05-06T10:01"))).unwrap();
    assert!(again.due.is_empty(), "{:?}", again.due.iter().map(|d| &d.text).collect::<Vec<_>>());
}

#[test]
fn a_block_whose_work_is_done_offers_nothing_at_its_end() {
    let (_directory, lumenna) = open();
    lumenna.set_setting("default-block-reminders", "").unwrap();
    let deep = block(&lumenna, "Deep work", "2026-05-06", "09:00");
    let essay = lumenna.add_task("Essay").unwrap().task.unwrap().id;
    lumenna.assign(&essay, &deep, Some("2026-05-06".to_owned()), Some(30)).unwrap();
    lumenna.complete_task(&essay).unwrap();
    let due = lumenna.reminders_due(at("2026-05-06T09:59"), Some(at("2026-05-06T10:01"))).unwrap();
    assert!(due.due.is_empty());
}
