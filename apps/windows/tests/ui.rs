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
const GO_TRASH: &str = "cmd:143";
const GO_HISTORY: &str = "cmd:146";
const PUT_IN_BLOCK: &str = "cmd:169";
const ERASE_PERMANENTLY: &str = "cmd:171";

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
fn the_window_and_the_taskbar_show_the_apps_own_icon_not_windows_default() {
    use windows::Win32::UI::WindowsAndMessaging::{GCLP_HICON, GCLP_HICONSM, GetClassLongPtrW, HICON};
    let app = App::launch(|_| {});
    // Its amber disc, above the check: Windows' own application icon has no amber.
    const AMBER: [u8; 3] = [0xFF, 0xC8, 0x3D];
    for (which, index) in [("small", GCLP_HICONSM), ("large", GCLP_HICON)] {
        let icon = HICON(unsafe { GetClassLongPtrW(app.window, index) } as _);
        assert!(!icon.is_invalid(), "the window has a {which} icon");
        let colour = colour_in(icon, (0.5, 0.28)).unwrap_or_else(|| panic!("the {which} icon is read"));
        let near = colour.iter().zip(AMBER).all(|(got, want)| got.abs_diff(want) <= 8);
        assert!(near, "the {which} icon is the app's: {colour:02X?}");
    }
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
    app.post(&["select:Write report, 0 of 1 subtask done", "left"]);
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
    for page in lumenna_records::fields::settings_pages(true, false).into_iter().map(|page| page.title) {
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

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// A Run key value of a test's own (`LUMENNA_RUN_NAME`), gone again however the test ends.
struct RunValue(String);

impl Drop for RunValue {
    fn drop(&mut self) {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteKeyValueW};
        use windows::core::HSTRING;
        unsafe {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(RUN_KEY), &HSTRING::from(&self.0));
        }
    }
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn opening_at_sign_in_is_the_cores_setting_and_toggling_it_writes_the_run_key_and_says_the_cores_sentence() {
    let run = RunValue(format!("Lumenna UI test {}", std::process::id()));
    let app = App::launch_with(&[("LUMENNA_RUN_NAME", &run.0)], |_| {});
    let words = lumenna_records::background::background_words(lumenna_records::background::BackgroundKind::WindowsSignIn);
    app.post(&[SETTINGS, "select:General"]);
    let named = automation::wait(Duration::from_secs(5), || {
        app.front()
            .into_iter()
            .find(|l| l.trim_start().starts_with(&format!("CheckBox '{}'", words.title)) && l.contains(&words.hint))
    });
    let named = named.unwrap_or_else(|| panic!("the box is the core's, its hint described: {:#?}", app.front()));
    assert!(named.contains(" unchecked"), "{named}");
    let said = |sentence: &str| {
        automation::wait(Duration::from_secs(5), || {
            app.front().iter().any(|l| l.contains(sentence) && l.contains("live=polite")).then_some(())
        })
        .is_some()
    };
    app.post(&[&format!("focus:{}", words.title), "space"]);
    assert!(said(&words.on), "{:#?}", app.front());
    let command = registry_string(RUN_KEY, &run.0).expect("the Run key is written");
    assert!(command.contains("--background") && command.contains("--profile"), "{command}");
    app.post(&["space"]);
    assert!(said(&words.off), "{:#?}", app.front());
    assert_eq!(registry_string(RUN_KEY, &run.0), None, "the Run key is removed");
    app.post(&["esc"]);
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
fn a_projects_colour_is_picked_from_the_palette_and_drawn_beside_its_name_never_in_it() {
    // Places and pixels both in physical pixels, whatever the display's scale.
    unsafe {
        windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    // Light, so the mark is the palette's light teal.
    let app = App::launch_with(&[("LUMENNA_DARK", "0")], |lumenna| {
        lumenna.add_project("Garden", None).unwrap();
    });
    // Today, Tasks, Projects, Inbox, Garden.
    app.post(&["f6", "home", "down", "down", "down", "down", "context"]);
    choose_from_menu(&app, "Colour...");
    let pick = automation::wait(Duration::from_secs(5), || {
        let front = app.front();
        front.iter().any(|l| l.contains("ListItem")).then_some(front)
    })
    .expect("the Colour pick opens");
    let choices: Vec<&String> = pick.iter().filter(|l| l.contains("ListItem")).collect();
    assert_eq!(choices.len(), 12, "No colour and the palette's eleven: {choices:#?}");
    assert!(choices[0].contains("No colour"), "No colour comes first: {choices:#?}");
    // Red, orange, yellow, green, teal: each choice's mark beside its name.
    app.post(&["down", "down", "down", "down", "down"]);
    let dialog = app.automation.front(app.window);
    let teal = app.automation.place_of(dialog, "ListItem 'teal").expect("teal is offered");
    assert!(marked(teal, 0, TEAL), "teal's choice has its mark");
    let red = app.automation.place_of(dialog, "ListItem 'red").expect("red is offered");
    assert!(!marked(red, 0, TEAL), "red's choice has no teal mark");
    app.post(&["enter"]);
    let coloured = automation::wait(Duration::from_secs(5), || {
        let projects = app.store().list_projects().unwrap();
        projects.rows.iter().find(|p| p.title == "Garden").and_then(|p| p.swatch.clone())
    });
    assert_eq!(coloured.map(|swatch| swatch.name).as_deref(), Some("teal"));
    // The mark is before the sidebar's row, and its name is the words alone.
    let garden = automation::wait(Duration::from_secs(5), || {
        let lines = app.automation.dump(app.window, 10);
        lines.iter().find(|l| l.contains("TreeItem 'Garden")).filter(|l| l.contains("teal")).cloned()
    })
    .expect("the sidebar says Garden's colour");
    assert!(garden.contains("TreeItem 'Garden, "), "{garden}");
    let row = app.automation.place_of(app.window, "TreeItem 'Garden").expect("Garden is in the sidebar");
    assert!(automation::wait(Duration::from_secs(5), || marked(row, 40, TEAL).then_some(())).is_some(), "Garden's row has its mark");
    let inbox = app.automation.place_of(app.window, "TreeItem 'Inbox").expect("the Inbox is in the sidebar");
    assert!(!marked(inbox, 40, TEAL), "the Inbox has no colour, and no mark");
}

/// The palette's teal on a light background.
const TEAL: [u8; 3] = [0x00, 0x79, 0x6B];

/// Whether `colour` is on the screen along the middle of `place`, or as far as `before` to its
/// left, where a tree draws an item's image beside the text it reports.
fn marked(place: windows::Win32::Foundation::RECT, before: i32, colour: [u8; 3]) -> bool {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
    };
    // A strip five pixels high, copied off the screen at once.
    let (left, top) = (place.left - before, (place.top + place.bottom) / 2 - 2);
    let (width, height) = (place.right - left, 5);
    let mut pixels = vec![0u8; (width * height * 4).max(0) as usize];
    unsafe {
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let strip = CreateCompatibleBitmap(screen, width, height);
        let old = SelectObject(memory, strip.into());
        let _ = BitBlt(memory, 0, 0, width, height, Some(screen), left, top, SRCCOPY);
        SelectObject(memory, old);
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        GetDIBits(memory, strip, 0, height as u32, Some(pixels.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS);
        let _ = DeleteObject(strip.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
    }
    pixels.chunks(4).any(|pixel| [pixel[2], pixel[1], pixel[0]].iter().zip(colour).all(|(got, want)| got.abs_diff(want) <= 12))
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
    let will_add = format!("Edit '{}:'", lumenna_records::fields::quick_add_words(false).will_add);
    let read_back = |focus: &str| focus.starts_with(&will_add) && focus.contains("Call Sam") && focus.contains("priority 1");
    assert!(app.lands(read_back), "{}", app.focus());
    app.post(&["enter"]);
    assert!(app.says(|s| s.contains("Call Sam")), "{}", app.status());
    let tasks = app.store().list_tasks("").unwrap();
    assert!(tasks.rows.iter().any(|t| t.title == "Call Sam"));
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_jump_lists_new_task_started_again_opens_quick_add_in_the_copy_already_running() {
    let app = App::launch(|_| {});
    // What a Jump List's task runs: the app again, which hands the way in to the copy running.
    let status = Command::new(env!("CARGO_BIN_EXE_lumenna"))
        .arg("--profile")
        .arg(app.profile.path())
        .arg("--no-shortcuts")
        .args(["--launch", "new-task"])
        .status()
        .expect("the second copy starts");
    assert!(status.success(), "the second copy hands over and leaves");
    assert!(app.lands(|focus| focus.starts_with("Edit 'Task:'")), "{}", app.focus());
    app.post(&["text:Call Sam", "enter"]);
    assert!(app.says(|s| s.contains("Call Sam")), "{}", app.status());
    assert!(app.store().list_tasks("").unwrap().rows.iter().any(|t| t.title == "Call Sam"));
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
fn a_saved_filters_query_and_a_blocks_tasks_from_complete_as_the_filter_does() {
    let app = App::launch(|lumenna| {
        lumenna.add_project("Work", None).unwrap();
        lumenna.add_filter("Urgent", "p1").unwrap();
    });
    // Today, Tasks, Projects, Inbox, Work, Labels, Saved Filters, Urgent.
    app.post(&["f6", "home", "down", "down", "down", "down", "down", "down", "down"]);
    assert!(app.focus().contains("TreeItem 'Urgent"), "{}", app.focus());
    app.post(&["context"]);
    choose_from_menu(&app, "Change Query...");
    let asked = automation::wait(Duration::from_secs(5), || {
        app.front().iter().any(|l| l.contains("Edit 'Query:'") && l.contains("FOCUSED")).then_some(())
    });
    assert!(asked.is_some(), "Change Query asks for the query: {:#?}", app.front());
    // Nothing opens by itself; Down offers what fits.
    app.post(&["end", "text: #Wo"]);
    assert!(!app.automation.menu_open());
    app.post(&["down"]);
    assert_eq!(app.automation.menu_items(), ["MenuItem 'Work, project' [1 of 1, level 0] key=w FOCUSED"]);
    app.post(&["enter"]);
    assert!(app.focus().contains("value='p1 #Work'"), "{}", app.focus());
    app.post(&["esc"]);

    app.post(&["cmd:101", "focus:Tasks from:", "text:#Wo", "down"]);
    assert_eq!(app.automation.menu_items(), ["MenuItem 'Work, project' [1 of 1, level 0] key=w FOCUSED"]);
    app.post(&["enter"]);
    assert!(app.focus().contains("value='#Work'"), "{}", app.focus());
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
    // order, as a keyboard reaches it, past the clock beside the start.
    app.post(&["cmd:101", "text:Commute", "tab", "text:8am", "tab", "tab", "tab", "down"]);
    let form = app.front();
    assert!(unnamed(&form).is_empty(), "{:#?}", unnamed(&form));
    let flag = |name: &str| form.iter().find(|l| l.contains(&format!("CheckBox '{name}'"))).cloned().unwrap_or_default();
    assert!(flag("Takes tasks").contains(" unchecked"), "a break takes no tasks: {form:#?}");
    assert!(flag("Counts toward hours for work").contains(" unchecked"), "{}", flag("Counts toward hours for work"));
    assert!(form.iter().any(|l| l.contains("ComboBox 'Colour")), "the colour is one of the palette's: {form:#?}");
    // Set apart from its kind — someone who works on the train — then the flags, the first
    // day and its calendar, and the right-hand column: repeats, last day, shortest length,
    // filter, and the colour, a drop-down of the palette's names that its letters choose from.
    app.post(&[
        "tab",
        "space",
        "tab",
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
fn a_new_blocks_start_and_day_have_the_systems_pickers_beside_them_which_fill_their_fields() {
    let app = App::launch(|_| {});
    app.post(&["cmd:101", "focus:Starts at:"]);
    let before = value_of(&app.focus());
    // The clock after the start, named for it and what it does, saying its own time.
    app.post(&["tab"]);
    let clock = app.focus();
    assert!(clock.starts_with("Pane 'Starts at, Choose a Time' msaa-value='"), "{clock}");
    // The calendar after the day, its button named too.
    let form = app.front();
    assert!(form.iter().any(|l| l.trim_start().starts_with("Pane 'Day, Choose a Day' msaa-value='")), "{form:#?}");
    assert!(form.iter().any(|l| l.trim_start().starts_with("Button 'Choose a Day'")), "{form:#?}");
    // Its hour turned on by one: the start is that time, as if typed.
    app.post(&["up"]);
    let wanted = an_hour_on(&before);
    let filled = automation::wait(Duration::from_secs(5), || {
        app.front().iter().any(|l| l.contains("Edit 'Starts at:'") && l.contains(&format!("value='{wanted}'"))).then_some(())
    });
    assert!(filled.is_some(), "{before} to {wanted}: {:#?}", app.front());
    app.post(&["esc"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_time_setting_has_a_clock_beside_it_applied_when_it_is_left() {
    let app = App::launch(|_| {});
    app.post(&[SETTINGS, "select:Planning", "focus:Day ends:"]);
    let before = value_of(&app.focus());
    app.post(&["tab"]);
    assert!(app.focus().starts_with("Pane 'Day ends, Choose a Time' msaa-value='"), "{}", app.focus());
    // Typed into the field as the arrows turn it, and applied, and said, once it is left.
    app.post(&["up", "tab"]);
    let wanted = an_hour_on(&before);
    let said = automation::wait(Duration::from_secs(5), || {
        app.front().iter().any(|l| l.contains("Day ends: ") && l.contains("live=polite")).then_some(())
    });
    assert!(said.is_some(), "{before} to {wanted}: {:#?}", app.front());
    app.post(&["esc"]);
    assert_eq!(app.store().settings(Some("day-end".to_owned())).unwrap().announcement, wanted);
}

/// A field's value, as UI Automation reads it.
fn value_of(element: &str) -> String {
    element.split("value='").nth(1).and_then(|v| v.split('\'').next()).unwrap_or_default().to_owned()
}

/// `HH:MM` an hour later, as a clock's arrow turns it.
fn an_hour_on(time: &str) -> String {
    let hour: u32 = time.get(..2).and_then(|h| h.parse().ok()).unwrap_or(0);
    format!("{:02}{}", (hour + 1) % 24, time.get(2..).unwrap_or(":00"))
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
    // Named as the core names a block's list.
    let title = lumenna_records::words::reminders_for("Deep work");
    assert!(dialog.iter().any(|l| l.contains(&format!("Window '{title}'"))), "{dialog:#?}");
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
    const ID: &str = "dev.emassey0135.Lumenna.UiTest";
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
    // It shows the app's icon, which the identity names as a file.
    let icon = registry_string(&format!("Software\\Classes\\AppUserModelId\\{ID}"), "IconUri");
    windows_registry_forget(ID);
    let xml = shown.expect("a toast was shown");
    assert!(xml.contains("Call the bank"), "{xml}");
    let icon = icon.expect("the identity names an icon");
    let image = std::fs::read(&icon).unwrap_or_else(|why| panic!("{icon}: {why}"));
    assert!(image.starts_with(b"\x89PNG"), "{icon} is a PNG");
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
    // Down skips separators, so it counts the items before it: from the one highlighted, which a
    // menu opened from the keyboard may start on, else from before the first.
    let from = items.iter().position(|i| i.ends_with(" FOCUSED") || i.contains(" FOCUSED "));
    let between = |start: usize| items[start..at].iter().filter(|i| !i.starts_with("Separator")).count();
    let downs = from.map_or_else(|| between(0) + 1, between);
    for _ in 0..downs {
        app.post(&["down"]);
    }
    app.post(&["enter"]);
    names
}

/// A text value of this user's registry.
fn registry_string(key: &str, name: &str) -> Option<String> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::HSTRING;
    let mut buffer = [0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;
    let (key, name) = (HSTRING::from(key), HSTRING::from(name));
    unsafe { RegGetValueW(HKEY_CURRENT_USER, &key, &name, RRF_RT_REG_SZ, None, Some(buffer.as_mut_ptr().cast()), Some(&mut size)) }
        .ok()
        .ok()?;
    let length = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buffer[..length]))
}

/// The colour at a point of an icon, drawn at its own size, `at` its share of the width and
/// height from the top left.
fn colour_in(icon: windows::Win32::UI::WindowsAndMessaging::HICON, at: (f64, f64)) -> Option<[u8; 3]> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};
    let mut info = ICONINFO::default();
    unsafe { GetIconInfo(icon, &mut info) }.ok()?;
    let mut bitmap = BITMAP::default();
    unsafe { GetObjectW(info.hbmColor.into(), size_of::<BITMAP>() as i32, Some((&raw mut bitmap).cast())) };
    let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (width * height * 4).max(0) as usize];
    let screen = unsafe { GetDC(None) };
    let rows = unsafe {
        GetDIBits(screen, info.hbmColor, 0, height as u32, Some(pixels.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS)
    };
    unsafe {
        ReleaseDC(None, screen);
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
    }
    if rows == 0 {
        return None;
    }
    let (x, y) = ((f64::from(width) * at.0) as usize, (f64::from(height) * at.1) as usize);
    let pixel = &pixels[(y * width as usize + x) * 4..][..3];
    Some([pixel[2], pixel[1], pixel[0]])
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
    app.post(&[SETTINGS, "select:Devices and Sync", "invoke:Pair a Device..."]);
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
    for page in lumenna_records::fields::settings_pages(true, false).into_iter().map(|page| page.title) {
        app.post(&[&format!("select:{page}")]);
        fits_and_nothing_overlaps(&app, &format!("Settings, {page}"));
    }
    app.post(&["select:Devices and Sync", "invoke:Pair a Device..."]);
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
fn history_lists_what_was_done_with_no_filter_and_its_rows_are_tasks() {
    let app = App::launch(|lumenna| {
        add(lumenna, "Call the bank");
        add(lumenna, "Water plants");
        let rows = lumenna.list_tasks("").unwrap().rows;
        let bank = rows.iter().find(|r| r.title == "Call the bank").unwrap().id.clone();
        lumenna.complete_task(&bank).unwrap();
    });
    app.post(&[GO_HISTORY]);
    let lines = automation::wait(Duration::from_secs(5), || {
        let lines = app.automation.dump(app.window, 10);
        lines.iter().any(|l| l.trim_start().starts_with("Tree 'History'")).then_some(lines)
    })
    .expect("History is shown");
    let rows: Vec<&String> = lines
        .iter()
        .skip_while(|l| !l.trim_start().starts_with("Tree 'History'"))
        .skip(1)
        .take_while(|l| l.contains("TreeItem"))
        .collect();
    assert_eq!(rows.len(), 1, "only what was done: {rows:#?}");
    assert!(rows[0].contains("Call the bank") && rows[0].contains("completed today"), "{}", rows[0]);
    assert!(!lines.iter().any(|l| l.contains("Edit 'Filter")), "History has no filter: {lines:#?}");
    // Its row is a task's, with a task's actions.
    app.post(&["focus:History", "home", "context"]);
    let menu: Vec<String> = app.automation.menu_items().iter().map(|i| i.split('\'').nth(1).unwrap_or_default().to_owned()).collect();
    app.post(&["esc"]);
    assert!(menu.iter().any(|item| item == "Mark Not Done"), "{menu:?}");
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
        [
            "Mark Done",
            "Edit Details",
            "Put in a Block...",
            "Move to Project...",
            "Make Subtask Of...",
            "Wait For...",
            "Reminders...",
            "Move to Trash",
            "Collapse"
        ],
        "the task's actions, as the core gives them, and the app's folding of what is under it"
    );
    // Its own subtask is not somewhere it can go.
    app.post(&["cmd:164"]);
    assert!(app.says(|s| s.contains("no task it could go under")), "{}", app.status());
    // No pick opens at all: the main window stays in front, whose own lists (what the task waits
    // for) are not a pick's.
    let front = app.front();
    assert!(!front.iter().any(|l| l.contains("Window 'Make Subtask Of")), "Write report was offered somewhere to go under: {front:#?}");

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


/// Trashes the task called `title`.
fn trash(lumenna: &Lumenna, title: &str) {
    add(lumenna, title);
    let rows = lumenna.list_tasks("").unwrap().rows;
    let id = rows.iter().find(|r| r.title == title).unwrap().id.clone();
    lumenna.trash_task(&id).unwrap();
}

/// The titles of what is in the trash, as another process sees it.
fn in_the_trash(app: &App) -> Vec<String> {
    app.store().list_tasks("deleted").unwrap().rows.into_iter().map(|r| r.title).collect()
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_trashed_tasks_menu_offers_erase_permanently_after_delete_from_trash_and_its_confirmation_erases_it() {
    let app = App::launch(|lumenna| trash(lumenna, "Old secret"));
    app.post(&[GO_TRASH, "home", "context"]);
    let menu: Vec<String> = app
        .automation
        .menu_items()
        .iter()
        .filter(|item| !item.starts_with("Separator"))
        .map(|item| item.split('\'').nth(1).unwrap_or_default().to_owned())
        .collect();
    app.post(&["esc"]);
    assert_eq!(menu, ["Restore", "Delete from Trash...", "Erase Permanently..."], "the core's, in its order");

    // Delete is still Delete from Trash, never Erase Permanently.
    app.post(&["delete"]);
    let asked = app.front();
    assert!(asked.iter().any(|l| l.contains("Delete Old secret from the trash?")), "{asked:#?}");
    assert!(!asked.iter().any(|l| l.contains("rebuilds your database")), "{asked:#?}");
    app.post(&["esc"]);

    // Chosen from the row's menu, it asks first, Cancel the default, and cancelling erases nothing.
    app.post(&["context"]);
    choose_from_menu(&app, "Erase Permanently...");
    let asked = automation::wait(Duration::from_secs(5), || {
        let front = app.front();
        front.iter().any(|l| l.contains("Erase Old secret permanently?")).then_some(front)
    })
    .unwrap_or_else(|| panic!("it asks: {:#?}", app.front()));
    assert!(asked.iter().any(|l| l.contains("rebuilds your database")), "{asked:#?}");
    assert!(asked.iter().any(|l| l.contains("Button 'Cancel'") && l.contains("FOCUSED")), "Cancel is the default: {asked:#?}");
    app.post(&["esc"]);
    assert_eq!(in_the_trash(&app), ["Old secret"], "nothing was erased");

    // The Task menu's, answered, erases it.
    app.post(&[ERASE_PERMANENTLY]);
    automation::wait(Duration::from_secs(5), || app.front().iter().any(|l| l.contains("Erase Old secret permanently?")).then_some(()))
        .unwrap_or_else(|| panic!("the Task menu's asks too: {:#?}", app.front()));
    app.post(&["invoke:Erase Permanently"]);
    assert!(app.says(|s| s.contains("Erased Old secret permanently")), "{}", app.status());
    assert!(in_the_trash(&app).is_empty(), "{:?}", in_the_trash(&app));
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn space_on_a_trashed_task_restores_it() {
    let app = App::launch(|lumenna| trash(lumenna, "Old receipt"));
    app.post(&[GO_TRASH, "home", "space"]);
    let restored = automation::wait(Duration::from_secs(5), || in_the_trash(&app).is_empty().then_some(()));
    assert!(restored.is_some(), "{:?}", in_the_trash(&app));
    let open: Vec<String> = app.store().list_tasks("").unwrap().rows.into_iter().map(|r| r.title).collect();
    assert_eq!(open, ["Old receipt"]);
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn settings_offers_erase_everything_deleted_and_its_confirmation_empties_the_trash() {
    let app = App::launch(|lumenna| {
        trash(lumenna, "Old secret");
        trash(lumenna, "Old draft");
        add(lumenna, "Water plants");
    });
    app.post(&[SETTINGS, "select:Export and Import"]);
    let page = app.front();
    let at = |name: &str| page.iter().position(|l| l.trim_start().starts_with(&format!("Button '{name}")));
    let erase = at("Erase Everything Deleted...").unwrap_or_else(|| panic!("a button of its own: {page:#?}"));
    assert!(at("Import or Restore...").is_some_and(|import| import < erase), "after the rest: {page:#?}");
    // The core's caption, under the button and said with it.
    let caption = lumenna_records::actions::STORE_ACTIONS;
    assert!(page[erase].contains(&format!("msaa-description='{caption}'")), "{}", page[erase]);
    assert!(page[erase + 1].trim_start().starts_with(&format!("Text '{caption}'")), "under it: {page:#?}");

    app.post(&["invoke:Erase Everything Deleted..."]);
    automation::wait(Duration::from_secs(5), || {
        app.front().iter().any(|l| l.contains("Erase everything deleted permanently?")).then_some(())
    })
    .unwrap_or_else(|| panic!("it asks: {:#?}", app.front()));
    app.post(&["invoke:Erase Everything Deleted"]);
    // Said on the page's own status line, the sheet being in front.
    let said = automation::wait(Duration::from_secs(10), || {
        app.front()
            .iter()
            .any(|l| l.contains("Erased 2 tasks and the history of everything deleted") && l.contains("live=polite"))
            .then_some(())
    });
    assert!(said.is_some(), "{:#?}", app.front());
    app.post(&["esc"]);
    assert!(in_the_trash(&app).is_empty(), "{:?}", in_the_trash(&app));
    let left: Vec<String> = app.store().list_tasks("").unwrap().rows.into_iter().map(|r| r.title).collect();
    assert_eq!(left, ["Water plants"]);
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
    // Alt+Enter, as the menu command it stands for: the selected task's details, the form the
    // app hands back once it has run the row's Edit.
    app.post(&[GO_TASKS, "home", "cmd:113"]);
    assert!(app.lands(|focus| focus.starts_with("Edit 'Title:'")), "{}", app.focus());
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
fn go_to_day_has_a_calendar_beside_its_line_open_on_the_day_shown_which_fills_the_line() {
    let app = App::launch(|_| {});
    let tomorrow = jiff::Zoned::now().date().tomorrow().unwrap();
    app.post(&[NEXT_DAY]);
    let shown = automation::wait(Duration::from_secs(5), || Some(app.status()).filter(|s| s.starts_with("Tomorrow.")));
    assert!(shown.is_some(), "{}", app.status());
    app.post(&["cmd:183"]);
    let asked = automation::wait(Duration::from_secs(5), || app.focus().starts_with("Edit 'Day:'").then_some(()));
    assert!(asked.is_some(), "the line comes first: {}", app.focus());
    // The calendar after it, named for the line and what it does, on the day shown.
    app.post(&["tab"]);
    let calendar = app.focus();
    assert!(calendar.starts_with("Pane 'Day, Choose a Day' msaa-value='"), "{calendar}");
    assert!(app.front().iter().any(|l| l.trim_start().starts_with("Button 'Choose a Day'")), "{:#?}", app.front());
    // Turned on, it puts its day in the line as the core reads one, and Go goes there.
    app.post(&["up"]);
    let line = app.front().into_iter().find(|l| l.contains("Edit 'Day:'")).unwrap_or_default();
    let day: jiff::civil::Date = value_of(&line).parse().unwrap_or_else(|_| panic!("an ISO day: {line}"));
    assert!(day > tomorrow, "{day} after {tomorrow}");
    app.post(&["enter"]);
    let gone = automation::wait(Duration::from_secs(5), || {
        (!app.status().starts_with("Tomorrow.") && app.focus().contains("TreeItem")).then_some(())
    });
    assert!(gone.is_some(), "{} {}", app.status(), app.focus());
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
    assert!(app.says(|s| s == "That does not apply here."), "{}", app.status());
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
    // What the details list besides the form, under the core's heading, its history among them.
    let about = format!("Edit '{}:'", lumenna_records::fields::details_words(false).about);
    let history = window.iter().find(|l| l.trim_start().starts_with(&about)).unwrap_or_else(|| panic!("{window:#?}"));
    assert!(history.contains("1 sitting"), "{history}");
    assert!(app.focus().contains("Write report") && !app.focus().contains("sitting"), "{}", app.focus());
}

#[test]
#[ignore = "opens a window: cargo test -p lumenna-windows --test ui -- --ignored"]
fn a_block_that_ended_with_work_short_offers_carry_forward_on_its_notification() {
    use windows::UI::Notifications::ToastNotificationManager;
    use windows::core::HSTRING;

    const ID: &str = "dev.emassey0135.Lumenna.UiTest";
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

    const ID: &str = "dev.emassey0135.Lumenna.UiTest";
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
