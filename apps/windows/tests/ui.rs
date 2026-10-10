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
        Self::launch_with(&[], seed)
    }

    /// Starts the app as `launch` does, with `env` set as well.
    fn launch_with(env: &[(&str, &str)], seed: impl FnOnce(&Lumenna)) -> Self {
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
            ..NewBlock::default()
        };
        lumenna.add_block(every_day).unwrap();
        let lunch = NewBlock { title: "Lunch".to_owned(), kind: "break".to_owned(), at: "12pm".to_owned(), minutes: 45, date: None, repeat: Some("every day".to_owned()), ..NewBlock::default() };
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
        lumenna_desktop::speech::sitting(&plan.blocks[0].assignments[0])
    };
    app.post(&[&format!("select:{}", row(&app.store())), "space"]);
    assert!(app.status().starts_with("Started timer"), "{}", app.status());
    app.post(&["space"]);
    assert!(app.status().to_lowercase().contains("paused"), "{}", app.status());
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
    assert!(app.status().starts_with("Added"), "{}", app.status());
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
fn the_pairing_dialog_says_an_empty_code_takes_the_clipboard() {
    let app = App::launch(|_| {});
    app.post(&[SETTINGS, "select:Devices", "invoke:Pair a Device..."]);
    let dialog = app.front();
    assert!(dialog.iter().any(|l| l.contains("Window 'Pair a Device'")), "{dialog:#?}");
    assert!(dialog.iter().any(|l| l.contains("Left empty, the code on the clipboard is used.")), "{dialog:#?}");
    assert!(unnamed(&dialog).is_empty(), "{:#?}", unnamed(&dialog));
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
fn lum_rpc_reaches_the_running_app_through_its_pipe_and_the_app_shows_what_it_added() {
    use std::io::{BufRead, BufReader, Write};

    let app = App::launch(|_| {});
    let started = std::time::Instant::now();
    // The app serves once its sync service holds the endpoint; until then lum serves itself.
    let (mut rpc, mut replies, answer) = loop {
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
        if answer.contains(r#""process":"Lumenna for Windows""#) || started.elapsed() > Duration::from_secs(20) {
            break (rpc, replies, answer);
        }
        let _ = rpc.kill();
        let _ = rpc.wait();
        std::thread::sleep(Duration::from_millis(500));
    };
    assert!(answer.contains(r#""process":"Lumenna for Windows""#), "the app answers, not lum: {answer}");
    writeln!(rpc.stdin.as_mut().unwrap(), r#"{{"jsonrpc":"2.0","id":2,"method":"task.add","params":{{"text":"Sent through the pipe"}}}}"#).unwrap();
    let mut added = String::new();
    replies.read_line(&mut added).unwrap();
    assert!(added.contains("Sent through the pipe"), "{added}");
    // A client whose input ends is answered, then lum rpc ends with it: the relay tells the
    // app the client has gone, as a pipe has no half-close to say it.
    drop(rpc.stdin.take());
    let (ended, exited) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = ended.send(rpc.wait());
    });
    assert!(exited.recv_timeout(Duration::from_secs(10)).is_ok(), "lum rpc ends when its input does");
    // Another connection's write, which the app's poll redraws for within a second or so.
    app.post(&[GO_TASKS]);
    automation::wait(Duration::from_secs(5), || {
        app.automation.dump(app.window, 6).iter().any(|l| l.contains("TreeItem 'Sent through the pipe")).then_some(())
    })
    .expect("the app lists the task lum added through it");
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn lum_sync_runs_its_round_on_the_running_app() {
    let app = App::launch(|_| {});
    // The app answers at its pipe only while it holds the endpoint, and then lum cannot run a
    // round of its own: a round lum reports is the app's.
    automation::wait(Duration::from_secs(20), || lumenna_surface::endpoint::connect(app.profile.path()).map(drop))
        .expect("the app serves its pipe");
    let started = std::time::Instant::now();
    loop {
        let output = Command::new(lum())
            .arg("sync")
            .env("LUMENNA_PROFILE", app.profile.path())
            .env("LUMENNA_BACKUP_DIR", app.profile.path().with_extension("backups"))
            .output()
            .expect("lum runs");
        let said = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        // Not paired with anything, so the app's round reaches nobody, and says so.
        if output.status.success() && said.contains("not paired") {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "lum sync did not get a round from the app: {said}");
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Checks the window in front fits its screen's work area, and that nothing directly in it
/// lies outside it or over anything else: what Text size at its largest would break.
fn fits_and_nothing_overlaps(app: &App, what: &str) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};

    let front = app.automation.front(app.window);
    let (window, items) = app.automation.boxes(front);
    let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let _ = GetMonitorInfoW(MonitorFromWindow(front, MONITOR_DEFAULTTONEAREST), &mut monitor);
    }
    let work = monitor.rcWork;
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
        let block = NewBlock { title: "Deep work".to_owned(), at: "11pm".to_owned(), minutes: 30, kind: "work".to_owned(), ..NewBlock::default() };
        lumenna.add_block(block).unwrap();
    });
    app.post(&[NEW_TASK]);
    fits_and_nothing_overlaps(&app, "New Task");
    app.post(&["esc", "cmd:101"]);
    fits_and_nothing_overlaps(&app, "New Block");
    app.post(&["esc", GO_TASKS, "home", PUT_IN_BLOCK]);
    fits_and_nothing_overlaps(&app, "Put in a Block");
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
        lumenna.move_task(&outline, lumenna_surface::MoveTarget::Parent { id: report }).unwrap();
    });
    let names = |items: Vec<String>| -> Vec<String> {
        items.iter().map(|item| item.split('\'').nth(1).unwrap_or_default().to_owned()).collect()
    };
    app.post(&[GO_TASKS, "home", "context"]);
    let menu = names(app.automation.menu_items());
    app.post(&["esc"]);
    assert_eq!(
        menu,
        ["Mark Done", "Edit Details", "Put in a Block...", "Move to Project...", "Make Subtask Of...", "Wait For...", "Move to Trash"],
        "the task's actions, as the core gives them"
    );
    // Its own subtask is not somewhere it can go.
    app.post(&["cmd:164"]);
    let offered: Vec<String> = app.front().into_iter().filter(|l| l.contains("ListItem")).collect();
    assert!(offered.is_empty(), "Write report was offered as going under {offered:#?}");
    assert!(app.status().contains("no task it could go under"), "{}", app.status());

    // The Inbox keeps its name and place: only reordered and weighed.
    app.post(&["cmd:145", "select:Inbox, 2 open tasks", "context"]);
    let menu = names(app.automation.menu_items());
    app.post(&["esc"]);
    assert_eq!(menu, ["Weight..."], "the Inbox's menu");
    // Delete there says why not, in the core's words.
    app.post(&["delete"]);
    let why = lumenna_surface::not_offered(lumenna_surface::ActionKind::Delete, lumenna_surface::Subject::Project, false);
    assert_eq!(app.status(), lumenna_desktop::speech::sentence(&why));
    // F2 is Rename, as in Explorer; the Inbox keeps its name.
    app.post(&["f2"]);
    let why = lumenna_surface::not_offered(lumenna_surface::ActionKind::Rename, lumenna_surface::Subject::Project, false);
    assert_eq!(app.status(), lumenna_desktop::speech::sentence(&why));
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
