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
use lumenna_desktop::places::Place;
use lumenna_desktop::speech;
use lumenna_surface::{Change, Lumenna, Result};

use crate::clock::Locale;
use crate::core::{Core, Event, sentence};
use crate::detail::Detail;
use crate::sidebar::Sidebar;
use crate::task_actions::{self, Command};
use crate::blocks::BlockList;
use crate::day::DayView;
use crate::tasks::TaskList;
use crate::tree::Tree;
use crate::{prompts, quick_add};

/// The view in the middle pane.
#[derive(Clone)]
pub enum Content {
    Tasks(Rc<TaskList>),
    Day(Rc<DayView>),
    Blocks(Rc<BlockList>),
}

impl Content {
    fn widget(&self) -> gtk::Widget {
        match self {
            Self::Tasks(list) => list.widget.clone().upcast(),
            Self::Day(day) => day.widget.clone().upcast(),
            Self::Blocks(blocks) => blocks.widget.clone().upcast(),
        }
    }

    fn tree(&self) -> &Tree {
        match self {
            Self::Tasks(list) => &list.tree,
            Self::Day(day) => &day.tree,
            Self::Blocks(blocks) => &blocks.tree,
        }
    }

    fn reload(&self, app: &App) {
        match self {
            Self::Tasks(list) => list.list(app),
            Self::Day(day) => day.reload(app),
            Self::Blocks(blocks) => blocks.reload(app),
        }
    }

    fn has_focus(&self) -> bool {
        let widget = self.widget();
        widget.root().and_then(|root| root.focus()).is_some_and(|focus| focus.is_ancestor(&widget) || focus == widget)
    }
}

pub struct App {
    pub core: Core,
    pub application: gtk::Application,
    pub window: gtk::ApplicationWindow,
    pub clock: Locale,
    status: gtk::Label,
    pub sidebar: Rc<Sidebar>,
    content_slot: gtk::Box,
    content: RefCell<Option<Content>>,
    pub detail: Rc<Detail>,
    minute: Cell<i64>,
    /// Settings' Devices page, while it is open, to hear how a sync went.
    pub devices_page: RefCell<Option<std::rc::Weak<crate::settings::Devices>>>,
    /// What another process had written by the last tick (`Lumenna::outside_version`).
    outside: Cell<i64>,
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

/// An event from another thread, now on the main one.
pub fn receive(event: Event) {
    let Some(app) = app() else { return };
    match event {
        Event::Changed => {
            app.store_changed();
            crate::settings::devices_heard(&app, None);
        }
        Event::Say(text) => {
            app.say(&text);
            crate::settings::devices_heard(&app, Some(&text));
        }
        Event::ShowWindow => app.show_window(),
        Event::QuickAdd => app.quick_add(true),
        Event::SyncNow => app.core.sync_now(),
        Event::Quit => app.quit(),
    }
}

/// The application was activated: the first time, opens the store and the window; after
/// that — the app started again, which GApplication turns into this — shows the window.
pub fn activate(application: &gtk::Application, directory: &Path, background: bool, shortcuts: bool) {
    if let Some(app) = app() {
        app.show_window();
        return;
    }
    let core = match Core::open(directory) {
        Ok(core) => core,
        Err(error) => {
            let message = format!("Lumenna could not open its data. {}", sentence(&error));
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
    let app = App::build(application, core);
    APP.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&app)));
    app.start(background, shortcuts);
}

impl App {
    fn build(application: &gtk::Application, core: Core) -> Rc<Self> {
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
            core,
            application: application.clone(),
            window,
            clock: Locale::new(),
            status,
            sidebar,
            content_slot,
            content: RefCell::new(None),
            detail,
            minute: Cell::new(0),
            outside: Cell::new(0),
            devices_page: RefCell::new(None),
        });
        app.add_actions();

        application.set_menubar(Some(&menu_bar()));
        for (action, keys) in ACCELERATORS {
            application.set_accels_for_action(action, keys);
        }
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
        self.sidebar.reload(self);
        self.go(Place::Today, false);
        if !background {
            self.show_window();
        }
        self.minute.set(jiff::Timestamp::now().as_second() / 60);
        self.outside.set(self.core.lumenna.outside_version().unwrap_or(0));
        glib::timeout_add_seconds_local(1, || {
            if let Some(app) = app() {
                app.tick();
            }
            glib::ControlFlow::Continue
        });
        glib::timeout_add_seconds_local(60 * 60, || {
            if let Some(app) = app() {
                app.core.back_up_if_due();
            }
            glib::ControlFlow::Continue
        });
        self.core.start_syncing();
        self.core.back_up_if_due();
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
        // By `outside_version`, as every client does: it moves only for another process's
        // writes, which nothing else here notices — the app's own edits redraw as they are
        // made, and a sync's arrivals come through the listener.
        if let Ok(outside) = self.core.lumenna.outside_version()
            && outside != self.outside.replace(outside)
        {
            self.store_changed();
        }
        let minute = jiff::Timestamp::now().as_second() / 60;
        if minute != self.minute.get() {
            self.minute.set(minute);
            if let Some(day) = self.day() {
                day.minute(self);
            }
        }
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

    /// Says what a change did, and anything else worth saying.
    pub fn say_change(&self, change: &Change) {
        self.say(&speech::sentence(&speech::announcement(&change.announcement, &change.notices)));
    }

    pub fn fail(&self, message: &str) {
        prompts::fail(&self.window, message);
    }

    /// Runs a change and has every view read the store again. The caller then puts the
    /// selection where it belongs and says what happened, in that order, so the
    /// announcement follows the row focus lands on rather than being cut off by it.
    pub fn perform(&self, operation: impl FnOnce(&Lumenna) -> Result<Change>) -> Option<Change> {
        match operation(&self.core.lumenna) {
            Ok(change) => {
                self.store_changed();
                Some(change)
            }
            Err(error) => {
                self.fail(&sentence(&error));
                None
            }
        }
    }

    /// The store changed — here, in another process, or from another device. Everything
    /// showing it reads it again, keeping its selection.
    pub fn store_changed(&self) {
        self.sidebar.reload(self);
        if let Some(content) = self.content() {
            content.reload(self);
        }
        self.detail.reload(self);
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

    pub fn blocks(&self) -> Option<Rc<BlockList>> {
        match self.content()? {
            Content::Blocks(blocks) => Some(blocks),
            _ => None,
        }
    }

    /// Names the window after what it shows.
    pub fn set_title(&self, title: &str) {
        self.window.set_title(Some(&format!("{title} – Lumenna")));
    }

    /// Goes to a place: selects it in the sidebar, shows it, and — when `focus` — moves
    /// focus into it.
    pub fn go(&self, place: Place, focus: bool) {
        self.sidebar.select(&place);
        self.show_place(place);
        if focus {
            self.focus_content();
        }
    }

    /// Shows a place in the middle pane, and clears the details until something is chosen.
    pub fn show_place(&self, place: Place) {
        let content = match &place {
            Place::Today => Content::Day(DayView::new()),
            Place::Blocks => Content::Blocks(BlockList::new()),
            _ => Content::Tasks(TaskList::new(place.clone(), self.core.lumenna.clone())),
        };
        if let Some(old) = self.content.borrow_mut().take() {
            self.content_slot.remove(&old.widget());
        }
        self.content_slot.append(&content.widget());
        *self.content.borrow_mut() = Some(content.clone());
        self.set_title(&place.title());
        self.detail.show(self, None);
        content.reload(self);
    }

    /// Moves focus into the middle pane.
    pub fn focus_content(&self) {
        if let Some(content) = self.content() {
            content.tree().focus();
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

    /// Opens the selected task's details: focus to the first field.
    pub fn open_detail(&self) {
        self.detail.focus();
    }

    // -------------------------------------------------------------------------------------
    // Showing and hiding
    // -------------------------------------------------------------------------------------

    pub fn show_window(&self) {
        self.window.present();
    }

    fn quit(&self) {
        self.core.stop_syncing();
        self.application.quit();
    }

    /// Quick add, from the menu or from anywhere: from the menu it starts with the place
    /// shown, so a task added while looking at a project lands in it; from anywhere it starts
    /// empty, and stands on its own unless the window is up.
    fn quick_add(self: &Rc<Self>, from_anywhere: bool) {
        let prefix =
            if from_anywhere { String::new() } else { self.task_list().map(|l| l.quick_add_prefix()).unwrap_or_default() };
        let app = Rc::clone(self);
        spawn(async move {
            let parent = app.window.is_visible().then(|| app.window.clone().upcast::<gtk::Window>());
            let lumenna = app.core.lumenna.clone();
            let Some(change) = quick_add::run(parent.as_ref(), &app.application, lumenna, &prefix).await else { return };
            app.store_changed();
            if let (Some(list), Some(task)) = (app.task_list(), change.task.as_ref()) {
                list.land_on(&task.id);
            }
            app.say_change(&change);
        });
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

    /// The task the Task menu acts on: the one in the details when focus is there, else the
    /// one selected in the list.
    fn task_in_hand(&self) -> Option<String> {
        if self.detail.has_focus() {
            return self.detail.task_id();
        }
        match self.content()? {
            Content::Tasks(list) if !list.is_trash() => list.selected().map(|row| row.id),
            Content::Day(day) => day.selected_task(),
            _ => None,
        }
    }

    /// The task selected in the trash, for its two commands.
    fn trashed_in_hand(&self) -> Option<String> {
        match self.content()? {
            Content::Tasks(list) if list.is_trash() => list.selected().map(|row| row.id),
            _ => None,
        }
    }

    fn add_actions(self: &Rc<Self>) {
        let simple = |name: &str, run: fn(&Rc<App>)| {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(move |_, _| {
                if let Some(app) = app() {
                    run(&app);
                }
            });
            self.window.add_action(&action);
        };
        simple("new-task", |app| app.quick_add(false));
        simple("sync-now", |app| app.core.sync_now());
        simple("settings", |app| crate::settings::show(app, crate::settings::Page::General));
        simple("devices", |app| crate::settings::show(app, crate::settings::Page::Devices));
        simple("export-import", |app| crate::settings::show(app, crate::settings::Page::Export));
        simple("back-up", |app| match app.core.lumenna.backup(None) {
            Ok(done) => app.say(&speech::announcement(&done.announcement, &done.notices)),
            Err(error) => app.fail(&sentence(&error)),
        });
        simple("restore-backup", |app| {
            let app = Rc::clone(app);
            spawn(async move {
                let window = app.window.clone().upcast::<gtk::Window>();
                let types = [("Lumenna backups", "*.lumbak")];
                if let Some(said) = crate::settings::import(&app, &window, "Restore From a Backup", &types).await {
                    app.say(&said);
                }
            });
        });
        simple("new-project", |app| {
            let app = Rc::clone(app);
            spawn(async move { crate::sidebar::new_project(&app, None).await });
        });
        simple("new-label", |app| {
            let app = Rc::clone(app);
            spawn(async move { crate::sidebar::new_label(&app).await });
        });
        simple("new-filter", |app| {
            let app = Rc::clone(app);
            spawn(async move { crate::sidebar::new_filter(&app).await });
        });
        simple("close-window", |app| app.window.set_visible(false));
        simple("quit", |app| app.quit());
        simple("undo", |app| {
            // In a field, its own typing; elsewhere, the store.
            if let Some(field) = app.editing() {
                let _ = field.activate_action("text.undo", None);
            } else if let Some(change) = app.perform(Lumenna::undo) {
                app.say_change(&change);
            }
        });
        simple("redo", |app| {
            if let Some(field) = app.editing() {
                let _ = field.activate_action("text.redo", None);
            } else if let Some(change) = app.perform(Lumenna::redo) {
                app.say_change(&change);
            }
        });
        simple("filter", |app| {
            if app.task_list().is_none_or(|list| list.is_trash()) {
                app.go(Place::Tasks, false);
            }
            if let Some(list) = app.task_list() {
                list.focus_filter();
            }
        });
        simple("go-today", |app| app.go(Place::Today, true));
        simple("go-tasks", |app| app.go(Place::Tasks, true));
        simple("go-blocks", |app| app.go(Place::Blocks, true));
        simple("go-trash", |app| app.go(Place::Trash, true));
        simple("next-pane", |app| app.move_pane(1));
        simple("previous-pane", |app| app.move_pane(-1));
        simple("open-task", |app| {
            if app.task_in_hand().is_some() {
                app.open_detail();
            }
        });
        simple("save-task", |app| app.detail.save(app));
        simple("new-block", |app| {
            if app.day().is_none() && app.blocks().is_none() {
                app.go(Place::Today, false);
            }
            if let Some(day) = app.day() {
                day.add_block(app, None, None);
            } else if app.blocks().is_some() {
                let app = Rc::clone(app);
                spawn(async move {
                    let fields = crate::block_form::new_fields("09:00", 60);
                    let window = app.window.clone().upcast::<gtk::Window>();
                    let purpose = crate::block_form::Purpose::Add { date: "today".to_owned() };
                    let Some(change) = crate::block_form::run(&window, app.core.lumenna.clone(), purpose, fields, None).await else {
                        return;
                    };
                    app.store_changed();
                    if let (Some(blocks), Some(series)) = (app.blocks(), change.affected.blocks.first()) {
                        blocks.land_on(series);
                    }
                    app.say_change(&change);
                });
            }
        });
        // The day's commands go to Today first, from anywhere.
        let on_day = |name: &str, run: fn(&Rc<App>, &Rc<DayView>)| {
            simple_with(self, name, move |app| {
                if app.day().is_none() {
                    app.go(Place::Today, true);
                }
                if let Some(day) = app.day() {
                    run(app, &day);
                }
            });
        };
        on_day("previous-day", |app, day| day.step(app, -1));
        on_day("next-day", |app, day| day.step(app, 1));
        on_day("go-to-now", |app, day| day.go_to_now(app, true));
        on_day("go-to-day", |app, day| day.ask_for_day(app));
        let task_command = |name: &str, command: Command| {
            simple_with(self, name, move |app| {
                if let Some(id) = app.task_in_hand() {
                    task_actions::run(app, command.clone(), &id);
                }
            });
        };
        task_command("mark-done", Command::MarkDone);
        task_command("put-in-block", Command::PutInBlock);
        task_command("move-to-project", Command::MoveToProject);
        task_command("make-subtask", Command::MakeSubtask);
        task_command("move-to-top", Command::MoveToTop);
        task_command("wait-for", Command::WaitFor);
        task_command("trash-task", Command::Trash);
        let trashed_command = |name: &str, command: Command| {
            simple_with(self, name, move |app| {
                if let Some(id) = app.trashed_in_hand() {
                    task_actions::run(app, command.clone(), &id);
                }
            });
        };
        trashed_command("restore-task", Command::Restore);
        trashed_command("erase-task", Command::Erase);
        let stop_waiting = gio::SimpleAction::new("stop-waiting", Some(glib::VariantTy::STRING));
        stop_waiting.connect_activate(|_, parameter| {
            let (Some(app), Some(other)) = (app(), parameter.and_then(|p| p.get::<String>())) else { return };
            if let Some(id) = app.task_in_hand() {
                task_actions::run(&app, Command::StopWaiting(other), &id);
            }
        });
        self.window.add_action(&stop_waiting);
        simple("about", |app| {
            gtk::AboutDialog::builder()
                .transient_for(&app.window)
                .modal(true)
                .program_name("Lumenna")
                .comments("Tasks, and the time blocks you work through them in.")
                .version(env!("CARGO_PKG_VERSION"))
                .license_type(gtk::License::Agpl30)
                .website("https://github.com/emassey0135/lumenna")
                .build()
                .present();
        });
    }
}

/// Adds a window action that runs `run` with the app.
fn simple_with(app: &App, name: &str, run: impl Fn(&Rc<App>) + 'static) {
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| {
        if let Some(app) = self::app() {
            run(&app);
        }
    });
    app.window.add_action(&action);
}

/// The shortcuts, by action. GNOME's conventions where it has one: Ctrl+Shift+Z redoes.
const ACCELERATORS: &[(&str, &[&str])] = &[
    ("win.new-task", &["<Control>n"]),
    ("win.sync-now", &["F5"]),
    ("win.settings", &["<Control>comma"]),
    ("win.close-window", &["<Control>w"]),
    ("win.quit", &["<Control>q"]),
    ("win.undo", &["<Control>z"]),
    ("win.redo", &["<Control><Shift>z", "<Control>y"]),
    ("win.filter", &["<Control>f"]),
    ("win.go-today", &["<Control>1"]),
    ("win.go-tasks", &["<Control>2"]),
    ("win.go-blocks", &["<Control>3"]),
    ("win.go-trash", &["<Control>4"]),
    ("win.next-pane", &["F6"]),
    ("win.previous-pane", &["<Shift>F6"]),
    ("win.mark-done", &["<Control>k"]),
    ("win.save-task", &["<Control>s"]),
    ("win.put-in-block", &["<Control>b"]),
    ("win.move-to-project", &["<Control><Shift>m"]),
    ("win.previous-day", &["<Control>Page_Up"]),
    ("win.next-day", &["<Control>Page_Down"]),
    ("win.go-to-now", &["<Control>t"]),
    ("win.go-to-day", &["<Control>g"]),
    ("win.new-block", &["<Control><Shift>n"]),
];

/// The menu bar. Mnemonics are GTK's underscores.
fn menu_bar() -> gio::Menu {
    let item = |label: &str, action: &str| gio::MenuItem::new(Some(label), Some(action));
    // A key shown beside an item that the widget with focus answers itself, so it is not
    // bound as an accelerator.
    let shown = |label: &str, action: &str, key: &str| {
        let item = item(label, action);
        item.set_attribute_value("accel", Some(&key.to_variant()));
        item
    };
    let section = |items: Vec<gio::MenuItem>| {
        let section = gio::Menu::new();
        for item in items {
            section.append_item(&item);
        }
        section
    };
    let submenu = |sections: Vec<gio::Menu>| {
        let menu = gio::Menu::new();
        for part in sections {
            menu.append_section(None, &part);
        }
        menu
    };
    let bar = gio::Menu::new();
    bar.append_submenu(
        Some("_File"),
        &submenu(vec![
            section(vec![
                item("_New Task…", "win.new-task"),
                item("New _Block…", "win.new-block"),
                item("New _Project…", "win.new-project"),
                item("New _Label…", "win.new-label"),
                item("New Saved _Filter…", "win.new-filter"),
            ]),
            section(vec![
                item("_Sync Now", "win.sync-now"),
                item("_Devices and Pairing…", "win.devices"),
                item("Back _Up Now", "win.back-up"),
                item("_Restore From a Backup…", "win.restore-backup"),
                item("_Export and Import…", "win.export-import"),
            ]),
            section(vec![item("Se_ttings…", "win.settings")]),
            section(vec![item("_Close Window", "win.close-window"), item("_Quit", "win.quit")]),
        ]),
    );
    bar.append_submenu(
        Some("_Edit"),
        &submenu(vec![
            section(vec![item("_Undo", "win.undo"), item("_Redo", "win.redo")]),
            section(vec![item("_Filter Tasks", "win.filter")]),
        ]),
    );
    bar.append_submenu(
        Some("_View"),
        &submenu(vec![
            section(vec![
                item("_Today", "win.go-today"),
                item("T_asks", "win.go-tasks"),
                item("_Blocks", "win.go-blocks"),
                item("T_rash", "win.go-trash"),
            ]),
            section(vec![item("_Next Pane", "win.next-pane"), item("_Previous Pane", "win.previous-pane")]),
        ]),
    );
    bar.append_submenu(
        Some("_Task"),
        &submenu(vec![
            section(vec![
                item("_Mark Done", "win.mark-done"),
                shown("_Edit Details", "win.open-task", "Return"),
                item("_Save Changes", "win.save-task"),
            ]),
            section(vec![
                item("Put in a _Block…", "win.put-in-block"),
                item("Move to _Project…", "win.move-to-project"),
                item("Make S_ubtask Of…", "win.make-subtask"),
                item("Move to _Top Level", "win.move-to-top"),
                item("_Wait For…", "win.wait-for"),
            ]),
            section(vec![
                shown("Move to T_rash", "win.trash-task", "Delete"),
                item("Rest_ore From Trash", "win.restore-task"),
                item("Erase _for Good…", "win.erase-task"),
            ]),
        ]),
    );
    bar.append_submenu(
        Some("_Day"),
        &submenu(vec![
            section(vec![
                item("_Previous Day", "win.previous-day"),
                item("_Next Day", "win.next-day"),
                item("Go to N_ow", "win.go-to-now"),
                item("_Go to Day…", "win.go-to-day"),
            ]),
            section(vec![item("_Add Block…", "win.new-block")]),
        ]),
    );
    bar.append_submenu(Some("_Help"), &submenu(vec![section(vec![item("_About Lumenna", "win.about")])]));
    bar
}
