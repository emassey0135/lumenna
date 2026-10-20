//! The app, driven as a person with a screen reader drives it, and read back as one hears it.
//!
//! Each test starts the real `lumenna.exe` on a store of its own, seeded through the surface,
//! drives it through its own message queue (`automation`), and asserts on what native UI
//! Automation reports — what NVDA and Narrator are given — as the Apple apps' UI tests do with
//! XCUITest. They open windows and take the foreground, so they are ignored by default:
//!
//! ```text
//! cargo test -p lumenna-windows --test ui -- --ignored
//! ```
//!
//! They take turns (`ONE_WINDOW_AT_A_TIME`): a posted key needs its window active, and two at
//! once would take the foreground from each other.

#![cfg(windows)]

#[path = "automation/mod.rs"]
mod automation;

use std::process::{Child, Command};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use automation::Automation;
use lumenna_host::{Lumenna, NewBlock};
use windows::Win32::Foundation::HWND;


static ONE_WINDOW_AT_A_TIME: Mutex<()> = Mutex::new(());

// The menu commands a test gives for a Ctrl shortcut (`src/win/menu.rs`).
const NEW_TASK: &str = "cmd:100";
const SETTINGS: &str = "cmd:111";
const GO_TASKS: &str = "cmd:141";
const PUT_IN_BLOCK: &str = "cmd:169";

/// A running app on a store of its own.
struct App {
    child: Child,
    window: HWND,
    automation: Automation,
    profile: tempfile::TempDir,
    _backups: tempfile::TempDir,
    _turn: MutexGuard<'static, ()>,
}

impl App {
    /// Starts the app on a fresh store, filled in by `seed` first.
    fn launch(seed: impl FnOnce(&Lumenna)) -> Self {
        Self::launch_with(&[], seed)
    }

    /// Starts the app as `launch` does, with `env` set as well.
    fn launch_with(env: &[(&str, &str)], seed: impl FnOnce(&Lumenna)) -> Self {
        let turn = ONE_WINDOW_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let profile = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let lumenna = Lumenna::open(profile.path().to_str().unwrap()).unwrap();
        // A test's blocks and tasks would otherwise come due as real toasts on the screen of
        // whoever runs it. A test of reminders turns them back on.
        lumenna.set_setting("reminders", "false").unwrap();
        seed(&lumenna);
        drop(lumenna);
        let child = Command::new(env!("CARGO_BIN_EXE_lumenna"))
            .arg("--profile")
            .arg(profile.path())
            // The shortcuts from anywhere are the person's, held by their own copy; a test's
            // must not take them, nor say another program has.
            .arg("--no-shortcuts")
            .env("LUMENNA_BACKUP_DIR", backups.path())
            .envs(env.iter().copied())
            .spawn()
            .expect("the app starts");
        let automation = Automation::new();
        let window = automation.window_of(child.id(), Duration::from_secs(30)).expect("the app shows its window");
        // Settled on Today, with the store read: the day drawn, and focus in it. A cold start
        // — the first test — takes longer than the rest, and a key posted before then is lost.
        automation::wait(Duration::from_secs(30), || automation.named(window, "The day")).expect("the day is drawn");
        automation.activate(window);
        automation::wait(Duration::from_secs(10), || automation.focus(window).contains("TreeItem").then_some(()))
            .expect("focus is on the day");
        Self { child, window, automation, profile, _backups: backups, _turn: turn }
    }

    /// Takes these steps, in order.
    fn post(&self, steps: &[&str]) {
        for step in steps {
            self.automation.post(self.window, step).unwrap_or_else(|why| panic!("{why}"));
        }
    }

    /// Where focus is, as a screen reader would say it.
    fn focus(&self) -> String {
        self.automation.focus(self.window)
    }

    /// What the status line last said.
    fn status(&self) -> String {
        self.automation.status(self.window).unwrap_or_default()
    }

    /// Whether the status line comes to say what `holds` looks for: what follows a write is
    /// said with its answer, which a slow machine takes a while to give.
    fn says(&self, holds: impl Fn(&str) -> bool) -> bool {
        automation::wait(Duration::from_secs(5), || holds(&self.status()).then_some(())).is_some()
    }

    /// Whether focus comes to where `holds` looks for it: the app lands it with a write's answer.
    fn lands(&self, holds: impl Fn(&str) -> bool) -> bool {
        automation::wait(Duration::from_secs(5), || holds(&self.focus()).then_some(())).is_some()
    }

    /// The window in front — a dialog or sheet when one is open — one element a line.
    fn front(&self) -> Vec<String> {
        self.automation.dump(self.automation.front(self.window), 10)
    }

    /// The store, as another process sees it.
    fn store(&self) -> std::sync::Arc<Lumenna> {
        Lumenna::open(self.profile.path().to_str().unwrap()).unwrap()
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The lines naming a control a screen reader would reach and hear nothing for.
fn unnamed(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| {
            let line = line.trim_start();
            ["Edit ''", "ComboBox ''", "List ''", "Button ''", "CheckBox ''", "Tree ''", "Tab ''"]
                .iter()
                .any(|kind| line.starts_with(kind))
                // A tab strip is named by the tab selected in it.
                && !line.starts_with("Tab ''")
        })
        .cloned()
        .collect()
}

fn add(lumenna: &Lumenna, text: &str) {
    lumenna.add_task(text).unwrap();
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn the_day_opens_with_its_summary_first_and_focus_on_a_row() {
    let app = App::launch(|_| {});
    let lines = app.automation.dump(app.window, 6);
    let day = lines.iter().position(|l| l.trim_start().starts_with("Tree 'The day'")).expect("the day is a tree");
    let first = lines[day + 1..].iter().find(|l| l.contains("TreeItem")).expect("the day has rows");
    assert!(first.contains("TreeItem 'Today."), "the summary comes first: {first}");
    assert!(app.focus().contains("TreeItem"), "focus lands on a row: {}", app.focus());
    // A copy started with --no-shortcuts leaves them to the person's own, and says nothing
    // about another program having them.
    assert!(!app.status().contains("already uses"), "{}", app.status());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn space_checks_a_task_off_and_focus_moves_to_the_one_that_took_its_place() {
    let app = App::launch(|lumenna| {
        add(lumenna, "Buy milk");
        add(lumenna, "Call Sam");
    });
    app.post(&[GO_TASKS, "home", "space"]);
    assert!(app.lands(|f| f.contains("TreeItem 'Call Sam'")), "{}", app.focus());
    assert!(app.says(|s| s.starts_with("Completed Buy milk")), "{}", app.status());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_list_gone_back_to_opens_on_its_row_and_folding_and_another_processs_write_leaves_focus_alone() {
    let app = App::launch(|lumenna| {
        add(lumenna, "Write report");
        add(lumenna, "Outline");
        add(lumenna, "Call Sam");
        let rows = lumenna.list_tasks("").unwrap().rows;
        let report = rows.iter().find(|r| r.title == "Write report").unwrap().id.clone();
        let outline = rows.iter().find(|r| r.title == "Outline").unwrap().id.clone();
        lumenna.move_task(&outline, lumenna_host::MoveTarget::Parent { id: report }).unwrap();
    });
    let outline_listed = || app.automation.dump(app.window, 10).iter().any(|l| l.contains("TreeItem 'Outline"));
    app.post(&[GO_TASKS]);
    assert!(outline_listed(), "Outline is listed under Write report");
    // The tree's own Left folds the row, as the app's Collapse.
    app.post(&["select:Write report", "left"]);
    assert!(!outline_listed(), "Outline is folded away under Write report");
    app.post(&["select:Call Sam", "cmd:140", GO_TASKS]);
    assert!(app.focus().contains("TreeItem 'Call Sam'"), "back on the row left in hand: {}", app.focus());
    assert!(!outline_listed(), "still folded on coming back");
    // A write from elsewhere redraws the list and moves nothing.
    app.store().add_task("Water plants").unwrap();
    let listed = automation::wait(Duration::from_secs(5), || {
        app.automation.dump(app.window, 10).iter().any(|l| l.contains("TreeItem 'Water plants'")).then_some(())
    });
    assert!(listed.is_some(), "Water plants never appeared");
    assert!(app.focus().contains("TreeItem 'Call Sam'"), "focus stays where it was: {}", app.focus());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn typing_in_the_details_is_kept_through_a_change_elsewhere_and_saving_sends_only_what_changed() {
    let app = App::launch(|lumenna| add(lumenna, "Buy milk"));
    app.post(&[GO_TASKS, "home", "f6", "cmd:125", "text:Buy oat milk"]);
    // Another device moves its due date meanwhile.
    let store = app.store();
    let id = store.list_tasks("").unwrap().rows[0].id.clone();
    store.edit_task(&id, lumenna_host::TaskEdit { priority: Some(1), ..Default::default() }).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    assert!(app.focus().contains("Buy oat milk"), "the typing is kept: {}", app.focus());
    app.post(&["cmd:162"]);
    let saved = automation::wait(Duration::from_secs(5), || {
        store.show_task(&id).ok().filter(|shown| shown.task.title == "Buy oat milk")
    });
    let saved = saved.unwrap_or_else(|| {
        panic!("the title was saved: {:?}, said {}, focus {}", store.show_task(&id).map(|s| s.task.title), app.status(), app.focus())
    });
    assert_eq!(saved.task.priority, 1, "the priority set elsewhere holds: only the title was sent");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_details_field_with_focus_takes_a_change_made_elsewhere_once_nothing_is_typed() {
    let app = App::launch(|lumenna| add(lumenna, "Buy milk"));
    app.post(&[GO_TASKS, "home", "f6"]);
    assert!(app.focus().starts_with("Edit 'Title:'"), "{}", app.focus());
    let store = app.store();
    let id = store.list_tasks("").unwrap().rows[0].id.clone();
    store.edit_task(&id, lumenna_host::TaskEdit { title: Some("Buy oat milk".to_owned()), ..Default::default() }).unwrap();
    let taken = automation::wait(Duration::from_secs(5), || app.focus().contains("value='Buy oat milk'").then_some(()));
    assert!(taken.is_some(), "the field shows the change: {}", app.focus());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn f6_goes_round_the_panes() {
    let app = App::launch(|lumenna| add(lumenna, "Buy milk"));
    app.post(&[GO_TASKS, "f6"]);
    assert!(app.focus().starts_with("Edit 'Title:'"), "the details: {}", app.focus());
    app.post(&["f6"]);
    assert!(app.focus().starts_with("Tree 'Places'"), "the places: {}", app.focus());
    app.post(&["f6"]);
    assert!(app.focus().starts_with("Tree 'Tasks'"), "back to the list: {}", app.focus());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_task_another_process_adds_appears_without_a_key_pressed() {
    let app = App::launch(|lumenna| add(lumenna, "Buy milk"));
    app.post(&[GO_TASKS]);
    // As `lum` would, from a process of its own. Noticing by `refresh` alone could miss this
    // when the app's sync loop refreshed first (the GTK app's tests caught it); this has not
    // reproduced that race here, but holds the app to noticing at all.
    for title in ["Call Sam", "Water plants", "Pay rent"] {
        app.store().add_task(title).unwrap();
        let listed = automation::wait(Duration::from_secs(5), || {
            app.automation.dump(app.window, 8).iter().any(|l| l.contains(&format!("TreeItem '{title}'"))).then_some(())
        });
        assert!(listed.is_some(), "{title} never appeared");
    }
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn every_control_in_the_main_window_is_named() {
    let app = App::launch(|lumenna| add(lumenna, "Buy milk"));
    app.post(&[GO_TASKS, "home"]);
    let missing = unnamed(&app.automation.dump(app.window, 10));
    assert!(missing.is_empty(), "unnamed: {missing:#?}");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn every_settings_page_names_its_controls() {
    let app = App::launch(|_| {});
    app.post(&[SETTINGS]);
    for page in ["General", "Planning", "Devices", "Backups", "Export and Import"] {
        app.post(&[&format!("select:{page}")]);
        let lines = app.front();
        assert!(lines.iter().any(|l| l.contains(&format!("TabItem '{page}' selected"))), "{page} is shown");
        let missing = unnamed(&lines);
        assert!(missing.is_empty(), "unnamed on {page}: {missing:#?}");
    }
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_setting_changed_in_settings_is_said_and_kept() {
    let app = App::launch(|_| {});
    app.post(&[SETTINGS, "select:Planning", "focus:Day ends:", "text:7:30", "tab"]);
    // Said once the store has taken it, which is a write's answer away.
    let said = automation::wait(Duration::from_secs(5), || {
        app.front().iter().any(|l| l.contains("Day ends: 7:30") && l.contains("live=polite")).then_some(())
    });
    assert!(said.is_some(), "{:#?}", app.front());
    app.post(&["esc"]);
    assert_eq!(app.store().settings(Some("day-end".to_owned())).unwrap().announcement, "07:30");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn delete_in_the_sidebar_asks_before_deleting_a_project() {
    let app = App::launch(|lumenna| {
        lumenna.add_project("Garden", None).unwrap();
    });
    // Today, Tasks, Projects, Inbox, Garden.
    app.post(&["f6", "home", "down", "down", "down", "down"]);
    assert!(app.focus().contains("TreeItem 'Garden"), "{}", app.focus());
    app.post(&["delete"]);
    let dialog = app.front();
    // The question once, as the main instruction, under a title bar naming the app.
    assert!(dialog.iter().any(|l| l.contains("Window 'Lumenna'")), "{dialog:#?}");
    assert_eq!(dialog.iter().filter(|l| l.contains("Delete Garden?")).count(), 1, "{dialog:#?}");
    assert!(dialog.iter().any(|l| l.contains("Button 'Cancel'") && l.contains("FOCUSED")), "Cancel is the default: {dialog:#?}");
    app.post(&["esc"]);
    let projects = app.store().list_projects().unwrap();
    assert!(projects.rows.iter().any(|p| p.title == "Garden"), "nothing was deleted");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_project_renamed_from_the_sidebar_is_shown_under_its_new_name_its_row_still_in_hand() {
    let app = App::launch(|lumenna| {
        lumenna.add_project("Garden", None).unwrap();
    });
    // Today, Tasks, Projects, Inbox, Garden.
    app.post(&["f6", "home", "down", "down", "down", "down", "f2"]);
    assert!(app.front().iter().any(|l| l.contains("Edit") && l.contains("FOCUSED")), "F2 asks for the name: {:#?}", app.front());
    app.post(&["text:Allotment", "enter"]);
    let renamed = automation::wait(Duration::from_secs(5), || app.focus().contains("TreeItem 'Allotment").then_some(()));
    assert!(renamed.is_some(), "the sidebar's row in hand follows the rename: {}", app.focus());
    let lines = app.automation.dump(app.window, 6);
    assert!(lines.iter().any(|l| l.trim_start().starts_with("Tree 'Allotment'")), "the list shown is the project's: {lines:#?}");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn the_keyboard_opens_a_rows_context_menu() {
    let app = App::launch(|lumenna| add(lumenna, "Buy milk"));
    app.post(&[GO_TASKS, "home", "context"]);
    assert!(app.automation.menu_open(), "a menu is open");
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn quick_add_reads_back_what_it_will_add_before_adding_it() {
    let app = App::launch(|_| {});
    app.post(&[NEW_TASK, "text:Call Sam tomorrow p1", "tab"]);
    let read_back = |focus: &str| focus.starts_with("Edit 'Will add:'") && focus.contains("Call Sam") && focus.contains("priority 1");
    assert!(app.lands(read_back), "{}", app.focus());
    app.post(&["enter"]);
    assert!(app.says(|s| s.contains("Call Sam")), "{}", app.status());
    let tasks = app.store().list_tasks("").unwrap();
    assert!(tasks.rows.iter().any(|t| t.title == "Call Sam"));
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn completion_offers_only_what_fits_name_first_with_the_first_highlighted() {
    let app = App::launch(|lumenna| {
        lumenna.add_project("Work", None).unwrap();
        lumenna.add_project("Errands", None).unwrap();
    });
    app.post(&[NEW_TASK, "text:Call #Wo", "down"]);
    // What was typed narrowed it already; the name comes first, so its letter — W, not the
    // P of "project" — finds it; and it has focus as the menu opens, so it is read at once.
    // (UI Automation's system-wide focused element names the menu bar while a popup is open;
    // a screen reader follows the item's own focus, which is what this reads.)
    assert_eq!(app.automation.menu_items(), ["MenuItem 'Work, project' [1 of 1, level 0] key=w FOCUSED"]);
    app.post(&["enter"]);
    assert!(app.focus().contains("value='Call #Work'"), "{}", app.focus());
    assert!(!app.automation.menu_open());
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_task_is_put_in_a_block_from_this_weeks_work_blocks_or_another_days() {
    let app = App::launch(|lumenna| {
        add(lumenna, "Write report");
        // Ending at midnight, so today's is never over yet, whenever the test runs: a block
        // already over is not offered to put a task in.
        let every_day = NewBlock {
            title: "Deep work".to_owned(),
            at: "11pm".to_owned(),
            minutes: 60,
            date: None,
            kind: "work".to_owned(),
            repeat: Some("every day".to_owned()),
            ..NewBlock::default()
        };
        lumenna.add_block(every_day).unwrap();
        let lunch = NewBlock { title: "Lunch".to_owned(), kind: "break".to_owned(), at: "12pm".to_owned(), minutes: 45, date: None, repeat: Some("every day".to_owned()), ..NewBlock::default() };
        lumenna.add_block(lunch).unwrap();
    });
    app.post(&[GO_TASKS, "home", PUT_IN_BLOCK]);
    let picker = app.front();
    let choices: Vec<&String> = picker.iter().filter(|l| l.trim_start().starts_with("ListItem")).collect();
    assert_eq!(choices.len(), 8, "a work block a day for a week, no breaks, then another day: {choices:#?}");
    assert!(choices[0].contains("Today") && choices[0].contains("Deep work"), "{}", choices[0]);
    assert!(choices[7].contains("Another Day"), "{}", choices[7]);
    app.post(&["enter", "text:30", "enter"]);
    assert!(app.says(|s| s.starts_with("Scheduled Write report into Deep work")), "{}", app.status());

    // A block further off: Another Day asks which, then offers that day's.
    let later = jiff::Zoned::now().date().checked_add(jiff::Span::new().days(10)).unwrap();
    app.post(&[PUT_IN_BLOCK, "end", "enter"]);
    let asked = automation::wait(Duration::from_secs(5), || app.focus().starts_with("Edit").then_some(()));
    assert!(asked.is_some(), "Another Day asks which day: {}", app.focus());
    app.post(&[&format!("text:{later}"), "enter"]);
    let offered = automation::wait(Duration::from_secs(5), || {
        let picker = app.front();
        let choices: Vec<String> = picker.into_iter().filter(|l| l.trim_start().starts_with("ListItem")).collect();
        (choices.len() == 2).then_some(choices)
    });
    let offered = offered.unwrap_or_else(|| panic!("that day's one work block, then another day: {:#?}", app.front()));
    assert!(offered[0].contains("Deep work") && !offered[0].contains("Today"), "{}", offered[0]);
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn space_starts_and_pauses_a_sittings_timer_and_the_menu_offers_resume_and_stop() {
    let app = App::launch(|lumenna| {
        add(lumenna, "Write report");
        let block = NewBlock {
            title: "Deep work".to_owned(),
            at: "11:30pm".to_owned(),
            minutes: 25,
            kind: "work".to_owned(),
            ..NewBlock::default()
        };
        lumenna.add_block(block).unwrap();
        let task = lumenna.list_tasks("").unwrap().rows[0].id.clone();
        let block = lumenna.plan(None).unwrap().blocks[0].series.clone();
        lumenna.assign(&task, &block, None, Some(20)).unwrap();
    });
    // The sitting's row, by the name the app gives it.
    let row = |lumenna: &Lumenna| {
        let plan = lumenna.plan(None).unwrap();
        lumenna_app::speech::sitting(&plan.blocks[0].assignments[0])
    };
    app.post(&[&format!("select:{}", row(&app.store())), "space"]);
    assert!(app.says(|s| s.starts_with("Started timer")), "{}", app.status());
    app.post(&["space"]);
    assert!(app.says(|s| s.to_lowercase().contains("paused")), "{}", app.status());
    assert!(app.focus().contains("paused"), "the row says so: {}", app.focus());
    app.post(&["context"]);
    let items = app.automation.menu_items();
    assert!(items.iter().any(|i| i.contains("Resume Timer")), "{items:#?}");
    assert!(items.iter().any(|i| i.contains("Stop Timer")), "{items:#?}");
    assert!(!items.iter().any(|i| i.contains("Pause Timer")), "{items:#?}");
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn the_block_form_has_every_setting_and_its_flags_follow_the_kind() {
    let app = App::launch(|_| {});
    // New Block, called Commute, at 8am, then Break as its kind — through the form in its tab
    // order, as a keyboard reaches it.
    app.post(&["cmd:101", "text:Commute", "tab", "text:8am", "tab", "tab", "down"]);
    let form = app.front();
    assert!(unnamed(&form).is_empty(), "{:#?}", unnamed(&form));
    let flag = |name: &str| form.iter().find(|l| l.contains(&format!("CheckBox '{name}'"))).cloned().unwrap_or_default();
    assert!(flag("Takes tasks").contains(" unchecked"), "a break takes no tasks: {form:#?}");
    assert!(flag("Counts toward hours for work").contains(" unchecked"), "{}", flag("Counts toward hours for work"));
    // Set apart from its kind — someone who works on the train — then the flags, the first
    // day, and the right-hand column: repeats, last day, shortest length, filter, colour.
    app.post(&[
        "tab",
        "space",
        "tab",
        "tab",
        "tab",
        "tab",
        "text:every weekday",
        "tab",
        "tab",
        "tab",
        "text:#Inbox",
        "tab",
        "text:teal",
        "enter",
    ]);
    assert!(app.says(|s| s.starts_with("Added")), "{}", app.status());
    let store = app.store();
    let id = store.list_blocks().unwrap().rows[0].id.clone();
    let shown = store.show_block(&id).unwrap();
    assert_eq!((shown.kind.as_str(), shown.accepts_tasks, shown.counts_capacity), ("break", true, false));
    assert_eq!(shown.task_filter.as_deref(), Some("#Inbox"));
    assert_eq!(shown.colour.as_deref(), Some("teal"));
    assert!(shown.repeats, "{shown:?}");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_block_form_refused_stays_open_as_typed_with_why() {
    let app = App::launch(|lumenna| {
        let block = NewBlock { title: "Deep work".to_owned(), at: "11pm".to_owned(), minutes: 60, kind: "work".to_owned(), ..NewBlock::default() };
        lumenna.add_block(block).unwrap();
    });
    app.post(&["cmd:142", "home", "enter"]);
    let opened = automation::wait(Duration::from_secs(5), || app.front().iter().any(|l| l.contains("Change Deep work")).then_some(()));
    assert!(opened.is_some(), "Enter changes the block: {:#?}", app.front());
    app.post(&["focus:Starts at:", "text:blorp", "enter"]);
    let refused = automation::wait(Duration::from_secs(5), || app.front().iter().any(|l| l.contains("Window 'Lumenna'")).then_some(()));
    assert!(refused.is_some(), "why is said: {:#?}", app.front());
    app.post(&["enter"]);
    let kept = automation::wait(Duration::from_secs(5), || {
        app.front().iter().any(|l| l.contains("Change Deep work")).then_some(()).filter(|_| app.focus().contains("blorp"))
    });
    assert!(kept.is_some(), "the form stays, as typed: {} {:#?}", app.focus(), app.front());
    app.post(&["esc"]);
    let store = app.store();
    let id = store.list_blocks().unwrap().rows[0].id.clone();
    assert_eq!(store.show_block(&id).unwrap().start, "23:00", "nothing was saved");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_new_blocks_start_is_the_cores_and_follows_its_day_until_the_start_is_changed() {
    // The day starts at midnight, so another day's start (00:00) is never today's, which is
    // the next whole hour, whatever time the test runs.
    let app = App::launch(|store| {
        store.set_setting("day-start", "00:00").unwrap();
    });
    let store = app.store();
    let before = store.new_block_start(None).unwrap();
    app.post(&["cmd:101", "focus:Starts at:"]);
    let after = store.new_block_start(None).unwrap();
    let today = app.focus();
    assert!(today.contains(&format!("value='{before}'")) || today.contains(&format!("value='{after}'")), "{today}");
    assert!(!today.contains("value='00:00'"), "{today}");
    // Another day asks the core again.
    app.post(&["focus:Day:", "text:tomorrow", "focus:Starts at:"]);
    assert!(app.focus().contains("value='00:00'"), "{}", app.focus());
    // A start the person typed stays, whatever day follows.
    app.post(&["text:7pm", "focus:Day:", "text:today", "focus:Starts at:"]);
    assert!(app.focus().contains("value='7pm'"), "{}", app.focus());
    app.post(&["esc"]);
    assert!(store.list_blocks().unwrap().rows.is_empty());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_blocks_reminders_are_listed_added_and_removed_as_the_core_offers_them() {
    let app = App::launch(|lumenna| {
        let block = NewBlock { title: "Deep work".to_owned(), at: "11pm".to_owned(), minutes: 60, kind: "work".to_owned(), ..NewBlock::default() };
        lumenna.add_block(block).unwrap();
    });
    let store = app.store();
    let series = store.list_blocks().unwrap().rows[0].id.clone();
    let defaults = store.reminders_of(series.clone()).unwrap();
    app.post(&["cmd:142", "home", "context"]);
    let menu = choose_from_menu(&app, "Reminders...");
    let at = |name: &str| menu.iter().position(|m| m == name);
    assert!(at("Reminders...") < at("Delete Block..."), "{menu:?}");
    let dialog = app.front();
    assert!(dialog.iter().any(|l| l.contains("Window 'Reminders'")), "{dialog:#?}");
    assert!(dialog.iter().any(|l| l.contains(&defaults.reminders[0].words)), "the default is listed: {dialog:#?}");
    assert!(dialog.iter().any(|l| l.contains(&defaults.announcement)), "{dialog:#?}");
    assert!(unnamed(&dialog).is_empty(), "{:#?}", unnamed(&dialog));

    // Add Reminder asks for a phrase, and the block's own replaces the default.
    app.post(&["invoke:Add Reminder...", "text:at end", "enter"]);
    let own = store.reminders_of(series.clone()).unwrap();
    assert!(own.reminders.len() == 1 && own.reminders[0].own, "{own:?}");
    let dialog = app.front();
    assert!(dialog.iter().any(|l| l.contains(&own.reminders[0].words)), "{dialog:#?}");
    // A phrase the core cannot read is refused, and asked again.
    app.post(&["invoke:Add Reminder...", "text:whenever", "enter"]);
    assert!(app.front().iter().any(|l| l.contains("Window 'Lumenna'")), "the refusal: {:#?}", app.front());
    app.post(&["enter", "esc"]);
    assert_eq!(store.reminders_of(series.clone()).unwrap().reminders.len(), 1);

    // Delete in the list removes it, and the default is back.
    app.post(&["focus:Reminders:", "delete"]);
    let after = store.reminders_of(series).unwrap();
    assert!(after.reminders.iter().all(|r| !r.own), "{after:?}");
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_reminder_that_comes_due_is_a_windows_notification_with_the_cores_buttons() {
    use windows::UI::Notifications::ToastNotificationManager;
    use windows::core::HSTRING;

    // An identity of the test's own, so it shows and clears nothing of the person's.
    const ID: &str = "io.github.emassey0135.Lumenna.UiTest";
    let due = jiff::Zoned::now().checked_sub(jiff::Span::new().minutes(1)).unwrap();
    let app = App::launch_with(&[("LUMENNA_APP_ID", ID)], |lumenna| {
        lumenna.set_setting("reminders", "true").unwrap();
        lumenna.add_task(&format!("Call the bank {} at {}", due.date(), due.strftime("%H:%M"))).unwrap();
        let task = lumenna.list_tasks("").unwrap().rows[0].id.clone();
        lumenna.add_reminder(task, "due".to_owned(), 0).unwrap();
        // As if this device last gave reminders a few minutes ago, so the minute just gone is
        // owed now rather than after the next one.
        let since = jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_mins(3)).unwrap();
        let settings = lumenna.path().join("device-settings");
        let mut text = std::fs::read_to_string(&settings).unwrap_or_default();
        text.push_str(&format!("reminders-delivered\t{since}\n"));
        std::fs::write(&settings, text).unwrap();
    });
    let history = ToastNotificationManager::History().unwrap();
    let shown = automation::wait(Duration::from_secs(15), || {
        let toasts = history.GetHistoryWithId(&HSTRING::from(ID)).ok()?;
        (0..toasts.Size().ok()?).find_map(|i| toasts.GetAt(i).ok()?.Content().ok()?.GetXml().ok().map(|x| x.to_string()))
    });
    let _ = history.ClearWithId(&HSTRING::from(ID));
    windows_registry_forget(ID);
    let xml = shown.expect("a toast was shown");
    assert!(xml.contains("Call the bank"), "{xml}");
    let store = app.store();
    let task = store.list_tasks("").unwrap().rows[0].id.clone();
    // Its buttons are the core's.
    let words = store.reminders_due(jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_mins(3)).unwrap().to_string(), None).unwrap();
    let reminder = words.due.iter().find(|r| r.target == task).expect("the core has it due");
    assert!(xml.contains(&reminder.dismiss) && xml.contains(&reminder.snooze), "{xml}");
}

/// Chooses `name` from the context menu that is open, by the arrow keys as the keyboard
/// reaches it, and returns every item it had.
fn choose_from_menu(app: &App, name: &str) -> Vec<String> {
    let items = app.automation.menu_items();
    let names: Vec<String> = items.iter().map(|i| i.split('\'').nth(1).unwrap_or_default().to_owned()).collect();
    let at = names.iter().position(|n| n == name).unwrap_or_else(|| panic!("no {name} in {names:?}"));
    // Down skips separators, so it counts the items before it.
    let before = items[..at].iter().filter(|i| !i.starts_with("Separator")).count();
    for _ in 0..=before {
        app.post(&["down"]);
    }
    app.post(&["enter"]);
    names
}

/// Removes the identity a test gave Windows for its toasts.
fn windows_registry_forget(id: &str) {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteTreeW};
    unsafe {
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, &windows::core::HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{id}")));
    }
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn the_pairing_dialog_asks_for_a_code_when_the_field_is_empty() {
    let app = App::launch(|_| {});
    app.post(&[SETTINGS, "select:Devices", "invoke:Pair a Device..."]);
    let dialog = app.front();
    assert!(dialog.iter().any(|l| l.contains("Window 'Pair a Device'")), "{dialog:#?}");
    assert!(unnamed(&dialog).is_empty(), "{:#?}", unnamed(&dialog));
    app.post(&["invoke:Pair Using This Code"]);
    let asked = app.front();
    assert!(asked.iter().any(|l| l.contains("Type or paste the code the other device shows.")), "{asked:#?}");
    app.post(&["enter"]);
    let dialog = app.front();
    assert!(dialog.iter().any(|l| l.contains("Window 'Pair a Device'")), "{dialog:#?}");
    assert!(dialog.iter().all(|l| !l.contains("Connecting")), "{dialog:#?}");
    app.post(&["esc"]);
}

/// `lum`, built beside the app: `cargo build -p lumenna-cli` with the same target directory.
fn lum() -> std::path::PathBuf {
    let lum = std::path::Path::new(env!("CARGO_BIN_EXE_lumenna")).with_file_name("lum.exe");
    assert!(lum.exists(), "build lum first, into the same target directory: cargo build -p lumenna-cli");
    lum
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn unpaired_the_app_holds_no_endpoint_so_lum_rpc_answers_itself_and_the_app_shows_what_it_added() {
    use std::io::{BufRead, BufReader, Write};

    // Nothing paired: the app keeps nothing in sync, so it neither holds the endpoint nor
    // serves the pipe, and the firewall asks nothing until the first pairing.
    let app = App::launch(|_| {});
    std::thread::sleep(Duration::from_secs(2));
    assert!(lumenna_host::endpoint::connect(app.profile.path()).is_none(), "an unpaired app serves no pipe");
    let mut rpc = Command::new(lum())
        .arg("rpc")
        .env("LUMENNA_PROFILE", app.profile.path())
        .env("LUMENNA_BACKUP_DIR", app.profile.path().with_extension("backups"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("lum runs");
    let mut replies = BufReader::new(rpc.stdout.take().unwrap());
    writeln!(rpc.stdin.as_mut().unwrap(), r#"{{"jsonrpc":"2.0","id":1,"method":"initialize"}}"#).unwrap();
    let mut answer = String::new();
    replies.read_line(&mut answer).unwrap();
    assert!(answer.contains(r#""process":"lum rpc""#), "lum answers itself: {answer}");
    writeln!(rpc.stdin.as_mut().unwrap(), r#"{{"jsonrpc":"2.0","id":2,"method":"task.add","params":{{"text":"Sent through lum"}}}}"#).unwrap();
    let mut added = String::new();
    replies.read_line(&mut added).unwrap();
    assert!(added.contains("Sent through lum"), "{added}");
    drop(rpc.stdin.take());
    let _ = rpc.wait();
    // Another process's write, which the app takes in within a second or so.
    app.post(&[GO_TASKS]);
    automation::wait(Duration::from_secs(5), || {
        app.automation.dump(app.window, 6).iter().any(|l| l.contains("TreeItem 'Sent through lum")).then_some(())
    })
    .expect("the app lists the task lum added");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn unpaired_lum_sync_says_it_is_not_paired_without_finding_the_app() {
    let app = App::launch(|_| {});
    std::thread::sleep(Duration::from_secs(2));
    let output = Command::new(lum())
        .arg("sync")
        .env("LUMENNA_PROFILE", app.profile.path())
        .env("LUMENNA_BACKUP_DIR", app.profile.path().with_extension("backups"))
        .output()
        .expect("lum runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("not paired"), "{said}");
    assert!(lumenna_host::endpoint::connect(app.profile.path()).is_none(), "the app holds no endpoint");
}
/// Checks the window in front fits its screen's work area, and that nothing directly in it
/// lies outside it or over anything else: what Text size at its largest would break.
/// The work area of the screen `window` is on.
fn work_area(window: windows::Win32::Foundation::HWND) -> windows::Win32::Foundation::RECT {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};

    let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let _ = GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), &mut monitor);
    }
    monitor.rcWork
}

fn fits_and_nothing_overlaps(app: &App, what: &str) {
    use windows::Win32::Foundation::RECT;

    let front = app.automation.front(app.window);
    // Otherwise what is measured is the main window, and a dialog that never opened reads as
    // one that does not fit.
    assert!(front != app.window, "{what} did not open: {:#?}", app.front());
    let (window, items) = app.automation.boxes(front);
    let work = work_area(front);
    assert!(
        window.left >= work.left && window.top >= work.top && window.right <= work.right && window.bottom <= work.bottom,
        "{what} is off the screen: {window:?} in {work:?}"
    );
    let inside = |r: &RECT| r.left >= window.left && r.top >= window.top && r.right <= window.right && r.bottom <= window.bottom;
    let over = |a: &RECT, b: &RECT| a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
    // A property sheet's tab strip spans its page, and the title bar is the frame's.
    let laid: Vec<&(String, RECT)> = items
        .iter()
        .filter(|(line, r)| {
            r.right > r.left && r.bottom > r.top && !["Tab ", "TitleBar ", "Pane ", "Window "].iter().any(|k| line.starts_with(k))
        })
        .collect();
    for (line, r) in &laid {
        assert!(inside(r), "{what}: {line} lies outside it: {r:?} in {window:?}");
    }
    // A group box frames the controls inside it.
    let laid: Vec<&&(String, RECT)> = laid.iter().filter(|(line, _)| !line.starts_with("Group ")).collect();
    for (i, (a, ra)) in laid.iter().enumerate() {
        for (b, rb) in &laid[i + 1..] {
            assert!(!over(ra, rb), "{what}: {a} {ra:?} overlaps {b} {rb:?}");
        }
    }
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn every_dialog_fits_the_screen_at_the_largest_text_size_with_nothing_overlapping() {
    // Windows' largest Text size, 225%, without changing the person's own setting.
    let app = App::launch_with(&[("LUMENNA_TEXT_SCALE", "2.25")], |lumenna| {
        add(lumenna, "Write report");
        // Ending at midnight, so it is never over and Put in a Block always has it to offer.
        let block = NewBlock { title: "Deep work".to_owned(), at: "11pm".to_owned(), minutes: 60, kind: "work".to_owned(), ..NewBlock::default() };
        lumenna.add_block(block).unwrap();
    });
    // The main window fits too, whatever the screen.
    let (window, _) = app.automation.boxes(app.window);
    let work = work_area(app.window);
    assert!(
        window.left >= work.left && window.top >= work.top && window.right <= work.right && window.bottom <= work.bottom,
        "the main window is off the screen: {window:?} in {work:?}"
    );
    app.post(&[NEW_TASK]);
    fits_and_nothing_overlaps(&app, "New Task");
    app.post(&["esc", "cmd:101"]);
    fits_and_nothing_overlaps(&app, "New Block");
    app.post(&["esc", GO_TASKS, "home", PUT_IN_BLOCK]);
    fits_and_nothing_overlaps(&app, "Put in a Block");
    app.post(&["esc", "cmd:142", "home", "context"]);
    choose_from_menu(&app, "Reminders...");
    fits_and_nothing_overlaps(&app, "Reminders");
    app.post(&["esc", "cmd:140", "cmd:184"]);
    fits_and_nothing_overlaps(&app, "Plan the Day");
    app.post(&["esc", "cmd:185"]);
    fits_and_nothing_overlaps(&app, "Replan from Now");
    app.post(&["esc", SETTINGS]);
    for page in ["General", "Planning", "Devices", "Backups", "Export and Import"] {
        app.post(&[&format!("select:{page}")]);
        fits_and_nothing_overlaps(&app, &format!("Settings, {page}"));
    }
    app.post(&["select:Devices", "invoke:Pair a Device..."]);
    fits_and_nothing_overlaps(&app, "Pair a Device");
    app.post(&["esc", "esc", "f1"]);
    fits_and_nothing_overlaps(&app, "Keyboard Shortcuts");
    app.post(&["esc"]);
}

/// Windows' own Text size, as the Settings app sets it: its slider and its Apply button, so
/// what Windows does when a person changes it is what the app is given. What this found set
/// is put back when it is dropped, whatever happened in between.
struct TextSize<'a> {
    automation: &'a Automation,
    was: u32,
    opened_settings: bool,
}

impl<'a> TextSize<'a> {
    fn new(automation: &'a Automation) -> Self {
        let opened_settings = automation.window_named("Settings").is_none();
        Self { automation, was: Self::now(), opened_settings }
    }

    /// The setting, in percent: no value is 100.
    fn now() -> u32 {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
        let mut value = 0u32;
        let mut size = 4u32;
        let read = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                windows::core::w!(r"Software\Microsoft\Accessibility"),
                windows::core::w!("TextScaleFactor"),
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut value).cast()),
                Some(&mut size),
            )
        };
        if read.is_ok() { value } else { 100 }
    }

    /// Moves the slider to `percent` and applies it, waiting until Windows has.
    fn set(&self, percent: u32) -> Result<(), String> {
        Command::new("explorer.exe").arg("ms-settings:easeofaccess-display").status().map_err(|e| e.to_string())?;
        let settings = automation::wait(Duration::from_secs(20), || {
            self.automation.window_named("Settings").filter(|w| self.automation.named(*w, "Apply").is_some())
        })
        .ok_or("the Settings app did not show Text size")?;
        self.automation.set_range(settings, "Text size", f64::from(percent))?;
        std::thread::sleep(automation::SETTLE);
        self.automation.invoke(settings, "Apply")?;
        automation::wait(Duration::from_secs(20), || (Self::now() == percent).then_some(()))
            .ok_or_else(|| format!("Text size did not become {percent}%"))
    }
}

impl Drop for TextSize<'_> {
    fn drop(&mut self) {
        if Self::now() != self.was
            && let Err(why) = self.set(self.was)
        {
            eprintln!("COULD NOT PUT TEXT SIZE BACK to {}%: {why}. Set it in Settings, Accessibility, Text size.", self.was);
        }
        if self.opened_settings
            && let Some(settings) = self.automation.window_named("Settings")
        {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    Some(settings),
                    windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                );
            }
        }
    }
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn text_grows_without_a_restart_when_windows_text_size_is_raised() {
    // It changes Text size for the whole PC while it runs, so only when asked.
    if std::env::var_os("LUMENNA_CHANGE_TEXT_SIZE").is_none() {
        eprintln!("skipped: set LUMENNA_CHANGE_TEXT_SIZE=1 to let it raise Windows' Text size, and put it back");
        return;
    }
    let app = App::launch(|lumenna| {
        add(lumenna, "Write report");
        add(lumenna, "Call Sam");
    });
    app.post(&[GO_TASKS, "home"]);
    // The heights things are drawn at: Win32's fields offer no text pattern, so UI
    // Automation has no font size to give for them.
    let measure = || {
        let height = |element: Option<windows::Win32::UI::Accessibility::IUIAutomationElement>, what: &str| {
            let place = unsafe { element.unwrap_or_else(|| panic!("no {what}")).CurrentBoundingRectangle() }.unwrap();
            place.bottom - place.top
        };
        [
            height(app.automation.named(app.window, "Write report"), "row for the task"),
            height(app.automation.focusable(app.window, "Title:"), "Title field"),
            // The first thing named Title is its label.
            height(app.automation.named(app.window, "Title:"), "Title label"),
        ]
    };
    let before = measure();

    let text_size = TextSize::new(&app.automation);
    let was = text_size.was;
    let raised = if was >= 200 { was - 50 } else { was + 50 };
    text_size.set(raised).unwrap_or_else(|why| panic!("{why}"));
    // Half the change at least: a row and a field add padding that does not grow with text.
    let enough = f64::from(raised - was) / f64::from(was) / 2.0;
    let grew = |now: &[i32; 3]| now.iter().zip(&before).all(|(n, b)| f64::from(*n) >= f64::from(*b) * (1.0 + enough));
    let after = automation::wait(Duration::from_secs(10), || Some(measure()).filter(grew));
    drop(text_size);

    let names = ["the task's row", "the Title field", "the Title label"];
    eprintln!("Text size {was}% -> {raised}%: {}", names.iter().zip(before).zip(after.unwrap_or_else(measure)).map(|((n, b), a)| format!("{n} {b}px -> {a}px")).collect::<Vec<_>>().join(", "));
    assert!(after.is_some(), "the app did not grow its text without a restart when Text size went from {was}% to {raised}%");
}
#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_rows_menu_is_the_cores_and_never_offers_a_task_as_its_own_parent() {
    let app = App::launch(|lumenna| {
        add(lumenna, "Write report");
        add(lumenna, "Outline");
        let rows = lumenna.list_tasks("").unwrap().rows;
        let report = rows.iter().find(|r| r.title == "Write report").unwrap().id.clone();
        let outline = rows.iter().find(|r| r.title == "Outline").unwrap().id.clone();
        lumenna.move_task(&outline, lumenna_host::MoveTarget::Parent { id: report }).unwrap();
    });
    let names = |items: Vec<String>| -> Vec<String> {
        items.iter().map(|item| item.split('\'').nth(1).unwrap_or_default().to_owned()).collect()
    };
    app.post(&[GO_TASKS, "home", "context"]);
    let menu = names(app.automation.menu_items());
    app.post(&["esc"]);
    assert_eq!(
        menu,
        ["Mark Done", "Edit Details", "Put in a Block...", "Move to Project...", "Make Subtask Of...", "Wait For...", "Move to Trash", "Collapse"],
        "the task's actions, as the core gives them, and the app's folding of what is under it"
    );
    // Its own subtask is not somewhere it can go.
    app.post(&["cmd:164"]);
    let offered: Vec<String> = app.front().into_iter().filter(|l| l.contains("ListItem")).collect();
    assert!(offered.is_empty(), "Write report was offered as going under {offered:#?}");
    assert!(app.says(|s| s.contains("no task it could go under")), "{}", app.status());

    // The Inbox keeps its name and place: only reordered and weighed.
    app.post(&["cmd:145", "select:Inbox, 2 open tasks", "context"]);
    let menu = names(app.automation.menu_items());
    app.post(&["esc"]);
    assert_eq!(menu, ["Weight..."], "the Inbox's menu");
    // Delete there says why not, in the core's words.
    app.post(&["delete"]);
    let why = lumenna_host::not_offered(lumenna_host::ActionKind::Delete, lumenna_host::Subject::Project, false);
    assert_eq!(app.status(), lumenna_app::speech::sentence(&why));
    // F2 is Rename, as in Explorer; the Inbox keeps its name.
    app.post(&["f2"]);
    let why = lumenna_host::not_offered(lumenna_host::ActionKind::Rename, lumenna_host::Subject::Project, false);
    assert_eq!(app.status(), lumenna_app::speech::sentence(&why));
}


#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn f1_lists_every_key_and_alt_enter_opens_the_tasks_details() {
    let app = App::launch(|lumenna| add(lumenna, "Write report"));
    app.post(&["f1"]);
    let help = app.front();
    let text = help.iter().find(|l| l.trim_start().starts_with("Edit 'Shortcuts:'")).cloned().unwrap_or_default();
    assert!(text.contains("New Task: Ctrl+N") && text.contains("Exit: Ctrl+Q") && text.contains("Rename: F2"), "{help:#?}");
    assert!(app.focus().starts_with("Edit 'Shortcuts:'"), "focus starts in the list: {}", app.focus());
    app.post(&["esc"]);
    // Alt+Enter, as the menu command it stands for: the selected task's details.
    app.post(&[GO_TASKS, "home", "cmd:113"]);
    assert!(app.focus().starts_with("Edit 'Title:'"), "{}", app.focus());
}

// ---------------------------------------------------------------------------------------
// The planner: the day's own actions, its proposals, a choice's reason, a task's history
// ---------------------------------------------------------------------------------------

const NEXT_DAY: &str = "cmd:181";
const PLAN_DAY: &str = "cmd:184";
const REPLAN: &str = "cmd:185";
const WHAT_NOW: &str = "cmd:186";
const END_DAY: &str = "cmd:187";

/// Today, once the last few minutes of the day are over: a test that seeds today's blocks
/// and the app it starts must agree on which day that is.
fn today_clear_of_midnight() -> jiff::civil::Date {
    let now = jiff::Zoned::now();
    let left = 24 * 60 - (i32::from(now.hour()) * 60 + i32::from(now.minute()));
    if left <= 3 {
        std::thread::sleep(Duration::from_secs(u64::try_from(left * 60 + 5).unwrap_or(0)));
    }
    jiff::Zoned::now().date()
}

/// A work block on `date` from `at` for `minutes`, happening once; its identifier.
fn work_block(lumenna: &Lumenna, title: &str, date: jiff::civil::Date, at: &str, minutes: u32) -> String {
    let block = NewBlock {
        title: title.to_owned(),
        at: at.to_owned(),
        minutes,
        date: Some(date.to_string()),
        kind: "work".to_owned(),
        ..NewBlock::default()
    };
    lumenna.add_block(block).unwrap();
    lumenna.plan(Some(date.to_string())).unwrap().blocks.into_iter().find(|b| b.title == title).unwrap().id
}

/// The whole of today, so it is under way whenever the test runs: 1440 minutes, since one
/// short of that is over in the day's last minute.
fn all_day(lumenna: &Lumenna, today: jiff::civil::Date) -> String {
    work_block(lumenna, "Deep work", today, "00:00", 1440)
}

/// The rows of the window in front that are tree items, as described.
fn tree_items(app: &App) -> Vec<String> {
    app.front().into_iter().map(|l| l.trim_start().to_owned()).filter(|l| l.starts_with("TreeItem")).collect()
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn the_next_day_says_itself_and_go_to_day_asks_again_with_a_phrase_it_cannot_read() {
    let app = App::launch(|_| {});
    app.post(&[NEXT_DAY]);
    let said = automation::wait(Duration::from_secs(5), || Some(app.status()).filter(|s| s.starts_with("Tomorrow.")));
    assert!(said.is_some(), "the new day's heading is said: {}", app.status());
    assert!(app.focus().contains("TreeItem 'Tomorrow."), "on its summary: {}", app.focus());
    // Go to Day is the app's question; what it cannot read is said, and asked again as typed.
    app.post(&["cmd:183"]);
    let asked = automation::wait(Duration::from_secs(5), || app.focus().starts_with("Edit").then_some(()));
    assert!(asked.is_some(), "Go to Day asks for a day: {}", app.focus());
    app.post(&["text:blorp", "enter"]);
    let refused = automation::wait(Duration::from_secs(5), || {
        app.front().iter().any(|l| l.contains("Window 'Lumenna'")).then_some(())
    });
    assert!(refused.is_some(), "the refusal is said: {:#?}", app.front());
    app.post(&["enter"]);
    let again = automation::wait(Duration::from_secs(5), || app.focus().contains("blorp").then_some(()));
    assert!(again.is_some(), "asked again with what was typed: {}", app.focus());
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn the_days_own_actions_are_its_summary_rows_and_the_day_menus() {
    let today = today_clear_of_midnight();
    let app = App::launch(|lumenna| {
        all_day(lumenna, today);
    });
    app.post(&["home", "context"]);
    let names = choose_from_menu(&app, "End the Day...");
    let at = |name: &str| names.iter().position(|n| n == name).unwrap_or_else(|| panic!("no {name} in {names:?}"));
    assert!(
        at("Plan the Day...") < at("Replan from Now...")
            && at("Replan from Now...") < at("What Should I Work On?...")
            && at("What Should I Work On?...") < at("End the Day..."),
        "in the core's order: {names:?}"
    );
    // Asked before it is done, in the core's words.
    let asked = app.front();
    assert!(asked.iter().any(|l| l.contains("End the day?")), "{asked:#?}");
    app.post(&["esc"]);
    // A day with nothing to do to it says so, rather than nothing.
    app.post(&[NEXT_DAY, END_DAY]);
    assert_eq!(app.status(), "That does not apply here.");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn what_should_i_work_on_says_why_each_is_offered_and_starts_the_one_chosen() {
    let today = today_clear_of_midnight();
    let app = App::launch(|lumenna| {
        all_day(lumenna, today);
        add(lumenna, "Write report p1");
    });
    app.post(&[WHAT_NOW]);
    let picker = app.front();
    let offered = picker.iter().find(|l| l.trim_start().starts_with("ListItem 'Write report")).unwrap_or_else(|| panic!("{picker:#?}"));
    // Why, as its description; never in its name.
    let name = offered.split('\'').nth(1).unwrap_or_default();
    assert!(!name.contains("Suggested"), "{offered}");
    assert!(offered.contains("help='Suggested because") && offered.contains("msaa-description='Suggested because"), "{offered}");
    app.post(&["enter"]);
    let plan = app.store().plan(None).unwrap();
    let started: Vec<&str> = plan.blocks.iter().flat_map(|b| &b.assignments).map(|a| a.title.as_str()).collect();
    assert_eq!(started, ["Write report"], "{}", app.status());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn replan_from_now_says_its_proposal_and_makes_only_the_changes_left_checked() {
    let today = today_clear_of_midnight();
    let app = App::launch(|lumenna| {
        let deep = all_day(lumenna, today);
        // A whole day each: neither fits in what is left of today, whenever the test runs.
        for title in ["Essay", "Email"] {
            let task = lumenna.add_task(title).unwrap().task.unwrap().id;
            lumenna.assign(&task, &deep, Some(today.to_string()), Some(1440)).unwrap();
        }
    });
    app.post(&[REPLAN]);
    let screen = app.front();
    assert!(screen.iter().any(|l| l.contains("Window 'Replan from Now'")), "{screen:#?}");
    assert!(unnamed(&screen).is_empty(), "{:#?}", unnamed(&screen));
    let rows = tree_items(&app);
    assert_eq!(rows.len(), 2, "{screen:#?}");
    assert!(rows.iter().all(|r| r.contains(" checked")), "every change checked to begin with: {rows:#?}");
    // The whole proposal is said as it opens.
    let front = app.automation.front(app.window);
    let said = automation::wait(Duration::from_secs(5), || app.automation.status(front).filter(|s| !s.is_empty()));
    assert!(said.as_deref().is_some_and(|s| s.contains("Essay") && s.contains("Email")), "{said:?}");
    // The first left unchecked: only the second is made.
    let kept = if rows[0].contains("Essay") { "Essay" } else { "Email" };
    app.post(&["space"]);
    assert!(tree_items(&app)[0].contains(" unchecked"), "{:#?}", tree_items(&app));
    app.post(&["enter"]);
    assert!(app.says(|s| s.starts_with("Replanned")), "{}", app.status());
    let plan = app.store().plan(None).unwrap();
    let left: Vec<&str> = plan.blocks[0].assignments.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(left, [kept]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn plan_the_day_says_why_each_sitting_and_proposes_again_around_one_turned_down() {
    let tomorrow = today_clear_of_midnight().tomorrow().unwrap();
    let app = App::launch(|lumenna| {
        work_block(lumenna, "Morning", tomorrow, "09:00", 30);
        work_block(lumenna, "Afternoon", tomorrow, "14:00", 30);
        for task in ["Report p1", "Email p2", "Filing p3"] {
            add(lumenna, task);
        }
    });
    app.post(&[NEXT_DAY, PLAN_DAY]);
    let rows = tree_items(&app);
    assert_eq!(rows.len(), 2, "{:#?}", app.front());
    assert!(rows[0].starts_with("TreeItem 'Report goes in Morning"), "{rows:#?}");
    assert!(
        rows[0].contains("help='Suggested because priority 1.'") && rows[0].contains("msaa-description='Suggested because priority 1.'"),
        "its reason, as its description: {rows:#?}"
    );
    // Report turned down: Email is planned, and Morning is offered to Filing instead.
    app.post(&["space", "enter"]);
    let front = app.automation.front(app.window);
    assert!(front != app.window, "the screen stays while there is more to plan");
    assert_eq!(app.automation.status(front).as_deref(), Some("Planned 1 task. Now: Plans 1 task: Filing in Morning."));
    let rows = tree_items(&app);
    assert!(rows.len() == 1 && rows[0].starts_with("TreeItem 'Filing goes in Morning") && rows[0].contains(" checked"), "{rows:#?}");
    app.post(&["esc"]);
    let plan = app.store().plan(Some(tomorrow.to_string())).unwrap();
    let planned: Vec<(&str, &str)> =
        plan.blocks.iter().flat_map(|b| b.assignments.iter().map(move |a| (b.title.as_str(), a.title.as_str()))).collect();
    assert_eq!(planned, [("Afternoon", "Email")]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_tasks_history_is_on_its_own_screen_and_never_in_the_list() {
    let today = today_clear_of_midnight();
    let app = App::launch(|lumenna| {
        let deep = all_day(lumenna, today);
        let task = lumenna.add_task("Write report").unwrap().task.unwrap().id;
        lumenna.assign(&task, &deep, Some(today.to_string()), Some(30)).unwrap();
    });
    app.post(&[GO_TASKS, "home"]);
    let window = app.front();
    let history = window.iter().find(|l| l.trim_start().starts_with("Edit 'History:'")).unwrap_or_else(|| panic!("{window:#?}"));
    assert!(history.contains("1 sitting"), "{history}");
    assert!(app.focus().contains("Write report") && !app.focus().contains("sitting"), "{}", app.focus());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_block_that_ended_with_work_short_offers_carry_forward_on_its_notification() {
    use windows::UI::Notifications::ToastNotificationManager;
    use windows::core::HSTRING;

    const ID: &str = "io.github.emassey0135.Lumenna.UiTest";
    // A block of a minute that ended a minute ago, so its end is owed now, and one tomorrow
    // with room for what it left short, in this week whatever day it is.
    let now = jiff::Zoned::now();
    if now.hour() == 0 && now.minute() < 3 {
        std::thread::sleep(Duration::from_secs(180));
    }
    let start = jiff::Zoned::now().checked_sub(jiff::Span::new().minutes(2)).unwrap();
    let today = start.date();
    let app = App::launch_with(&[("LUMENNA_APP_ID", ID)], |lumenna| {
        lumenna.set_setting("reminders", "true").unwrap();
        lumenna.set_setting("week-start", &format!("{:?}", today.weekday()).to_lowercase()).unwrap();
        let deep = work_block(lumenna, "Deep work", today, &start.strftime("%H:%M").to_string(), 1);
        work_block(lumenna, "Admin", today.tomorrow().unwrap(), "09:00", 60);
        let task = lumenna.add_task("Essay").unwrap().task.unwrap().id;
        lumenna.assign(&task, &deep, Some(today.to_string()), Some(30)).unwrap();
        let since = jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_mins(3)).unwrap();
        let settings = lumenna.path().join("device-settings");
        let mut text = std::fs::read_to_string(&settings).unwrap_or_default();
        text.push_str(&format!("reminders-delivered\t{since}\n"));
        std::fs::write(&settings, text).unwrap();
    });
    let history = ToastNotificationManager::History().unwrap();
    let shown = automation::wait(Duration::from_secs(15), || {
        let toasts = history.GetHistoryWithId(&HSTRING::from(ID)).ok()?;
        (0..toasts.Size().ok()?)
            .filter_map(|i| toasts.GetAt(i).ok()?.Content().ok()?.GetXml().ok().map(|x| x.to_string()))
            .find(|xml| xml.contains("Essay"))
    });
    let _ = history.ClearWithId(&HSTRING::from(ID));
    windows_registry_forget(ID);
    let xml = shown.expect("the block's end was shown");
    let since = jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_mins(5)).unwrap();
    let due = app.store().reminders_due(since.to_string(), None).unwrap();
    let texts: Vec<(&str, &str)> = due.due.iter().map(|r| (r.text.as_str(), r.carry.as_str())).collect();
    let end = due.due.iter().find(|r| !r.carry.is_empty()).unwrap_or_else(|| panic!("the core offers to carry it: {texts:#?}
{xml}"));
    assert!(xml.contains(&format!(r#"content="{}" arguments="carry""#, end.carry)), "{xml}");
}

/// Now, as minutes into today, once there are `room` minutes of the day left after it: a
/// test laying blocks out from now waits out the end of the day rather than run past it.
fn minutes_with_room(room: i32) -> (jiff::civil::Date, i32) {
    loop {
        let now = jiff::Zoned::now();
        let minutes = i32::from(now.hour()) * 60 + i32::from(now.minute());
        if minutes + room < 24 * 60 {
            return (now.date(), minutes);
        }
        std::thread::sleep(Duration::from_secs(30));
    }
}

/// A clock time `minutes` into the day.
fn clock(minutes: i32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn replan_offers_other_options_and_taking_one_shows_its_proposal_in_place() {
    // Under way until three minutes from now, a break due to start inside it, and work after:
    // the break has to move, unless it is skipped.
    let (today, now) = minutes_with_room(12);
    let app = App::launch(|lumenna| {
        work_block(lumenna, "Deep work", today, "00:00", u32::try_from(now + 3).unwrap());
        let lunch = NewBlock {
            title: "Lunch".to_owned(),
            at: clock(now + 2),
            minutes: 2,
            date: Some(today.to_string()),
            kind: "break".to_owned(),
            ..NewBlock::default()
        };
        lumenna.add_block(lunch).unwrap();
        work_block(lumenna, "Review", today, &clock(now + 4), 4);
    });
    app.post(&[REPLAN]);
    let screen = app.front();
    assert!(screen.iter().any(|l| l.trim_start().starts_with("Text 'Other options:'")), "{screen:#?}");
    let option = screen
        .iter()
        .map(|l| l.trim_start())
        .find(|l| l.starts_with("Button 'Skip Lunch today, your only break left today."))
        .unwrap_or_else(|| panic!("{screen:#?}"));
    let name = option.split('\'').nth(1).unwrap().to_owned();
    app.post(&[&format!("invoke:{name}")]);
    let front = app.automation.front(app.window);
    assert_eq!(app.automation.status(front).as_deref(), Some("Lunch is skipped today."));
    let rows = tree_items(&app);
    assert!(rows.len() == 1 && rows[0].starts_with("TreeItem 'Lunch is skipped today") && rows[0].contains(" checked"), "{rows:#?}");
    // Nothing does better than nothing moving: no other options, and no heading for them.
    let screen = app.front();
    assert!(!screen.iter().any(|l| l.contains("Other options") || l.contains("'Skip ")), "{screen:#?}");
    assert!(app.focus().contains("Lunch is skipped today"), "on what is proposed now: {}", app.focus());
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_blocks_work_outrunning_it_is_a_notification_with_the_cores_replan_button() {
    use windows::UI::Notifications::ToastNotificationManager;
    use windows::core::HSTRING;

    const ID: &str = "io.github.emassey0135.Lumenna.UiTest";
    // A block of two minutes that started a minute ago, ten minutes of work planned in it (an
    // overrun of a few minutes is not said),
    // and another block straight after it to run into.
    let (today, mut now) = minutes_with_room(6);
    while now < 2 {
        std::thread::sleep(Duration::from_secs(30));
        now = minutes_with_room(6).1;
    }
    let app = App::launch_with(&[("LUMENNA_APP_ID", ID)], |lumenna| {
        lumenna.set_setting("reminders", "true").unwrap();
        let deep = work_block(lumenna, "Deep work", today, &clock(now - 1), 2);
        work_block(lumenna, "Meeting", today, &clock(now + 1), 1);
        let task = lumenna.add_task("Essay").unwrap().task.unwrap().id;
        lumenna.assign(&task, &deep, Some(today.to_string()), Some(10)).unwrap();
        let since = jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_mins(3)).unwrap();
        let settings = lumenna.path().join("device-settings");
        let mut text = std::fs::read_to_string(&settings).unwrap_or_default();
        text.push_str(&format!("reminders-delivered\t{since}\n"));
        std::fs::write(&settings, text).unwrap();
    });
    let history = ToastNotificationManager::History().unwrap();
    let shown = automation::wait(Duration::from_secs(15), || {
        let toasts = history.GetHistoryWithId(&HSTRING::from(ID)).ok()?;
        (0..toasts.Size().ok()?)
            .filter_map(|i| toasts.GetAt(i).ok()?.Content().ok()?.GetXml().ok().map(|x| x.to_string()))
            .find(|xml| xml.contains("more than it holds"))
    });
    let _ = history.ClearWithId(&HSTRING::from(ID));
    windows_registry_forget(ID);
    let since = jiff::Timestamp::now().checked_sub(jiff::SignedDuration::from_mins(5)).unwrap();
    let due = app.store().reminders_due(since.to_string(), None).unwrap();
    let texts: Vec<(&str, &str)> = due.due.iter().map(|r| (r.text.as_str(), r.replan.as_str())).collect();
    let xml = shown.unwrap_or_else(|| panic!("the overrun was shown: {texts:#?}"));
    let overrun = due.due.iter().find(|r| !r.replan.is_empty()).unwrap_or_else(|| panic!("the core offers to replan: {texts:#?}"));
    assert!(xml.contains(&format!(r#"content="{}" arguments="replan""#, overrun.replan)), "{xml}");
}
