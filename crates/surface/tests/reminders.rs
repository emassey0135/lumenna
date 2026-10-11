//! Reminders through the surface: what an app is handed, what it says, and dealing with one.

use jiff::{SignedDuration, Timestamp, Zoned};
use lumenna_surface::{Lumenna, NewBlock};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

/// A one-off block on today `minutes_from_now` from now, as `HH:MM`, so the tests do not
/// depend on the hour they run.
fn block_in(lumenna: &Lumenna, title: &str, minutes_from_now: i64, assign: Option<&str>) -> String {
    let start = Zoned::now().checked_add(SignedDuration::from_mins(minutes_from_now)).unwrap();
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: format!("{:02}:{:02}", start.hour(), start.minute()),
            minutes: 30,
            date: Some(start.date().to_string()),
            kind: "work".to_owned(),
            ..NewBlock::default()
        })
        .unwrap();
    let block = lumenna.list_blocks().unwrap().rows.into_iter().find(|r| r.title == title).unwrap();
    if let Some(task) = assign {
        let id = lumenna.add_task(task).unwrap().affected.tasks[0].clone();
        lumenna.assign(&id, &block.id, None, None).unwrap();
    }
    block.id.split('@').next().unwrap().to_owned()
}

fn hours(h: i64) -> String {
    Timestamp::now().checked_add(SignedDuration::from_hours(h)).unwrap().to_string()
}

#[test]
fn a_block_takes_the_default_and_its_notification_says_what_it_holds() {
    let (_directory, lumenna) = open();
    block_in(&lumenna, "Deep work", 60, Some("Review PR"));
    let upcoming = lumenna.upcoming_reminders(hours(3)).unwrap();
    let deep = upcoming.due.iter().find(|n| n.title == "Deep work").expect("the default, 5 minutes before");
    assert_eq!(deep.text, "Deep work starts in 5 minutes — 1 task assigned, Review PR");
    assert_eq!(deep.kind, "block");
}

#[test]
fn terse_announcements_leave_out_what_a_block_holds() {
    let (_directory, lumenna) = open();
    lumenna.set_setting("verbosity", "terse").unwrap();
    block_in(&lumenna, "Deep work", 60, Some("Review PR"));
    let upcoming = lumenna.upcoming_reminders(hours(3)).unwrap();
    assert_eq!(upcoming.due[0].text, "Deep work starts in 5 minutes");
}

#[test]
fn a_reminder_of_its_own_replaces_the_defaults_and_can_be_removed() {
    let (_directory, lumenna) = open();
    let series = block_in(&lumenna, "Deep work", 60, None);
    assert_eq!(lumenna.reminders_of(series.clone()).unwrap().announcement, "1 reminder, the defaults");
    let added = lumenna.add_reminder(series.clone(), "end".to_owned(), 0).unwrap();
    assert_eq!(added.announcement, "Reminding of Deep work when it ends");
    let list = lumenna.reminders_of(series.clone()).unwrap();
    assert_eq!(list.reminders.len(), 1);
    assert!(list.reminders[0].own);
    assert_eq!(list.reminders[0].words, "when it ends");
    let texts: Vec<String> = lumenna.upcoming_reminders(hours(3)).unwrap().due.into_iter().map(|n| n.text).collect();
    assert_eq!(texts, ["Deep work is ending"]);
    lumenna.remove_reminder(list.reminders[0].id.clone()).unwrap();
    assert_eq!(lumenna.reminders_of(series).unwrap().announcement, "1 reminder, the defaults");
}

#[test]
fn a_dismissed_firing_is_gone_and_a_snoozed_one_comes_back_later() {
    let (_directory, lumenna) = open();
    block_in(&lumenna, "Deep work", 60, None);
    block_in(&lumenna, "Review", 90, None);
    let upcoming = lumenna.upcoming_reminders(hours(3)).unwrap().due;
    let (deep, review) = (&upcoming[0], &upcoming[1]);
    lumenna.dismiss_reminder(deep.key.clone(), deep.occurrence.clone()).unwrap();
    lumenna.snooze_reminder(review.key.clone(), review.occurrence.clone(), Some(10)).unwrap();
    let after = lumenna.upcoming_reminders(hours(3)).unwrap().due;
    assert_eq!(after.len(), 1, "only the snoozed one");
    assert_eq!(after[0].title, "Review");
    let at: Timestamp = after[0].at.parse().unwrap();
    assert!(at < Timestamp::now().checked_add(SignedDuration::from_mins(11)).unwrap(), "ten minutes from now");
}

#[test]
fn what_came_while_nothing_ran_is_one_summary_and_what_is_fresh_is_said_alone() {
    let (_directory, lumenna) = open();
    let task = lumenna.add_task("Call the bank").unwrap().affected.tasks[0].clone();
    let due = Zoned::now().checked_sub(SignedDuration::from_mins(40)).unwrap();
    lumenna
        .edit_task(&task, lumenna_surface::TaskEdit { due: Some(format!("{} {:02}:{:02}", due.date(), due.hour(), due.minute())), ..Default::default() })
        .unwrap();
    lumenna.add_reminder(task, "due".to_owned(), 0).unwrap();
    let fresh = lumenna.add_task("Water plants").unwrap().affected.tasks[0].clone();
    let soon = Zoned::now().checked_sub(SignedDuration::from_mins(1)).unwrap();
    lumenna
        .edit_task(&fresh, lumenna_surface::TaskEdit { due: Some(format!("{} {:02}:{:02}", soon.date(), soon.hour(), soon.minute())), ..Default::default() })
        .unwrap();
    lumenna.add_reminder(fresh, "due".to_owned(), 0).unwrap();
    let since = Timestamp::now().checked_sub(SignedDuration::from_hours(2)).unwrap().to_string();
    let due = lumenna.reminders_due(since, None).unwrap();
    assert_eq!(due.missed_summary, "1 reminder missed: Call the bank was due.");
    assert_eq!(due.due.len(), 1);
    assert_eq!(due.due[0].text, "Water plants is due now");
}

#[test]
fn reminders_turned_off_on_this_device_give_nothing_here() {
    let (_directory, lumenna) = open();
    block_in(&lumenna, "Deep work", 60, None);
    lumenna.set_setting("reminders", "off").unwrap();
    let upcoming = lumenna.upcoming_reminders(hours(3)).unwrap();
    assert!(upcoming.due.is_empty());
    assert_eq!(upcoming.announcement, "Reminders are off on this device");
    let setting = lumenna.settings(Some("reminders".to_owned())).unwrap().settings.remove(0);
    assert!(!setting.syncs, "it is this device's alone");
}

#[test]
fn one_process_gives_reminders_and_delivery_picks_up_where_it_stopped() {
    let (_directory, lumenna) = open();
    let lock = lumenna.take_reminders().unwrap().expect("nobody else holds it");
    assert!(lumenna.take_reminders().unwrap().is_none(), "a second holder would say every reminder twice");
    block_in(&lumenna, "Deep work", 60, None);
    let fire: Timestamp = lumenna.upcoming_reminders(hours(3)).unwrap().due[0].at.parse().unwrap();
    let minutes = |m: i64| fire.checked_add(SignedDuration::from_mins(m)).unwrap().to_string();
    // The first delivery starts from its own moment: nothing is owed from before it ran.
    assert!(lumenna.deliver_reminders(lock.clone(), Some(minutes(-2))).unwrap().due.is_empty());
    let given = lumenna.deliver_reminders(lock.clone(), Some(minutes(1))).unwrap();
    assert_eq!(given.due.len(), 1, "the default, five minutes before Deep work");
    assert!(lumenna.deliver_reminders(lock.clone(), Some(minutes(2))).unwrap().due.is_empty(), "given once");
    drop(lock);
    assert!(lumenna.take_reminders().unwrap().is_some(), "free again once let go");
}
