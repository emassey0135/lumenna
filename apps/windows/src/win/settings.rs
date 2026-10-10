//! Settings, in Windows' own shape: a property sheet of pages, opened with
//! Ctrl+Comma — General, Planning, Devices, Backups, and Export and Import, as on the Mac.
//!
//! Like the Mac's, every page applies a change as it is made; a text field, when it is left.
//! Each page has its own status line, a polite live region, for what a change did: the main
//! window's is behind the sheet.

use std::cell::RefCell;

use lumenna_surface::{ActionKind, ExportFormat, Imported, Setting, Subject, not_offered};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Controls::{
    HKM_GETHOTKEY, HKM_SETHOTKEY, HOTKEYF_ALT, HOTKEYF_CONTROL, HOTKEYF_SHIFT, NMHDR, PSN_APPLY, PSN_KILLACTIVE,
    PSN_RESET,
};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_AUTOCHECKBOX, BS_GROUPBOX, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL,
    CB_RESETCONTENT, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, EN_KILLFOCUS, ES_AUTOHSCROLL, ES_MULTILINE,
    ES_NUMBER, ES_READONLY, GetParent, IDCANCEL, IDOK, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL,
    LBN_SELCHANGE, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LBS_WANTKEYBOARDINPUT, PostMessageW, WM_DESTROY, WM_NOTIFY, WM_VKEYTOITEM, WS_BORDER, WS_TABSTOP,
    WS_VSCROLL,
};
use windows::core::HSTRING;

use super::app::App;
use super::core::{Poster, WM_SAY, WM_STORE_CHANGED, sentence, said};
use super::dialog::{self, Class, Dialog, Template};
use super::{a11y, actions, controls, pairing, prompts, shortcuts, system};
use crate::devices;
use crate::shortcut::{Kind, Shortcut};
use crate::speech;

/// Every page's status line.
const STATUS: u16 = 199;
const WIDTH: i16 = 252;
const HEIGHT: i16 = 230;

/// The pages, in order. Only some are opened directly, from the menu bar; the rest are named
/// so the order is written down once.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum Page {
    General = 0,
    Planning = 1,
    Devices = 2,
    Backups = 3,
    Export = 4,
}

/// Opens Settings on a page.
pub fn show(app: &App, page: Page) {
    let general = General { app };
    let planning = Planning { app, known: RefCell::new(Vec::new()) };
    let devices = Devices { app, list: RefCell::new(Vec::new()) };
    let backups = Backups { app, known: RefCell::new(Vec::new()) };
    let export = Export { app };
    let pages: [&dyn Dialog; 5] = [&general, &planning, &devices, &backups, &export];
    dialog::sheet(app.main, "Settings", &pages, page as usize);
}

/// Reads a file in — a JSON export or a backup, the core tells which — and says what it did.
/// From the menu bar, where the main window says it.
pub fn import(app: &App, owner: HWND, title: &str, types: &[(&str, &str)]) -> Option<String> {
    let path = system::open_file(owner, title, types)?;
    match app.core.lumenna.import(&path.display().to_string()) {
        Ok(imported) => {
            app.store_changed();
            Some(match imported {
                Imported::Export { done } => speech::announcement(&done.announcement, &done.notices),
                Imported::Backup { done } => speech::announcement(&done.announcement, &done.notices),
            })
        }
        Err(error) => {
            prompts::fail(owner, &sentence(&error));
            None
        }
    }
}

/// What a backup file is called in a file dialog.
pub const BACKUPS: [(&str, &str); 2] = [("Lumenna backups", "*.lumbak"), ("All files", "*.*")];
/// What can be read in.
pub const IMPORTABLE: [(&str, &str); 2] = [("Lumenna exports and backups", "*.json;*.lumbak"), ("All files", "*.*")];

// ---------------------------------------------------------------------------------------
// What every page shares
// ---------------------------------------------------------------------------------------

/// A page, its status line first: placed at the bottom, but made before anything else, since
/// Windows names an empty static after the label made just before it — and a status line
/// named after the page's footer would read that footer out twice.
fn page(title: &str) -> Template {
    Template::page(title, WIDTH, HEIGHT).item(Class::Static, "", STATUS, SS_NOPREFIX.0, 7, HEIGHT - 30, WIDTH - 14, 24)
}

/// Says something on a page's status line.
fn say(page: HWND, text: &str) {
    let status = dialog::item(page, STATUS);
    controls::set_text(status, &speech::sentence(text));
    a11y::changed(status);
}

/// The sheet a page is in, which owns the dialogs a page opens.
fn sheet(page: HWND) -> HWND {
    unsafe { GetParent(page).unwrap_or(page) }
}

/// Every setting, as the core describes it: its name, its value and what it can be.
fn values(app: &App) -> Vec<Setting> {
    app.core.lumenna.settings(None).map(|s| s.settings).unwrap_or_default()
}

fn value(known: &[Setting], key: &str) -> String {
    known.iter().find(|s| s.key == key).map(|s| s.value.clone()).unwrap_or_default()
}

/// A setting's name as a control's label: the core's title, with `letter` as its access key
/// and, before a field, a colon.
fn titled(known: &[Setting], key: &str, letter: char, colon: bool) -> String {
    let title = known.iter().find(|s| s.key == key).map_or(key, |s| s.title.as_str());
    format!("{}{}", devices::marked(title, letter, '&'), if colon { ":" } else { "" })
}

/// Fills a drop-down list with a setting's options, the value selected. A value set
/// elsewhere that is not among them is listed as itself, since it is still the value.
fn fill_options(combo: HWND, known: &[Setting], key: &str) {
    let Some(setting) = known.iter().find(|s| s.key == key) else { return };
    let mut names: Vec<&str> = setting.options.iter().map(|o| o.title.as_str()).collect();
    let mut position = setting.options.iter().position(|o| o.id == setting.value);
    if position.is_none() && !setting.value.is_empty() {
        names.push(&setting.value);
        position = Some(names.len() - 1);
    }
    fill_combo(combo, &names, position);
}

/// The value chosen in a setting's drop-down list, if it is one of its options.
fn chosen_option(combo: HWND, known: &[Setting], key: &str) -> Option<String> {
    let setting = known.iter().find(|s| s.key == key)?;
    setting.options.get(selected(combo)?).map(|o| o.id.clone())
}

/// Changes a setting and says so; on a failure says why and returns false.
fn set(app: &App, page: HWND, key: &str, to: &str) -> bool {
    match app.core.lumenna.set_setting(key, to) {
        Ok(change) => {
            say(page, &speech::announcement(&change.announcement, &change.notices));
            app.store_changed();
            true
        }
        Err(error) => {
            prompts::fail(sheet(page), &sentence(&error));
            false
        }
    }
}

fn fill_combo(combo: HWND, items: &[&str], selected: Option<usize>) {
    controls::send(combo, CB_RESETCONTENT, 0, 0);
    for item in items {
        let text = HSTRING::from(*item);
        controls::send(combo, CB_ADDSTRING, 0, text.as_ptr() as isize);
    }
    controls::send(combo, CB_SETCURSEL, selected.unwrap_or(usize::MAX), 0);
}

fn selected(combo: HWND) -> Option<usize> {
    usize::try_from(controls::send(combo, CB_GETCURSEL, 0, 0)).ok()
}

/// Whether a page notification asks the page to finish what is being typed: leaving the page,
/// or closing the sheet either way.
fn finishing(message: u32, lparam: LPARAM) -> bool {
    message == WM_NOTIFY && {
        let code = unsafe { &*(lparam.0 as *const NMHDR) }.code;
        code == PSN_KILLACTIVE || code == PSN_APPLY || code == PSN_RESET
    }
}

const BUTTON: u32 = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
const FIELD: u32 = ES_AUTOHSCROLL as u32 | WS_BORDER.0 | WS_TABSTOP.0;
const READ_ONLY: u32 = ES_AUTOHSCROLL as u32 | ES_READONLY as u32 | WS_BORDER.0 | WS_TABSTOP.0;
const LIST: u32 = CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0;

// ---------------------------------------------------------------------------------------
// General: opening at sign-in, and the shortcuts from anywhere
// ---------------------------------------------------------------------------------------

const AT_SIGN_IN: u16 = 100;
const SHORTCUT: u16 = 110;
const CHANGE: u16 = 120;
const TOGGLE: u16 = 130;

struct General<'a> {
    app: &'a App,
}

impl General<'_> {
    fn show_shortcuts(&self, page: HWND) {
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            let index = index as u16;
            controls::set_text(dialog::item(page, SHORTCUT + index), &shortcuts::describe(kind));
            let on = shortcuts::current(kind).is_some();
            let toggle = dialog::item(page, TOGGLE + index);
            controls::set_text(toggle, if on { "Turn Off" } else { "Turn On" });
            let verb = if on { "Turn off" } else { "Turn on" };
            a11y::set_name(toggle, &format!("{verb} the shortcut for {}", kind.name()));
        }
    }

    fn change(&self, page: HWND, kind: Kind) {
        // While the keys are chosen, pressing the current ones must reach the field, not
        // summon the window.
        shortcuts::unregister_all(self.app.main);
        let recorder = Recorder { kind, chosen: RefCell::new(None) };
        dialog::run(Some(sheet(page)), &recorder);
        let chosen = recorder.chosen.into_inner();
        shortcuts::register_all(self.app.main);
        if let Some(shortcut) = chosen {
            match shortcuts::change(self.app.main, kind, Some(shortcut)) {
                Ok(said) => say(page, &said),
                Err(why) => prompts::fail(sheet(page), &why),
            }
            self.show_shortcuts(page);
        }
    }

    fn toggle(&self, page: HWND, kind: Kind) {
        let to = if shortcuts::current(kind).is_some() { None } else { Some(kind.standard()) };
        match shortcuts::change(self.app.main, kind, to) {
            Ok(said) => say(page, &said),
            Err(why) => prompts::fail(sheet(page), &why),
        }
        self.show_shortcuts(page);
    }
}

impl Dialog for General<'_> {
    fn template(&self) -> Template {
        let mut template = page("General")
            .item(Class::Button, "&Open Lumenna when you sign in to Windows", AT_SIGN_IN, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, 7, 7, 238, 10)
            .item(
                Class::Static,
                "Lumenna stays running in the notification area when its window is closed, so your devices stay in sync. Exit it from the File menu or the notification area.",
                u16::MAX,
                SS_NOPREFIX.0,
                7,
                20,
                238,
                26,
            )
            .item(Class::Button, "Shortcuts from anywhere", u16::MAX, BS_GROUPBOX as u32, 7, 50, 238, 92);
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            let y = 62 + index as i16 * 38;
            let index = index as u16;
            template = template
                .item(Class::Static, &format!("{}:", kind.name()), u16::MAX, SS_NOPREFIX.0, 14, y, 224, 9)
                .item(Class::Edit, "", SHORTCUT + index, READ_ONLY, 14, y + 10, 110, 14)
                .item(Class::Button, "Change...", CHANGE + index, BUTTON, 128, y + 10, 52, 14)
                .item(Class::Button, "Turn Off", TOGGLE + index, BUTTON, 184, y + 10, 54, 14);
        }
        let footer = "These work in any program, so they take their keys from whatever is in front. Control+Alt+Shift, because Control+Alt alone is AltGr on many keyboards, where it types letters.";
        template.item(Class::Static, footer, u16::MAX, SS_NOPREFIX.0, 7, 148, 238, 36)
    }

    fn init(&self, page: HWND) -> bool {
        a11y::make_live(dialog::item(page, STATUS));
        controls::check(dialog::item(page, AT_SIGN_IN), sign_in::on());
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            a11y::set_name(dialog::item(page, CHANGE + index as u16), &format!("Change the shortcut for {}", kind.name()));
        }
        self.show_shortcuts(page);
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        if u32::from(code) != BN_CLICKED {
            return None;
        }
        match id {
            AT_SIGN_IN => {
                let on = controls::checked(dialog::item(page, AT_SIGN_IN));
                if sign_in::set(on, self.app.core.lumenna.path()) {
                    say(page, if on { "Lumenna opens when you sign in" } else { "Lumenna no longer opens when you sign in" });
                } else {
                    prompts::fail(sheet(page), "Windows would not change what starts when you sign in.");
                    controls::check(dialog::item(page, AT_SIGN_IN), sign_in::on());
                }
            }
            id if (CHANGE..CHANGE + 2).contains(&id) => self.change(page, Kind::ALL[usize::from(id - CHANGE)]),
            id if (TOGGLE..TOGGLE + 2).contains(&id) => self.toggle(page, Kind::ALL[usize::from(id - TOGGLE)]),
            _ => {}
        }
        None
    }
}

/// Choosing new keys for a shortcut, with Windows' own hotkey control.
struct Recorder {
    kind: Kind,
    chosen: RefCell<Option<Shortcut>>,
}

const KEYS: u16 = 100;

impl Dialog for Recorder {
    fn template(&self) -> Template {
        let message = format!(
            "Press the new keys for {}: Control, Alt or both, with Shift, and a letter, digit or function key. Backspace clears them.",
            self.kind.name()
        );
        Template::new("Change Shortcut", 240, 92)
            .item(Class::Static, &message, u16::MAX, SS_NOPREFIX.0, 7, 7, 226, 28)
            .item(Class::Static, "&Keys:", u16::MAX, 0, 7, 38, 226, 9)
            .named("msctls_hotkey32", "", KEYS, WS_BORDER.0 | WS_TABSTOP.0, 7, 48, 226, 14)
            .item(Class::Button, "OK", IDOK.0 as u16, windows::Win32::UI::WindowsAndMessaging::BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0, 129, 71, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, BUTTON, 183, 71, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        if let Some(current) = shortcuts::current(self.kind) {
            let mut flags = 0;
            if current.shift {
                flags |= HOTKEYF_SHIFT;
            }
            if current.control {
                flags |= HOTKEYF_CONTROL;
            }
            if current.alt {
                flags |= HOTKEYF_ALT;
            }
            let value = (current.key as usize & 0xFF) | ((flags as usize) << 8);
            controls::send(dialog::item(hwnd, KEYS), HKM_SETHOTKEY, value, 0);
        }
        false
    }

    fn command(&self, hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => {
                let value = controls::send(dialog::item(hwnd, KEYS), HKM_GETHOTKEY, 0, 0) as u32;
                let (key, flags) = (value & 0xFF, (value >> 8) & 0xFF);
                if key == 0 {
                    prompts::fail(hwnd, "Press the keys first, or choose Cancel.");
                    return None;
                }
                let shortcut = Shortcut {
                    control: flags & HOTKEYF_CONTROL != 0,
                    alt: flags & HOTKEYF_ALT != 0,
                    shift: flags & HOTKEYF_SHIFT != 0,
                    windows: false,
                    key: key as u16,
                };
                if let Some(warning) = shortcut.warning() {
                    let title = format!("Use {}?", shortcut.describe());
                    if prompts::choose(hwnd, &title, warning, &["Use These Keys Anyway"], true) != Some(0) {
                        return None;
                    }
                }
                *self.chosen.borrow_mut() = Some(shortcut);
                Some(1)
            }
            id if id == IDCANCEL.0 => Some(0),
            _ => None,
        }
    }
}

/// Opening at sign-in: the current user's Run key, as every Windows program does it. Started
/// that way, the app opens in the notification area without its window (`--background`).
mod sign_in {
    use super::system;

    const NAME: &str = "Lumenna";

    pub fn on() -> bool {
        system::read_string(system::RUN_KEY, NAME).is_some()
    }

    /// Turns it on or off for the profile at `profile`.
    pub fn set(on: bool, profile: &std::path::Path) -> bool {
        if !on {
            system::delete_value(system::RUN_KEY, NAME);
            return !self::on();
        }
        let Ok(exe) = std::env::current_exe() else { return false };
        let mut command = format!("\"{}\" --background", exe.display());
        // A profile other than the usual one is the one opened at sign-in, too.
        if crate::profile::default_directory().as_deref() != Some(profile) {
            command.push_str(&format!(" --profile \"{}\"", profile.display()));
        }
        system::write_string(system::RUN_KEY, NAME, &command)
    }
}

// ---------------------------------------------------------------------------------------
// Planning: what syncs to every device
// ---------------------------------------------------------------------------------------

const CASCADE: u16 = 200;
const DAY_START: u16 = 201;
const DAY_END: u16 = 202;
const ALL_DAY: u16 = 203;
const VERBOSITY: u16 = 204;
const WEEK_START: u16 = 205;

const TIMES: [(u16, &str); 3] = [(DAY_START, "day-start"), (DAY_END, "day-end"), (ALL_DAY, "all-day-reminder-hour")];
const CHOICES: [(u16, &str); 2] = [(VERBOSITY, "verbosity"), (WEEK_START, "week-start")];

struct Planning<'a> {
    app: &'a App,
    known: RefCell<Vec<Setting>>,
}

impl Planning<'_> {
    fn load(&self, page: HWND) {
        let known = values(self.app);
        controls::check(dialog::item(page, CASCADE), value(&known, "cascade-complete-subtasks") == "true");
        for (id, key) in TIMES {
            controls::set_text(dialog::item(page, id), &value(&known, key));
        }
        for (id, key) in CHOICES {
            fill_options(dialog::item(page, id), &known, key);
        }
        *self.known.borrow_mut() = known;
    }

    /// A time field, when it is left: written if it changed, and put back if it does not read.
    fn commit_times(&self, page: HWND) {
        for (id, key) in TIMES {
            let field = dialog::item(page, id);
            let typed = controls::text(field).trim().to_owned();
            let was = value(&self.known.borrow(), key);
            if typed != was && !set(self.app, page, key, &typed) {
                controls::set_text(field, &was);
            }
        }
        self.load(page);
    }
}

impl Dialog for Planning<'_> {
    fn template(&self) -> Template {
        // The names are the core's; where each goes, and its access key, are this page's.
        let known = values(self.app);
        let name = |key, letter, colon| titled(&known, key, letter, colon);
        page("Planning")
            .item(Class::Button, &name("cascade-complete-subtasks", 'C', false), CASCADE, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, 7, 7, 238, 10)
            .item(Class::Static, &name("day-start", 's', true), u16::MAX, 0, 7, 24, 110, 9)
            .item(Class::Edit, "", DAY_START, FIELD, 7, 34, 110, 14)
            .item(Class::Static, &name("day-end", 'e', true), u16::MAX, 0, 128, 24, 110, 9)
            .item(Class::Edit, "", DAY_END, FIELD, 128, 34, 110, 14)
            .item(Class::Static, &name("all-day-reminder-hour", 'A', true), u16::MAX, 0, 7, 54, 110, 9)
            .item(Class::Edit, "", ALL_DAY, FIELD, 7, 64, 110, 14)
            .item(Class::Static, &name("verbosity", 'm', true), u16::MAX, 0, 7, 84, 110, 9)
            .item(Class::ComboBox, "", VERBOSITY, LIST, 7, 94, 110, 60)
            .item(Class::Static, &name("week-start", 'W', true), u16::MAX, 0, 128, 84, 110, 9)
            .item(Class::ComboBox, "", WEEK_START, LIST, 128, 94, 110, 120)
            .item(Class::Static, "These sync to all your devices.", u16::MAX, SS_NOPREFIX.0, 7, 116, 238, 9)
    }

    fn init(&self, page: HWND) -> bool {
        a11y::make_live(dialog::item(page, STATUS));
        let known = values(self.app);
        for (id, key) in TIMES.iter().chain(&CHOICES) {
            if let Some(hint) = known.iter().find(|s| s.key == *key).map(|s| &s.hint).filter(|h| !h.is_empty()) {
                a11y::set_description(dialog::item(page, *id), hint);
            }
        }
        self.load(page);
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        let code = u32::from(code);
        match (id, code) {
            (CASCADE, BN_CLICKED) => {
                let on = controls::checked(dialog::item(page, CASCADE));
                set(self.app, page, "cascade-complete-subtasks", if on { "true" } else { "false" });
                self.load(page);
            }
            (id, CBN_SELCHANGE) if CHOICES.iter().any(|(c, _)| *c == id) => {
                let key = CHOICES.iter().find(|(c, _)| *c == id).map_or("", |(_, k)| *k);
                let chosen = chosen_option(dialog::item(page, id), &self.known.borrow(), key);
                if let Some(chosen) = chosen {
                    set(self.app, page, key, &chosen);
                }
                self.load(page);
            }
            (id, EN_KILLFOCUS) if TIMES.iter().any(|(t, _)| *t == id) => self.commit_times(page),
            _ => {}
        }
        None
    }

    fn message(&self, page: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if finishing(message, lparam) {
            self.commit_times(page);
            return Some(0);
        }
        None
    }
}

// ---------------------------------------------------------------------------------------
// Devices: sync status, pairing, renaming, unpairing
// ---------------------------------------------------------------------------------------

const SYNC_STATUS: u16 = 300;
const DEVICES: u16 = 301;
const SYNC_NOW: u16 = 302;
const PAIR: u16 = 303;
const RENAME: u16 = 304;
const UNPAIR: u16 = 305;

struct Devices<'a> {
    app: &'a App,
    list: RefCell<Vec<lumenna_surface::DeviceView>>,
}

impl Devices<'_> {
    fn load(&self, page: HWND) {
        match self.app.core.lumenna.sync_status() {
            Ok(status) => {
                let text = speech::sentence(&speech::announcement(&status.announcement, &status.notices));
                controls::set_text(dialog::item(page, SYNC_STATUS), &text);
                let list = dialog::item(page, DEVICES);
                let kept = usize::try_from(controls::send(list, LB_GETCURSEL, 0, 0)).unwrap_or(0);
                controls::send(list, LB_RESETCONTENT, 0, 0);
                let now = jiff::Timestamp::now();
                for device in &status.devices {
                    let text = HSTRING::from(devices::line(device, now));
                    controls::send(list, LB_ADDSTRING, 0, text.as_ptr() as isize);
                }
                controls::send(list, LB_SETCURSEL, kept.min(status.devices.len().saturating_sub(1)), 0);
                *self.list.borrow_mut() = status.devices;
                self.offer(page);
            }
            Err(error) => controls::set_text(dialog::item(page, SYNC_STATUS), &sentence(&error)),
        }
    }

    fn chosen(&self, page: HWND) -> Option<lumenna_surface::DeviceView> {
        let index = usize::try_from(controls::send(dialog::item(page, DEVICES), LB_GETCURSEL, 0, 0)).ok()?;
        self.list.borrow().get(index).cloned()
    }

    /// The chosen device's action of `kind`, as the core offers it: a device cannot unpair
    /// itself, so its own row has no Unpair.
    fn act(&self, page: HWND, kind: ActionKind) {
        let Some(device) = self.chosen(page) else { return };
        let Some(action) = actions::of_kind(&device.actions, &[kind]) else {
            return say(page, &not_offered(kind, Subject::Device, device.this_device));
        };
        if let Some((change, _)) = actions::run(self.app, sheet(page), &action, || {}) {
            self.load(page);
            say(page, &speech::announcement(&change.announcement, &change.notices));
        }
    }

    /// Rename and Unpair as the chosen device offers them. The button with focus stays as it
    /// is, since disabling it would send focus nowhere.
    fn offer(&self, page: HWND) {
        let offered = self.chosen(page).map(|device| device.actions).unwrap_or_default();
        for (id, kind) in [(RENAME, ActionKind::Rename), (UNPAIR, ActionKind::Unpair)] {
            let button = dialog::item(page, id);
            let has = actions::of_kind(&offered, &[kind]).is_some();
            if has || controls::focused() != button {
                controls::enable(button, has);
            }
        }
    }
}
impl Dialog for Devices<'_> {
    fn template(&self) -> Template {
        let list = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_WANTKEYBOARDINPUT) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        let status = READ_ONLY | ES_MULTILINE as u32 | WS_VSCROLL.0;
        
        page("Devices")
            .item(Class::Static, "S&ync status:", u16::MAX, 0, 7, 7, 238, 9)
            .item(Class::Edit, "", SYNC_STATUS, status, 7, 17, 238, 28)
            .item(Class::Static, "Paired &devices:", u16::MAX, 0, 7, 50, 238, 9)
            .item(Class::ListBox, "", DEVICES, list, 7, 60, 238, 80)
            .item(Class::Button, "Sync &Now", SYNC_NOW, BUTTON, 7, 146, 56, 14)
            .item(Class::Button, "&Pair a Device...", PAIR, BUTTON, 67, 146, 70, 14)
            .item(Class::Button, "&Rename...", RENAME, BUTTON, 141, 146, 50, 14)
            .item(Class::Button, "&Unpair...", UNPAIR, BUTTON, 195, 146, 50, 14)
    }

    fn init(&self, page: HWND) -> bool {
        a11y::make_live(dialog::item(page, STATUS));
        a11y::set_description(dialog::item(page, DEVICES), "Delete unpairs the selected device.");
        self.load(page);
        self.app.devices_page.set(Some(page));
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        if id == DEVICES && u32::from(code) == LBN_SELCHANGE {
            self.offer(page);
            return None;
        }
        if u32::from(code) != BN_CLICKED {
            return None;
        }
        match id {
            SYNC_NOW => {
                say(page, "Syncing");
                // The round's result comes back to this page, which says it.
                self.app.core.sync_now(Poster::new(page));
            }
            PAIR => {
                if let Some(said) = pairing::run(sheet(page), self.app.core.lumenna.clone()) {
                    self.load(page);
                    self.app.store_changed();
                    say(page, &said);
                }
            }
            RENAME => self.act(page, ActionKind::Rename),
            UNPAIR => self.act(page, ActionKind::Unpair),
            _ => {}
        }
        None
    }

    fn message(&self, page: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        match message {
            WM_SAY => {
                let text = unsafe { said(lparam) };
                self.load(page);
                say(page, &text);
                Some(0)
            }
            // From the app (`devices_changed`): the rest of the window has been redrawn.
            WM_STORE_CHANGED if wparam.0 == FROM_APP => {
                self.load(page);
                Some(0)
            }
            WM_STORE_CHANGED => {
                self.load(page);
                self.app.store_changed();
                Some(0)
            }
            WM_DESTROY => {
                self.app.devices_page.set(None);
                None
            }
            // Delete in the list unpairs, as it removes in every other list.
            WM_VKEYTOITEM if controls::low_word(wparam.0) == VK_DELETE.0 => {
                self.act(page, ActionKind::Unpair);
                Some(-2)
            }
            WM_VKEYTOITEM => Some(-1),
            _ => None,
        }
    }
}

/// Marks a `WM_STORE_CHANGED` the app sent the Devices page, so the page does not send it
/// back.
const FROM_APP: usize = 1;

/// Has the open Devices page read the store again.
pub fn devices_changed(page: HWND) {
    unsafe {
        let _ = PostMessageW(Some(page), WM_STORE_CHANGED, WPARAM(FROM_APP), LPARAM(0));
    }
}

// ---------------------------------------------------------------------------------------
// Backups: this device's alone
// ---------------------------------------------------------------------------------------

const EVERY: u16 = 400;
const KEEP: u16 = 401;
const FOLDER: u16 = 402;
const CHOOSE: u16 = 403;
const BACK_UP: u16 = 404;
const RESTORE: u16 = 405;

struct Backups<'a> {
    app: &'a App,
    known: RefCell<Vec<Setting>>,
}

impl Backups<'_> {
    fn load(&self, page: HWND) {
        let known = values(self.app);
        fill_options(dialog::item(page, EVERY), &known, "backup-every");
        controls::set_text(dialog::item(page, KEEP), &value(&known, "backup-keep"));
        controls::set_text(dialog::item(page, FOLDER), &value(&known, "backup-dir"));
        *self.known.borrow_mut() = known;
    }

    fn commit_keep(&self, page: HWND) {
        let field = dialog::item(page, KEEP);
        let typed = controls::text(field).trim().to_owned();
        let was = value(&self.known.borrow(), "backup-keep");
        if typed != was && !set(self.app, page, "backup-keep", &typed) {
            controls::set_text(field, &was);
        }
        self.load(page);
    }
}

impl Dialog for Backups<'_> {
    fn template(&self) -> Template {
        // The names and what backups are are the core's; where each goes is this page's.
        let known = values(self.app);
        let name = |key, letter| titled(&known, key, letter, true);
        let footer = known.iter().find(|s| s.key == "backup-every").map(|s| s.hint.clone()).unwrap_or_default();
        page("Backups")
            .item(Class::Static, &name("backup-every", 'A'), u16::MAX, 0, 7, 7, 120, 9)
            .item(Class::ComboBox, "", EVERY, LIST, 7, 17, 120, 70)
            .item(Class::Static, &name("backup-keep", 'k'), u16::MAX, 0, 134, 7, 111, 9)
            .item(Class::Edit, "", KEEP, FIELD | ES_NUMBER as u32, 134, 17, 60, 14)
            .item(Class::Static, &name("backup-dir", 't'), u16::MAX, 0, 7, 37, 238, 9)
            .item(Class::Edit, "", FOLDER, READ_ONLY, 7, 47, 182, 14)
            .item(Class::Button, "C&hoose...", CHOOSE, BUTTON, 193, 47, 52, 14)
            .item(Class::Button, "&Back Up Now", BACK_UP, BUTTON, 7, 67, 70, 14)
            .item(Class::Button, "&Restore From a Backup...", RESTORE, BUTTON, 81, 67, 100, 14)
            .item(Class::Static, &footer, u16::MAX, SS_NOPREFIX.0, 7, 88, 238, 36)
    }

    fn init(&self, page: HWND) -> bool {
        a11y::make_live(dialog::item(page, STATUS));
        self.load(page);
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        let code = u32::from(code);
        match (id, code) {
            (EVERY, CBN_SELCHANGE) => {
                let chosen = chosen_option(dialog::item(page, EVERY), &self.known.borrow(), "backup-every");
                if let Some(chosen) = chosen {
                    set(self.app, page, "backup-every", &chosen);
                }
                self.load(page);
            }
            (KEEP, EN_KILLFOCUS) => self.commit_keep(page),
            (CHOOSE, BN_CLICKED) => {
                let current = value(&self.known.borrow(), "backup-dir");
                let start = std::path::PathBuf::from(&current);
                if let Some(folder) = system::choose_folder(sheet(page), "Back Up Here", Some(&start)) {
                    set(self.app, page, "backup-dir", &folder.display().to_string());
                    self.load(page);
                }
            }
            (BACK_UP, BN_CLICKED) => match self.app.core.lumenna.backup(None) {
                Ok(done) => say(page, &speech::announcement(&done.announcement, &done.notices)),
                Err(error) => prompts::fail(sheet(page), &sentence(&error)),
            },
            (RESTORE, BN_CLICKED) => {
                if let Some(said) = import(self.app, sheet(page), "Restore From a Backup", &BACKUPS) {
                    say(page, &said);
                }
            }
            _ => {}
        }
        None
    }

    fn message(&self, page: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if finishing(message, lparam) {
            self.commit_keep(page);
            return Some(0);
        }
        None
    }
}

// ---------------------------------------------------------------------------------------
// Export and import: what you have now, and reading it back
// ---------------------------------------------------------------------------------------

const EXPORT: u16 = 500;
const IMPORT: u16 = 510;

struct Export<'a> {
    app: &'a App,
}

impl Export<'_> {
    fn export(&self, page: HWND, format: ExportFormat) {
        let today = jiff::Zoned::now().date();
        let name = devices::export_name(format, today);
        let extension = name.rsplit('.').next().unwrap_or("txt").to_owned();
        let pattern = format!("*.{extension}");
        let kind = format!("{} files", format.word());
        let Some(path) = system::save_file(sheet(page), "Export", &name, &[(&kind, &pattern)]) else { return };
        // The dialog already asked about replacing a file that was there.
        match self.app.core.lumenna.export(format, Some(path.display().to_string()), true) {
            Ok(done) => say(page, &speech::announcement(&done.announcement, &done.notices)),
            Err(error) => prompts::fail(sheet(page), &sentence(&error)),
        }
    }
}

impl Dialog for Export<'_> {
    fn template(&self) -> Template {
        let mut template = page("Export and Import");
        for (index, export) in devices::EXPORTS.iter().enumerate() {
            let label = devices::marked(export.label, export.key, '&');
            template = template.item(Class::Button, &label, EXPORT + index as u16, BUTTON, 7, 7 + index as i16 * 18, 180, 14);
        }
        let footer = "An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this device lacks and removes nothing.";
        template
            .item(Class::Button, "&Import or Restore...", IMPORT, BUTTON, 7, 79, 180, 14)
            .item(Class::Static, footer, u16::MAX, SS_NOPREFIX.0, 7, 100, 238, 36)
    }

    fn init(&self, page: HWND) -> bool {
        a11y::make_live(dialog::item(page, STATUS));
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        if u32::from(code) != BN_CLICKED {
            return None;
        }
        if let Some(export) = id.checked_sub(EXPORT).and_then(|i| devices::EXPORTS.get(usize::from(i))) {
            self.export(page, export.format);
        } else if id == IMPORT
            && let Some(said) = import(self.app, sheet(page), "Import or Restore", &IMPORTABLE)
        {
            say(page, &said);
        }
        None
    }
}
