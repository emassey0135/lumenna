//! Reminders: giving this device's while the app runs, and a task's or block's list of them.
//!
//! When each fires and what it says are the core's (`deliver_reminders`); this shows each as
//! a desktop notification, so GNOME shows it, Orca reads it as it reads every other one, and
//! the notification's own buttons dismiss or snooze it. One process per device gives them
//! (`take_reminders`): while `lum notify` or another copy holds them, this asks again now and
//! then, as the sync service waits its turn.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_desktop::speech;
use lumenna_host::actions::{Action, ActionKind, Subject};
use lumenna_host::{DueReminder, ReminderLock, ReminderView};

use crate::actions::{self, Asking};
use crate::core::sentence;
use crate::tree::{Item, Tree};

use crate::window::App;

/// How often a waiting copy asks for the reminders, and how often the holder gives them.
const ASK_EVERY: u32 = 5;
const DELIVER_EVERY: u32 = 20;

thread_local! {
    static LOCK: RefCell<Option<Arc<ReminderLock>>> = const { RefCell::new(None) };
}

/// Starts giving reminders, now if no other process gives them, else when it stops.
pub fn start(app: &Rc<App>) {
    add_actions(app);
    let since = Rc::new(Cell::new(0));
    let tick = move || {
        let Some(app) = crate::window::app() else { return };
        let held = LOCK.with(|lock| lock.borrow().clone());
        let lock = match held {
            Some(lock) => lock,
            None => match app.core.lumenna.take_reminders() {
                Ok(Some(lock)) => {
                    LOCK.with(|slot| *slot.borrow_mut() = Some(lock.clone()));
                    since.set(DELIVER_EVERY);
                    lock
                }
                Ok(None) => return,
                Err(error) => {
                    eprintln!("Reminders: {}", sentence(&error));
                    return;
                }
            },
        };
        since.set(since.get() + ASK_EVERY);
        if since.get() >= DELIVER_EVERY {
            since.set(0);
            deliver(&app, lock);
        }
    };
    tick();
    glib::timeout_add_seconds_local(ASK_EVERY, move || {
        tick();
        glib::ControlFlow::Continue
    });
}

fn deliver(app: &App, lock: Arc<ReminderLock>) {
    let found = match app.core.lumenna.deliver_reminders(lock, crate::clock::pinned_instant()) {
        Ok(found) => found,
        Err(error) => return eprintln!("Reminders: {}", sentence(&error)),
    };
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
    let answer = |name: &str, run: fn(&App, String, String) -> lumenna_host::Result<lumenna_host::Change>| {
        let action = gio::SimpleAction::new(name, Some(pair));
        action.connect_activate(move |_, parameter| {
            let (Some(app), Some((key, occurrence))) = (crate::window::app(), parameter.and_then(|p| p.get::<(String, String)>())) else {
                return;
            };
            app.application.withdraw_notification(&id(&key, &occurrence));
            match run(&app, key, occurrence) {
                Ok(_) => app.store_changed(),
                Err(error) => eprintln!("Reminders: {}", sentence(&error)),
            }
        });
        action
    };
    app.application.add_action(&answer("reminder-dismiss", |app, key, occurrence| {
        app.core.lumenna.dismiss_reminder(key, occurrence)
    }));
    app.application.add_action(&answer("reminder-snooze", |app, key, occurrence| {
        app.core.lumenna.snooze_reminder(key, occurrence, None)
    }));
    let carry = gio::SimpleAction::new("reminder-carry", Some(glib::VariantTy::new("(sss)").expect("a valid type")));
    carry.connect_activate(|_, parameter| {
        let (Some(app), Some((key, occurrence, target))) =
            (crate::window::app(), parameter.and_then(|p| p.get::<(String, String, String)>()))
        else {
            return;
        };
        app.application.withdraw_notification(&id(&key, &occurrence));
        match app.core.lumenna.carry_rest_forward(&format!("{target}@{occurrence}")) {
            Ok(change) => {
                app.store_changed();
                app.say_change(&change);
            }
            Err(error) => app.fail(&sentence(&error)),
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
/// and the list's (Add Reminder), all asked over this window and said on its status line.
struct List {
    target: String,
    window: gtk::Window,
    status: gtk::Label,
    tree: Rc<Tree>,
    add: gtk::Button,
    reminders: RefCell<Vec<ReminderView>>,
    actions: RefCell<Vec<Action>>,
}

impl List {
    fn asking(self: &Rc<Self>) -> Asking {
        let weak = Rc::downgrade(self);
        let say: actions::Say = Rc::new(move |text: &str| {
            if let Some(list) = weak.upgrade() {
                list.say(text);
            }
        });
        Asking { window: self.window.clone(), say: Some(say) }
    }

    fn say(&self, text: &str) {
        self.status.set_label(text);
        crate::window::announce(&self.status, text);
    }

    /// Runs one of the list's or a reminder's actions, then shows the list as it now is.
    fn run(self: &Rc<Self>, app: &Rc<App>, action: Action) {
        actions::run_from(app, action, Some(self.after()), self.asking());
    }

    /// Shows the list as an action left it: on the reminder just added, or, with the last
    /// one removed and the list hidden, on Add Reminder, so focus never goes nowhere.
    fn after(self: &Rc<Self>) -> actions::After {
        let weak = Rc::downgrade(self);
        Rc::new(move |app, _, _| {
            let Some(list) = weak.upgrade() else { return };
            let before: Vec<String> = list.reminders.borrow().iter().map(|r| r.id.clone()).collect();
            list.reload(app);
            let added = list.reminders.borrow().iter().map(|r| r.id.clone()).find(|id| !before.contains(id));
            if let Some(added) = added {
                list.tree.focus();
                list.tree.select_key_or_near(Some(&added), None);
            } else if list.tree.len() == 0 {
                list.add.grab_focus();
            }
        })
    }

    /// The selected reminder's action of `kinds`, or why it has none.
    fn act_on_selected(self: &Rc<Self>, app: &Rc<App>, kinds: &[ActionKind]) {
        let reminder = self.tree.selected().and_then(|index| self.reminders.borrow().get(index).cloned());
        match reminder.as_ref().and_then(|r| actions::find(&r.actions, kinds)) {
            Some(action) => self.run(app, action.clone()),
            None => self.say(&lumenna_host::not_offered(kinds[0], Subject::Reminder, false)),
        }
    }

    /// Fills the list from the core; the first time, focus goes to it, or to Add Reminder
    /// when there is nothing in it.
    fn reload(&self, app: &App) -> bool {
        let key = self.tree.selected().and_then(|index| self.tree.key(index));
        let near = self.tree.selected().or(Some(0));
        match app.core.lumenna.reminders_of(self.target.clone()) {
            Ok(list) => {
                self.status.set_label(&speech::sentence(&speech::announcement(&list.announcement, &list.notices)));
                let items = list.reminders.iter().map(|r| Item { key: r.id.clone(), text: r.words.clone(), depth: 0, ..Default::default() }).collect();
                *self.reminders.borrow_mut() = list.reminders;
                *self.actions.borrow_mut() = list.actions;
                if let Some(add) = actions::find(&self.actions.borrow(), &[ActionKind::New]) {
                    self.add.set_label(&format!("{}…", add.title.replace('_', "__")));
                }
                if self.tree.set(items) {
                    self.tree.select_key_or_near(key.as_deref(), near);
                }
                self.tree.fit(8);
                true
            }
            Err(error) => {
                self.status.set_label(&sentence(&error));
                false
            }
        }
    }
}

/// Opens the Reminders list of a task or block series, from its Reminders action.
pub fn show(app: &Rc<App>, action: &Action) {
    let name = match action.subject {
        Subject::Task => app.core.lumenna.show_task(&action.target).map(|shown| shown.task.title).ok(),
        _ => app.core.lumenna.show_block(&action.target).map(|shown| shown.title).ok(),
    };
    let title = name.map_or_else(|| action.title.clone(), |name| format!("{} for {name}", action.title));
    let status = gtk::Label::builder().xalign(0.0).wrap(true).build();
    let tree = Tree::new(&action.title);
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
        target: action.target.clone(),
        window: window.clone(),
        status,
        tree,
        add: add.clone(),
        reminders: RefCell::new(Vec::new()),
        actions: RefCell::new(Vec::new()),
    });
    if !list.reload(app) {
        return app.fail(&list.status.label());
    }
    {
        let weak = Rc::downgrade(&list);
        add.connect_clicked(move |_| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let action = actions::find(&list.actions.borrow(), &[ActionKind::New]).cloned();
            match action {
                Some(action) => list.run(&app, action),
                None => list.say(&lumenna_host::not_offered(ActionKind::New, Subject::Reminder, false)),
            }
        });
    }
    {
        let weak = Rc::downgrade(&list);
        remove.connect_clicked(move |_| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            list.act_on_selected(&app, actions::DELETE);
        });
    }
    {
        let weak = Rc::downgrade(&list);
        list.tree.connect_key(move |key, modifiers, _| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return glib::Propagation::Proceed };
            if matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) && modifiers.is_empty() {
                list.act_on_selected(&app, actions::DELETE);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    {
        let weak = Rc::downgrade(&list);
        list.tree.connect_menu(move |index, point| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(reminder) = list.reminders.borrow().get(index).cloned() else { return };
            let actions = if reminder.actions.is_empty() { list.actions.borrow().clone() } else { reminder.actions };
            let (menu, group) = actions::menu_from(&actions, Some(list.after()), Some(list.asking()));
            app.popup(&menu, list.tree.view.upcast_ref(), point, Some(&group));
        });
    }
    {
        let window = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        });
    }
    let escape = gtk::EventControllerKey::new();
    {
        let window = window.downgrade();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(window) = window.upgrade()
            {
                window.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(escape);
    // What the list holds, and whether they are its own or the defaults, said once the
    // window is in front and Orca has read its title and focus, which would cut it off.
    {
        let weak = Rc::downgrade(&list);
        let said = Cell::new(false);
        window.connect_is_active_notify(move |window| {
            if !window.is_active() || said.replace(true) {
                return;
            }
            let weak = weak.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
                if let Some(list) = weak.upgrade() {
                    list.say(&list.status.label());
                }
            });
        });
    }
    window.present();
    if list.tree.len() > 0 {
        list.tree.focus();
    } else {
        add.grab_focus();
    }
    // The list lives as long as its window.
    window.connect_destroy(move |_| {
        let _ = &list;
    });
}
