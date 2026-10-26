//! Settings, in Windows' own shape: a property sheet of pages, opened with Ctrl+Comma —
//! General, Planning, Devices, Backups, and Export and Import, as on the Mac — drawn from the
//! app's Settings and Devices (`view.settings`, `view.devices`).
//!
//! Like the Mac's, every page applies a change as it is made; a text field, when it is left.
//! What each setting is called, what it takes and what it can be are the core's; what it holds,
//! what was typed and why a line was refused are the app's. Each page has its own status line,
//! a polite live region, where what the app says goes while the sheet is open: the main
//! window's is behind it. Opening at sign-in is the app's setting carried out here (the Run
//! key); the shortcuts from anywhere are this PC's alone, and the window's own.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use lumenna_app::lines::Lines;
use lumenna_app::settings::SettingsEvent;
use lumenna_app::{ActingOn, BackgroundEvent, DevicesEvent, Side, ViewModel};
use lumenna_ffi::ShellEvent;
use lumenna_records::background::{self, BackgroundKind};
use lumenna_records::{ActionKind, ExportFormat, Offered, Setting};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Controls::{
    HKM_GETHOTKEY, HKM_SETHOTKEY, HOTKEYF_ALT, HOTKEYF_CONTROL, HOTKEYF_SHIFT, NM_KILLFOCUS, NMHDR, PSN_APPLY, PSN_KILLACTIVE,
    PSN_RESET, PSN_SETACTIVE,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_F2};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_AUTOCHECKBOX, BS_GROUPBOX, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL, ES_MULTILINE, ES_NUMBER, ES_READONLY,
    GetParent, IDCANCEL, IDOK, LB_ADDSTRING, LB_GETCURSEL, LB_GETCOUNT, LB_RESETCONTENT, LB_SETCURSEL, LBN_SELCHANGE,
    LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LBS_WANTKEYBOARDINPUT, WM_NOTIFY, WM_VKEYTOITEM, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::app::{self, App};
use super::core::{Poster, WM_EVENT};
use super::dialog::{self, Class, Dialog, Template};
use super::{a11y, actions, controls, package, pairing, picker, prompts, shortcuts, system};
use crate::devices;
use crate::shortcut::{Kind, Shortcut};
use crate::speech;

/// Every page's status line.
const STATUS: u16 = 199;
const WIDTH: i16 = 252;
const HEIGHT: i16 = 250;

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

/// The sheet, while it is open.
pub struct Sheet {
    general: General,
    planning: Planning,
    devices: Devices,
    backups: Backups,
    export: Export,
    /// The status line of the page in front, where what the app says goes.
    live: Cell<HWND>,
    /// The refusal last said of each line, so each is said once.
    refused: RefCell<BTreeMap<String, String>>,
}

impl Sheet {
    /// A page came to the front: what the app says goes to its status line now.
    fn front(&self, app: &App, page: HWND) {
        let status = dialog::item(page, STATUS);
        let was = self.live.replace(status);
        if was.is_invalid() {
            app.open_live(status);
        } else {
            app.move_live(was, status);
        }
    }

    /// Draws the app's Settings and Devices on whichever pages exist: a property sheet makes a
    /// page the first time it is shown.
    pub fn draw(&self, app: &App, view: &ViewModel, lines: &Lines, still: bool) {
        let settings: Vec<(Setting, String)> = view
            .settings
            .as_ref()
            .map(|s| s.settings.iter().map(|shown| (shown.setting.clone(), shown.typed.clone())).collect())
            .unwrap_or_default();
        if !settings.is_empty() {
            self.general.draw(&settings);
            self.planning.draw(&settings, still);
            self.backups.draw(&settings, still);
        }
        self.devices.draw(app, view, lines);
        // A line refused stays as typed, and why is said once, over the sheet.
        let problems: Vec<(String, String)> = view
            .settings
            .iter()
            .flat_map(|s| &s.settings)
            .filter_map(|shown| shown.problem.clone().map(|problem| (shown.setting.key.clone(), problem)))
            .collect();
        let mut refused = self.refused.borrow_mut();
        refused.retain(|key, _| problems.iter().any(|(k, _)| k == key));
        for (key, problem) in problems {
            if refused.get(&key) != Some(&problem) {
                refused.insert(key, problem.clone());
                let owner = sheet(self.live.get());
                app.defer(move |_| prompts::fail(owner, &speech::sentence(&problem)));
            }
        }
    }
}

/// Opens Settings on a page.
pub fn show(app: &App, page: Page) {
    app.crux.send(ShellEvent::Settings(SettingsEvent::Open));
    let described = app.described_settings();
    let open = Rc::new(Sheet {
        general: General { hwnd: Cell::new(HWND::default()), described: described.clone() },
        planning: Planning { hwnd: Cell::new(HWND::default()), described: described.clone() },
        devices: Devices { hwnd: Cell::new(HWND::default()), list: RefCell::new(Vec::new()), landing: Cell::new(0) },
        backups: Backups { hwnd: Cell::new(HWND::default()), described },
        export: app.crux.view().settings.map_or_else(
            || Export { offered: Vec::new(), caption: String::new() },
            |settings| Export { offered: settings.offered, caption: settings.offered_caption },
        ),
        live: Cell::new(HWND::default()),
        refused: RefCell::new(BTreeMap::new()),
    });
    app.open_settings(Some(Rc::clone(&open)));
    let pages: [&dyn Dialog; 5] = [&open.general, &open.planning, &open.devices, &open.backups, &open.export];
    dialog::sheet(app.main, "Settings", &pages, page as usize);
    app.open_settings(None);
    app.crux.send(ShellEvent::Settings(SettingsEvent::Close));
    let live = open.live.get();
    if !live.is_invalid() {
        app.close_live(live);
    }
}

/// Asks the app to read in a file — a JSON export or a backup, the core tells which — and
/// say what it did.
pub fn import(owner: HWND, title: &str, types: &[(&str, &str)]) {
    let Some(path) = system::open_file(owner, title, types) else { return };
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    match std::fs::read(&path) {
        Ok(bytes) => app::send(ShellEvent::Settings(SettingsEvent::Import { name, bytes })),
        Err(error) => prompts::fail(owner, &format!("Lumenna could not read {}. {error}.", path.display())),
    }
}

/// Saves a file the app made — an export — where the person chooses.
pub fn save(owner: HWND, file: &lumenna_app::SaveFile) {
    let extension = file.name.rsplit('.').next().unwrap_or("txt").to_owned();
    let pattern = format!("*.{extension}");
    let kind = format!("{} files", extension.to_uppercase());
    let Some(path) = system::save_file(owner, "Save", &file.name, &[(&kind, &pattern)]) else { return };
    // The dialog already asked about replacing a file that was there.
    if let Err(error) = std::fs::write(&path, &file.bytes) {
        prompts::fail(owner, &format!("Lumenna could not save {}. {error}.", path.display()));
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

/// Says something on a page's status line: what the window, not the app, has to say.
fn say(page: HWND, text: &str) {
    let status = dialog::item(page, STATUS);
    controls::set_text(status, &speech::sentence(text));
    a11y::changed(status);
}

/// The sheet a page is in, which owns the dialogs a page opens.
fn sheet(page: HWND) -> HWND {
    unsafe { GetParent(page).unwrap_or(page) }
}

fn value(known: &[(Setting, String)], key: &str) -> String {
    known.iter().find(|(s, _)| s.key == key).map(|(s, _)| s.value.clone()).unwrap_or_default()
}

/// What a setting's field shows: as typed, or its value.
fn typed(known: &[(Setting, String)], key: &str) -> String {
    known.iter().find(|(s, _)| s.key == key).map(|(_, typed)| typed.clone()).unwrap_or_default()
}

/// A setting's name as a control's label: the core's title, with `letter` as its access key
/// and, before a field, a colon.
fn titled(described: &[Setting], key: &str, letter: char, colon: bool) -> String {
    let title = described.iter().find(|s| s.key == key).map_or(key, |s| s.title.as_str());
    format!("{}{}", devices::marked(title, letter, '&'), if colon { ":" } else { "" })
}

/// Fills a drop-down list with a setting's options, the value selected, where what it shows
/// differs. A value set elsewhere that is not among them is listed as itself, since it is still
/// the value.
/// `label` with an access key on its first letter no other control on the page has, which is
/// then taken: the words are the core's, the key the platform's.
fn keyed(label: &str, taken: &mut Vec<char>) -> String {
    let free = label.char_indices().find(|(_, c)| c.is_alphabetic() && !taken.iter().any(|t| t.eq_ignore_ascii_case(c)));
    let Some((at, key)) = free else { return label.to_owned() };
    taken.push(key);
    format!("{}&{}", &label[..at], &label[at..])
}

fn fill_options(combo: HWND, known: &[(Setting, String)], key: &str) {
    let Some((setting, _)) = known.iter().find(|(s, _)| s.key == key) else { return };
    let mut names: Vec<&str> = setting.options.iter().map(|o| o.title.as_str()).collect();
    let mut position = setting.options.iter().position(|o| o.id == setting.value);
    if position.is_none() && !setting.value.is_empty() {
        names.push(&setting.value);
        position = Some(names.len() - 1);
    }
    let count = controls::send(combo, windows::Win32::UI::WindowsAndMessaging::CB_GETCOUNT, 0, 0);
    if usize::try_from(count).ok() != Some(names.len()) {
        fill_combo(combo, &names, position);
    } else if selected(combo) != position {
        controls::send(combo, CB_SETCURSEL, position.unwrap_or(usize::MAX), 0);
    }
}

/// The value chosen in a setting's drop-down list, if it is one of its options.
fn chosen_option(combo: HWND, described: &[Setting], key: &str) -> Option<String> {
    let setting = described.iter().find(|s| s.key == key)?;
    setting.options.get(selected(combo)?).map(|o| o.id.clone())
}

/// Changes a setting, as the app does: it says what that did, or why not.
fn set(key: &str, to: &str) {
    app::send(ShellEvent::Settings(SettingsEvent::Set { key: key.to_owned(), value: to.to_owned() }));
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

/// The code of a page notification, if `message` is one.
fn notification(message: u32, lparam: LPARAM) -> Option<u32> {
    (message == WM_NOTIFY).then(|| unsafe { &*(lparam.0 as *const NMHDR) }.code)
}

/// Whether a page notification asks the page to finish what is being typed: leaving the page,
/// or closing the sheet either way.
fn finishing(code: Option<u32>) -> bool {
    code.is_some_and(|code| code == PSN_KILLACTIVE || code == PSN_APPLY || code == PSN_RESET)
}

/// A page came to the front: what the app says goes to its status line.
fn fronted(page: HWND) {
    app::with(|app| {
        if let Some(open) = app.settings_open() {
            open.front(app, page);
        }
    });
}

/// The lines typed into fields of `typed` (control, key): sent as typed, so the app keeps them
/// through a redraw, and applied when the field is left if they differ from the value.
struct Typed;

impl Typed {
    fn typed(page: HWND, fields: &[(u16, &str)], id: u16, code: u32) -> bool {
        let Some((_, key)) = fields.iter().find(|(field, _)| *field == id) else { return false };
        let field = dialog::item(page, id);
        match code {
            EN_CHANGE if controls::focused() == field => {
                app::send(ShellEvent::Settings(SettingsEvent::Type { key: (*key).to_owned(), value: controls::text(field) }));
            }
            EN_KILLFOCUS => Self::commit(page, &[(id, key)]),
            _ => {}
        }
        true
    }

    /// Applies each field of `fields` whose text differs from the setting's value.
    fn commit(page: HWND, fields: &[(u16, &str)]) {
        let known = app::current_settings();
        for (id, key) in fields {
            let text = controls::text(dialog::item(page, *id)).trim().to_owned();
            if !known.is_empty() && text != value(&known, key) {
                set(key, &text);
            }
        }
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
const AT_SIGN_IN_HINT: u16 = 101;
const SHORTCUT: u16 = 110;
const CHANGE: u16 = 120;
const TOGGLE: u16 = 130;
const REMINDERS: u16 = 140;

struct General {
    hwnd: Cell<HWND>,
    described: Vec<Setting>,
}

impl General {
    fn draw(&self, known: &[(Setting, String)]) {
        let page = self.hwnd.get();
        if page.is_invalid() {
            return;
        }
        let on = value(known, "reminders") == "true";
        let reminders = dialog::item(page, REMINDERS);
        if controls::checked(reminders) != on {
            controls::check(reminders, on);
        }
        // Opening at sign-in, as the app offers it: its words, and as chosen until Windows has
        // made the change.
        if let Some((setting, chosen)) = known.iter().find(|(s, _)| s.key == background::KEY) {
            let at_sign_in = dialog::item(page, AT_SIGN_IN);
            let title = devices::marked(&setting.title, 'O', '&');
            if controls::text(at_sign_in) != title {
                controls::set_text(at_sign_in, &title);
            }
            let hint = dialog::item(page, AT_SIGN_IN_HINT);
            if controls::text(hint) != setting.hint {
                controls::set_text(hint, &setting.hint);
                a11y::set_description(at_sign_in, &setting.hint);
            }
            let on = chosen == "true";
            if controls::checked(at_sign_in) != on {
                controls::check(at_sign_in, on);
            }
        }
    }

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

    fn change(&self, app: &App, page: HWND, kind: Kind) {
        // While the keys are chosen, pressing the current ones must reach the field, not
        // summon the window.
        shortcuts::unregister_all(app.main);
        let recorder = Recorder { kind, chosen: RefCell::new(None) };
        dialog::run(Some(sheet(page)), &recorder);
        let chosen = recorder.chosen.into_inner();
        shortcuts::register_all(app.main);
        if let Some(shortcut) = chosen {
            match shortcuts::change(app.main, kind, Some(shortcut)) {
                Ok(said) => say(page, &said),
                Err(why) => prompts::fail(sheet(page), &why),
            }
            self.show_shortcuts(page);
        }
    }

    fn toggle(&self, app: &App, page: HWND, kind: Kind) {
        let to = if shortcuts::current(kind).is_some() { None } else { Some(kind.standard()) };
        match shortcuts::change(app.main, kind, to) {
            Ok(said) => say(page, &said),
            Err(why) => prompts::fail(sheet(page), &why),
        }
        self.show_shortcuts(page);
    }
}

impl Dialog for General {
    fn template(&self) -> Template {
        // The core's words, which the app's own row replaces as it comes.
        let words = background::background_words(BackgroundKind::WindowsSignIn);
        let mut template = page("General")
            .item(Class::Button, &devices::marked(&words.title, 'O', '&'), AT_SIGN_IN, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, 7, 7, 238, 10)
            .item(Class::Static, &words.hint, AT_SIGN_IN_HINT, SS_NOPREFIX.0, 7, 20, 238, 26)
            .item(Class::Button, &titled(&self.described, "reminders", 'R', false), REMINDERS, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, 7, 50, 238, 10)
            .item(Class::Button, "Shortcuts from anywhere", u16::MAX, BS_GROUPBOX as u32, 7, 64, 238, 92);
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            let y = 76 + index as i16 * 38;
            let index = index as u16;
            template = template
                .item(Class::Static, &format!("{}:", kind.name()), u16::MAX, SS_NOPREFIX.0, 14, y, 224, 9)
                .item(Class::Edit, "", SHORTCUT + index, READ_ONLY, 14, y + 10, 110, 14)
                .item(Class::Button, "Change...", CHANGE + index, BUTTON, 128, y + 10, 52, 14)
                .item(Class::Button, "Turn Off", TOGGLE + index, BUTTON, 184, y + 10, 54, 14);
        }
        let footer = "These work in any program, so they take their keys from whatever is in front. Control+Alt+Shift, because Control+Alt alone is AltGr on many keyboards, where it types letters.";
        template.item(Class::Static, footer, u16::MAX, SS_NOPREFIX.0, 7, 160, 238, 36)
    }

    fn init(&self, page: HWND) -> bool {
        self.hwnd.set(page);
        a11y::make_live(dialog::item(page, STATUS));
        let at_sign_in = dialog::item(page, AT_SIGN_IN);
        app::with(|app| controls::check(at_sign_in, opens_at_sign_in(&app.directory)));
        a11y::set_description(at_sign_in, &controls::text(dialog::item(page, AT_SIGN_IN_HINT)));
        let reminders = dialog::item(page, REMINDERS);
        if let Some(setting) = self.described.iter().find(|s| s.key == "reminders") {
            controls::check(reminders, setting.value == "true");
            if !setting.hint.is_empty() {
                a11y::set_description(reminders, &setting.hint);
            }
        }
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
        app::with(|app| match id {
            // The app hands it back to be done (`keep_running`), and says how it went.
            AT_SIGN_IN => {
                let on = controls::checked(dialog::item(page, AT_SIGN_IN));
                set(background::KEY, if on { "true" } else { "false" });
            }
            REMINDERS => {
                let on = controls::checked(dialog::item(page, REMINDERS));
                set("reminders", if on { "true" } else { "false" });
            }
            id if (CHANGE..CHANGE + 2).contains(&id) => self.change(app, page, Kind::ALL[usize::from(id - CHANGE)]),
            id if (TOGGLE..TOGGLE + 2).contains(&id) => self.toggle(app, page, Kind::ALL[usize::from(id - TOGGLE)]),
            _ => {}
        });
        None
    }

    fn message(&self, page: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if notification(message, lparam) == Some(PSN_SETACTIVE) {
            fronted(page);
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

/// What this PC keeps running in the background, as the app is told it: whether Lumenna opens
/// at sign-in on the profile at `profile`. Sent at start and after every change.
pub fn background(profile: &std::path::Path) -> ShellEvent {
    ShellEvent::Background(BackgroundEvent::Offered { kind: BackgroundKind::WindowsSignIn, on: opens_at_sign_in(profile) })
}

/// Whether Lumenna opens at sign-in on the profile at `profile`: the Run key's, or a package's
/// startup task's, which a package has one of.
fn opens_at_sign_in(profile: &std::path::Path) -> bool {
    if package::packaged() { package::opens_at_sign_in() } else { sign_in::on(profile) }
}

/// Opens Lumenna at sign-in, or stops, as the app asked, and tells the app how that went, which
/// it says in its own words. A package asks Windows off the window's thread and posts the
/// answer back (`WM_EVENT`).
pub fn keep_running(app: &App, on: bool) {
    let answer = |result: Result<(), String>, profile: &std::path::Path| match result {
        Ok(()) => background(profile),
        Err(reason) => ShellEvent::Background(BackgroundEvent::Failed { reason }),
    };
    if package::packaged() {
        let window = Poster::new(app.main);
        let profile = app.directory.clone();
        std::thread::spawn(move || {
            window.send(WM_EVENT, answer(package::open_at_sign_in(on), &profile));
        });
        return;
    }
    app.crux.send(answer(sign_in::set(on, &app.directory), &app.directory));
}

/// Opening at sign-in, for an ordinary install: the current user's Run key, as every Windows
/// program does it, a value for each profile. Started that way, the app opens in the
/// notification area without its window (`--background`).
pub mod sign_in {
    use std::path::Path;

    use super::system;
    use crate::win::tray;

    /// What every value of the app's starts with; the usual profile's is this alone.
    const NAME: &str = "Lumenna";

    /// The Run key's value for the profile at `profile`: the usual one's is "Lumenna", and each
    /// other's a name of its own, so each profile opens at sign-in, or not, by itself.
    /// `LUMENNA_RUN_NAME` gives the UI tests one of their own, so they never change what opens
    /// at the person's sign-in.
    fn name(profile: &Path) -> String {
        if let Some(name) = std::env::var("LUMENNA_RUN_NAME").ok().filter(|name| !name.is_empty()) {
            return name;
        }
        named(profile, usual(profile))
    }

    fn named(profile: &Path, usual: bool) -> String {
        if usual { NAME.to_owned() } else { format!("{NAME} {}", tray::profile_key(profile)) }
    }

    fn usual(profile: &Path) -> bool {
        crate::profile::default_directory().as_deref() == Some(profile)
    }

    /// What the value runs: this executable, in the background, on the profile at `profile`,
    /// named only when it is not the usual one.
    fn command(exe: &Path, profile: &Path, usual: bool) -> String {
        let mut command = format!("\"{}\" --background", exe.display());
        if !usual {
            command.push_str(&format!(" --profile \"{}\"", profile.display()));
        }
        command
    }

    /// Whether this executable opens on the profile at `profile` at sign-in: the value is there
    /// and starts this program on this profile, not one moved or another copy's.
    pub fn on(profile: &Path) -> bool {
        let Ok(exe) = std::env::current_exe() else { return false };
        system::read_string(system::RUN_KEY, &name(profile))
            .is_some_and(|stored| stored.eq_ignore_ascii_case(&command(&exe, profile, usual(profile))))
    }

    /// Turns it on or off for the profile at `profile`, or says why Windows would not.
    pub fn set(on: bool, profile: &Path) -> Result<(), String> {
        // Windows' message is a sentence; the app's own sentence ends it.
        let reason = |error: &dyn std::fmt::Display| error.to_string().trim().trim_end_matches('.').to_owned();
        if !on {
            return system::delete_value(system::RUN_KEY, &name(profile)).map_err(|error| reason(&error.message()));
        }
        let exe = std::env::current_exe().map_err(|error| reason(&error))?;
        system::write_string(system::RUN_KEY, &name(profile), &command(&exe, profile, usual(profile)))
            .map_err(|error| reason(&error.message()))
    }

    /// Takes away every profile's value that starts this executable: what uninstalling it leaves
    /// behind otherwise, since another profile's name is its own.
    pub fn forget_all() {
        let Ok(exe) = std::env::current_exe() else { return };
        let starts = format!("\"{}\"", exe.display()).to_lowercase();
        for name in system::value_names(system::RUN_KEY) {
            let ours = name == NAME || name.starts_with(&format!("{NAME} "));
            if ours && system::read_string(system::RUN_KEY, &name).is_some_and(|command| command.to_lowercase().starts_with(&starts)) {
                let _ = system::delete_value(system::RUN_KEY, &name);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_usual_profile_opens_at_sign_in_as_lumenna_and_another_under_a_name_of_its_own() {
            let work = Path::new(r"D:\work");
            assert_eq!(named(Path::new(r"C:\data"), true), "Lumenna");
            assert_eq!(named(work, false), format!("Lumenna {}", tray::profile_key(work)));
            assert_ne!(named(work, false), named(Path::new(r"D:\home"), false));
        }

        #[test]
        fn a_profile_other_than_the_usual_one_is_named_in_what_opens_at_sign_in() {
            let exe = Path::new(r"C:\Programs\Lumenna\lumenna.exe");
            assert_eq!(command(exe, Path::new(r"C:\data"), true), r#""C:\Programs\Lumenna\lumenna.exe" --background"#);
            assert_eq!(
                command(exe, Path::new(r"D:\work"), false),
                r#""C:\Programs\Lumenna\lumenna.exe" --background --profile "D:\work""#
            );
        }
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
const TASK_REMINDERS: u16 = 206;
const BLOCK_REMINDERS: u16 = 207;
const URGENCY: u16 = 208;
const TASK_ORDER: u16 = 209;
const DAY_START_CLOCK: u16 = 211;
const DAY_END_CLOCK: u16 = 212;
const ALL_DAY_CLOCK: u16 = 213;

/// The times, each with the system's clock beside its field: the field, the clock, the setting.
const CLOCKS: [(u16, u16, &str); 3] =
    [(DAY_START, DAY_START_CLOCK, "day-start"), (DAY_END, DAY_END_CLOCK, "day-end"), (ALL_DAY, ALL_DAY_CLOCK, "all-day-reminder-hour")];

/// What is typed, and written when the field is left, never as each key is pressed.
const TYPED: [(u16, &str); 6] = [
    (DAY_START, "day-start"),
    (DAY_END, "day-end"),
    (ALL_DAY, "all-day-reminder-hour"),
    (TASK_REMINDERS, "default-task-reminders"),
    (BLOCK_REMINDERS, "default-block-reminders"),
    (URGENCY, "urgency-weights"),
];
const CHOICES: [(u16, &str); 3] = [(VERBOSITY, "verbosity"), (WEEK_START, "week-start"), (TASK_ORDER, "task-order")];

struct Planning {
    hwnd: Cell<HWND>,
    described: Vec<Setting>,
}

impl Planning {
    fn draw(&self, known: &[(Setting, String)], still: bool) {
        let page = self.hwnd.get();
        if page.is_invalid() {
            return;
        }
        let cascade = dialog::item(page, CASCADE);
        let on = value(known, "cascade-complete-subtasks") == "true";
        if controls::checked(cascade) != on {
            controls::check(cascade, on);
        }
        for (id, key) in TYPED {
            controls::write_typed(dialog::item(page, id), &typed(known, key), still);
        }
        for (_, clock, key) in CLOCKS {
            picker::show(dialog::item(page, clock), picker::Kind::Time, &value(known, key));
        }
        for (id, key) in CHOICES {
            fill_options(dialog::item(page, id), known, key);
        }
    }
}

impl Dialog for Planning {
    fn template(&self) -> Template {
        // The names are the core's; where each goes, and its access key, are this page's.
        let name = |key, letter, colon| titled(&self.described, key, letter, colon);
        page("Planning")
            .item(Class::Button, &name("cascade-complete-subtasks", 'C', false), CASCADE, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, 7, 7, 238, 10)
            .item(Class::Static, &name("day-start", 's', true), u16::MAX, 0, 7, 24, 110, 9)
            .item(Class::Edit, "", DAY_START, FIELD, 7, 34, 50, 14)
            .named(picker::CLASS, "", DAY_START_CLOCK, picker::Kind::Time.style(), 61, 34, 56, 14)
            .item(Class::Static, &name("day-end", 'e', true), u16::MAX, 0, 128, 24, 110, 9)
            .item(Class::Edit, "", DAY_END, FIELD, 128, 34, 50, 14)
            .named(picker::CLASS, "", DAY_END_CLOCK, picker::Kind::Time.style(), 182, 34, 56, 14)
            .item(Class::Static, &name("all-day-reminder-hour", 'A', true), u16::MAX, 0, 7, 54, 110, 9)
            .item(Class::Edit, "", ALL_DAY, FIELD, 7, 64, 50, 14)
            .named(picker::CLASS, "", ALL_DAY_CLOCK, picker::Kind::Time.style(), 61, 64, 56, 14)
            .item(Class::Static, &name("task-order", 'l', true), u16::MAX, 0, 128, 54, 110, 9)
            .item(Class::ComboBox, "", TASK_ORDER, LIST, 128, 64, 110, 60)
            .item(Class::Static, &name("verbosity", 'm', true), u16::MAX, 0, 7, 84, 110, 9)
            .item(Class::ComboBox, "", VERBOSITY, LIST, 7, 94, 110, 60)
            .item(Class::Static, &name("week-start", 'W', true), u16::MAX, 0, 128, 84, 110, 9)
            .item(Class::ComboBox, "", WEEK_START, LIST, 128, 94, 110, 120)
            .item(Class::Static, &name("urgency-weights", 'u', true), u16::MAX, 0, 7, 114, 238, 9)
            .item(Class::Edit, "", URGENCY, FIELD, 7, 124, 238, 14)
            .item(Class::Static, &name("default-task-reminders", 't', true), u16::MAX, 0, 7, 144, 238, 9)
            .item(Class::Edit, "", TASK_REMINDERS, FIELD, 7, 154, 238, 14)
            .item(Class::Static, &name("default-block-reminders", 'b', true), u16::MAX, 0, 7, 174, 238, 9)
            .item(Class::Edit, "", BLOCK_REMINDERS, FIELD, 7, 184, 238, 14)
            .item(Class::Static, "These sync to all your devices.", u16::MAX, SS_NOPREFIX.0, 7, 204, 238, 9)
    }

    fn init(&self, page: HWND) -> bool {
        self.hwnd.set(page);
        a11y::make_live(dialog::item(page, STATUS));
        for (id, key) in TYPED.iter().chain(&CHOICES) {
            if let Some(hint) = self.described.iter().find(|s| s.key == *key).map(|s| &s.hint).filter(|h| !h.is_empty()) {
                a11y::set_description(dialog::item(page, *id), hint);
            }
        }
        for (_, clock, key) in CLOCKS {
            let title = self.described.iter().find(|s| s.key == key).map_or(key, |s| s.title.as_str());
            picker::ready(dialog::item(page, clock), picker::Kind::Time, title, &picker::Kind::Time.choose());
        }
        // As described, until the app's own settings arrive.
        let known: Vec<(Setting, String)> = self.described.iter().map(|s| (s.clone(), s.value.clone())).collect();
        self.draw(&known, true);
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        let code = u32::from(code);
        if Typed::typed(page, &TYPED, id, code) {
            return None;
        }
        match (id, code) {
            (CASCADE, BN_CLICKED) => {
                let on = controls::checked(dialog::item(page, CASCADE));
                set("cascade-complete-subtasks", if on { "true" } else { "false" });
            }
            (id, CBN_SELCHANGE) if CHOICES.iter().any(|(c, _)| *c == id) => {
                let key = CHOICES.iter().find(|(c, _)| *c == id).map_or("", |(_, k)| *k);
                if let Some(chosen) = chosen_option(dialog::item(page, id), &self.described, key) {
                    set(key, &chosen);
                }
            }
            _ => {}
        }
        None
    }

    fn message(&self, page: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        // A time picked is its field's, typed there as the arrows change it and applied when the
        // clock is left, as a field is: applied at each arrow, every step would be said.
        if let Some(id) = picker::changed(message, lparam)
            && let Some((field, _, key)) = CLOCKS.iter().find(|(_, clock, _)| *clock == id)
        {
            if let Some(time) = picker::read(dialog::item(page, id), picker::Kind::Time) {
                controls::set_text(dialog::item(page, *field), &time);
                app::send(ShellEvent::Settings(SettingsEvent::Type { key: (*key).to_owned(), value: time }));
            }
            return Some(0);
        }
        let code = notification(message, lparam);
        if code == Some(NM_KILLFOCUS)
            && let Some((field, _, key)) =
                CLOCKS.iter().find(|(_, clock, _)| i32::from(*clock) == unsafe { &*(lparam.0 as *const NMHDR) }.idFrom as i32)
        {
            Typed::commit(page, &[(*field, key)]);
        }
        if code == Some(PSN_SETACTIVE) {
            fronted(page);
        }
        if finishing(code) {
            Typed::commit(page, &TYPED);
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

struct Devices {
    hwnd: Cell<HWND>,
    list: RefCell<Vec<lumenna_records::DeviceView>>,
    /// The landing count of the list's row in hand when last drawn.
    landing: Cell<u32>,
}

impl Devices {
    /// Draws the app's Devices page: how syncing is going, and the devices, the row in hand
    /// the app's.
    fn draw(&self, app: &App, view: &ViewModel, lines: &Lines) {
        let page = self.hwnd.get();
        let Some(shown) = view.devices.as_ref().filter(|_| !page.is_invalid()) else { return };
        let status = dialog::item(page, SYNC_STATUS);
        let said = speech::sentence(&shown.status);
        if controls::text(status) != said {
            controls::set_text(status, &said);
        }
        let devices = shown.list.as_ref().map(|list| list.devices.clone()).unwrap_or_default();
        let list = dialog::item(page, DEVICES);
        let texts: Vec<String> = lines.devices.iter().map(|line| line.text.clone()).collect();
        let same = *self.list.borrow() == devices
            && usize::try_from(controls::send(list, LB_GETCOUNT, 0, 0)).ok() == Some(texts.len());
        if !same {
            let kept = usize::try_from(controls::send(list, LB_GETCURSEL, 0, 0)).unwrap_or(0);
            controls::send(list, LB_RESETCONTENT, 0, 0);
            for text in &texts {
                let text = HSTRING::from(text.as_str());
                controls::send(list, LB_ADDSTRING, 0, text.as_ptr() as isize);
            }
            controls::send(list, LB_SETCURSEL, kept.min(texts.len().saturating_sub(1)), 0);
            *self.list.borrow_mut() = devices;
        }
        // Where a change made from the list lands: the same device, or the one in its place.
        let held = &view.devices_held;
        if held.landing != self.landing.replace(held.landing)
            && let Some(at) = held.focus.as_ref().and_then(|key| self.list.borrow().iter().position(|d| &d.node_id == key))
        {
            controls::send(list, LB_SETCURSEL, at, 0);
            if controls::lost(app.main) || controls::focused().is_invalid() {
                controls::focus(list);
            }
        }
        self.offer(page);
    }

    fn chosen(&self, page: HWND) -> Option<lumenna_records::DeviceView> {
        let index = usize::try_from(controls::send(dialog::item(page, DEVICES), LB_GETCURSEL, 0, 0)).ok()?;
        self.list.borrow().get(index).cloned()
    }

    /// The chosen device's action of `kind`, as the app's: a device cannot unpair itself, so
    /// its own row has no Unpair, and the app says why on this page's status line.
    fn act(&self, page: HWND, kind: ActionKind) {
        let Some(device) = self.chosen(page) else { return };
        match actions::of_kind(&device.offered, &[kind]) {
            Some(action) => app::send(ShellEvent::ActOn(ActingOn::Held(Side::Devices, device.node_id), action)),
            None => {
                app::send(ShellEvent::Hold(Side::Devices, device.node_id));
                app::send(ShellEvent::Run(vec![kind]));
            }
        }
    }

    /// Rename and Unpair as the chosen device offers them. The button with focus stays as it
    /// is, since disabling it would send focus nowhere.
    fn offer(&self, page: HWND) {
        let offered = self.chosen(page).map(|device| device.offered).unwrap_or_default();
        for (id, kind) in [(RENAME, ActionKind::Rename), (UNPAIR, ActionKind::Unpair)] {
            let button = dialog::item(page, id);
            let has = actions::of_kind(&offered, &[kind]).is_some();
            if has || controls::focused() != button {
                controls::enable(button, has);
            }
        }
    }
}

impl Dialog for Devices {
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
        self.hwnd.set(page);
        a11y::make_live(dialog::item(page, STATUS));
        a11y::set_description(dialog::item(page, DEVICES), "Delete unpairs the selected device, and F2 renames it.");
        self.offer(page);
        // The devices and how syncing is going, as the app finds them.
        app::send(ShellEvent::Devices(DevicesEvent::Show));
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        if id == DEVICES && u32::from(code) == LBN_SELCHANGE {
            if let Some(device) = self.chosen(page) {
                app::send(ShellEvent::Hold(Side::Devices, device.node_id));
            }
            self.offer(page);
            return None;
        }
        if u32::from(code) != BN_CLICKED {
            return None;
        }
        match id {
            SYNC_NOW => app::send(ShellEvent::Devices(DevicesEvent::SyncNow)),
            PAIR => app::with(|app| pairing::run(app, sheet(page))),
            RENAME => self.act(page, ActionKind::Rename),
            UNPAIR => self.act(page, ActionKind::Unpair),
            _ => {}
        }
        None
    }

    fn message(&self, page: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if notification(message, lparam) == Some(PSN_SETACTIVE) {
            fronted(page);
            return None;
        }
        match message {
            // Delete in the list unpairs, as it removes in every other list.
            // F2 renames, as in Explorer.
            WM_VKEYTOITEM if controls::low_word(wparam.0) == VK_F2.0 => {
                self.act(page, ActionKind::Rename);
                Some(-2)
            }
            WM_VKEYTOITEM if controls::low_word(wparam.0) == VK_DELETE.0 => {
                // What Delete runs is the core's (`keys::runs`): here, Unpair.
                if let Some(device) = self.chosen(page) {
                    app::send(ShellEvent::Hold(Side::Devices, device.node_id));
                    app::send(ShellEvent::Key("delete".into()));
                }
                Some(-2)
            }
            WM_VKEYTOITEM => Some(-1),
            _ => None,
        }
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

const KEPT: [(u16, &str); 1] = [(KEEP, "backup-keep")];

struct Backups {
    hwnd: Cell<HWND>,
    described: Vec<Setting>,
}

impl Backups {
    fn draw(&self, known: &[(Setting, String)], still: bool) {
        let page = self.hwnd.get();
        if page.is_invalid() {
            return;
        }
        fill_options(dialog::item(page, EVERY), known, "backup-every");
        controls::write_typed(dialog::item(page, KEEP), &typed(known, "backup-keep"), still);
        let folder = dialog::item(page, FOLDER);
        let dir = value(known, "backup-dir");
        if controls::text(folder) != dir {
            controls::set_text(folder, &dir);
        }
    }
}

impl Dialog for Backups {
    fn template(&self) -> Template {
        // The names and what backups are are the core's; where each goes is this page's.
        let name = |key, letter| titled(&self.described, key, letter, true);
        let footer = self.described.iter().find(|s| s.key == "backup-every").map(|s| s.hint.clone()).unwrap_or_default();
        page("Backups")
            .item(Class::Static, &name("backup-every", 'A'), u16::MAX, 0, 7, 7, 120, 9)
            .item(Class::ComboBox, "", EVERY, LIST, 7, 17, 120, 70)
            .item(Class::Static, &name("backup-keep", 'k'), u16::MAX, 0, 134, 7, 111, 9)
            .item(Class::Edit, "", KEEP, FIELD | ES_NUMBER as u32, 134, 17, 60, 14)
            .item(Class::Static, &name("backup-dir", 't'), u16::MAX, 0, 7, 37, 238, 9)
            .item(Class::Edit, "", FOLDER, READ_ONLY, 7, 47, 182, 14)
            .item(Class::Button, "C&hoose...", CHOOSE, BUTTON, 193, 47, 52, 14)
            .item(Class::Button, "&Back Up Now", BACK_UP, BUTTON, 7, 67, 70, 14)
            .item(Class::Button, "&Restore from a Backup...", RESTORE, BUTTON, 81, 67, 100, 14)
            .item(Class::Static, &footer, u16::MAX, SS_NOPREFIX.0, 7, 88, 238, 36)
    }

    fn init(&self, page: HWND) -> bool {
        self.hwnd.set(page);
        a11y::make_live(dialog::item(page, STATUS));
        let known: Vec<(Setting, String)> = self.described.iter().map(|s| (s.clone(), s.value.clone())).collect();
        self.draw(&known, true);
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        let code = u32::from(code);
        if Typed::typed(page, &KEPT, id, code) {
            return None;
        }
        match (id, code) {
            (EVERY, CBN_SELCHANGE) => {
                if let Some(chosen) = chosen_option(dialog::item(page, EVERY), &self.described, "backup-every") {
                    set("backup-every", &chosen);
                }
            }
            (CHOOSE, BN_CLICKED) => {
                let current = controls::text(dialog::item(page, FOLDER));
                let start = std::path::PathBuf::from(&current);
                if let Some(folder) = system::choose_folder(sheet(page), "Back Up Here", Some(&start)) {
                    set("backup-dir", &folder.display().to_string());
                }
            }
            // Into this device's backups, as the automatic ones are; where is said on this page,
            // the dialog in front, once it is taken.
            (BACK_UP, BN_CLICKED) => app::with(App::back_up),
            (RESTORE, BN_CLICKED) => import(sheet(page), "Restore from a Backup", &BACKUPS),
            _ => {}
        }
        None
    }

    fn message(&self, page: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        let code = notification(message, lparam);
        if code == Some(PSN_SETACTIVE) {
            fronted(page);
        }
        if finishing(code) {
            Typed::commit(page, &KEPT);
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
/// The store's own actions, from here on: Erase Everything Deleted.
const OFFERED: u16 = 520;

struct Export {
    /// What the app offers to do to the store as a whole, a button each after the rest.
    offered: Vec<Offered>,
    /// What they take, under their buttons.
    caption: String,
}

impl Dialog for Export {
    fn template(&self) -> Template {
        let mut template = page("Export and Import");
        let mut keys = vec!['I'];
        for (index, export) in devices::EXPORTS.iter().enumerate() {
            let label = devices::marked(export.label, export.key, '&');
            keys.push(export.key);
            template = template.item(Class::Button, &label, EXPORT + index as u16, BUTTON, 7, 7 + index as i16 * 18, 180, 14);
        }
        template = template.item(Class::Button, "&Import or Restore...", IMPORT, BUTTON, 7, 79, 180, 14);
        for (index, action) in self.offered.iter().enumerate() {
            let label = keyed(&actions::label(action), &mut keys);
            template = template.item(Class::Button, &label, OFFERED + index as u16, BUTTON, 7, 97 + index as i16 * 18, 180, 14);
        }
        let mut below = 97 + self.offered.len() as i16 * 18;
        if !self.offered.is_empty() {
            template = template.item(Class::Static, &self.caption, u16::MAX, SS_NOPREFIX.0, 7, below - 2, 238, 18);
            below += 20;
        }
        let footer = "An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this device lacks and removes nothing.";
        template.item(Class::Static, footer, u16::MAX, SS_NOPREFIX.0, 7, below + 3, 238, 36)
    }

    fn init(&self, page: HWND) -> bool {
        a11y::make_live(dialog::item(page, STATUS));
        // The caption is said with each button, as a screen reader reads a button alone.
        for index in 0..self.offered.len() {
            a11y::set_description(dialog::item(page, OFFERED + index as u16), &self.caption);
        }
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        if u32::from(code) != BN_CLICKED {
            return None;
        }
        // The app makes the export, which is then saved where the person chooses.
        if let Some(export) = id.checked_sub(EXPORT).and_then(|i| devices::EXPORTS.get(usize::from(i))) {
            let format: ExportFormat = export.format;
            app::send(ShellEvent::Settings(SettingsEvent::Export(format)));
        } else if id == IMPORT {
            import(sheet(page), "Import or Restore", &IMPORTABLE);
        } else if let Some(action) = id.checked_sub(OFFERED).and_then(|i| self.offered.get(usize::from(i))) {
            // The app asks its question as it asks a row's, over the sheet.
            app::send(ShellEvent::ActOn(ActingOn::Settings, action.clone()));
        }
        None
    }

    fn message(&self, page: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if notification(message, lparam) == Some(PSN_SETACTIVE) {
            fronted(page);
        }
        None
    }
}
