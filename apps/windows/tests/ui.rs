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
use lumenna_surface::{Lumenna, NewBlock};
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
        let turn = ONE_WINDOW_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let profile = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        seed(&Lumenna::open(profile.path().to_str().unwrap()).unwrap());
        let child = Command::new(env!("CARGO_BIN_EXE_lumenna"))
            .arg("--profile")
            .arg(profile.path())
            // The shortcuts from anywhere are the person's, held by their own copy; a test's
            // must not take them, nor say another program has.
            .arg("--no-shortcuts")
            .env("LUMENNA_BACKUP_DIR", backups.path())
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
    assert!(app.focus().contains("TreeItem 'Call Sam'"), "{}", app.focus());
    assert!(app.status().starts_with("Completed Buy milk"), "{}", app.status());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn f6_goes_round_the_panes() {
    let app = App::launch(|lumenna| add(lumenna, "Buy milk"));
    app.post(&[GO_TASKS, "f6"]);
    assert!(app.focus().starts_with("Edit 'Title'"), "the details: {}", app.focus());
    app.post(&["f6"]);
    assert!(app.focus().starts_with("Tree 'Places'"), "the places: {}", app.focus());
    app.post(&["f6"]);
    assert!(app.focus().starts_with("Tree 'Tasks'"), "back to the list: {}", app.focus());
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
    let page = app.front();
    assert!(page.iter().any(|l| l.contains("Day-end is now 7:30") && l.contains("live=polite")), "{page:#?}");
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
    assert!(dialog.iter().any(|l| l.contains("Window 'Delete Garden?'")), "{dialog:#?}");
    assert!(dialog.iter().any(|l| l.contains("Button 'Cancel'") && l.contains("FOCUSED")), "Cancel is the default: {dialog:#?}");
    app.post(&["esc"]);
    let projects = app.store().list_projects().unwrap();
    assert!(projects.rows.iter().any(|p| p.title == "Garden"), "nothing was deleted");
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
    let focus = app.focus();
    assert!(focus.starts_with("Edit 'Will add:'") && focus.contains("Call Sam") && focus.contains("priority 1"), "{focus}");
    app.post(&["enter"]);
    assert!(app.status().contains("Call Sam"), "{}", app.status());
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
fn a_task_is_put_in_a_block_from_this_weeks_work_blocks() {
    let app = App::launch(|lumenna| {
        add(lumenna, "Write report");
        let every_day = NewBlock {
            title: "Deep work".to_owned(),
            at: "9am".to_owned(),
            minutes: 120,
            date: None,
            kind: "work".to_owned(),
            repeat: Some("every day".to_owned()),
        };
        lumenna.add_block(every_day).unwrap();
        let lunch = NewBlock { title: "Lunch".to_owned(), kind: "break".to_owned(), at: "12pm".to_owned(), minutes: 45, date: None, repeat: Some("every day".to_owned()) };
        lumenna.add_block(lunch).unwrap();
    });
    app.post(&[GO_TASKS, "home", PUT_IN_BLOCK]);
    let picker = app.front();
    let choices: Vec<&String> = picker.iter().filter(|l| l.trim_start().starts_with("ListItem")).collect();
    assert_eq!(choices.len(), 7, "a work block a day for a week, and no breaks: {choices:#?}");
    assert!(choices[0].contains("Today") && choices[0].contains("Deep work"), "{}", choices[0]);
    app.post(&["enter", "text:30", "enter"]);
    assert!(app.status().starts_with("Scheduled Write report into Deep work"), "{}", app.status());
}
