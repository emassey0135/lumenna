//! Planning against a dependency: allowed, and said.

use lumenna_host::{Lumenna, NewBlock};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(lumenna: &Lumenna, title: &str, at: &str) -> String {
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes: 60,
            date: Some("tomorrow".to_owned()),
            kind: "work".to_owned(),
            ..NewBlock::default()
        })
        .unwrap();
    lumenna.list_blocks().unwrap().rows.into_iter().find(|r| r.title == title).unwrap().id
}

#[test]
fn a_task_put_before_what_it_waits_for_is_told_so_and_its_pick_says_which_blocks_come_first() {
    let (_directory, lumenna) = open();
    let morning = block(&lumenna, "Morning", "09:00");
    let afternoon = block(&lumenna, "Afternoon", "14:00");
    let data = lumenna.add_task("get the data").unwrap().affected.tasks[0].clone();
    let report = lumenna.add_task("write the report").unwrap().affected.tasks[0].clone();
    lumenna.add_dependency(&report, &data).unwrap();

    // Nothing planned for what it waits for: said, wherever it goes.
    let early = lumenna.assign(&report, &morning, None, None).unwrap();
    assert_eq!(early.notices, ["write the report waits for get the data, which is not done and in no block yet."]);
    lumenna.undo().unwrap();

    lumenna.assign(&data, &afternoon, None, None).unwrap();
    let rows = lumenna.list_tasks("").unwrap().rows;
    let put = rows.iter().find(|r| r.id == report).unwrap().actions.iter().find(|a| a.title == "Put in a Block").unwrap().clone();
    let choices = lumenna.choices(put).unwrap().choices;
    let reason = |title: &str| choices.iter().find(|c| c.title == title).unwrap().reason.clone();
    assert_eq!(
        reason("Morning").as_deref(),
        Some("write the report waits for get the data, which is in Afternoon later that day, after this block.")
    );
    assert_eq!(reason("Afternoon"), None, "the same block is no earlier");

    let early = lumenna.assign(&report, &morning, None, None).unwrap();
    assert!(early.notices[0].ends_with("after this block."), "{:?}", early.notices);
    lumenna.undo().unwrap();
    // After it, nothing to say.
    let late = lumenna.assign(&report, &afternoon, None, None).unwrap();
    assert!(late.notices.is_empty(), "{:?}", late.notices);
}
