//! A block's planned work outrunning its time is said before the clock says it, and not when
//! it would only spill into free time.

use lumenna_surface::{Lumenna, NewBlock};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    lumenna.set_setting("default-block-reminders", "").unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(lumenna: &Lumenna, title: &str, at: &str, minutes: u32) -> String {
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes,
            date: Some("2026-05-06".to_owned()),
            kind: "work".to_owned(),
            ..NewBlock::default()
        })
        .unwrap();
    lumenna.list_blocks().unwrap().rows.into_iter().find(|r| r.title == title).unwrap().id
}

fn plan(lumenna: &Lumenna, block: &str, title: &str, minutes: u32) {
    let task = lumenna.add_task(title).unwrap().task.unwrap().id;
    lumenna.assign(&task, block, Some("2026-05-06".to_owned()), Some(minutes)).unwrap();
}

fn at(civil: &str) -> String {
    format!("2026-05-06T{civil}").parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::system()).unwrap().timestamp().to_string()
}

/// What is said between two times of the day, other than a block's end.
fn said(lumenna: &Lumenna, from: &str, until: &str) -> Vec<(String, String)> {
    lumenna.reminders_due(at(from), Some(at(until))).unwrap().due.into_iter().map(|d| (d.text, d.replan)).collect()
}

#[test]
fn work_planned_beyond_a_block_is_said_as_it_starts_with_a_replan_button() {
    let (_directory, lumenna) = open();
    let deep = block(&lumenna, "Deep work", "09:00", 60);
    block(&lumenna, "Meeting", "10:00", 60);
    plan(&lumenna, &deep, "Essay", 40);
    plan(&lumenna, &deep, "Email", 30);
    assert_eq!(
        said(&lumenna, "08:59", "09:01"),
        [("Deep work has 1 hour left and 1 hour 10 minutes of work planned, more than it holds".to_owned(), "Replan".to_owned())]
    );
}

#[test]
fn work_that_fits_is_said_only_once_idle_time_has_let_it_outrun_the_block() {
    let (_directory, lumenna) = open();
    let deep = block(&lumenna, "Deep work", "09:00", 60);
    block(&lumenna, "Meeting", "10:00", 60);
    plan(&lumenna, &deep, "Essay", 30);
    plan(&lumenna, &deep, "Email", 20);
    // 50 minutes of work, nothing logged: at 9:15 there are 45 minutes left, 5 short.
    assert!(said(&lumenna, "08:59", "09:14").is_empty());
    let at_quarter_past = said(&lumenna, "09:14", "09:16");
    assert_eq!(at_quarter_past.len(), 1);
    assert!(at_quarter_past[0].0.starts_with("Deep work has 45 minutes left and 50 minutes of work planned"), "{at_quarter_past:?}");
}

#[test]
fn an_overrun_that_fits_the_free_time_after_the_block_is_not_said() {
    let (_directory, lumenna) = open();
    let deep = block(&lumenna, "Deep work", "09:00", 60);
    plan(&lumenna, &deep, "Essay", 40);
    plan(&lumenna, &deep, "Email", 30);
    // Nothing after it until the day ends: running over costs nothing.
    assert!(said(&lumenna, "08:59", "09:59").is_empty());
}

#[test]
fn dismissing_an_overrun_holds_for_that_block() {
    let (_directory, lumenna) = open();
    let deep = block(&lumenna, "Deep work", "09:00", 60);
    block(&lumenna, "Meeting", "10:00", 60);
    plan(&lumenna, &deep, "Essay", 30);
    plan(&lumenna, &deep, "Email", 40);
    let due = lumenna.reminders_due(at("08:59"), Some(at("09:01"))).unwrap().due;
    assert_eq!(due.len(), 1, "70 minutes in 60");
    lumenna.dismiss_reminder(due[0].key.clone(), due[0].occurrence.clone()).unwrap();
    assert!(said(&lumenna, "08:59", "09:59").is_empty(), "dismissed for this block");
}
