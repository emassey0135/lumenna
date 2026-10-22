//! Reminders: giving this device's while the app runs, and a task's or block's list of them.
//!
//! When each fires and what it says are the core's (`Device::deliver_reminders`); this shows
//! each as a desktop notification, so GNOME shows it, Orca reads it as it reads every other one,
//! and the notification's own buttons dismiss, snooze, carry forward or replan. One process per
//! device gives them: while `lum notify` or another copy has the right, the device says so
//! (`elsewhere`), and this asks again every few seconds, as the sync service waits its turn.
//!
//! A task's or block's list of reminders is the app's (`ViewModel::reminders`), drawn as a
//! dialog while the app has it open.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_app::{ActingOn, Side};
use lumenna_ffi::ShellEvent;
use lumenna_records::reminders::{DueReminder, ReminderList};
use lumenna_records::{ActionKind, Change, LumennaError};

use crate::actions;
use crate::tree::{Item, Landing, Tree};
use crate::window::App;

/// How often a waiting copy asks for the reminders, and how often the holder gives them.
const ASK_EVERY: u32 = 5;
const DELIVER_EVERY: u32 = 20;

/// Starts giving reminders, now if no other process gives them, else when it stops.
pub fn start(app: &Rc<App>) {
    add_actions(app);
    // Seconds since the last look, and whether this process gives them: until it does, it
    // looks every few seconds for its turn.
    let since = Rc::new(Cell::new(DELIVER_EVERY));
    let giving = Rc::new(Cell::new(false));
    let tick = move || {
        let Some(app) = crate::window::app() else { return };
        since.set(since.get() + ASK_EVERY);
        if giving.get() && since.get() < DELIVER_EVERY {
            return;
        }
        since.set(0);
        match app.model.device.deliver_reminders(crate::clock::pinned_instant()) {
            Ok(found) => {
                giving.set(!found.elsewhere);
                deliver(&app, &found);
            }
            Err(error) => eprintln!("Reminders: {}", error.message()),
        }
    };
    tick();
    glib::timeout_add_seconds_local(ASK_EVERY, move || {
        tick();
        glib::ControlFlow::Continue
    });
}

fn deliver(app: &App, found: &lumenna_records::reminders::DueReminders) {
    for due in &found.due {
        notify(app, due);
    }
    if !found.missed_summary.is_empty() {
        let notification = gio::Notification::new("Lumenna");
        notification.set_body(Some(&found.missed_summary));
        notification.set_default_action("app.show-window");
        app.application.send_notification(Some("reminders-missed"), &notification);
    }
}

/// One reminder as a notification: its title and message, the core's two buttons, and
/// activating it opens what it is about.
fn notify(app: &App, due: &DueReminder) {
    let notification = gio::Notification::new(&due.title);
    notification.set_body(Some(&due.text));
    notification.set_priority(gio::NotificationPriority::High);
    let which = (due.key.clone(), due.occurrence.clone()).to_variant();
    notification.add_button_with_target_value(&due.dismiss, "app.reminder-dismiss", Some(&which));
    notification.add_button_with_target_value(&due.snooze, "app.reminder-snooze", Some(&which));
    // When a block's work outruns its time, the core's button that opens Replan from Now.
    if !due.replan.is_empty() {
        notification.add_button_with_target_value(&due.replan, "app.reminder-replan", Some(&which));
    }
    // At a block's end with work left short, the core's offer to carry it forward; the
    // message already ends in the question.
    if !due.carry.is_empty() {
        let carry = (due.key.clone(), due.occurrence.clone(), due.target.clone()).to_variant();
        notification.add_button_with_target_value(&due.carry, "app.reminder-carry", Some(&carry));
    }
    let target = (due.kind.clone(), due.target.clone(), due.occurrence.clone()).to_variant();
    notification.set_default_action_and_target_value("app.reminder-open", Some(&target));
    app.application.send_notification(Some(&id(&due.key, &due.occurrence)), &notification);
}

fn id(key: &str, occurrence: &str) -> String {
    format!("reminder-{key}-{occurrence}")
}

/// The notifications' buttons and their opening, as the application's actions: a
/// notification reaches the app by name, whether its window is up or not.
fn add_actions(app: &App) {
    let pair = glib::VariantTy::new("(ss)").expect("a valid type");
    let answer = |name: &str, run: fn(&App, String, String) -> Result<Change, LumennaError>| {
        let action = gio::SimpleAction::new(name, Some(pair));
        action.connect_activate(move |_, parameter| {
            let (Some(app), Some((key, occurrence))) = (crate::window::app(), parameter.and_then(|p| p.get::<(String, String)>())) else {
                return;
            };
            app.application.withdraw_notification(&id(&key, &occurrence));
            match run(&app, key, occurrence) {
                Ok(_) => app.model.tick(),
                Err(error) => eprintln!("Reminders: {}", error.message()),
            }
        });
        action
    };
    app.application.add_action(&answer("reminder-dismiss", |app, key, occurrence| {
        app.model.device.dismiss_reminder(key, occurrence)
    }));
    app.application.add_action(&answer("reminder-snooze", |app, key, occurrence| {
        app.model.device.snooze_reminder(key, occurrence, None)
    }));
    let carry = gio::SimpleAction::new("reminder-carry", Some(glib::VariantTy::new("(sss)").expect("a valid type")));
    carry.connect_activate(|_, parameter| {
        let (Some(app), Some((key, occurrence, target))) =
            (crate::window::app(), parameter.and_then(|p| p.get::<(String, String, String)>()))
        else {
            return;
        };
        app.application.withdraw_notification(&id(&key, &occurrence));
        match app.model.device.carry_rest_forward(format!("{target}@{occurrence}")) {
            Ok(change) => {
                app.model.tick();
                app.say(&lumenna_records::words::spoken(&change.announcement, &change.notices));
            }
            Err(error) => app.fail(error.message()),
        }
    });
    app.application.add_action(&carry);
    let open = gio::SimpleAction::new("reminder-open", Some(glib::VariantTy::new("(sss)").expect("a valid type")));
    open.connect_activate(|_, parameter| {
        let (Some(app), Some((kind, target, occurrence))) =
            (crate::window::app(), parameter.and_then(|p| p.get::<(String, String, String)>()))
        else {
            return;
        };
        open_target(&app, &kind, &target, &occurrence);
    });
    app.application.add_action(&open);
    let replan = gio::SimpleAction::new("reminder-replan", Some(pair));
    replan.connect_activate(|_, parameter| {
        let (Some(app), Some((key, occurrence))) =
            (crate::window::app(), parameter.and_then(|p| p.get::<(String, String)>()))
        else {
            return;
        };
        app.application.withdraw_notification(&id(&key, &occurrence));
        app.show_window();
        crate::proposal::replan(&app);
    });
    app.application.add_action(&replan);
    let show = gio::SimpleAction::new("show-window", None);
    show.connect_activate(|_, _| {
        if let Some(app) = crate::window::app() {
            app.show_window();
        }
    });
    app.application.add_action(&show);
}

/// Opens what a reminder is about: a task's details, or the block on its day.
fn open_target(app: &Rc<App>, kind: &str, target: &str, occurrence: &str) {
    app.show_window();
    if kind == "block" {
        app.show_block(target, occurrence.get(..10).unwrap_or(occurrence));
    } else {
        app.detail.open(app, target);
    }
}

// ---------------------------------------------------------------------------------------
// A task's or block's list of reminders
// ---------------------------------------------------------------------------------------

/// The Reminders dialog: the target's reminders, each with its own actions (Remove Reminder),
/// and the list's (Add Reminder). Their questions are asked over it, and what the app says
/// while it is in front is said on its status line.
struct List {
    window: gtk::Window,
    status: gtk::Label,
    tree: Rc<Tree>,
    add: gtk::Button,
    shown: RefCell<Option<ReminderList>>,
    landing: Landing,
    /// What to say once the window is in front, until it is.
    waiting: RefCell<Option<Vec<String>>>,
}

thread_local! {
    static OPEN: RefCell<Option<Rc<List>>> = const { RefCell::new(None) };
    /// What the next list opened is called: "Reminders for Call the bank".
    static TITLE: RefCell<String> = const { RefCell::new(String::new()) };
}

fn send(event: ShellEvent) {
    if let Some(app) = crate::window::app() {
        app.model.send(event);
    }
}

/// Names the reminder list about to open: `title` ("Reminders"), for what it is for.
pub fn name(title: &str, of: Option<&str>) {
    TITLE.set(of.map_or_else(|| title.to_owned(), |name| format!("{title} for {name}")));
}

/// The list's window, when it is in front: where its questions are asked from.
pub fn in_front() -> Option<gtk::Window> {
    OPEN.with_borrow(Clone::clone).map(|list| list.window.clone()).filter(GtkWindowExt::is_active)
}

/// Says `text` on the list, if it is open: whether it was.
pub fn say(text: &str) -> bool {
    let Some(list) = OPEN.with_borrow(Clone::clone) else { return false };
    if let Some(waiting) = list.waiting.borrow_mut().as_mut() {
        waiting.push(text.to_owned());
        return true;
    }
    list.status.set_label(text);
    crate::window::announce(&list.status, text);
    true
}

/// Draws the list the app has open: opens its window when it opens, closes it when the app
/// closes it.
pub fn draw(app: &App) {
    let view = app.model.view();
    let open = OPEN.with_borrow(Clone::clone);
    match (view.reminders.clone(), open) {
        (None, None) => {}
        (None, Some(list)) => {
            OPEN.set(None);
            list.window.destroy();
        }
        (Some(reminders), open) => {
            let list = open.unwrap_or_else(|| {
                let list = build(app);
                OPEN.set(Some(Rc::clone(&list)));
                list
            });
            list.show(app, reminders, &view.reminders_held);
        }
    }
}

impl List {
    fn show(&self, app: &App, reminders: ReminderList, held: &lumenna_app::HeldView) {
        let first = self.shown.borrow().is_none();
        let lines = app.model.lines();
        let items = reminders
            .reminders
            .iter()
            .enumerate()
            .map(|(at, reminder)| Item {
                key: reminder.id.clone(),
                text: lines.reminders.get(at).map_or_else(|| reminder.words.clone(), |line| line.text.clone()),
                ..Default::default()
            })
            .collect();
        if let Some(add) = actions::find(&reminders.offered, &[ActionKind::New]) {
            self.add.set_label(&format!("{}…", add.title.replace('_', "__")));
        }
        let had_focus = self.tree.has_focus();
        let before: Vec<String> =
            self.shown.borrow().as_ref().map(|list| list.reminders.iter().map(|r| r.id.clone()).collect()).unwrap_or_default();
        let added = (!first).then(|| reminders.reminders.iter().map(|r| r.id.clone()).find(|id| !before.contains(id))).flatten();
        let rebuilt = self.tree.set(items);
        self.tree.fit(8);
        let empty = reminders.reminders.is_empty();
        if first {
            let said = lumenna_records::words::spoken(&reminders.announcement, &reminders.notices);
            self.status.set_label(&said);
            if let Some(waiting) = self.waiting.borrow_mut().as_mut() {
                waiting.push(said);
            }
            self.window.present();
            if empty {
                crate::prompts::focus_on(&self.add);
            } else {
                self.tree.focus();
            }
        }
        *self.shown.borrow_mut() = Some(reminders);
        // The last one removed: focus goes to Add Reminder, never nowhere.
        if empty && had_focus {
            self.add.grab_focus();
            return;
        }
        // A reminder just added: focus goes to it, where the person can hear what it says.
        if let Some(index) = added.and_then(|id| self.tree.index_of(&id)) {
            self.tree.focus();
            self.tree.select(index);
            return;
        }
        self.landing.land(&self.tree, &self.window, held.focus.as_deref(), held.landing, rebuilt);
    }

    /// Runs one of a reminder's actions, held there so what it changes lands there; one the
    /// reminder lacks is refused by the app, which says why.
    fn act_on_selected(&self, kinds: &[ActionKind]) {
        let shown = self.shown.borrow().clone();
        let Some(reminder) = self.tree.selected().and_then(|index| shown.as_ref().and_then(|list| list.reminders.get(index).cloned())) else {
            return;
        };
        send(ShellEvent::Hold(Side::Reminders, reminder.id));
        send(ShellEvent::Run(kinds.to_vec()));
    }
}


fn build(app: &App) -> Rc<List> {
    let title = TITLE.take();
    let title = if title.is_empty() { "Reminders".to_owned() } else { title };
    let status = gtk::Label::builder().xalign(0.0).wrap(true).build();
    let tree = Tree::new("Reminders");
    let add = gtk::Button::with_label("Add Reminder…");
    let remove = gtk::Button::with_label("Remove Reminder");
    let close = gtk::Button::with_label("Close");
    let buttons = gtk::Box::builder().spacing(8).margin_top(12).build();
    buttons.append(&add);
    buttons.append(&remove);
    let spacer = gtk::Box::builder().hexpand(true).build();
    buttons.append(&spacer);
    buttons.append(&close);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    body.append(&status);
    body.append(&tree.widget);
    body.append(&buttons);
    let window = gtk::Window::builder()
        .title(&title)
        .modal(true)
        .transient_for(&app.window)
        .destroy_with_parent(true)
        .default_width(420)
        .child(&body)
        .build();
    window.set_default_widget(Some(&close));
    let list = Rc::new(List {
        window: window.clone(),
        status,
        tree,
        add: add.clone(),
        shown: RefCell::new(None),
        landing: Landing::default(),
        waiting: RefCell::new(Some(Vec::new())),
    });
    {
        let weak = Rc::downgrade(&list);
        add.connect_clicked(move |_| {
            let Some(list) = weak.upgrade() else { return };
            let action = list.shown.borrow().as_ref().and_then(|shown| actions::find(&shown.offered, &[ActionKind::New]).cloned());
            if let Some(action) = action {
                send(ShellEvent::ActOn(ActingOn::Reminders, action));
            }
        });
    }
    {
        let weak = Rc::downgrade(&list);
        remove.connect_clicked(move |_| {
            if let Some(list) = weak.upgrade() {
                list.act_on_selected(actions::DELETE);
            }
        });
    }
    {
        // The person's moves are the app's to know, so a change lands back on the row.
        let weak = Rc::downgrade(&list);
        list.tree.connect_selected(move |index| {
            let Some(list) = weak.upgrade() else { return };
            if let Some(key) = index.and_then(|index| list.tree.key(index)) {
                send(ShellEvent::Hold(Side::Reminders, key));
            }
        });
    }
    {
        let weak = Rc::downgrade(&list);
        list.tree.connect_key(move |key, modifiers, _| {
            let Some(list) = weak.upgrade() else { return glib::Propagation::Proceed };
            if matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) && modifiers.is_empty() {
                list.act_on_selected(actions::DELETE);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    {
        let weak = Rc::downgrade(&list);
        list.tree.connect_menu(move |index, point| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(shown) = list.shown.borrow().clone() else { return };
            let Some(reminder) = shown.reminders.get(index).cloned() else { return };
            // A reminder of the defaults offers nothing of its own: the list's own actions, then.
            let (offered, on) = if reminder.offered.is_empty() {
                (shown.offered.clone(), ActingOn::Reminders)
            } else {
                (reminder.offered.clone(), ActingOn::Held(Side::Reminders, reminder.id.clone()))
            };
            let (menu, group) = actions::menu(&offered, &on, None);
            app.popup(&menu, list.tree.view.upcast_ref(), point, Some(&group));
        });
    }
    close.connect_clicked(|_| send(ShellEvent::CloseReminders));
    window.connect_close_request(|_| {
        send(ShellEvent::CloseReminders);
        glib::Propagation::Stop
    });
    let escape = gtk::EventControllerKey::new();
    escape.connect_key_pressed(|_, key, _, _| {
        if key == gdk::Key::Escape {
            send(ShellEvent::CloseReminders);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(escape);
    // What the list holds, and whether they are its own or the defaults, said once the
    // window is in front and Orca has read its title and focus, which would cut it off.
    let weak = Rc::downgrade(&list);
    let said = Cell::new(false);
    window.connect_is_active_notify(move |window| {
        if !window.is_active() || said.replace(true) {
            return;
        }
        let weak = weak.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
            let Some(list) = weak.upgrade() else { return };
            for text in list.waiting.take().unwrap_or_default() {
                say(&text);
            }
        });
    });
    list
}
