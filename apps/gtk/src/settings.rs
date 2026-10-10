//! Settings: a window of tabs, opened with Ctrl+Comma — General, Planning,
//! Devices, Backups, and Export and Import, as on the Mac and Windows.
//!
//! Every page applies a change as it is made; a text field when it is left or Enter is
//! pressed in it. Each page has a status line of its own, announced, for what a change did:
//! the main window's is behind this one.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_desktop::{devices, profile, speech};
use lumenna_surface::actions::{ActionKind, Subject};
use lumenna_surface::{DeviceView, ExportFormat, Imported, Setting, SettingKind};

use crate::core::sentence;
use crate::window::{App, spawn};
use crate::tree::{Item, Tree};
use crate::{pairing, prompts};

/// The pages opened directly, by their place in the window (Planning is 1, Backups 3).
#[derive(Clone, Copy)]
pub enum Page {
    General = 0,
    Devices = 2,
    Export = 4,
}

const PAGES: [&str; 5] = ["General", "Planning", "Devices", "Backups", "Export and Import"];

thread_local! {
    /// The settings window, while it is open: opening it again brings it forward.
    static OPEN: RefCell<Option<(gtk::Window, Rc<Tabs>)>> = const { RefCell::new(None) };
}

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

/// Opens Settings on a page.
pub fn show(app: &Rc<App>, page: Page) {
    if let Some((window, tabs)) = OPEN.with(|open| open.borrow().clone()) {
        tabs.go(page as usize, false);
        window.present();
        return;
    }
    let stack = gtk::Stack::new();
    let pages: [gtk::Widget; 5] = [general(app), planning(app), devices_page(app), backups(app), export(app)];
    for (title, page) in PAGES.iter().zip(pages) {
        stack.add_titled(&page, Some(title), title);
    }
    let switcher = gtk::StackSwitcher::builder().stack(&stack).halign(gtk::Align::Center).margin_top(6).build();
    let tabs = Rc::new(Tabs { stack: stack.clone(), switcher: switcher.clone() });
    tabs.go(page as usize, false);
    {
        // A click on a tab opens its page too.
        let tabs = Rc::downgrade(&tabs);
        stack.connect_visible_child_name_notify(move |_| {
            if let Some(tabs) = tabs.upgrade() {
                tabs.settle();
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
        OPEN.with(|open| open.borrow_mut().take());
        glib::Propagation::Proceed
    });
    OPEN.with(|open| *open.borrow_mut() = Some((window.clone(), Rc::clone(&tabs))));
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

// ---------------------------------------------------------------------------------------
// What every page shares
// ---------------------------------------------------------------------------------------

/// A page: its controls, then a footer if it has one, and its status line at the bottom.
struct PageBox {
    body: gtk::Box,
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
        let status = gtk::Label::builder().wrap(true).xalign(0.0).vexpand(true).valign(gtk::Align::End).build();
        Self { body, status }
    }

    fn add(&self, widget: &impl IsA<gtk::Widget>) {
        self.body.append(widget);
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

    fn finish(self) -> (gtk::Widget, Status) {
        self.body.append(&self.status);
        let scrolled = gtk::ScrolledWindow::builder().child(&self.body).hscrollbar_policy(gtk::PolicyType::Never).build();
        (scrolled.upcast(), Status(self.status))
    }
}

/// A page's status line.
#[derive(Clone)]
struct Status(gtk::Label);

impl Status {
    fn say(&self, text: &str) {
        let text = speech::sentence(text);
        self.0.set_label(&text);
        crate::window::announce(&self.0, &text);
    }
}

/// A setting as the core describes it: its name, its control, what it can be.
fn setting(app: &App, key: &str) -> Setting {
    app.core
        .lumenna
        .settings(Some(key.to_owned()))
        .ok()
        .and_then(|listing| listing.settings.into_iter().find(|s| s.key == key))
        .unwrap_or_else(|| Setting {
            key: key.to_owned(),
            value: String::new(),
            title: key.to_owned(),
            kind: SettingKind::default(),
            options: Vec::new(),
            syncs: false,
            hint: String::new(),
        })
}

fn value(app: &App, key: &str) -> String {
    setting(app, key).value
}

/// A setting's value as its field shows it: a time of day in this desktop's clock.
fn shown(app: &App, key: &str) -> String {
    let setting = setting(app, key);
    match setting.kind {
        SettingKind::Time if !setting.value.is_empty() => speech::Clock::time(&app.clock, &setting.value),
        _ => setting.value,
    }
}

/// A setting's control, by its kind, with its name above it — `mnemonic` marked in it, the
/// one thing about it that is this app's — and its hint as its description. A folder is
/// not here: it is a row with a button of its own.
fn setting_field(app: &Rc<App>, page: &PageBox, status: &Status, key: &'static str, mnemonic: char) {
    let shown = setting(app, key);
    let label = devices::marked(&shown.title, mnemonic, '_');
    match shown.kind {
        SettingKind::Toggle => {
            let check = prompts::check(&label);
            check.set_active(shown.value == "true");
            describe(&check, &shown.hint);
            page.add(&check);
            let (app, status) = (Rc::downgrade(app), status.clone());
            check.connect_toggled(move |check| {
                if let Some(app) = app.upgrade() {
                    set(&app, &status, key, if check.is_active() { "true" } else { "false" });
                }
            });
        }
        SettingKind::Choice => page.field(&label, &setting_choice(app, status, &shown)),
        SettingKind::Time | SettingKind::Number | SettingKind::Folder => {
            page.field(&label, &setting_entry(app, status, key, &shown.hint));
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

/// Changes a setting and says so; on a failure says why and returns false.
fn set(app: &App, status: &Status, key: &str, to: &str) -> bool {
    match app.core.lumenna.set_setting(key, to) {
        Ok(change) => {
            status.say(&speech::announcement(&change.announcement, &change.notices));
            app.store_changed();
            true
        }
        Err(error) => {
            prompts::fail(&window_of(&status.0, app), &sentence(&error));
            false
        }
    }
}

/// A text field for a setting, written when it is left or Enter is pressed in it — if it
/// changed, and put back if what was typed does not read.
fn setting_entry(app: &Rc<App>, status: &Status, key: &'static str, description: &str) -> gtk::Entry {
    let entry = gtk::Entry::builder().text(shown(app, key)).build();
    describe(&entry, description);
    let commit = {
        let (app, status) = (Rc::downgrade(app), status.clone());
        move |entry: &gtk::Entry| {
            let Some(app) = app.upgrade() else { return };
            let typed = entry.text().trim().to_owned();
            let was = shown(&app, key);
            if typed != was {
                if set(&app, &status, key, &typed) {
                    // As read: "8am" becomes the clock's "8:00 AM".
                    entry.set_text(&shown(&app, key));
                } else {
                    entry.set_text(&was);
                }
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
    entry.connect_activate(commit);
    entry
}

/// A drop-down of a setting's options: their names shown, their values written.
fn setting_choice(app: &Rc<App>, status: &Status, shown: &Setting) -> gtk::DropDown {
    let key = shown.key.clone();
    let current = shown.value.clone();
    let mut choices: Vec<(String, String)> = shown.options.iter().map(|o| (o.title.clone(), o.id.clone())).collect();
    // A value set elsewhere — `lum config set backup-every 3d` — is shown as it is.
    if !current.is_empty() && !choices.iter().any(|(_, v)| *v == current) {
        choices.push((current.clone(), current.clone()));
    }
    let labels: Vec<&str> = choices.iter().map(|(l, _)| l.as_str()).collect();
    let dropdown = gtk::DropDown::from_strings(&labels);
    describe(&dropdown, &shown.hint);
    if let Some(position) = choices.iter().position(|(_, v)| *v == current) {
        dropdown.set_selected(position as u32);
    }
    let (app, status) = (Rc::downgrade(app), status.clone());
    dropdown.connect_selected_notify(move |dropdown| {
        let Some(app) = app.upgrade() else { return };
        if let Some((_, to)) = choices.get(dropdown.selected() as usize)
            && *to != value(&app, &key)
        {
            set(&app, &status, &key, to);
        }
    });
    dropdown
}

// ---------------------------------------------------------------------------------------
// General: this computer's own
// ---------------------------------------------------------------------------------------

fn general(app: &Rc<App>) -> gtk::Widget {
    let page = PageBox::new();
    let at_sign_in = prompts::check("_Open Lumenna when you sign in, in the background");
    at_sign_in.set_active(autostart_file(app).is_some_and(|file| file.exists()));
    page.add(&at_sign_in);
    page.footer(
        "Lumenna keeps running after its window is closed, so it syncs and reminds you. Start it again, from the menu or with lumenna-gtk, to bring the window back. This setting is this computer's alone.",
    );
    let (widget, status) = page.finish();
    let app = Rc::downgrade(app);
    at_sign_in.connect_toggled(move |check| {
        let Some(app) = app.upgrade() else { return };
        match set_autostart(&app, check.is_active()) {
            Ok(()) if check.is_active() => status.say("Lumenna opens when you sign in"),
            Ok(()) => status.say("Lumenna no longer opens when you sign in"),
            Err(error) => {
                prompts::fail(&window_of(check, &app), &format!("That could not be changed. {error}"));
                check.set_active(!check.is_active());
            }
        }
    });
    widget
}

/// Where the sign-in entry for this profile goes: the XDG autostart directory, named after
/// the application identifier, so each profile has its own.
fn autostart_file(app: &App) -> Option<PathBuf> {
    let id = app.application.application_id()?;
    Some(glib::user_config_dir().join("autostart").join(format!("{id}.desktop")))
}

fn set_autostart(app: &App, on: bool) -> std::io::Result<()> {
    let Some(file) = autostart_file(app) else { return Ok(()) };
    if !on {
        return match std::fs::remove_file(&file) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let program = std::env::current_exe()?;
    let directory = PathBuf::from(app.core.lumenna.directory());
    let mut exec = format!("{} --background", quoted(&program));
    if profile::default_directory().as_deref() != Some(directory.as_path()) {
        exec.push_str(&format!(" --profile {}", quoted(&directory)));
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &file,
        format!(
            "[Desktop Entry]\nType=Application\nName=Lumenna\nComment=Tasks and time blocks\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
        ),
    )
}

/// A path as a desktop entry's Exec line takes it.
fn quoted(path: &Path) -> String {
    let text = path.display().to_string();
    if text.chars().any(|c| c.is_whitespace() || "\"'\\$`".contains(c)) {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`"))
    } else {
        text
    }
}

// ---------------------------------------------------------------------------------------
// Planning: the store's, synced to every device
// ---------------------------------------------------------------------------------------

fn planning(app: &Rc<App>) -> gtk::Widget {
    let page = PageBox::new();
    let status = Status(page.status.clone());
    setting_field(app, &page, &status, "cascade-complete-subtasks", 'C');
    setting_field(app, &page, &status, "day-start", 's');
    setting_field(app, &page, &status, "day-end", 'e');
    setting_field(app, &page, &status, "all-day-reminder-hour", 'A');
    setting_field(app, &page, &status, "verbosity", 'm');
    setting_field(app, &page, &status, "week-start", 'W');
    page.footer("These sync to all your devices.");
    page.finish().0
}

// ---------------------------------------------------------------------------------------
// Devices: sync status, pairing, renaming, unpairing
// ---------------------------------------------------------------------------------------

pub struct Devices {
    sync_status: gtk::Entry,
    label: gtk::Label,
    list: Rc<Tree>,
    devices: RefCell<Vec<DeviceView>>,
    status: Status,
}

impl Devices {
    fn load(&self, app: &App) {
        match app.core.lumenna.sync_status() {
            Ok(status) => {
                let text = speech::sentence(&speech::announcement(&status.announcement, &status.notices));
                self.sync_status.set_text(&text);
                let key = self.list.selected().and_then(|index| self.list.key(index));
                let near = self.list.selected().or(Some(0));
                let now = jiff::Timestamp::now();
                let items = status
                    .devices
                    .iter()
                    .map(|device| Item { key: device.node_id.clone(), text: devices::line(device, now), depth: 0 })
                    .collect();
                // An empty list is no use to reach: the sync status already says there is
                // nothing paired.
                self.label.set_visible(!status.devices.is_empty());
                self.list.widget.set_visible(!status.devices.is_empty());
                *self.devices.borrow_mut() = status.devices;
                if self.list.set(items) {
                    self.list.select_key_or_near(key.as_deref(), near);
                }
            }
            Err(error) => self.sync_status.set_text(&sentence(&error)),
        }
    }

    fn chosen(&self) -> Option<DeviceView> {
        let index = self.list.selected()?;
        self.devices.borrow().get(index).cloned()
    }

    /// Runs the chosen device's action of one of `kinds`, asked over this window and said on
    /// this page. A device the core offers no such action for (Unpair on this one) has none.
    fn act(&self, app: &Rc<App>, kinds: &[ActionKind]) {
        let Some(device) = self.chosen() else { return };
        let Some(action) = crate::actions::find(&device.actions, kinds) else {
            let why = lumenna_surface::actions::not_offered(kinds[0], Subject::Device, device.this_device);
            return self.status.say(&why);
        };
        let status = self.status.clone();
        let from = crate::actions::Asking {
            window: window_of(&self.list.view, app),
            say: Some(Rc::new(move |text: &str| status.say(text))),
        };
        let after: crate::actions::After = Rc::new(|app, _, _| devices_heard(app, None));
        crate::actions::run_from(app, action.clone(), Some(after), from);
    }
}

fn devices_page(app: &Rc<App>) -> gtk::Widget {
    let page = PageBox::new();
    let sync_status = prompts::read_only_text();
    page.field("S_ync status", &sync_status);
    let list = Tree::new("Paired devices");
    list.fit(160);
    list.view.update_property(&[gtk::accessible::Property::Description("Delete unpairs the selected device.")]);
    let label = gtk::Label::builder().label("Paired _devices").use_underline(true).xalign(0.0).margin_top(6).build();
    label.set_mnemonic_widget(Some(&list.view));
    page.add(&label);
    page.add(&list.widget);
    let buttons = gtk::Box::builder().spacing(6).margin_top(6).build();
    let sync_now = gtk::Button::with_mnemonic("Sync _Now");
    let pair = gtk::Button::with_mnemonic("_Pair a Device…");
    let rename = gtk::Button::with_mnemonic("_Rename…");
    let unpair = gtk::Button::with_mnemonic("_Unpair…");
    for button in [&sync_now, &pair, &rename, &unpair] {
        buttons.append(button);
    }
    page.add(&buttons);
    let (widget, status) = page.finish();
    let devices = Rc::new(Devices {
        sync_status,
        label,
        list: Rc::clone(&list),
        devices: RefCell::new(Vec::new()),
        status: status.clone(),
    });
    devices.load(app);
    *app.devices_page.borrow_mut() = Some(Rc::downgrade(&devices));

    {
        let status = status.clone();
        let app = Rc::downgrade(app);
        sync_now.connect_clicked(move |_| {
            if let Some(app) = app.upgrade() {
                status.say("Syncing");
                // The round's result is said by the app, which tells this page too.
                app.core.sync_now();
            }
        });
    }
    {
        let (devices, app) = (Rc::clone(&devices), Rc::downgrade(app));
        pair.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let (devices, window) = (Rc::clone(&devices), window_of(button, &app));
            spawn(async move {
                if let Some(said) = pairing::run(&window, app.core.lumenna.clone()).await {
                    devices.load(&app);
                    app.store_changed();
                    devices.status.say(&said);
                }
            });
        });
    }
    for (button, kind) in [(&rename, ActionKind::Rename), (&unpair, ActionKind::Unpair)] {
        let (devices, app) = (Rc::downgrade(&devices), Rc::downgrade(app));
        button.connect_clicked(move |_| {
            if let (Some(app), Some(devices)) = (app.upgrade(), devices.upgrade()) {
                devices.act(&app, &[kind]);
            }
        });
    }
    // Delete in the list unpairs, as it removes in every other list.
    {
        let (devices, app) = (Rc::downgrade(&devices), Rc::downgrade(app));
        list.connect_key(move |key, modifiers, _| {
            if !matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) || !modifiers.is_empty() {
                return glib::Propagation::Proceed;
            }
            if let (Some(app), Some(devices)) = (app.upgrade(), devices.upgrade()) {
                devices.act(&app, &[ActionKind::Unpair]);
            }
            glib::Propagation::Stop
        });
    }
    widget
}

/// The Devices page's answer to a sync round or an arrival, when the page is open.
pub fn devices_heard(app: &App, said: Option<&str>) {
    let devices = app.devices_page.borrow().as_ref().and_then(std::rc::Weak::upgrade);
    if let Some(devices) = devices {
        devices.load(app);
        if let Some(said) = said {
            devices.status.say(said);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Backups: this device's alone
// ---------------------------------------------------------------------------------------

fn backups(app: &Rc<App>) -> gtk::Widget {
    let page = PageBox::new();
    let status = Status(page.status.clone());
    setting_field(app, &page, &status, "backup-every", 'A');
    setting_field(app, &page, &status, "backup-keep", 'k');
    let folder = gtk::Entry::builder().text(value(app, "backup-dir")).editable(false).hexpand(true).build();
    let choose = gtk::Button::with_mnemonic("C_hoose…");
    let row = gtk::Box::builder().spacing(6).build();
    row.append(&folder);
    row.append(&choose);
    let label = gtk::Label::builder().label(devices::marked(&setting(app, "backup-dir").title, 't', '_')).use_underline(true).xalign(0.0).margin_top(6).build();
    label.set_mnemonic_widget(Some(&folder));
    page.add(&label);
    page.add(&row);
    let buttons = gtk::Box::builder().spacing(6).margin_top(6).build();
    let back_up = gtk::Button::with_mnemonic("_Back Up Now");
    let restore = gtk::Button::with_mnemonic("_Restore from a Backup…");
    buttons.append(&back_up);
    buttons.append(&restore);
    page.add(&buttons);
    page.footer(
        "These settings are this device's alone, and do not sync.",
    );
    let (widget, status) = page.finish();
    {
        let (app, status, folder) = (Rc::downgrade(app), status.clone(), folder.clone());
        choose.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let (status, folder, window) = (status.clone(), folder.clone(), window_of(button, &app));
            spawn(async move {
                let dialog = gtk::FileDialog::builder().title("Back Up Here").modal(true).build();
                let current = value(&app, "backup-dir");
                if !current.is_empty() {
                    dialog.set_initial_folder(Some(&gio::File::for_path(&current)));
                }
                let Ok(chosen) = dialog.select_folder_future(Some(&window)).await else { return };
                if let Some(path) = chosen.path()
                    && set(&app, &status, "backup-dir", &path.display().to_string())
                {
                    folder.set_text(&value(&app, "backup-dir"));
                }
            });
        });
    }
    {
        let (app, status) = (Rc::downgrade(app), status.clone());
        back_up.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            match app.core.lumenna.backup(None) {
                Ok(done) => status.say(&speech::announcement(&done.announcement, &done.notices)),
                Err(error) => prompts::fail(&window_of(button, &app), &sentence(&error)),
            }
        });
    }
    {
        let (app, status) = (Rc::downgrade(app), status.clone());
        restore.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let (status, window) = (status.clone(), window_of(button, &app));
            spawn(async move {
                if let Some(said) = import(&app, &window, "Restore from a Backup", &[("Lumenna backups", "*.lumbak")]).await {
                    status.say(&said);
                }
            });
        });
    }
    widget
}

/// Reads a file in — a JSON export or a backup, the core tells which — and returns what it
/// did, to say.
pub async fn import(app: &App, window: &gtk::Window, title: &str, types: &[(&str, &str)]) -> Option<String> {
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
    let path = dialog.open_future(Some(window)).await.ok()?.path()?;
    match app.core.lumenna.import(&path.display().to_string()) {
        Ok(imported) => {
            app.store_changed();
            Some(match imported {
                Imported::Export { done } => speech::announcement(&done.announcement, &done.notices),
                Imported::Backup { done } => speech::announcement(&done.announcement, &done.notices),
            })
        }
        Err(error) => {
            prompts::tell(window, &sentence(&error)).await;
            None
        }
    }
}

/// What can be read in.
pub const IMPORTABLE: [(&str, &str); 1] = [("Lumenna exports and backups", "*.json;*.lumbak")];

// ---------------------------------------------------------------------------------------
// Export and import: what you have now, and reading it back
// ---------------------------------------------------------------------------------------

fn export(app: &Rc<App>) -> gtk::Widget {
    let page = PageBox::new();
    let mut buttons = Vec::new();
    for export in devices::EXPORTS {
        let button = gtk::Button::with_mnemonic(&devices::marked(export.label, export.key, '_'));
        button.set_halign(gtk::Align::Start);
        page.add(&button);
        buttons.push((export.format, button));
    }
    let import_button = gtk::Button::with_mnemonic("_Import or Restore…");
    import_button.set_halign(gtk::Align::Start);
    page.add(&import_button);
    page.footer(
        "An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this device lacks and removes nothing.",
    );
    let (widget, status) = page.finish();
    for (format, button) in buttons {
        let (app, status) = (Rc::downgrade(app), status.clone());
        button.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let (status, window) = (status.clone(), window_of(button, &app));
            spawn(async move { export_to(&app, &window, &status, format).await });
        });
    }
    {
        let (app, status) = (Rc::downgrade(app), status.clone());
        import_button.connect_clicked(move |button| {
            let Some(app) = app.upgrade() else { return };
            let (status, window) = (status.clone(), window_of(button, &app));
            spawn(async move {
                if let Some(said) = import(&app, &window, "Import or Restore", &IMPORTABLE).await {
                    status.say(&said);
                }
            });
        });
    }
    widget
}

async fn export_to(app: &App, window: &gtk::Window, status: &Status, format: ExportFormat) {
    let today = jiff::Zoned::now().date();
    let name = devices::export_name(format, today);
    let dialog = gtk::FileDialog::builder().title("Export").initial_name(&name).modal(true).build();
    let Some(path) = dialog.save_future(Some(window)).await.ok().and_then(|file| file.path()) else { return };
    // The dialog already asked about replacing a file that was there.
    match app.core.lumenna.export(format, Some(path.display().to_string()), true) {
        Ok(done) => status.say(&speech::announcement(&done.announcement, &done.notices)),
        Err(error) => prompts::tell(window, &sentence(&error)).await,
    }
}
