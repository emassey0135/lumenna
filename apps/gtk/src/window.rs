//! The main window: three panes and a status line, the menu bar, and the actions.
//!
//! The panes are the places (a tree), the view for the place chosen, and the chosen task's
//! details, as Files and Evolution lay theirs out. **F6 and Shift+F6 move between them**;
//! GTK has no pane traversal of its own.
//!
//! What a change did is written on the status line along the bottom and announced through
//! `gtk_accessible_announce`, after focus has moved to the row it lands on — Orca reads an
//! announcement as a message, which would otherwise be cut off by the focus change.
//!
//! Every command is an action in the menu bar (GMenu), with its shortcut beside it, so the
//! menu bar is how anyone finds out what the app can do. The shortcuts are GTK's
//! application accelerators. Some keys are shown in the menu without being bound, because the
//! widget with focus answers them itself: Space and Delete on a list, Enter.
//!
//! Closing the window hides it; the app stays resident, syncing, until Quit.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};
use lumenna_records::places::Place;

use crate::clock::Locale;
use crate::core::Event;
use crate::detail::Detail;
use crate::sidebar::Sidebar;
use crate::day::DayView;
use crate::tasks::TaskList;
use crate::prompts;

/// The view in the middle pane.
#[derive(Clone)]
pub enum Content {
    Tasks(Rc<TaskList>),
    Day(Rc<DayView>),
}

impl Content {
    fn widget(&self) -> gtk::Widget {
        match self {
            Self::Tasks(list) => list.widget.clone().upcast(),
            Self::Day(day) => day.widget.clone().upcast(),
        }
    }

    fn reload(&self, app: &App) {
        match self {
            Self::Tasks(list) => list.draw(app),
            Self::Day(day) => day.draw(app),
        }
    }

    fn has_focus(&self) -> bool {
        let widget = self.widget();
        widget.root().and_then(|root| root.focus()).is_some_and(|focus| focus.is_ancestor(&widget) || focus == widget)
    }
}

pub struct App {
    /// The app the screens draw, as they move onto it one by one.
    pub model: crate::model::Model,
    pub application: gtk::Application,
    pub window: gtk::ApplicationWindow,
    pub clock: Locale,
    status: gtk::Label,
    pub sidebar: Rc<Sidebar>,
    content_slot: gtk::Box,
    content: RefCell<Option<Content>>,
    pub detail: Rc<Detail>,
    /// Whether focus goes into the middle pane once the app has gone to the place it shows.
    focus_on_show: Cell<bool>,
    /// Opening at sign-in, which the app's background setting asks for.
    autostart: crate::autostart::Autostart,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// The app, if it has started. Cloned out, so no borrow is held while it runs.
pub fn app() -> Option<Rc<App>> {
    APP.with(|app| app.borrow().clone())
}

thread_local! {
    /// Whether a popover menu is open, and what waits for it to close.
    static MENU_OPEN: Cell<bool> = const { Cell::new(false) };
    static AFTER_MENU: RefCell<Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>>> =
        RefCell::new(Vec::new());
}

/// Runs a future on the main thread: what awaits a dialog.
///
/// A command chosen from a popover menu runs while the menu is still up, and a dialog mapped
/// then is not given focus — the compositor hands it back to the main window as the menu goes,
/// and typing meant for the dialog's field is lost. So while a menu is open, the future waits
/// for it to close.
pub fn spawn(future: impl std::future::Future<Output = ()> + 'static) {
    if MENU_OPEN.with(Cell::get) {
        AFTER_MENU.with(|waiting| waiting.borrow_mut().push(Box::pin(future)));
    } else {
        glib::spawn_future_local(future);
    }
}

/// Announces `text` from the window `widget` is in.
///
/// From the window, not the status line that shows it: GTK dropped an announcement from a
/// label in the settings window, whose accessible object was never made, while one from the
/// window itself always arrives.
pub fn announce(widget: &impl IsA<gtk::Widget>, text: &str) {
    match widget.root() {
        Some(root) => root.announce(text, gtk::AccessibleAnnouncementPriority::Medium),
        None => widget.as_ref().announce(text, gtk::AccessibleAnnouncementPriority::Medium),
    }
}

thread_local! {
    /// Whether a fold the tree asked for is on its way: the tree reports the row's state, so the
    /// app's "Expanded" or "Collapsed" for it is not said as well.
    pub static QUIET_FOLD: Cell<bool> = const { Cell::new(false) };
}

/// A turn the app left, now on the main thread: drawn, then said.
pub fn take(turn: lumenna_ffi::Turn) {
    let Some(app) = app() else { return };
    if !app.model.take(&turn) {
        return;
    }
    app.draw();
    for file in &turn.files {
        crate::settings::save(&app, file);
    }
    // Opening at sign-in is this desktop's to do, as the person chose; the app says how it went.
    if let Some(on) = turn.background {
        if crate::sandboxed() {
            // The portal asks the person, over the window, and answers later.
            spawn(async move {
                let Some(app) = self::app() else { return };
                let event = app.autostart.ask(on, Some(&app.window)).await;
                app.model.send(event);
            });
        } else {
            app.model.send(app.autostart.keep(on));
        }
    }
    for said in &turn.said {
        if matches!(said.as_str(), "Expanded" | "Collapsed") && QUIET_FOLD.replace(false) {
            continue;
        }
        // A screen in front says it on its own status line: the main window's is behind it.
        if !(crate::quick_add::say(said) || crate::pairing::say(said) || crate::reminders::say(said) || crate::proposal::say(said) || crate::settings::say(said)) {
            app.say(said);
        }
    }
}

/// An event from another thread, now on the main one.
pub fn receive(event: Event) {
    let Some(app) = app() else { return };
    match event {
        // From the tray, quick add comes up over whatever is in front, as from anywhere.
        Event::Command(id) if id == "new-task" => app.quick_add(true),
        Event::Command(id) => app.command(&id),
        Event::ToggleTimer => crate::tray::toggle_timer(&app),
    }
}

/// A launch shortcut chosen from outside (`--launch <id>`, a desktop entry's action), on the
/// first start or from another: the window comes up and the app does it.
pub fn launch(id: &str) {
    let Some(app) = app() else { return };
    app.show_window();
    app.model.send(lumenna_ffi::ShellEvent::Launch(id.to_owned()));
}

/// The application was activated: the first time, opens the store and the window; after
/// that — the app started again, which GApplication turns into this — shows the window.
pub fn activate(application: &gtk::Application, directory: &Path, background: bool, shortcuts: bool) {
    if let Some(app) = app() {
        app.show_window();
        return;
    }
    let clock = Locale::new();
    let id = application.application_id().map_or_else(|| crate::ID.to_owned(), |id| id.to_string());
    let autostart = crate::autostart::Autostart::new(&id, directory);
    let model = match crate::model::Model::open(directory, &clock, &autostart) {
        Ok(model) => model,
        Err(error) => {
            let message = lumenna_ffi::store_unopened(error.message().to_string());
            let dialog = gtk::AlertDialog::builder().message(message).build();
            let application = application.clone();
            let hold = application.hold();
            dialog.choose(None::<&gtk::Window>, None::<&gio::Cancellable>, move |_| {
                drop(hold);
                application.quit();
            });
            return;
        }
    };
    let app = App::build(application, model, clock, autostart);
    APP.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&app)));
    app.start(background, shortcuts);
    // In a sandbox, opening at sign-in is the portal's, renewed at each start while it is on.
    spawn(async move {
        let Some(app) = self::app() else { return };
        if let Some(event) = app.autostart.renew().await {
            app.model.send(event);
        }
    });
}

impl App {
    fn build(application: &gtk::Application, model: crate::model::Model, clock: Locale, autostart: crate::autostart::Autostart) -> Rc<Self> {
        let window = gtk::ApplicationWindow::builder()
            .application(application)
            .title("Lumenna")
            .default_width(1100)
            .default_height(700)
            .show_menubar(true)
            .build();
        let sidebar = Sidebar::new();
        let content_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let detail = Detail::new();
        detail.widget.update_property(&[gtk::accessible::Property::Label("Details")]);
        let sidebar_pane = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(4)
            .build();
        sidebar_pane.append(&sidebar.tree.widget);
        let inner = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .start_child(&content_slot)
            .end_child(&detail.widget)
            .resize_start_child(true)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .position(460)
            .build();
        let outer = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .start_child(&sidebar_pane)
            .end_child(&inner)
            .resize_start_child(false)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .position(240)
            .vexpand(true)
            .build();
        let status = gtk::Label::builder()
            .xalign(0.0)
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(8)
            .margin_end(8)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&outer);
        body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        body.append(&status);
        window.set_child(Some(&body));

        let app = Rc::new(Self {
            model,
            application: application.clone(),
            window,
            clock,
            status,
            sidebar,
            content_slot,
            content: RefCell::new(None),
            detail,
            focus_on_show: Cell::new(false),
            autostart,
        });
        app.add_actions();

        crate::menus::follow(application, &app.model.view().commands);
        crate::menus::accelerators(application);
        // Hidden, not closed: the app stays resident, syncing.
        app.window.connect_close_request(|window| {
            window.set_visible(false);
            glib::Propagation::Stop
        });
        app
    }

    /// Shows the window on Today — unless started in the background, at sign-in — and
    /// starts what runs beside it.
    fn start(self: &Rc<Self>, background: bool, shortcuts: bool) {
        // Resident: closing the window does not end the app.
        std::mem::forget(self.application.hold());
        self.go(Place::Today, false);
        crate::appimage::keep_current(&self.application);
        if !background {
            self.show_window();
        }
        glib::timeout_add_seconds_local(1, || {
            if let Some(app) = app() {
                app.tick();
            }
            glib::ControlFlow::Continue
        });
        glib::timeout_add_seconds_local(60 * 60, || {
            if let Some(app) = app() {
                app.model.back_up_if_due();
            }
            glib::ControlFlow::Continue
        });
        // Resident, so in front for as long as it runs: the device's sync process once another
        // device is paired, when the app starts syncing, serving the command surface meanwhile.
        self.model.in_front(true);
        self.model.serve_commands();
        self.model.back_up_if_due();
        crate::reminders::start(self);
        crate::tray::start();
        if shortcuts {
            let parent = self.window.is_visible().then(|| self.window.clone().upcast::<gtk::Window>());
            // The app's own identifier, whatever the profile: the portal knows an app by its
            // desktop entry, and there is one. A second profile's copy runs --no-shortcuts.
            crate::shortcuts::start(
                crate::ID.to_owned(),
                parent,
                |kind| {
                    let Some(app) = app() else { return };
                    match kind {
                        crate::shortcuts::Kind::Show => app.show_window(),
                        crate::shortcuts::Kind::QuickAdd => app.quick_add(true),
                    }
                },
                |said| {
                    if let Some(app) = app() {
                        app.say(&said);
                    }
                },
            );
        }
    }

    /// Once a second: what another process wrote — `lum` shares this store — and once a
    /// minute, the clock.
    fn tick(&self) {
        // The app looks for itself whether the store or the minute moved: another process's
        // writes, a sync's arrivals, the clock.
        self.model.tick();
    }

    // -------------------------------------------------------------------------------------
    // Saying, failing, changing
    // -------------------------------------------------------------------------------------

    /// Writes a sentence on the status line, and announces it — once focus has landed
    /// on the row a change put it on (`tree::when_settled`).
    pub fn say(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let status = self.status.clone();
        let text = text.to_owned();
        crate::tree::when_settled(move || {
            status.set_label(&text);
            announce(&status, &text);
        });
    }

    pub fn fail(&self, message: &str) {
        prompts::fail(&self.window, message);
    }

    /// Opens a menu over `parent`: at a point in it after a right-click, else beside the
    /// widget with focus — the row a keyboard user asked about.
    ///
    /// `actions`, if given, are the row's own, under the `row.` prefix.
    pub fn popup(&self, menu: &gio::Menu, parent: &gtk::Widget, point: Option<(f64, f64)>, actions: Option<&gio::SimpleActionGroup>) {
        let popover = gtk::PopoverMenu::from_model(Some(menu));
        if let Some(actions) = actions {
            popover.insert_action_group("row", Some(actions));
        }
        popover.set_parent(parent);
        popover.set_has_arrow(false);
        popover.set_halign(gtk::Align::Start);
        let rectangle = match point {
            Some((x, y)) => Some(gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)),
            None => gtk::prelude::GtkWindowExt::focus(&self.window).and_then(|focus| focus.compute_bounds(parent)).map(|bounds| {
                gtk::gdk::Rectangle::new(
                    bounds.x() as i32 + 24,
                    bounds.y() as i32,
                    1,
                    bounds.height() as i32,
                )
            }),
        };
        popover.set_pointing_to(rectangle.as_ref());
        popover.connect_closed(|popover| {
            let popover = popover.clone();
            // After the action it chose has run, which the closing precedes.
            glib::idle_add_local_once(move || {
                popover.unparent();
                MENU_OPEN.with(|open| open.set(false));
                for future in AFTER_MENU.with(|waiting| waiting.take()) {
                    glib::spawn_future_local(future);
                }
            });
        });
        MENU_OPEN.with(|open| open.set(true));
        popover.popup();
    }

    /// Draws what the app last left on every screen drawn from it, and asks what it has open.
    fn draw(self: &Rc<Self>) {
        self.follow_place();
        crate::menus::follow(&self.application, &self.model.view().commands);
        self.sidebar.draw(self);
        if let Some(list) = self.task_list() {
            list.draw(self);
        }
        if let Some(day) = self.day() {
            day.draw(self);
        }
        self.detail.draw(self);
        crate::block_form::draw(self);
        crate::proposal::draw(self);
        crate::settings::draw(self);
        crate::reminders::draw(self);
        crate::quick_add::draw(self);
        crate::asking::update(self);
    }

    // -------------------------------------------------------------------------------------
    // Places and panes
    // -------------------------------------------------------------------------------------

    fn content(&self) -> Option<Content> {
        self.content.borrow().clone()
    }

    pub fn task_list(&self) -> Option<Rc<TaskList>> {
        match self.content()? {
            Content::Tasks(list) => Some(list),
            _ => None,
        }
    }

    pub fn day(&self) -> Option<Rc<DayView>> {
        match self.content()? {
            Content::Day(day) => Some(day),
            _ => None,
        }
    }

    /// Names the window after what it shows.
    pub fn set_title(&self, title: &str) {
        self.window.set_title(Some(&format!("{title} – Lumenna")));
    }

    /// Goes to a place: shows it, and — when `focus` — moves focus into it. The sidebar
    /// selects it as it draws.
    pub fn go(&self, place: Place, focus: bool) {
        self.show_place(place);
        if focus {
            self.focus_content();
        }
    }

    /// Shows a place in the middle pane, and clears the details until something is chosen.
    pub fn show_place(&self, place: Place) {
        self.model.send(lumenna_ffi::ShellEvent::Open(place.clone()));
        self.set_content(place);
    }

    /// The place the middle pane shows.
    fn content_place(&self) -> Option<Place> {
        match self.content()? {
            Content::Tasks(list) => Some(list.place.clone()),
            Content::Day(_) => Some(Place::Today),
        }
    }

    /// Follows the app to a place it went on to by itself — a place renamed, made or deleted —
    /// as the app holds it now: a turn made before the person last went somewhere still names
    /// where they were.
    fn follow_place(&self) {
        let Some(place) = self.model.view().place.clone() else { return };
        if self.content_place().as_ref() == Some(&place) {
            return;
        }
        if self.model.current().0.place.as_ref() == Some(&place) {
            self.set_content(place);
            if self.focus_on_show.take() {
                self.focus_content();
            }
        }
    }

    /// Moves through the days, going to the day first from another place, focus with it.
    fn day_move(&self, step: lumenna_app::DayMove) {
        if self.day().is_none() {
            self.focus_on_show.set(true);
        }
        self.model.send(lumenna_ffi::ShellEvent::Day(step));
    }

    /// Shows a block's day with the block in hand, as a reminder of it opens it.
    pub fn show_block(&self, series: &str, date: &str) {
        match self.day() {
            Some(day) => day.focus(),
            None => self.focus_on_show.set(true),
        }
        self.model.send(lumenna_ffi::ShellEvent::ShowBlock { series: series.to_owned(), date: date.to_owned() });
    }

    /// Puts a place's view in the middle pane, and clears the details until something is
    /// chosen.
    fn set_content(&self, place: Place) {
        let content = match &place {
            Place::Today => Content::Day(DayView::new()),
            _ => Content::Tasks(TaskList::new(place.clone())),
        };
        if let Some(old) = self.content.borrow_mut().take() {
            self.content_slot.remove(&old.widget());
        }
        self.content_slot.append(&content.widget());
        *self.content.borrow_mut() = Some(content.clone());
        self.set_title(&place.title());
        content.reload(self);
    }

    /// Moves focus into the middle pane.
    pub fn focus_content(&self) {
        match self.content() {
            Some(Content::Tasks(list)) => list.focus(),
            Some(Content::Day(day)) => day.focus(),
            None => {}
        }
    }

    /// Moves focus to the next pane, or the previous: places, the view, the details — the
    /// details only when they show a task.
    fn move_pane(&self, step: isize) {
        let current = if self.sidebar.tree.has_focus() {
            0
        } else if self.content().is_some_and(|c| c.has_focus()) {
            1
        } else if self.detail.has_focus() {
            2
        } else {
            // Nowhere in particular — the menu bar, the status line — counts as the places.
            if step > 0 { 2 } else { 1 }
        };
        let panes = if self.detail.task_id().is_some() { 3 } else { 2 };
        let next = (current as isize + step).rem_euclid(panes) as usize;
        match next {
            0 => self.sidebar.tree.focus(),
            1 => self.focus_content(),
            _ => {
                self.detail.focus();
            }
        }
    }

    // -------------------------------------------------------------------------------------
    // Showing and hiding
    // -------------------------------------------------------------------------------------

    pub fn show_window(&self) {
        self.window.present();
        // Run from an AppImage, the first window shown offers the applications menu, once.
        crate::appimage::offer(&self.window);
    }

    fn quit(&self) {
        // Serving stops before syncing does, and both before the app goes.
        self.model.in_front(false);
        self.application.quit();
    }

    /// Quick add, from the menu or from anywhere (`quick_add`).
    fn quick_add(&self, from_anywhere: bool) {
        crate::quick_add::open(self, from_anywhere);
    }

    // -------------------------------------------------------------------------------------
    // Commands
    // -------------------------------------------------------------------------------------

    /// The text field with focus, where the edit commands are the field's own.
    fn editing(&self) -> Option<gtk::Widget> {
        let focus = gtk::prelude::GtkWindowExt::focus(&self.window)?;
        let editable = focus.is::<gtk::Text>() || focus.is::<gtk::TextView>() || focus.is::<gtk::Editable>();
        editable.then_some(focus)
    }

    /// Every command the core's menus and tray name, as a window action `win.<id>`: what a
    /// row's or the day's command runs is the core's (`keys::what_runs`), handed on by its id;
    /// the rest are this window's to do.
    fn add_actions(self: &Rc<Self>) {
        for command in lumenna_app::keys::COMMANDS {
            let id = command.id;
            let action = gio::SimpleAction::new(id, None);
            action.connect_activate(move |_, _| {
                if let Some(app) = app() {
                    app.command(id);
                }
            });
            self.window.add_action(&action);
        }
    }

    /// Runs the command `id`, from the menu bar, its key, a button or the tray.
    pub fn command(self: &Rc<Self>, id: &str) {
        use lumenna_app::keys::Runs;
        use lumenna_ffi::ShellEvent;
        let key = || ShellEvent::Key(id.to_owned());
        match lumenna_app::keys::what_runs(id) {
            // A row's: the details' task where focus is there, or where the day shows a sitting,
            // whose task the details show; else the row in hand. Opening a row opens a block's
            // form in the day too. Whatever has none, the app says why.
            Runs::Row(_) => {
                let opening = matches!(id, "open-task" | "properties");
                if self.detail.has_focus() || (self.task_list().is_none() && !opening) {
                    self.model.send(ShellEvent::Details(lumenna_app::DetailsEvent::Key(id.to_owned())));
                } else {
                    self.model.send(key());
                }
            }
            // The day's own, on the day shown: from another place, Today first.
            Runs::Day(_) => {
                if self.day().is_none() {
                    self.go(Place::Today, true);
                }
                self.model.send(key());
            }
            Runs::App | Runs::Shell => self.own_command(id),
        }
    }

    /// The commands this window does itself: its windows, its panes, and what focus decides.
    fn own_command(self: &Rc<Self>, id: &str) {
        use lumenna_ffi::ShellEvent;
        match id {
            "show" => self.show_window(),
            "new-task" => self.quick_add(false),
            "sync-now" => self.model.send(ShellEvent::Devices(lumenna_app::DevicesEvent::SyncNow)),
            "settings" => crate::settings::show(self, crate::settings::Page::General),
            "devices" => crate::settings::show(self, crate::settings::Page::Devices),
            "export-import" => crate::settings::show(self, crate::settings::Page::Export),
            "back-up" => self.model.back_up(|done| {
                let Some(app) = self::app() else { return };
                match done {
                    Ok(done) => app.say(&lumenna_records::words::spoken(&done.announcement, &done.notices)),
                    Err(error) => app.fail(error.message()),
                }
            }),
            "restore-backup" => {
                let window = self.window.clone().upcast::<gtk::Window>();
                spawn(async move { crate::settings::import(&window, "Restore from a Backup", &[("Lumenna backups", "*.lumbak")]).await });
            }
            "new-project" => crate::sidebar::new_in(self, lumenna_records::places::SidebarGroup::Projects),
            "new-label" => crate::sidebar::new_in(self, lumenna_records::places::SidebarGroup::Labels),
            "new-filter" => crate::sidebar::new_in(self, lumenna_records::places::SidebarGroup::Filters),
            "close-window" => self.window.set_visible(false),
            "quit" => self.quit(),
            // In a field, its own typing; elsewhere, the store.
            "undo" => match self.editing() {
                Some(field) => drop(field.activate_action("text.undo", None)),
                None => self.model.send(ShellEvent::Undo),
            },
            "redo" => match self.editing() {
                Some(field) => drop(field.activate_action("text.redo", None)),
                None => self.model.send(ShellEvent::Redo),
            },
            "filter" => {
                if self.task_list().is_none_or(|list| !list.has_filter()) {
                    self.go(Place::Tasks, false);
                }
                if let Some(list) = self.task_list() {
                    list.focus_filter();
                }
            }
            "go-today" => self.go(Place::Today, true),
            "go-tasks" => self.go(Place::Tasks, true),
            "go-blocks" => self.go(Place::Blocks, true),
            "go-trash" => self.go(Place::Trash, true),
            "go-history" => self.go(Place::History, true),
            "next-pane" => self.move_pane(1),
            "previous-pane" => self.move_pane(-1),
            "save-task" => self.detail.save(self),
            "new-block" => {
                // Added where blocks are shown, to land on: the day, or the Blocks list.
                let blocks = self.task_list().is_some_and(|list| list.place == Place::Blocks);
                if self.day().is_none() && !blocks {
                    self.go(Place::Today, false);
                }
                crate::block_form::add(self);
            }
            // The day's moves go to Today first, from anywhere: the app's to do.
            "previous-day" => self.day_move(lumenna_app::DayMove::Previous),
            "next-day" => self.day_move(lumenna_app::DayMove::Next),
            "go-to-now" => self.day_move(lumenna_app::DayMove::Now),
            "go-to-day" => self.day_move(lumenna_app::DayMove::Ask),
            "keyboard-help" => crate::help::show(self),
            "about" => self.about(),
            _ => {}
        }
    }

    fn about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("Lumenna")
            .application_icon(crate::ID)
            .comments(lumenna_records::words::app_description())
            .version(env!("CARGO_PKG_VERSION"))
            .license_type(gtk::License::Agpl30)
            .website("https://github.com/emassey0135/lumenna")
            .developer_name("Elijah Massey")
            .build();
        // Its own title is "About", so Orca never heard whose; focus starts on the version.
        adw::prelude::AdwDialogExt::set_title(&about, "About Lumenna");
        adw::prelude::AdwDialogExt::present(&about, Some(&self.window));
    }
}
