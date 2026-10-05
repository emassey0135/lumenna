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
use lumenna_surface::{DeviceView, ExportFormat, Imported};

use crate::core::sentence;
use crate::window::{App, spawn};
use crate::{pairing, prompts};

const VERBOSITIES: [(&str, &str); 2] = [("Full sentences", "full"), ("Terse", "terse")];
const WEEKDAYS: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
const FREQUENCIES: [(&str, &str); 4] = [("Every 12 hours", "12h"), ("Every day", "1d"), ("Every week", "7d"), ("Off", "off")];

/// The pages opened directly, by their place in the window (Planning is 1, Backups 3).
#[derive(Clone, Copy)]
pub enum Page {
    General = 0,
    Devices = 2,
    Export = 4,
}

thread_local! {
    /// The settings window, while it is open: opening it again brings it forward.
    static OPEN: RefCell<Option<(gtk::Window, gtk::Notebook)>> = const { RefCell::new(None) };
}

/// Opens Settings on a page.
pub fn show(app: &Rc<App>, page: Page) {
    if let Some((window, notebook)) = OPEN.with(|open| open.borrow().clone()) {
        notebook.set_current_page(Some(page as u32));
        window.present();
        return;
    }
    let notebook = gtk::Notebook::new();
    let pages: [(&str, gtk::Widget); 5] = [
        ("General", general(app)),
        ("Planning", planning(app)),
        ("Devices", devices_page(app)),
        ("Backups", backups(app)),
        ("Export and Import", export(app)),
    ];
    for (title, page) in pages {
        notebook.append_page(&page, Some(&gtk::Label::new(Some(title))));
    }
    let window = gtk::Window::builder()
        .title("Settings")
        .transient_for(&app.window)
        .destroy_with_parent(true)
        .default_width(560)
        .default_height(480)
        .child(&notebook)
        .build();
    notebook.set_current_page(Some(page as u32));
    let escape = gtk::EventControllerKey::new();
    let closing = window.downgrade();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape
            && let Some(window) = closing.upgrade()
        {
            window.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(escape);
    window.connect_close_request(|_| {
        OPEN.with(|open| open.borrow_mut().take());
        glib::Propagation::Proceed
    });
    OPEN.with(|open| *open.borrow_mut() = Some((window.clone(), notebook.clone())));
    window.present();
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

fn values(app: &App) -> Vec<(String, String)> {
    app.core.lumenna.settings(None).map(|s| s.settings.into_iter().map(|s| (s.key, s.value)).collect()).unwrap_or_default()
}

fn value(app: &App, key: &str) -> String {
    values(app).into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default()
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
    let entry = gtk::Entry::builder().text(value(app, key)).build();
    if !description.is_empty() {
        entry.update_property(&[gtk::accessible::Property::Description(description)]);
    }
    let commit = {
        let (app, status) = (Rc::downgrade(app), status.clone());
        move |entry: &gtk::Entry| {
            let Some(app) = app.upgrade() else { return };
            let typed = entry.text().trim().to_owned();
            let was = value(&app, key);
            if typed != was && !set(&app, &status, key, &typed) {
                entry.set_text(&was);
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

/// A drop-down of `choices` for a setting: labels shown, values written.
fn setting_choice(app: &Rc<App>, status: &Status, key: &'static str, choices: Vec<(String, String)>) -> gtk::DropDown {
    let current = value(app, key);
    let mut choices = choices;
    // A value set elsewhere — `lum config set backup-every 3d` — is shown as it is.
    if !current.is_empty() && !choices.iter().any(|(_, v)| *v == current) {
        choices.push((format!("Every {current}"), current.clone()));
    }
    let labels: Vec<&str> = choices.iter().map(|(l, _)| l.as_str()).collect();
    let dropdown = gtk::DropDown::from_strings(&labels);
    if let Some(position) = choices.iter().position(|(_, v)| *v == current) {
        dropdown.set_selected(position as u32);
    }
    let (app, status) = (Rc::downgrade(app), status.clone());
    dropdown.connect_selected_notify(move |dropdown| {
        let Some(app) = app.upgrade() else { return };
        if let Some((_, to)) = choices.get(dropdown.selected() as usize)
            && *to != value(&app, key)
        {
            set(&app, &status, key, to);
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
    let cascade = prompts::check("_Completing a task completes its subtasks");
    cascade.set_active(value(app, "cascade-complete-subtasks") == "true");
    page.add(&cascade);
    let (widget, status) = {
        // The status line is needed by the fields, and goes last.
        let status = Status(page.status.clone());
        let time = "A time, such as 8:00 or 8am.";
        page.field("Day _starts", &setting_entry(app, &status, "day-start", time));
        page.field("Day _ends", &setting_entry(app, &status, "day-end", time));
        page.field("_All-day reminders at", &setting_entry(app, &status, "all-day-reminder-hour", time));
        let verbosities = VERBOSITIES.iter().map(|(l, v)| ((*l).to_owned(), (*v).to_owned())).collect();
        page.field("Announce_ments", &setting_choice(app, &status, "verbosity", verbosities));
        let days = WEEKDAYS.iter().map(|d| (speech::sentence(d), (*d).to_owned())).collect();
        page.field("_Week starts on", &setting_choice(app, &status, "week-start", days));
        page.footer("These sync to all your devices.");
        page.finish()
    };
    let app = Rc::downgrade(app);
    cascade.connect_toggled(move |check| {
        if let Some(app) = app.upgrade() {
            set(&app, &status, "cascade-complete-subtasks", if check.is_active() { "true" } else { "false" });
        }
    });
    widget
}

// ---------------------------------------------------------------------------------------
// Devices: sync status, pairing, renaming, unpairing
// ---------------------------------------------------------------------------------------

pub struct Devices {
    sync_status: gtk::Entry,
    list: gtk::ListBox,
    devices: RefCell<Vec<DeviceView>>,
    status: Status,
}

impl Devices {
    fn load(&self, app: &App) {
        match app.core.lumenna.sync_status() {
            Ok(status) => {
                let text = speech::sentence(&speech::announcement(&status.announcement, &status.notices));
                self.sync_status.set_text(&text);
                let kept = self.list.selected_row().map_or(0, |row| row.index().max(0));
                while let Some(row) = self.list.row_at_index(0) {
                    self.list.remove(&row);
                }
                let now = jiff::Timestamp::now();
                for device in &status.devices {
                    let label = gtk::Label::builder().label(devices::line(device, now)).xalign(0.0).margin_start(6).build();
                    self.list.append(&label);
                }
                let last = i32::try_from(status.devices.len()).unwrap_or(1) - 1;
                if let Some(row) = self.list.row_at_index(kept.min(last.max(0))) {
                    self.list.select_row(Some(&row));
                }
                *self.devices.borrow_mut() = status.devices;
            }
            Err(error) => self.sync_status.set_text(&sentence(&error)),
        }
    }

    fn chosen(&self) -> Option<DeviceView> {
        let index = usize::try_from(self.list.selected_row()?.index()).ok()?;
        self.devices.borrow().get(index).cloned()
    }

    fn change(&self, app: &App, operation: impl FnOnce(&lumenna_surface::Lumenna) -> lumenna_surface::Result<lumenna_surface::Change>) {
        match operation(&app.core.lumenna) {
            Ok(change) => {
                self.load(app);
                app.store_changed();
                self.status.say(&speech::announcement(&change.announcement, &change.notices));
            }
            Err(error) => prompts::fail(&window_of(&self.list, app), &sentence(&error)),
        }
    }

    async fn rename(&self, app: &App) {
        let Some(device) = self.chosen() else { return };
        let window = window_of(&self.list, app);
        let Some(name) = prompts::ask(&window, &format!("Rename {}", device.name), "_Name:", "", &device.name).await else {
            return;
        };
        self.change(app, |l| l.rename_device(&device.node_id, &name));
    }

    async fn unpair(&self, app: &App) {
        let Some(device) = self.chosen() else { return };
        let window = window_of(&self.list, app);
        if device.this_device {
            return prompts::tell(&window, "This is the device you are using. Unpair it from another one.").await;
        }
        let message = "It stops syncing with your devices but keeps everything it already has. Unpairing is for a device you replaced; it does not take data back from a lost one.";
        if prompts::confirm(&window, &format!("Unpair {}?", device.name), message, "Unpair").await {
            self.change(app, |l| l.unpair_device(&device.node_id));
        }
    }
}

fn devices_page(app: &Rc<App>) -> gtk::Widget {
    let page = PageBox::new();
    let sync_status = prompts::read_only_text();
    page.field("S_ync status", &sync_status);
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Browse).build();
    list.update_property(&[gtk::accessible::Property::Description("Delete unpairs the selected device.")]);
    // No scroller: a person has a handful of devices, and an empty list in one is a Tab stop
    // with nothing in it, while an empty list on its own is skipped.
    list.add_css_class("boxed-list");
    let label = gtk::Label::builder().label("Paired _devices").use_underline(true).xalign(0.0).margin_top(6).build();
    label.set_mnemonic_widget(Some(&list));
    page.add(&label);
    page.add(&list);
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
    let devices = Rc::new(Devices { sync_status, list: list.clone(), devices: RefCell::new(Vec::new()), status: status.clone() });
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
    {
        let (devices, app) = (Rc::clone(&devices), Rc::downgrade(app));
        rename.connect_clicked(move |_| {
            let (Some(app), devices) = (app.upgrade(), Rc::clone(&devices)) else { return };
            spawn(async move { devices.rename(&app).await });
        });
    }
    {
        let (devices, app) = (Rc::clone(&devices), Rc::downgrade(app));
        unpair.connect_clicked(move |_| {
            let (Some(app), devices) = (app.upgrade(), Rc::clone(&devices)) else { return };
            spawn(async move { devices.unpair(&app).await });
        });
    }
    // Delete in the list unpairs, as it removes in every other list.
    let keys = gtk::EventControllerKey::new();
    {
        let (devices, app) = (Rc::clone(&devices), Rc::downgrade(app));
        keys.connect_key_pressed(move |_, key, _, _| {
            if !matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) {
                return glib::Propagation::Proceed;
            }
            if let Some(app) = app.upgrade() {
                let devices = Rc::clone(&devices);
                spawn(async move { devices.unpair(&app).await });
            }
            glib::Propagation::Stop
        });
    }
    list.add_controller(keys);
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
    let frequencies = FREQUENCIES.iter().map(|(l, v)| ((*l).to_owned(), (*v).to_owned())).collect();
    page.field("_Automatic backups", &setting_choice(app, &status, "backup-every", frequencies));
    page.field("_Keep this many backups", &setting_entry(app, &status, "backup-keep", ""));
    let folder = gtk::Entry::builder().text(value(app, "backup-dir")).editable(false).hexpand(true).build();
    let choose = gtk::Button::with_mnemonic("C_hoose…");
    let row = gtk::Box::builder().spacing(6).build();
    row.append(&folder);
    row.append(&choose);
    let label = gtk::Label::builder().label("Backups go _to").use_underline(true).xalign(0.0).margin_top(6).build();
    label.set_mnemonic_widget(Some(&folder));
    page.add(&label);
    page.add(&row);
    let buttons = gtk::Box::builder().spacing(6).margin_top(6).build();
    let back_up = gtk::Button::with_mnemonic("_Back Up Now");
    let restore = gtk::Button::with_mnemonic("_Restore From a Backup…");
    buttons.append(&back_up);
    buttons.append(&restore);
    page.add(&buttons);
    page.footer(
        "A backup holds your whole history, including every task you deleted, so the store can be rebuilt from it. It stays on this device, as these settings do.",
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
                if let Some(said) = import(&app, &window, "Restore From a Backup", &[("Lumenna backups", "*.lumbak")]).await {
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
