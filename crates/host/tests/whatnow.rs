//! "What should I work on?": the block's own plan first, then what is suggested; choosing
//! one starts on it.

use lumenna_host::{Lumenna, NewBlock};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(lumenna: &Lumenna, title: &str, kind: &str, at: &str, minutes: u32) -> String {
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
    lumenna.list_blocks().unwrap().rows.into_iter().find(|r| r.title == title).unwrap().id
}

fn at(time: &str) -> Option<String> {
    Some(format!("2026-05-06T{time}"))
}

#[test]
fn in_a_block_its_own_sittings_come_first_and_choosing_one_starts_its_timer() {
    let (_directory, lumenna) = open();
    let deep = block(&lumenna, "Deep work", "work", "09:00", 60);
    let essay = lumenna.add_task("Essay p3").unwrap().task.unwrap().id;
    lumenna.assign(&essay, &deep, Some("2026-05-06".to_owned()), Some(30)).unwrap();
    lumenna.add_task("Email p1").unwrap();
    let offered = lumenna.what_should_i_work_on(at("09:20")).unwrap();
    let titles: Vec<_> = offered.choices.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["Essay", "Email"]);
    assert_eq!(offered.announcement, "2 things to work on");
    assert_eq!(offered.choices[0].reason.as_deref(), Some("Planned for Deep work, 30 minutes."));
    let started = lumenna.start_on(&offered.choices[0].id, at("09:20")).unwrap();
    assert_eq!(started.announcement, "Started on Essay");
    let email = lumenna.start_on(&offered.choices[1].id, at("09:21")).unwrap();
    assert_eq!(email.announcement, "Started on Email in Deep work");
}

#[test]
fn in_free_time_a_choice_gets_a_block_of_its_own_until_the_next() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Meeting", "event", "11:00", 60);
    lumenna.add_task("Email p1").unwrap();
    let offered = lumenna.what_should_i_work_on(at("10:40")).unwrap();
    assert_eq!(offered.choices.len(), 1);
    let started = lumenna.start_on(&offered.choices[0].id, at("10:40")).unwrap();
    assert_eq!(started.announcement, "Started on Email in a block of its own");
    let plan = lumenna.plan(Some("2026-05-06".to_owned())).unwrap();
    let own = plan.blocks.iter().find(|b| b.title == "Email").unwrap();
    assert_eq!((own.start.as_str(), own.duration_mins), ("10:40", 20), "until the meeting");
    lumenna.undo().unwrap();
    assert!(!lumenna.plan(Some("2026-05-06".to_owned())).unwrap().blocks.iter().any(|b| b.title == "Email"));
}

#[test]
fn a_block_that_takes_no_tasks_says_so() {
    let (_directory, lumenna) = open();
    block(&lumenna, "Lunch", "break", "12:00", 60);
    lumenna.add_task("Email p1").unwrap();
    let offered = lumenna.what_should_i_work_on(at("12:10")).unwrap();
    assert!(offered.choices.is_empty());
    assert_eq!(offered.announcement, "Lunch is under way, and takes no tasks.");
}
