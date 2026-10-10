//! What a block can be given beyond its time and kind, timers that pause, and the details
//! the core words for every app.

use lumenna_surface::{BlockEdit, BlockScope, DeviceView, Lumenna, NewBlock};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(title: &str, kind: &str) -> NewBlock {
    NewBlock {
        title: title.to_owned(),
        at: "00:00".to_owned(),
        minutes: 1439,
        date: Some("today".to_owned()),
        kind: kind.to_owned(),
        ..NewBlock::default()
    }
}

#[test]
fn a_block_keeps_its_notes_flags_floor_filter_last_day_and_colour() {
    let (_directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    lumenna
        .add_block(NewBlock {
            notes: Some("Phone off.".to_owned()),
            anchored: Some(true),
            min_minutes: Some(30),
            task_filter: Some("#Work".to_owned()),
            until: Some("2027-01-31".to_owned()),
            colour: Some("Teal".to_owned()),
            repeat: Some("every day".to_owned()),
            ..block("Deep work", "work")
        })
        .unwrap();
    let id = lumenna.list_blocks().unwrap().rows[0].id.clone();
    let shown = lumenna.show_block(&id).unwrap();
    assert_eq!(shown.notes, "Phone off.");
    assert!(shown.accepts_tasks && shown.counts_capacity && shown.anchored);
    assert_eq!(shown.min_minutes, Some(30));
    assert_eq!(shown.task_filter.as_deref(), Some("#Work"));
    assert_eq!(shown.until.as_deref(), Some("2027-01-31"));
    assert_eq!(shown.colour.as_deref(), Some("teal"));

    let clear = BlockEdit {
        notes: Some(String::new()),
        anchored: Some(false),
        min_minutes: Some(0),
        task_filter: Some(String::new()),
        until: Some("none".to_owned()),
        colour: Some(String::new()),
        ..BlockEdit::default()
    };
    lumenna.edit_block(&id, clear, BlockScope::Series).unwrap();
    let shown = lumenna.show_block(&id).unwrap();
    assert_eq!(shown.notes, "");
    assert!(!shown.anchored);
    assert_eq!((shown.min_minutes, shown.task_filter, shown.until, shown.colour), (None, None, None, None));
}

#[test]
fn one_day_takes_only_what_an_exception_holds() {
    let (_directory, lumenna) = open();
    lumenna.add_block(NewBlock { repeat: Some("every day".to_owned()), ..block("Run", "break") }).unwrap();
    let id = lumenna.list_blocks().unwrap().rows[0].id.clone();
    let today = lumenna.plan(None).unwrap().date;
    let notes = BlockEdit { notes: Some("hill".to_owned()), ..BlockEdit::default() };
    let refused = lumenna.edit_block(&id, notes, BlockScope::Occurrence { date: today.clone() });
    assert!(refused.unwrap_err().message().contains("every occurrence"));

    let takes = BlockEdit { accepts_tasks: Some(true), ..BlockEdit::default() };
    lumenna.edit_block(&id, takes, BlockScope::Occurrence { date: today }).unwrap();
    let day = lumenna.plan(None).unwrap();
    assert!(day.blocks[0].accepts_tasks);
    assert!(day.blocks[0].details.contains(&"takes tasks".to_owned()), "{:?}", day.blocks[0].details);
    assert!(day.blocks[0].details.contains(&"changed for this day".to_owned()));
}

#[test]
fn a_last_day_needs_a_repeating_block_and_a_filter_has_to_read() {
    let (_directory, lumenna) = open();
    let once = lumenna.add_block(NewBlock { until: Some("tomorrow".to_owned()), ..block("Dentist", "event") });
    assert!(once.unwrap_err().message().contains("happens once"));
    let filter = lumenna.add_block(NewBlock { task_filter: Some("due before:".to_owned()), ..block("Focus", "work") });
    assert!(filter.is_err(), "a filter that does not read is refused when it is set");
}

#[test]
fn a_break_that_takes_tasks_is_offered_for_them() {
    let (_directory, lumenna) = open();
    lumenna.add_block(NewBlock { accepts_tasks: Some(true), ..block("Train", "break") }).unwrap();
    lumenna.add_block(block("Lunch", "break")).unwrap();
    let offered: Vec<String> = lumenna.work_blocks(None, Some(1)).unwrap().blocks.into_iter().map(|b| b.title).collect();
    assert_eq!(offered, ["Train"]);
    let day = lumenna.plan(None).unwrap();
    let train = day.blocks.iter().find(|b| b.title == "Train").unwrap();
    assert_eq!(train.details[1..], ["break block".to_owned(), "now".to_owned(), "takes tasks".to_owned(), "nothing assigned".to_owned()]);
    let lunch = day.blocks.iter().find(|b| b.title == "Lunch").unwrap();
    assert!(!lunch.details.iter().any(|d| d.contains("assigned")), "{:?}", lunch.details);
}

#[test]
fn a_timer_pauses_resumes_and_stops() {
    let (_directory, lumenna) = open();
    lumenna.add_task("write the chapter").unwrap();
    lumenna.add_block(block("Focus", "work")).unwrap();
    let task = lumenna.list_tasks("").unwrap().rows[0].id.clone();
    let series = lumenna.plan(None).unwrap().blocks[0].series.clone();
    lumenna.assign(&task, &series, None, Some(45)).unwrap();
    let sitting = || lumenna.plan(None).unwrap().blocks[0].assignments[0].clone();

    lumenna.start_timer(&sitting().id).unwrap();
    assert!(sitting().running);
    assert_eq!(sitting().status, "in progress");

    let paused = lumenna.pause_timer(&sitting().id).unwrap();
    assert!(paused.changed);
    assert!(paused.announcement.starts_with("Paused timer"));
    assert!(!sitting().running);
    assert_eq!(sitting().status, "paused");
    assert_eq!(sitting().details[..2], ["paused".to_owned(), "45 minutes planned".to_owned()]);

    let again = lumenna.pause_timer(&sitting().id).unwrap();
    assert!(!again.changed, "pausing a paused timer changes nothing");

    assert_eq!(lumenna.start_timer(&sitting().id).unwrap().announcement, "Resumed timer");
    assert!(sitting().running);
    lumenna.pause_timer(&sitting().id).unwrap();

    // Stopping a paused sitting is what ends it.
    let stopped = lumenna.stop_timer(&sitting().id, None).unwrap();
    assert!(stopped.changed);
    assert_eq!(sitting().status, "worked");
}

fn device(this: bool, success: Option<&str>, error: Option<&str>, attempt: Option<&str>) -> DeviceView {
    DeviceView {
        name: "Kitchen Mac".to_owned(),
        platform: "macos".to_owned(),
        node_id: "ab12".to_owned(),
        this_device: this,
        paired_at: "2026-10-01T09:00:00Z".to_owned(),
        last_attempt: attempt.map(str::to_owned),
        last_success: success.map(str::to_owned),
        last_error: error.map(str::to_owned),
        schema_version: lumenna_core::model::SCHEMA_VERSION,
        status: Vec::new(),
        actions: Vec::new(),
    }
}

#[test]
fn a_device_says_how_syncing_with_it_last_went() {
    use lumenna_surface::words::device_status;
    let now: jiff::Timestamp = "2026-10-04T12:00:00Z".parse().unwrap();
    assert_eq!(device_status(&device(true, None, None, None), now, true), ["this device"]);
    assert_eq!(device_status(&device(false, None, None, None), now, true), ["not synced yet"]);
    assert_eq!(
        device_status(&device(false, Some("2026-10-04T11:55:00Z"), None, None), now, true),
        ["last synced 5 minutes ago"]
    );
    assert_eq!(
        device_status(
            &device(false, Some("2026-10-03T12:00:00Z"), Some("timed out"), Some("2026-10-04T11:00:00Z")),
            now,
            true
        ),
        ["last attempt 1 hour ago failed: timed out", "last synced 1 day ago"]
    );
}

#[test]
fn a_watch_says_only_a_devices_version_since_it_never_syncs_with_one_directly() {
    use lumenna_surface::words::device_status;
    let now: jiff::Timestamp = "2026-10-04T12:00:00Z".parse().unwrap();
    assert!(device_status(&device(false, None, None, None), now, false).is_empty(), "not \"not synced yet\"");
    let mut older = device(false, Some("2026-10-04T11:55:00Z"), None, None);
    older.schema_version = 0;
    assert_eq!(device_status(&older, now, false), ["runs an older version of Lumenna, so update it"]);
}

#[test]
fn the_days_hours_of_work_follow_the_flag_not_the_kind() {
    let (_directory, lumenna) = open();
    let short = |title: &str, at: &str, kind: &str| NewBlock { at: at.to_owned(), minutes: 60, ..block(title, kind) };
    lumenna.add_block(NewBlock { counts_capacity: Some(true), ..short("Study", "09:00", "break") }).unwrap();
    lumenna.add_block(NewBlock { counts_capacity: Some(false), ..short("Admin", "11:00", "work") }).unwrap();
    let summary = lumenna.plan(None).unwrap().summary;
    assert!(summary.contains("1 hour of work"), "{summary}");
}

#[test]
fn a_device_on_another_version_says_so() {
    use lumenna_surface::words::device_status;
    let now: jiff::Timestamp = "2026-10-04T12:00:00Z".parse().unwrap();
    let ours = lumenna_core::model::SCHEMA_VERSION;
    let older = DeviceView { schema_version: ours - 1, ..device(false, Some("2026-10-04T11:55:00Z"), None, None) };
    assert_eq!(device_status(&older, now, true)[0], "runs an older version of Lumenna, so update it");
    let newer = DeviceView { schema_version: ours + 1, ..device(false, None, None, None) };
    assert_eq!(device_status(&newer, now, true), ["runs a newer version of Lumenna, so update this device", "not synced yet"]);
}

#[test]
fn a_block_ending_at_midnight_is_never_past_on_its_own_day() {
    // Its end wraps to 00:00, which every time of day is at or after.
    let (_directory, lumenna) = open();
    lumenna.add_block(NewBlock { at: "23:00".to_owned(), minutes: 60, ..block("Late", "work") }).unwrap();
    let plan = lumenna.plan(None).unwrap();
    assert_ne!(plan.blocks[0].when, "past");
}
