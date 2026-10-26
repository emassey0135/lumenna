//! Settings: a window of tabs, opened with Ctrl+Comma — General, Planning, Devices, Backups,
//! and Export and Import, as on the Mac and Windows. The app's settings
//! (`ViewModel::settings`) and Devices page (`ViewModel::devices`), drawn.
//!
//! Every page applies a change as it is made; a text field when it is left or Enter is pressed
//! in it. What each setting is called, its control and its options are the core's; what a
//! field holds while typed, and why a line was refused, are the app's: the line stays as typed
//! beside why. What the app says while the window is in front is said on the page's own status
//! line: the main window's is behind this one.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_app::devices::marked;
use lumenna_app::{ActingOn, Asks, DevicesEvent, Offered, Side, SettingsEvent, SettingsView};
use lumenna_ffi::ShellEvent;
use lumenna_records::{ActionKind, ExportFormat, Setting, SettingKind};

use crate::prompts;
use crate::tree::{Item, Landing, Tree};
use crate::window::{App, spawn};

/// The pages opened directly, by their place in the window (Planning is 1, Backups 3).
#[derive(Clone, Copy)]
pub enum Page {
    General = 0,
    Devices = 2,
    Export = 4,
}

const PAGES: [&str; 5] = ["General", "Planning", "Devices", "Backups", "Export and Import"];

/// Each page's settings, in order, with the mnemonic each is marked with: the one thing
/// about a setting that is this app's.
/// Opening at sign-in (`background`) is offered by the app once this desktop has said it can.
const GENERAL: [(&str, char); 2] = [("background", 'O'), ("reminders", 'R')];
const PLANNING: [(&str, char); 10] = [
    ("cascade-complete-subtasks", 'C'),
    ("day-start", 's'),
    ("day-end", 'e'),
    ("all-day-reminder-hour", 'A'),
    ("default-task-reminders", 'R'),
    ("default-block-reminders", 'b'),
    ("verbosity", 'm'),
    ("week-start", 'W'),
    ("urgency-weights", 'u'),
    ("task-order", 'k'),
];
const BACKUPS: [(&str, char); 2] = [("backup-every", 'A'), ("backup-keep", 'k')];

/// The pages and the row of tabs over them.
///
/// A stack switcher, not a notebook: a notebook keeps keyboard focus itself, so Orca reached
/// its tab bar as an unnamed "grouping". A stack switcher's tabs take focus and are read as
/// tabs, but each is a Tab stop of its own; so only the current one takes Tab, and the arrows
/// move to the next and open it, as a tab bar does.
struct Tabs {
    stack: gtk::Stack,
    switcher: gtk::StackSwitcher,
}

impl Tabs {
    fn buttons(&self) -> Vec<gtk::Widget> {
        std::iter::successors(self.switcher.first_child(), |button| button.next_sibling()).collect()
    }

    fn current(&self) -> usize {
        let name = self.stack.visible_child_name();
        PAGES.iter().position(|page| name.as_deref() == Some(*page)).unwrap_or(0)
    }

    /// Only the current tab is a Tab stop.
    fn settle(&self) {
        let current = self.current();
        for (index, button) in self.buttons().iter().enumerate() {
            button.set_focusable(index == current);
        }
    }

    /// Opens a page, and puts focus on its tab when `focus`.
    fn go(&self, index: usize, focus: bool) {
        let index = index.min(PAGES.len() - 1);
        self.stack.set_visible_child_name(PAGES[index]);
        self.settle();
        if focus && let Some(button) = self.buttons().get(index) {
            button.grab_focus();
        }
    }

    /// Opens the page `step` along, wrapping round.
    fn step(&self, step: isize, focus: bool) {
        let count = PAGES.len() as isize;
        let next = (self.current() as isize + step).rem_euclid(count) as usize;
        self.go(next, focus);
    }
}

/// A setting's control.
enum Control {
    Toggle(gtk::CheckButton),
    Choice(gtk::DropDown, RefCell<Vec<String>>),
    Line(gtk::Entry, gtk::Label),
}

/// The window, while it is open.
struct Window {
    window: gtk::Window,
    tabs: Rc<Tabs>,
    /// Each page's status line, by its place.
    statuses: Vec<gtk::Label>,
    /// Where each page's settings go once the app has said what they are.
    slots: Vec<gtk::Box>,
    /// Each setting's control, once made.
    controls: RefCell<HashMap<String, Control>>,
    /// Whether the controls are being filled from the app, not changed by the person.
    filling: Rc<Cell<bool>>,
    /// The refusal last told for each line, so each is told once.
    told: RefCell<HashMap<String, String>>,
    folder: gtk::Entry,
    devices: Rc<DevicesPage>,
    /// Where what is done to the store as a whole goes, a button each with the core's caption
    /// under them, and what they are.
    store: gtk::Box,
    store_offered: RefCell<(Vec<Offered>, String)>,
    /// A file chosen to save the next one the app makes into: an export.
    saving: RefCell<Option<PathBuf>>,
}

thread_local! {
    static OPEN: RefCell<Option<Rc<Window>>> = const { RefCell::new(None) };
}

fn open() -> Option<Rc<Window>> {
    OPEN.with_borrow(Clone::clone)
}

fn send(event: ShellEvent) {
    if let Some(app) = crate::window::app() {
        app.model.send(event);
    }
}

/// The settings window, when it is in front: where the app's questions are asked from.
pub fn in_front() -> Option<gtk::Window> {
    open().map(|open| open.window.clone()).filter(GtkWindowExt::is_active)
}

/// Says `text` on the page shown, if the window is in front: whether it was.
pub fn say(text: &str) -> bool {
    let Some(open) = open().filter(|open| open.window.is_active()) else { return false };
    if let Some(status) = open.statuses.get(open.tabs.current()) {
        status.set_label(text);
        crate::window::announce(status, text);
    }
    true
}

/// A file the app made for the person to keep: an export, saved where they chose.
pub fn save(app: &App, file: &lumenna_app::SaveFile) {
    let Some(path) = open().and_then(|open| open.saving.take()) else { return };
    if let Err(error) = std::fs::write(&path, &file.bytes) {
        prompts::fail(&app.window, &format!("The export could not be saved. {error}"));
    }
}

/// Opens Settings on a page.
pub fn show(app: &Rc<App>, page: Page) {
    if let Some(open) = open() {
        open.tabs.go(page as usize, false);
        open.window.present();
        return;
    }
    let stack = gtk::Stack::new();
    let mut statuses = Vec::new();
    let mut slots = Vec::new();
    let pages = [general(), planning(), devices_page(), backups(app), export(app)];
    let mut folder = None;
    let mut devices = None;
    let mut store = None;
    for (title, built) in PAGES.iter().zip(pages) {
        stack.add_titled(&built.widget, Some(title), title);
        statuses.push(built.status);
        slots.push(built.slot);
        folder = folder.or(built.folder);
        devices = devices.or(built.devices);
        store = store.or(built.store);
    }
    let switcher = gtk::StackSwitcher::builder().stack(&stack).halign(gtk::Align::Center).margin_top(6).build();
    let tabs = Rc::new(Tabs { stack: stack.clone(), switcher: switcher.clone() });
    tabs.go(page as usize, false);
    {
        // A click on a tab opens its page too; the Devices page asks the app to list them.
        let tabs = Rc::downgrade(&tabs);
        stack.connect_visible_child_name_notify(move |stack| {
            if let Some(tabs) = tabs.upgrade() {
                tabs.settle();
            }
            if stack.visible_child_name().as_deref() == Some(PAGES[Page::Devices as usize]) {
                send(ShellEvent::Devices(DevicesEvent::Show));
            }
        });
    }
    let arrows = gtk::EventControllerKey::new();
    {
        let tabs = Rc::downgrade(&tabs);
        arrows.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(tabs) = tabs.upgrade() else { return glib::Propagation::Proceed };
            if !modifiers.is_empty() {
                return glib::Propagation::Proceed;
            }
            match key {
                gdk::Key::Right | gdk::Key::KP_Right => tabs.step(1, true),
                gdk::Key::Left | gdk::Key::KP_Left => tabs.step(-1, true),
                gdk::Key::Home | gdk::Key::KP_Home => tabs.go(0, true),
                gdk::Key::End | gdk::Key::KP_End => tabs.go(PAGES.len() - 1, true),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
    }
    switcher.add_controller(arrows);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&switcher);
    body.append(&stack);
    let window = gtk::Window::builder()
        .title("Preferences")
        .transient_for(&app.window)
        .destroy_with_parent(true)
        .default_width(560)
        .default_height(480)
        .child(&body)
        .build();
    // From anywhere in the window: Escape closes it; Control with Tab or Page Up and Down
    // goes to the next or previous page, onto its tab.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let (closing, tabs) = (window.downgrade(), Rc::downgrade(&tabs));
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            let step = match key {
                gdk::Key::Escape if modifiers.is_empty() => {
                    if let Some(window) = closing.upgrade() {
                        window.close();
                    }
                    return glib::Propagation::Stop;
                }
                gdk::Key::Tab | gdk::Key::KP_Tab if control && !shift => 1,
                gdk::Key::Tab | gdk::Key::KP_Tab | gdk::Key::ISO_Left_Tab if control && shift => -1,
                gdk::Key::Page_Down | gdk::Key::KP_Page_Down if control => 1,
                gdk::Key::Page_Up | gdk::Key::KP_Page_Up if control => -1,
                _ => return glib::Propagation::Proceed,
            };
            if let Some(tabs) = tabs.upgrade() {
                tabs.step(step, true);
            }
            glib::Propagation::Stop
        });
    }
    window.add_controller(keys);
    window.connect_close_request(|_| {
        OPEN.set(None);
        send(ShellEvent::Settings(SettingsEvent::Close));
        glib::Propagation::Proceed
    });
    let open = Rc::new(Window {
        window: window.clone(),
        tabs: Rc::clone(&tabs),
        statuses,
        slots,
        controls: RefCell::new(HashMap::new()),
        filling: Rc::new(Cell::new(false)),
        told: RefCell::new(HashMap::new()),
        folder: folder.unwrap_or_default(),
        devices: devices.unwrap_or_else(|| Rc::new(DevicesPage::empty())),
        store: store.unwrap_or_default(),
        store_offered: RefCell::new((Vec::new(), String::new())),
        saving: RefCell::new(None),
    });
    OPEN.set(Some(open));
    app.model.send(ShellEvent::Settings(SettingsEvent::Open));
    if matches!(page, Page::Devices) {
        app.model.send(ShellEvent::Devices(DevicesEvent::Show));
    }
    window.present();
    // Opening on the tab says where Settings opened.
    let tabs = Rc::downgrade(&tabs);
    glib::idle_add_local_once(move || {
        if let Some(tabs) = tabs.upgrade()
            && let Some(button) = tabs.buttons().get(tabs.current())
        {
            prompts::focus_on(button);
        }
    });
}

/// Draws what the app says of the settings and devices, while the window is open.
pub fn draw(app: &App) {
    let Some(open) = open() else { return };
    let view = app.model.view();
    if let Some(settings) = view.settings.as_ref().filter(|settings| !settings.settings.is_empty()) {
        open.fill(app, settings);
    }
    if let Some(settings) = &view.settings {
        open.offer(&settings.offered, &settings.offered_caption);
    }
    if let Some(devices) = &view.devices {
        open.devices.draw(app, devices, &view.devices_held);
        crate::pairing::draw(app, devices.pairing.as_ref(), &devices.words, &open.window);
    }
}

impl Window {
    /// Draws what the store as a whole offers (Erase Everything Deleted) as a button each, in
    /// its title, destructive ones drawn as such, and the core's caption under them; each is
    /// handed back to the app, which asks its question over this window.
    fn offer(&self, offered: &[Offered], caption: &str) {
        {
            let drawn = self.store_offered.borrow();
            if drawn.0 == offered && drawn.1 == caption {
                return;
            }
        }
        while let Some(child) = self.store.first_child() {
            self.store.remove(&child);
        }
        for action in offered {
            let title = if action.asks == Asks::Nothing { action.title.clone() } else { format!("{}…", action.title) };
            let mnemonic = title.chars().next().unwrap_or_default();
            let button = gtk::Button::with_mnemonic(&marked(&title, mnemonic, '_'));
            button.set_halign(gtk::Align::Start);
            if action.destructive {
                button.add_css_class("destructive-action");
            }
            // Orca says the caption with the button, as it says a field's hint; a label beside it
            // is read only by moving through the page's text.
            describe(&button, caption);
            let action = action.clone();
            button.connect_clicked(move |_| send(ShellEvent::ActOn(ActingOn::Settings, action.clone())));
            self.store.append(&button);
        }
        if !offered.is_empty() && !caption.is_empty() {
            // Libadwaita's caption: smaller, never dimmed, so it keeps its contrast.
            let label = gtk::Label::builder().label(caption).wrap(true).xalign(0.0).css_classes(["caption"]).build();
            self.store.append(&label);
        }
        *self.store_offered.borrow_mut() = (offered.to_vec(), caption.to_owned());
    }

    /// Makes each page's controls the first time the app says what the settings are, then
    /// shows what each holds — except a line focus is in, which holds the newest.
    fn fill(&self, app: &App, view: &SettingsView) {
        let find = |key: &str| view.settings.iter().find(|shown| shown.setting.key == key);
        if self.controls.borrow().is_empty() {
            for (page, keys) in [(0, &GENERAL[..]), (1, &PLANNING[..]), (3, &BACKUPS[..])] {
                for (key, mnemonic) in keys {
                    if let Some(shown) = find(key) {
                        let control = make(&self.slots[page], &shown.setting, *mnemonic, &self.filling);
                        self.controls.borrow_mut().insert((*key).to_owned(), control);
                    }
                }
            }
        }
        self.filling.set(true);
        for (key, control) in self.controls.borrow().iter() {
            let Some(shown) = find(key) else { continue };
            match control {
                Control::Toggle(check) => {
                    if check.is_active() != (shown.typed == "true") {
                        check.set_active(shown.typed == "true");
                    }
                }
                Control::Choice(dropdown, values) => {
                    // A value set elsewhere — `lum config set backup-every 3d` — is shown as it is.
                    let mut known = values.borrow_mut();
                    if !shown.typed.is_empty() && !known.contains(&shown.typed) {
                        known.push(shown.typed.clone());
                        if let Some(list) = dropdown.model().and_downcast::<gtk::StringList>() {
                            list.append(&shown.typed);
                        }
                    }
                    if let Some(position) = known.iter().position(|value| *value == shown.typed)
                        && dropdown.selected() != position as u32
                    {
                        dropdown.set_selected(position as u32);
                    }
                }
                Control::Line(entry, problem) => {
                    // A time is shown in this desktop's clock until it is typed over.
                    let text = if shown.setting.kind == SettingKind::Time && shown.typed == shown.setting.value && !shown.typed.is_empty() {
                        lumenna_app::speech::Clock::time(&app.clock, &shown.typed)
                    } else {
                        shown.typed.clone()
                    };
                    let typing = entry.root().and_then(|root| root.focus()).is_some_and(|focus| focus.is_ancestor(entry));
                    if entry.text() != text && !typing {
                        entry.set_text(&text);
                    }
                    problem.set_label(shown.problem.as_deref().unwrap_or_default());
                    problem.set_visible(shown.problem.is_some());
                    // A line refused is told once, and stays as typed beside why.
                    let told = self.told.borrow().get(key).cloned();
                    match &shown.problem {
                        Some(why) if told.as_ref() != Some(why) => {
                            self.told.borrow_mut().insert(key.clone(), why.clone());
                            prompts::fail(&self.window, why);
                        }
                        Some(_) => {}
                        None => {
                            self.told.borrow_mut().remove(key);
                        }
                    }
                }
            }
        }
        if let Some(folder) = find("backup-dir")
            && self.folder.text() != folder.typed
        {
            self.folder.set_text(&folder.typed);
        }
        self.filling.set(false);
    }
}

/// A setting's control, by its kind, with its name above it — `mnemonic` marked in it — and
/// its hint as its description, put in `slot`.
fn make(slot: &gtk::Box, setting: &Setting, mnemonic: char, filling: &Rc<Cell<bool>>) -> Control {
    let label = marked(&setting.title, mnemonic, '_');
    let key = setting.key.clone();
    let field = |widget: &gtk::Widget| {
        let named = gtk::Label::builder().label(&label).use_underline(true).xalign(0.0).margin_top(6).build();
        named.set_mnemonic_widget(Some(widget));
        slot.append(&named);
        slot.append(widget);
    };
    match setting.kind {
        SettingKind::Toggle => {
            let check = prompts::check(&label);
            describe(&check, &setting.hint);
            slot.append(&check);
            let filling = Rc::clone(filling);
            check.connect_toggled(move |check| {
                if !filling.get() {
                    send(ShellEvent::Settings(SettingsEvent::Set { key: key.clone(), value: check.is_active().to_string() }));
                }
            });
            Control::Toggle(check)
        }
        SettingKind::Choice => {
            let titles: Vec<&str> = setting.options.iter().map(|option| option.title.as_str()).collect();
            let dropdown = gtk::DropDown::from_strings(&titles);
            describe(&dropdown, &setting.hint);
            field(dropdown.upcast_ref());
            let values = RefCell::new(setting.options.iter().map(|option| option.id.clone()).collect::<Vec<_>>());
            let control = Control::Choice(dropdown.clone(), values);
            let filling = Rc::clone(filling);
            dropdown.connect_selected_notify(move |dropdown| {
                if filling.get() {
                    return;
                }
                let value = dropdown
                    .model()
                    .and_downcast::<gtk::StringList>()
                    .and_then(|_| open())
                    .and_then(|open| match open.controls.borrow().get(&key) {
                        Some(Control::Choice(_, values)) => values.borrow().get(dropdown.selected() as usize).cloned(),
                        _ => None,
                    });
                if let Some(value) = value {
                    send(ShellEvent::Settings(SettingsEvent::Set { key: key.clone(), value }));
                }
            });
            control
        }
        SettingKind::Time | SettingKind::Number | SettingKind::Folder | SettingKind::Line => {
            let entry = gtk::Entry::new();
            describe(&entry, &setting.hint);
            field(entry.upcast_ref());
            let problem = gtk::Label::builder().wrap(true).xalign(0.0).visible(false).build();
            slot.append(&problem);
            entry.update_relation(&[gtk::accessible::Relation::DescribedBy(&[problem.upcast_ref()])]);
            {
                // Typed, the line is the app's to keep until it is applied.
                let (filling, key) = (Rc::clone(filling), key.clone());
                entry.connect_changed(move |entry| {
                    if !filling.get() {
                        send(ShellEvent::Settings(SettingsEvent::Type { key: key.clone(), value: entry.text().to_string() }));
                    }
                });
            }
            // Applied when it is left or Enter is pressed in it, if it changed.
            let commit = {
                let key = key.clone();
                move |entry: &gtk::Entry| {
                    let Some(app) = crate::window::app() else { return };
                    let typed = entry.text().trim().to_owned();
                    let now = app
                        .model
                        .view()
                        .settings
                        .as_ref()
                        .and_then(|settings| settings.settings.iter().find(|shown| shown.setting.key == key).map(|shown| shown.setting.clone()));
                    let Some(now) = now else { return };
                    let shown = if now.kind == SettingKind::Time && !now.value.is_empty() {
                        lumenna_app::speech::Clock::time(&app.clock, &now.value)
                    } else {
                        now.value.clone()
                    };
                    if typed != shown && typed != now.value {
                        app.model.send(ShellEvent::Settings(SettingsEvent::Set { key: key.clone(), value: typed }));
                    }
                }
            };
            let leave = gtk::EventControllerFocus::new();
            {
                let entry = entry.clone();
                let commit = commit.clone();
                leave.connect_leave(move |_| commit(&entry));
            }
            entry.add_controller(leave);
            // A time has GTK's own clock beneath its line, applied, as the line is, once left.
            if setting.kind == SettingKind::Time {
                let clock = crate::pickers::clock(&entry, Some(&setting.value), Some(&setting.title));
                let leave = gtk::EventControllerFocus::new();
                {
                    let (entry, commit) = (entry.clone(), commit.clone());
                    leave.connect_leave(move |_| commit(&entry));
                }
                clock.add_controller(leave);
                slot.append(&clock);
            }
            entry.connect_activate(commit);
            Control::Line(entry, problem)
        }
    }
}

fn describe(widget: &impl IsA<gtk::Accessible>, hint: &str) {
    if !hint.is_empty() {
        widget.update_property(&[gtk::accessible::Property::Description(hint)]);
    }
}

/// The window a page is in, for the dialogs it opens.
fn window_of(widget: &impl IsA<gtk::Widget>, app: &App) -> gtk::Window {
    widget.root().and_downcast::<gtk::Window>().unwrap_or_else(|| app.window.clone().upcast())
}

// ---------------------------------------------------------------------------------------
// What every page shares
// ---------------------------------------------------------------------------------------

/// A page: its controls (some in a slot filled once the app says what the settings are), a
/// footer if it has one, and its status line at the bottom.
struct Built {
    widget: gtk::Widget,
    status: gtk::Label,
    slot: gtk::Box,
    folder: Option<gtk::Entry>,
    devices: Option<Rc<DevicesPage>>,
    store: Option<gtk::Box>,
}

struct PageBox {
    body: gtk::Box,
    slot: gtk::Box,
    status: gtk::Label,
}

impl PageBox {
    fn new() -> Self {
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let slot = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).build();
        let status = gtk::Label::builder().wrap(true).xalign(0.0).vexpand(true).valign(gtk::Align::End).build();
        Self { body, slot, status }
    }

    fn add(&self, widget: &impl IsA<gtk::Widget>) {
        self.body.append(widget);
    }

    /// Where the settings go, here among the rest.
    fn settings(&self) {
        self.body.append(&self.slot);
    }

    /// A field with its label above it, whose mnemonic names it.
    fn field(&self, label: &str, widget: &impl IsA<gtk::Widget>) {
        let label = gtk::Label::builder().label(label).use_underline(true).xalign(0.0).margin_top(6).build();
        label.set_mnemonic_widget(Some(widget));
        self.add(&label);
        self.add(widget);
    }

    fn footer(&self, text: &str) {
        self.add(&gtk::Label::builder().label(text).wrap(true).xalign(0.0).margin_top(12).build());
    }

    fn finish(self) -> Built {
        self.body.append(&self.status);
        let scrolled = gtk::ScrolledWindow::builder().child(&self.body).hscrollbar_policy(gtk::PolicyType::Never).build();
        Built { widget: scrolled.upcast(), status: self.status, slot: self.slot, folder: None, devices: None, store: None }
    }
}

/// Says something on a page's status line: what the shell does itself, not the app.
fn tell(status: &gtk::Label, text: &str) {
    status.set_label(text);
    crate::window::announce(status, text);
}

// ---------------------------------------------------------------------------------------
// General: this computer's own
// ---------------------------------------------------------------------------------------

fn general() -> Built {
    let page = PageBox::new();
    page.settings();
    page.footer("These settings are this computer's alone.");
    page.finish()
}

// ---------------------------------------------------------------------------------------
// Planning: the store's, synced to every device
// ---------------------------------------------------------------------------------------

fn planning() -> Built {
    let page = PageBox::new();
    page.settings();
    page.footer("These sync to all your devices.");
    page.finish()
}

// ---------------------------------------------------------------------------------------
// Devices: sync status, pairing, renaming, unpairing
// ---------------------------------------------------------------------------------------

struct DevicesPage {
    sync_status: gtk::Entry,
    label: gtk::Label,
    list: Rc<Tree>,
    devices: RefCell<Vec<lumenna_records::DeviceView>>,
    landing: Landing,
}

impl DevicesPage {
    fn empty() -> Self {
        Self {
            sync_status: prompts::read_only_text(),
            label: gtk::Label::new(None),
            list: Tree::new("Paired devices"),
            devices: RefCell::new(Vec::new()),
            landing: Landing::default(),
        }
    }

    fn draw(&self, app: &App, view: &lumenna_app::DevicesView, held: &lumenna_app::HeldView) {
        if self.sync_status.text() != view.status {
            self.sync_status.set_text(&view.status);
        }
        let devices = view.list.as_ref().map(|list| list.devices.clone()).unwrap_or_default();
        let lines = app.model.lines();
        let items = devices
            .iter()
            .enumerate()
            .map(|(at, device)| Item {
                key: device.node_id.clone(),
                text: lines.devices.get(at).map_or_else(|| device.name.clone(), |line| line.text.clone()),
                ..Default::default()
            })
            .collect();
        // An empty list is no use to reach: the sync status already says nothing is paired.
        self.label.set_visible(!devices.is_empty());
        self.list.widget.set_visible(!devices.is_empty());
        *self.devices.borrow_mut() = devices;
        let rebuilt = self.list.set(items);
        if let Some(window) = self.list.view.root().and_downcast::<gtk::Window>() {
            self.landing.land(&self.list, &window, held.focus.as_deref(), held.landing, rebuilt);
        }
    }

    /// Sends `event` for the chosen device (a `Run` of one kind, or a key), held there so what
    /// it changes lands there. A device without the action (Unpair on this one) is refused by
    /// the app, which says why.
    fn act(&self, app: &App, event: ShellEvent) {
        let Some(device) = self.list.selected().and_then(|index| self.devices.borrow().get(index).cloned()) else { return };
        app.model.send(ShellEvent::Hold(Side::Devices, device.node_id.clone()));
        app.model.send(event);
    }
}

fn devices_page() -> Built {
    let page = PageBox::new();
    let devices = Rc::new(DevicesPage::empty());
    page.field("S_ync status", &devices.sync_status);
    devices.list.fit(160);
    devices.list.view.update_property(&[gtk::accessible::Property::Description("Delete unpairs the selected device.")]);
    devices.label.set_label("Paired _devices");
    devices.label.set_use_underline(true);
    devices.label.set_xalign(0.0);
    devices.label.set_margin_top(6);
    devices.label.set_mnemonic_widget(Some(&devices.list.view));
    page.add(&devices.label);
    page.add(&devices.list.widget);
    let buttons = gtk::Box::builder().spacing(6).margin_top(6).build();
    let sync_now = gtk::Button::with_mnemonic("Sync _Now");
    let pair = gtk::Button::with_mnemonic("_Pair a Device…");
    let rename = gtk::Button::with_mnemonic("_Rename…");
    let unpair = gtk::Button::with_mnemonic("_Unpair…");
    for button in [&sync_now, &pair, &rename, &unpair] {
        buttons.append(button);
    }
    page.add(&buttons);
    sync_now.connect_clicked(|_| send(ShellEvent::Devices(DevicesEvent::SyncNow)));
    pair.connect_clicked(|_| {
        let name = glib::host_name().to_string();
        send(ShellEvent::Devices(DevicesEvent::Pair(lumenna_app::PairEvent::Open(name))));
    });
    for (button, kind) in [(&rename, ActionKind::Rename), (&unpair, ActionKind::Unpair)] {
        let devices = Rc::downgrade(&devices);
        button.connect_clicked(move |_| {
            if let (Some(app), Some(devices)) = (crate::window::app(), devices.upgrade()) {
                devices.act(&app, ShellEvent::Run(vec![kind]));
            }
        });
    }
    {
        // The person's moves in the list are the app's to know, so a change lands back here.
        let weak = Rc::downgrade(&devices);
        devices.list.connect_selected(move |index| {
            let Some(devices) = weak.upgrade() else { return };
            if let Some(key) = index.and_then(|index| devices.list.key(index)) {
                send(ShellEvent::Hold(Side::Devices, key));
            }
        });
    }
    {
        // Delete in the list unpairs, as it removes in every other list: the core's to say.
        let weak = Rc::downgrade(&devices);
        devices.list.connect_key(move |key, modifiers, _| {
            if !matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) || !modifiers.is_empty() {
                return glib::Propagation::Proceed;
            }
            if let (Some(app), Some(devices)) = (crate::window::app(), weak.upgrade()) {
                devices.act(&app, ShellEvent::Key("delete".to_owned()));
            }
            glib::Propagation::Stop
        });
    }
    let mut built = page.finish();
    built.devices = Some(devices);
    built
}

// ---------------------------------------------------------------------------------------
// Backups: this device's alone
// ---------------------------------------------------------------------------------------

fn backups(app: &Rc<App>) -> Built {
    let page = PageBox::new();
    page.settings();
    let folder = gtk::Entry::builder().editable(false).hexpand(true).build();
    let choose = gtk::Button::with_mnemonic("C_hoose…");
    let row = gtk::Box::builder().spacing(6).build();
    row.append(&folder);
    row.append(&choose);
    let label = gtk::Label::builder().label(marked("Back up to", 't', '_')).use_underline(true).xalign(0.0).margin_top(6).build();
    label.set_mnemonic_widget(Some(&folder));
    page.add(&label);
    page.add(&row);
    let buttons = gtk::Box::builder().spacing(6).margin_top(6).build();
    let back_up = gtk::Button::with_mnemonic("_Back Up Now");
    let restore = gtk::Button::with_mnemonic("_Restore from a Backup…");
    buttons.append(&back_up);
    buttons.append(&restore);
    page.add(&buttons);
    page.footer("These settings are this device's alone, and do not sync.");
    let mut built = page.finish();
    built.folder = Some(folder.clone());
    {
        let (app, folder) = (Rc::downgrade(app), folder.clone());
        choose.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let (folder, window) = (folder.clone(), window_of(button, &app));
            spawn(async move {
                let dialog = gtk::FileDialog::builder().title("Back Up Here").modal(true).build();
                let current = folder.text().to_string();
                if !current.is_empty() {
                    dialog.set_initial_folder(Some(&gio::File::for_path(&current)));
                }
                let Ok(chosen) = dialog.select_folder_future(Some(&window)).await else { return };
                if let Some(path) = chosen.path() {
                    app.model.send(ShellEvent::Settings(SettingsEvent::Set {
                        key: "backup-dir".to_owned(),
                        value: path.display().to_string(),
                    }));
                }
            });
        });
    }
    {
        let (app, status) = (Rc::downgrade(app), built.status.clone());
        back_up.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let (status, window) = (status.clone(), window_of(button, &app));
            app.model.back_up(move |done| match done {
                Ok(done) => tell(&status, &lumenna_records::words::spoken(&done.announcement, &done.notices)),
                Err(error) => prompts::fail(&window, error.message()),
            });
        });
    }
    restore.connect_clicked(|button| {
        let Some(app) = crate::window::app() else { return };
        let window = window_of(button, &app);
        spawn(async move { import(&window, "Restore from a Backup", &[("Lumenna backups", "*.lumbak")]).await });
    });
    // The folder's own label is the core's.
    if let Some(title) = app.model.view().settings.as_ref().and_then(|settings| {
        settings.settings.iter().find(|shown| shown.setting.key == "backup-dir").map(|shown| shown.setting.title.clone())
    }) {
        label.set_label(&marked(&title, 't', '_'));
    }
    built
}

/// Reads a file in — a JSON export or a backup, the app tells which by its name — and the app
/// says what it did.
pub async fn import(window: &gtk::Window, title: &str, types: &[(&str, &str)]) {
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    for (name, pattern) in types {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(name));
        for pattern in pattern.split(';') {
            filter.add_pattern(pattern);
        }
        filters.append(&filter);
    }
    let dialog = gtk::FileDialog::builder().title(title).modal(true).filters(&filters).build();
    let Some(path) = dialog.open_future(Some(window)).await.ok().and_then(|file| file.path()) else { return };
    match std::fs::read(&path) {
        Ok(bytes) => {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            send(ShellEvent::Settings(SettingsEvent::Import { name, bytes }));
        }
        Err(error) => prompts::tell(window, &format!("That file could not be read. {error}")).await,
    }
}

/// What can be read in.
pub const IMPORTABLE: [(&str, &str); 1] = [("Lumenna exports and backups", "*.json;*.lumbak")];

// ---------------------------------------------------------------------------------------
// Export and import: what you have now, and reading it back
// ---------------------------------------------------------------------------------------

fn export(app: &Rc<App>) -> Built {
    let page = PageBox::new();
    let mut buttons = Vec::new();
    for export in lumenna_records::exports::EXPORTS {
        let button = gtk::Button::with_mnemonic(&marked(export.label, export.key, '_'));
        button.set_halign(gtk::Align::Start);
        page.add(&button);
        buttons.push((export.format, button));
    }
    let import_button = gtk::Button::with_mnemonic("_Import or Restore…");
    import_button.set_halign(gtk::Align::Start);
    page.add(&import_button);
    // What is done to the store as a whole, once the app says what it is.
    let store = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).margin_top(6).build();
    page.add(&store);
    page.footer(
        "An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this device lacks and removes nothing.",
    );
    let mut built = page.finish();
    built.store = Some(store);
    for (format, button) in buttons {
        let app = Rc::downgrade(app);
        button.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let window = window_of(button, &app);
            spawn(async move { export_to(&window, format).await });
        });
    }
    import_button.connect_clicked(|button| {
        let Some(app) = crate::window::app() else { return };
        let window = window_of(button, &app);
        spawn(async move { import(&window, "Import or Restore", &IMPORTABLE).await });
    });
    built
}

/// Asks where an export goes, then has the app make it: the file it makes is saved there
/// (`save`), and the app says what it did.
async fn export_to(window: &gtk::Window, format: ExportFormat) {
    let today = jiff::Zoned::now().date();
    let name = lumenna_records::exports::export_name(format, today);
    let dialog = gtk::FileDialog::builder().title("Export").initial_name(&name).modal(true).build();
    // The dialog already asked about replacing a file that was there.
    let Some(path) = dialog.save_future(Some(window)).await.ok().and_then(|file| file.path()) else { return };
    if let Some(open) = open() {
        *open.saving.borrow_mut() = Some(path);
    }
    send(ShellEvent::Settings(SettingsEvent::Export(format)));
}
