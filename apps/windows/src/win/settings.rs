//! Settings, in Windows' own shape: a property sheet of pages, opened with Ctrl+Comma, drawn
//! from the app's Settings and Devices (`view.settings`, `view.devices`).
//!
//! The pages are the core's (`SettingsView::pages`): their titles, which settings each holds and
//! in what order, their buttons and the caption under them; each setting is a control by its
//! kind, named, described and offered as the core describes it. What is this page's: where each
//! goes, its access key, and what only Windows has — the shortcuts from anywhere on General, the
//! devices on Devices and Sync, the exports on Export and Import.
//!
//! Like the Mac's, every page applies a change as it is made; a text field, when it is left.
//! What it holds, what was typed and why a line was refused are the app's. Each page has its own
//! status line, a polite live region, where what the app says goes while the sheet is open: the
//! main window's is behind it. Opening at sign-in is the app's setting carried out here (the Run
//! key); the shortcuts from anywhere are this PC's alone, and the window's own.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use lumenna_app::hotkeys::{self, HotKeys, HotKind};
use lumenna_app::lines::Lines;
use lumenna_app::settings::SettingsEvent;
use lumenna_app::{ActingOn, BackgroundEvent, DevicesEvent, Side, ViewModel};
use lumenna_ffi::ShellEvent;
use lumenna_records::background::BackgroundKind;
use lumenna_records::fields::{SettingWords, SettingsPage, settings_pages, setting_words};
use lumenna_records::{ActionKind, ExportFormat, Offered, Setting, SettingKind};
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
use crate::speech;

/// Every page's status line.
const STATUS: u16 = 199;
const WIDTH: i16 = 252;
const HEIGHT: i16 = 250;
/// Where a page's content goes, in dialog units: its left edge, its width, and the second
/// column's left edge and width, for the short controls that sit two to a row.
const LEFT: i16 = 7;
const INNER: i16 = 238;
const HALF: i16 = 115;
const RIGHT: i16 = LEFT + HALF + 8;

/// The sheet, while it is open.
pub struct Sheet {
    pages: Vec<Page>,
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
        for page in &self.pages {
            if !settings.is_empty() {
                page.draw(&settings, still);
            }
            page.draw_devices(app, view, lines);
        }
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

/// Opens Settings on the page the core calls `id` (`general`, `devices`, `export`, …).
pub fn show(app: &App, id: &str) {
    app.crux.send(ShellEvent::Settings(SettingsEvent::Open));
    // The store's settings, and opening at sign-in, which is the app's row rather than the
    // store's, as Windows has it now.
    let mut described = app.described_settings();
    described.retain(|s| s.key != lumenna_records::background::KEY);
    described.push(lumenna_records::background::background_setting(BackgroundKind::WindowsSignIn, opens_at_sign_in(&app.directory)));
    let view = app.crux.view().settings;
    // The core's pages: the app's, else as the core lays them out for a shell that follows its
    // platform's clock, which this does (`Clock`).
    let pages = view.as_ref().map(|s| s.pages.clone()).filter(|pages| !pages.is_empty()).unwrap_or_else(|| settings_pages(true, false));
    let words = view.as_ref().map_or_else(setting_words, |s| s.words.clone());
    let (offered, offered_caption) = view.map(|s| (s.offered, s.offered_caption)).unwrap_or_default();
    let start = pages.iter().position(|page| page.id == id).unwrap_or(0);
    let open = Rc::new(Sheet {
        pages: pages
            .into_iter()
            .map(|page| Page {
                hwnd: Cell::new(HWND::default()),
                page,
                described: described.clone(),
                words: words.clone(),
                fields: RefCell::new(Vec::new()),
                buttons: RefCell::new(Vec::new()),
                offered: offered.clone(),
                offered_caption: offered_caption.clone(),
                devices: RefCell::new(Vec::new()),
                landing: Cell::new(0),
            })
            .collect(),
        live: Cell::new(HWND::default()),
        refused: RefCell::new(BTreeMap::new()),
    });
    app.open_settings(Some(Rc::clone(&open)));
    let pages: Vec<&dyn Dialog> = open.pages.iter().map(|page| page as &dyn Dialog).collect();
    dialog::sheet(app.main, "Settings", &pages, start);
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
fn page_template(title: &str) -> Template {
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

/// `label` with an access key on its first letter no other control on the page has, which is
/// then taken: the words are the core's, the key the platform's.
fn keyed(label: &str, taken: &mut Vec<char>) -> String {
    let free = label.char_indices().find(|(_, c)| c.is_alphabetic() && !taken.iter().any(|t| t.eq_ignore_ascii_case(c)));
    let Some((at, key)) = free else { return label.replace('&', "&&") };
    taken.push(key);
    format!("{}&{}", label[..at].replace('&', "&&"), label[at..].replace('&', "&&"))
}

/// Fills a drop-down list with a setting's options, the value selected, where what it shows
/// differs. A value set elsewhere that is not among them is listed as itself, since it is still
/// the value.
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

/// Applies each field of `fields` (control, key) whose text differs from the setting's value:
/// a line is sent as typed, so the app keeps it through a redraw, and applied when left.
fn commit(page: HWND, fields: &[(u16, String)]) {
    let known = app::current_settings();
    for (id, key) in fields {
        let text = controls::text(dialog::item(page, *id)).trim().to_owned();
        if !known.is_empty() && text != value(&known, key) {
            set(key, &text);
        }
    }
}

const BUTTON: u32 = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
const FIELD: u32 = ES_AUTOHSCROLL as u32 | WS_BORDER.0 | WS_TABSTOP.0;
const READ_ONLY: u32 = ES_AUTOHSCROLL as u32 | ES_READONLY as u32 | WS_BORDER.0 | WS_TABSTOP.0;
const LIST: u32 = CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0;

/// How many lines of the page's width `text` takes, roughly: for a static's height.
fn lines_of(text: &str) -> i16 {
    i16::try_from(text.chars().count() / 58 + 1).unwrap_or(4)
}

// ---------------------------------------------------------------------------------------
// A page
// ---------------------------------------------------------------------------------------

/// Each setting's control from here, two ids apiece: the control, then its picker or its Choose
/// button.
const FIELDS: u16 = 200;
/// The page's own buttons from here (`SettingsPage::buttons`).
const BUTTONS: u16 = 600;

// General: the shortcuts from anywhere, each its keys, Change and Turn Off.
const SHORTCUT: u16 = 110;
const CHANGE: u16 = 120;
const TOGGLE: u16 = 130;

// Devices and Sync: how syncing is going, and the devices, renamed and unpaired.
const SYNC_STATUS: u16 = 300;
const DEVICES: u16 = 301;
const RENAME: u16 = 304;
const UNPAIR: u16 = 305;

// Export and Import: each export, then the store's own actions (Erase Everything Deleted).
const EXPORT: u16 = 500;
const OFFERED: u16 = 520;

/// A setting's control, as its page made it.
struct Field {
    id: u16,
    key: String,
    kind: SettingKind,
}

/// A page of settings, as the core lays it out.
struct Page {
    hwnd: Cell<HWND>,
    page: SettingsPage,
    /// Every setting as the core describes it, to lay the page out by before the app's arrive.
    described: Vec<Setting>,
    words: SettingWords,
    /// Its settings' controls, made with its template.
    fields: RefCell<Vec<Field>>,
    /// Its own buttons, by id, as its template made them.
    buttons: RefCell<Vec<(u16, String)>>,
    /// What the store as a whole offers, a button each on Export and Import.
    offered: Vec<Offered>,
    /// What they take, under them.
    offered_caption: String,
    /// The devices as last drawn, on Devices and Sync.
    devices: RefCell<Vec<lumenna_records::DeviceView>>,
    /// The landing count of the devices' row in hand when last drawn.
    landing: Cell<u32>,
}

impl Page {
    fn is(&self, id: &str) -> bool {
        self.page.id == id
    }

    fn described(&self, key: &str) -> Option<&Setting> {
        self.described.iter().find(|s| s.key == key)
    }

    /// The fields typed into, applied when left.
    fn typed_fields(&self) -> Vec<(u16, String)> {
        let fields = self.fields.borrow();
        fields
            .iter()
            .filter(|field| matches!(field.kind, SettingKind::Time | SettingKind::Number | SettingKind::Line))
            .map(|field| (field.id, field.key.clone()))
            .collect()
    }

    /// Draws the app's settings on the page, if it exists.
    fn draw(&self, known: &[(Setting, String)], still: bool) {
        let page = self.hwnd.get();
        if page.is_invalid() {
            return;
        }
        for field in self.fields.borrow().iter() {
            let control = dialog::item(page, field.id);
            match field.kind {
                SettingKind::Toggle => {
                    let on = typed(known, &field.key) == "true";
                    if controls::checked(control) != on {
                        controls::check(control, on);
                    }
                }
                SettingKind::Choice => fill_options(control, known, &field.key),
                SettingKind::Time => {
                    controls::write_typed(control, &typed(known, &field.key), still);
                    picker::show(dialog::item(page, field.id + 1), picker::Kind::Time, &value(known, &field.key));
                }
                SettingKind::Number | SettingKind::Line => {
                    controls::write_typed(control, &typed(known, &field.key), still);
                }
                SettingKind::Folder => {
                    let folder = value(known, &field.key);
                    if controls::text(control) != folder {
                        controls::set_text(control, &folder);
                    }
                }
            }
        }
    }

    /// Draws the app's Devices on Devices and Sync, once the page exists: how syncing is going,
    /// and the devices, the row in hand the app's.
    fn draw_devices(&self, app: &App, view: &ViewModel, lines: &Lines) {
        let page = self.hwnd.get();
        if !self.is("devices") {
            return;
        }
        let Some(shown) = view.devices.as_ref().filter(|_| !page.is_invalid()) else { return };
        let status = dialog::item(page, SYNC_STATUS);
        let said = speech::sentence(&shown.status);
        if controls::text(status) != said {
            controls::set_text(status, &said);
        }
        let devices = shown.list.as_ref().map(|list| list.devices.clone()).unwrap_or_default();
        let list = dialog::item(page, DEVICES);
        let texts: Vec<String> = lines.devices.iter().map(|line| line.text.clone()).collect();
        let same = *self.devices.borrow() == devices && usize::try_from(controls::send(list, LB_GETCOUNT, 0, 0)).ok() == Some(texts.len());
        if !same {
            let kept = usize::try_from(controls::send(list, LB_GETCURSEL, 0, 0)).unwrap_or(0);
            controls::send(list, LB_RESETCONTENT, 0, 0);
            for text in &texts {
                let text = HSTRING::from(text.as_str());
                controls::send(list, LB_ADDSTRING, 0, text.as_ptr() as isize);
            }
            controls::send(list, LB_SETCURSEL, kept.min(texts.len().saturating_sub(1)), 0);
            *self.devices.borrow_mut() = devices;
        }
        // Where a change made from the list lands: the same device, or the one in its place.
        let held = &view.devices_held;
        if held.landing != self.landing.replace(held.landing)
            && let Some(at) = held.focus.as_ref().and_then(|key| self.devices.borrow().iter().position(|d| &d.node_id == key))
        {
            controls::send(list, LB_SETCURSEL, at, 0);
            if controls::lost(app.main) || controls::focused().is_invalid() {
                controls::focus(list);
            }
        }
        self.offer_device(page);
    }

    fn chosen_device(&self, page: HWND) -> Option<lumenna_records::DeviceView> {
        let index = usize::try_from(controls::send(dialog::item(page, DEVICES), LB_GETCURSEL, 0, 0)).ok()?;
        self.devices.borrow().get(index).cloned()
    }

    /// The chosen device's action of `kind`, as the app's: a device cannot unpair itself, so
    /// its own row has no Unpair, and the app says why on this page's status line.
    fn act_on_device(&self, page: HWND, kind: ActionKind) {
        let Some(device) = self.chosen_device(page) else { return };
        match actions::of_kind(&device.offered, &[kind]) {
            Some(action) => app::send(ShellEvent::ActOn(ActingOn::Held(Side::Devices, device.node_id), action)),
            None => {
                app::send(ShellEvent::Hold(Side::Devices, device.node_id));
                app::send(ShellEvent::Run(vec![kind]));
            }
        }
    }

    /// Rename and Unpair as the chosen device offers them, named as it names them. The button
    /// with focus stays as it is, since disabling it would send focus nowhere.
    fn offer_device(&self, page: HWND) {
        let offered = self.chosen_device(page).map(|device| device.offered).unwrap_or_default();
        for (id, kind) in [(RENAME, ActionKind::Rename), (UNPAIR, ActionKind::Unpair)] {
            let button = dialog::item(page, id);
            let has = actions::of_kind(&offered, &[kind]).is_some();
            if has || controls::focused() != button {
                controls::enable(button, has);
            }
        }
    }

    /// The shortcuts from anywhere as this PC has them, on General.
    fn show_shortcuts(&self, page: HWND) {
        let words = hotkeys::hotkey_words();
        for (index, kind) in HotKind::ALL.into_iter().enumerate() {
            let index = index as u16;
            controls::set_text(dialog::item(page, SHORTCUT + index), &shortcuts::describe(kind));
            let on = shortcuts::current(kind).is_some();
            let toggle = dialog::item(page, TOGGLE + index);
            let title = if on { &words.turn_off } else { &words.turn_on };
            controls::set_text(toggle, &title.replace('&', "&&"));
            a11y::set_name(toggle, &format!("{title} {}", kind.name(false)));
        }
    }

    fn change_shortcut(&self, app: &App, page: HWND, kind: HotKind) {
        // While the keys are chosen, pressing the current ones must reach the field, not
        // summon the window.
        shortcuts::unregister_all(app.main);
        let recorder = Recorder { kind, chosen: RefCell::new(None) };
        dialog::run(Some(sheet(page)), &recorder);
        let chosen = recorder.chosen.into_inner();
        shortcuts::register_all(app.main);
        if let Some(keys) = chosen {
            match shortcuts::change(app.main, kind, Some(keys)) {
                Ok(said) => say(page, &said),
                Err(why) => prompts::fail(sheet(page), &why),
            }
            self.show_shortcuts(page);
        }
    }

    fn toggle_shortcut(&self, app: &App, page: HWND, kind: HotKind) {
        let to = if shortcuts::current(kind).is_some() { None } else { Some(kind.standard()) };
        match shortcuts::change(app.main, kind, to) {
            Ok(said) => say(page, &said),
            Err(why) => prompts::fail(sheet(page), &why),
        }
        self.show_shortcuts(page);
    }

    /// One of the page's own buttons (`SettingsButton::id`).
    fn press(&self, app: &App, page: HWND, id: &str) {
        match id {
            "sync-now" => app::send(ShellEvent::Devices(DevicesEvent::SyncNow)),
            "pair" => pairing::run(app, sheet(page)),
            // Into this device's backups, as the automatic ones are; where is said on this page,
            // the dialog in front, once it is taken.
            "back-up" => app.back_up(),
            "restore-backup" => import(sheet(page), &self.button_title(id), &BACKUPS),
            "import" => import(sheet(page), &self.button_title(id), &IMPORTABLE),
            _ => {}
        }
    }

    /// A button's name, as the core words it: a file dialog it opens is titled so.
    fn button_title(&self, id: &str) -> String {
        self.page.buttons.iter().find(|b| b.id == id).map(|b| b.title.clone()).unwrap_or_default()
    }
}

impl Dialog for Page {
    fn template(&self) -> Template {
        let mut template = page_template(&self.page.title);
        let mut taken: Vec<char> = Vec::new();
        let mut fields = Vec::new();
        let mut y: i16 = 7;
        // A short control waits for another to share its row.
        let mut half_open = false;
        let advance = |y: &mut i16, half_open: &mut bool, tall: i16| {
            *half_open = false;
            *y += tall;
        };

        // Devices and Sync: how syncing is going and the devices, before its buttons.
        if self.is("devices") {
            let list = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_WANTKEYBOARDINPUT) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
            let status = READ_ONLY | ES_MULTILINE as u32 | WS_VSCROLL.0;
            template = template
                .item(Class::Static, &format!("{}:", keyed(&self.words.sync_status, &mut taken)), u16::MAX, 0, LEFT, y, INNER, 9)
                .item(Class::Edit, "", SYNC_STATUS, status, LEFT, y + 10, INNER, 28)
                .item(Class::Static, &format!("{}:", keyed(&self.words.devices, &mut taken)), u16::MAX, 0, LEFT, y + 43, INNER, 9)
                .item(Class::ListBox, "", DEVICES, list, LEFT, y + 53, INNER, 80);
            y += 139;
        }
        // Export and Import: each export first.
        if self.is("export") {
            for (index, export) in devices::EXPORTS.iter().enumerate() {
                let label = keyed(export.label, &mut taken);
                template = template.item(Class::Button, &label, EXPORT + index as u16, BUTTON, LEFT, y, 180, 14);
                y += 18;
            }
        }

        for (index, key) in self.page.keys.iter().enumerate() {
            let Some(setting) = self.described(key) else { continue };
            let id = FIELDS + 2 * index as u16;
            fields.push(Field { id, key: key.clone(), kind: setting.kind });
            let label = |taken: &mut Vec<char>| format!("{}:", keyed(&setting.title, taken));
            match setting.kind {
                SettingKind::Toggle => {
                    if half_open {
                        advance(&mut y, &mut half_open, 28);
                    }
                    template = template.item(Class::Button, &keyed(&setting.title, &mut taken), id, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, LEFT, y, INNER, 10);
                    y += 12;
                    // What it does, under it, as well as said with it.
                    if !setting.hint.is_empty() {
                        let tall = 9 * lines_of(&setting.hint);
                        template = template.item(Class::Static, &setting.hint, u16::MAX, SS_NOPREFIX.0, LEFT + 12, y, INNER - 12, tall);
                        y += tall;
                    }
                    y += 4;
                }
                SettingKind::Line | SettingKind::Folder => {
                    if half_open {
                        advance(&mut y, &mut half_open, 28);
                    }
                    template = template.item(Class::Static, &label(&mut taken), u16::MAX, 0, LEFT, y, INNER, 9);
                    if setting.kind == SettingKind::Folder {
                        template = template
                            .item(Class::Edit, "", id, READ_ONLY, LEFT, y + 10, INNER - 56, 14)
                            .item(Class::Button, &format!("{}...", keyed(&self.words.choose, &mut taken)), id + 1, BUTTON, LEFT + INNER - 52, y + 10, 52, 14);
                    } else {
                        template = template.item(Class::Edit, "", id, FIELD, LEFT, y + 10, INNER, 14);
                    }
                    y += 28;
                }
                SettingKind::Choice | SettingKind::Time | SettingKind::Number => {
                    let x = if half_open { RIGHT } else { LEFT };
                    template = template.item(Class::Static, &label(&mut taken), u16::MAX, 0, x, y, HALF, 9);
                    template = match setting.kind {
                        SettingKind::Choice => template.item(Class::ComboBox, "", id, LIST, x, y + 10, HALF, 120),
                        SettingKind::Number => template.item(Class::Edit, "", id, FIELD | ES_NUMBER as u32, x, y + 10, 60, 14),
                        _ => template
                            .item(Class::Edit, "", id, FIELD, x, y + 10, 52, 14)
                            .named(picker::CLASS, "", id + 1, picker::Kind::Time.style(), x + 56, y + 10, HALF - 56, 14),
                    };
                    if half_open {
                        advance(&mut y, &mut half_open, 28);
                    } else {
                        half_open = true;
                    }
                }
            }
        }
        if half_open {
            y += 28;
        }

        // The page's own buttons, in a row, Rename and Unpair after them on Devices and Sync.
        let mut buttons = Vec::new();
        let mut x = LEFT;
        let mut row: Vec<(u16, String, i16)> = Vec::new();
        for (index, button) in self.page.buttons.iter().enumerate() {
            let title = format!("{}{}", keyed(&button.title, &mut taken), if button.asks { "..." } else { "" });
            row.push((BUTTONS + index as u16, title, 0));
            buttons.push((BUTTONS + index as u16, button.id.clone()));
        }
        if self.is("devices") {
            // Named as a paired device's own actions are, which they run.
            let offered = lumenna_records::actions::device("", "", false);
            let title = |kind| offered.iter().find(|a| a.kind == kind).map(|a| a.title.clone()).unwrap_or_default();
            let (rename, unpair) = (title(ActionKind::Rename), title(ActionKind::Unpair));
            row.push((RENAME, format!("{}...", keyed(&rename, &mut taken)), 0));
            row.push((UNPAIR, format!("{}...", keyed(&unpair, &mut taken)), 0));
        }
        if !row.is_empty() {
            let each = ((INNER - 4 * (row.len() as i16 - 1)) / row.len() as i16).min(116);
            for (id, title, _) in row {
                template = template.item(Class::Button, &title, id, BUTTON, x, y, each, 14);
                x += each + 4;
            }
            y += 20;
        }
        *self.buttons.borrow_mut() = buttons;

        // Export and Import: the store's own actions, after the rest, their caption under them.
        if self.is("export") && !self.offered.is_empty() {
            for (index, action) in self.offered.iter().enumerate() {
                let label = keyed(&actions::label(action), &mut taken);
                template = template.item(Class::Button, &label, OFFERED + index as u16, BUTTON, LEFT, y, 180, 14);
                y += 18;
            }
            let tall = 9 * lines_of(&self.offered_caption);
            template = template.item(Class::Static, &self.offered_caption, u16::MAX, SS_NOPREFIX.0, LEFT, y - 2, INNER, tall);
            y += tall + 2;
        }

        // General: the shortcuts from anywhere, this PC's.
        if self.is("general") {
            let words = hotkeys::hotkey_words();
            template = template.item(Class::Button, &words.heading, u16::MAX, BS_GROUPBOX as u32, LEFT, y, INNER, 92);
            for (index, kind) in HotKind::ALL.into_iter().enumerate() {
                let top = y + 12 + index as i16 * 38;
                let index = index as u16;
                template = template
                    .item(Class::Static, &format!("{}:", kind.name(false)), u16::MAX, SS_NOPREFIX.0, LEFT + 7, top, INNER - 14, 9)
                    .item(Class::Edit, "", SHORTCUT + index, READ_ONLY, LEFT + 7, top + 10, 110, 14)
                    .item(Class::Button, &format!("{}...", words.change), CHANGE + index, BUTTON, LEFT + 121, top + 10, 52, 14)
                    .item(Class::Button, &words.turn_off, TOGGLE + index, BUTTON, LEFT + 177, top + 10, 54, 14);
            }
            y += 96;
            let tall = 9 * lines_of(&words.caption);
            template = template.item(Class::Static, &words.caption, u16::MAX, SS_NOPREFIX.0, LEFT, y, INNER, tall);
            y += tall + 4;
        }

        *self.fields.borrow_mut() = fields;
        // What the page says under all of it.
        if !self.page.caption.is_empty() {
            template = template.item(Class::Static, &self.page.caption, u16::MAX, SS_NOPREFIX.0, LEFT, y, INNER, 9 * lines_of(&self.page.caption));
        }
        template
    }

    fn init(&self, page: HWND) -> bool {
        self.hwnd.set(page);
        a11y::make_live(dialog::item(page, STATUS));
        for field in self.fields.borrow().iter() {
            let Some(setting) = self.described(&field.key) else { continue };
            // What it takes, said with it.
            if !setting.hint.is_empty() {
                a11y::set_description(dialog::item(page, field.id), &setting.hint);
            }
            if field.kind == SettingKind::Time {
                picker::ready(dialog::item(page, field.id + 1), picker::Kind::Time, &setting.title, &picker::Kind::Time.choose());
            }
        }
        // Opening at sign-in as Windows has it now, until the app's own arrives.
        if let Some(field) = self.fields.borrow().iter().find(|f| f.key == lumenna_records::background::KEY) {
            let at_sign_in = dialog::item(page, field.id);
            app::with(|app| controls::check(at_sign_in, opens_at_sign_in(&app.directory)));
        }
        if self.is("general") {
            for (index, kind) in HotKind::ALL.into_iter().enumerate() {
                a11y::set_name(dialog::item(page, CHANGE + index as u16), &hotkeys::change_shortcut_for(&kind.name(false)));
            }
            self.show_shortcuts(page);
        }
        if self.is("devices") {
            a11y::set_description(dialog::item(page, DEVICES), "Delete unpairs the selected device, and F2 renames it.");
            self.offer_device(page);
            // The devices and how syncing is going, as the app finds them.
            app::send(ShellEvent::Devices(DevicesEvent::Show));
        }
        if self.is("export") {
            // The caption is said with each button, as a screen reader reads a button alone.
            for index in 0..self.offered.len() {
                a11y::set_description(dialog::item(page, OFFERED + index as u16), &self.offered_caption);
            }
        }
        // As described, until the app's own settings arrive.
        let known: Vec<(Setting, String)> = self.described.iter().map(|s| (s.clone(), s.value.clone())).collect();
        self.draw(&known, true);
        false
    }

    fn command(&self, page: HWND, id: u16, code: u16) -> Option<isize> {
        let code = u32::from(code);
        // A setting's own control.
        let field = self.fields.borrow().iter().find(|f| f.id == id || f.id + 1 == id).map(|f| (f.id, f.key.clone(), f.kind));
        if let Some((field, key, kind)) = field {
            let control = dialog::item(page, field);
            match (kind, code) {
                (SettingKind::Toggle, BN_CLICKED) => set(&key, if controls::checked(control) { "true" } else { "false" }),
                (SettingKind::Choice, CBN_SELCHANGE) => {
                    if let Some(chosen) = chosen_option(control, &self.described, &key) {
                        set(&key, &chosen);
                    }
                }
                (SettingKind::Folder, BN_CLICKED) if id == field + 1 => {
                    let start = std::path::PathBuf::from(controls::text(control));
                    if let Some(folder) = system::choose_folder(sheet(page), &self.words.choose_folder, Some(&start)) {
                        set(&key, &folder.display().to_string());
                    }
                }
                (SettingKind::Time | SettingKind::Number | SettingKind::Line, EN_CHANGE) if id == field && controls::focused() == control => {
                    app::send(ShellEvent::Settings(SettingsEvent::Type { key, value: controls::text(control) }));
                }
                (SettingKind::Time | SettingKind::Number | SettingKind::Line, EN_KILLFOCUS) if id == field => commit(page, &[(field, key)]),
                _ => {}
            }
            return None;
        }
        if id == DEVICES && code == LBN_SELCHANGE {
            if let Some(device) = self.chosen_device(page) {
                app::send(ShellEvent::Hold(Side::Devices, device.node_id));
            }
            self.offer_device(page);
            return None;
        }
        if code != BN_CLICKED {
            return None;
        }
        let button = self.buttons.borrow().iter().find(|(b, _)| *b == id).map(|(_, which)| which.clone());
        let export = id.checked_sub(EXPORT).and_then(|i| devices::EXPORTS.get(usize::from(i)));
        let offered = id.checked_sub(OFFERED).and_then(|i| self.offered.get(usize::from(i)));
        app::with(|app| {
            if let Some(button) = &button {
                self.press(app, page, button);
            } else if id == RENAME {
                self.act_on_device(page, ActionKind::Rename);
            } else if id == UNPAIR {
                self.act_on_device(page, ActionKind::Unpair);
            } else if (CHANGE..CHANGE + 2).contains(&id) {
                self.change_shortcut(app, page, HotKind::ALL[usize::from(id - CHANGE)]);
            } else if (TOGGLE..TOGGLE + 2).contains(&id) {
                self.toggle_shortcut(app, page, HotKind::ALL[usize::from(id - TOGGLE)]);
            } else if let Some(export) = export {
                // The app makes the export, which is then saved where the person chooses.
                let format: ExportFormat = export.format;
                app::send(ShellEvent::Settings(SettingsEvent::Export(format)));
            } else if let Some(action) = offered {
                // The app asks its question as it asks a row's, over the sheet.
                app::send(ShellEvent::ActOn(ActingOn::Settings, action.clone()));
            }
        });
        None
    }

    fn message(&self, page: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        // A time picked is its field's, typed there as the arrows change it and applied when the
        // clock is left, as a field is: applied at each arrow, every step would be said.
        let clock_of = |picker: u16| {
            self.fields.borrow().iter().find(|f| f.kind == SettingKind::Time && f.id + 1 == picker).map(|f| (f.id, f.key.clone()))
        };
        if let Some(id) = picker::changed(message, lparam)
            && let Some((field, key)) = clock_of(id)
        {
            if let Some(time) = picker::read(dialog::item(page, id), picker::Kind::Time) {
                controls::set_text(dialog::item(page, field), &time);
                app::send(ShellEvent::Settings(SettingsEvent::Type { key, value: time }));
            }
            return Some(0);
        }
        let code = notification(message, lparam);
        if code == Some(NM_KILLFOCUS)
            && let Some((field, key)) = u16::try_from(unsafe { &*(lparam.0 as *const NMHDR) }.idFrom).ok().and_then(clock_of)
        {
            commit(page, &[(field, key)]);
        }
        if code == Some(PSN_SETACTIVE) {
            fronted(page);
            return None;
        }
        if finishing(code) {
            commit(page, &self.typed_fields());
            return Some(0);
        }
        if self.is("devices") {
            match message {
                // F2 renames, as in Explorer.
                WM_VKEYTOITEM if controls::low_word(wparam.0) == VK_F2.0 => {
                    self.act_on_device(page, ActionKind::Rename);
                    return Some(-2);
                }
                // What Delete runs is the core's (`keys::runs`): here, Unpair.
                WM_VKEYTOITEM if controls::low_word(wparam.0) == VK_DELETE.0 => {
                    if let Some(device) = self.chosen_device(page) {
                        app::send(ShellEvent::Hold(Side::Devices, device.node_id));
                        app::send(ShellEvent::Key("delete".into()));
                    }
                    return Some(-2);
                }
                WM_VKEYTOITEM => return Some(-1),
                _ => {}
            }
        }
        None
    }
}

/// Choosing new keys for a shortcut, with Windows' own hotkey control; its words the core's.
struct Recorder {
    kind: HotKind,
    chosen: RefCell<Option<HotKeys>>,
}

const KEYS: u16 = 100;

impl Dialog for Recorder {
    fn template(&self) -> Template {
        let words = hotkeys::hotkey_words();
        let message = hotkeys::press_keys_for(&self.kind.name(false));
        Template::new(&words.dialog, 240, 92)
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
                let words = hotkeys::hotkey_words();
                if key == 0 {
                    prompts::fail(hwnd, &words.press_first);
                    return None;
                }
                let keys = HotKeys {
                    control: flags & HOTKEYF_CONTROL != 0,
                    alt: flags & HOTKEYF_ALT != 0,
                    shift: flags & HOTKEYF_SHIFT != 0,
                    windows: false,
                    key: key as u16,
                };
                if let Some(warning) = keys.warning() {
                    let title = hotkeys::use_keys(&keys.describe());
                    if prompts::choose(hwnd, &title, warning, &[words.use_anyway.as_str()], true) != Some(0) {
                        return None;
                    }
                }
                *self.chosen.borrow_mut() = Some(keys);
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
