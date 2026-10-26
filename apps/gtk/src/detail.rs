//! The task in hand's details, to read and edit, in the right-hand pane: the app's details
//! (`ViewModel::details`), drawn.
//!
//! Stock fields, each named by the label above it through the label's mnemonic, which is also
//! the field's Alt shortcut. The mnemonics avoid the menu bar's letters (F E V T D H). What a
//! field takes is its accessible description. Which task is shown, the typing not yet saved and
//! what saving sends are the app's: each field typed goes to it (`DetailsEvent::Field`), and it
//! keeps the draft through a change elsewhere and saves only what changed. The buttons are the
//! task's primary actions as it offers them now (Mark Done or Mark Not Done, Move to Trash),
//! after Save; Actions… opens every one it offers, as a row's menu does. Every word around the
//! form is the core's (`details_words`), and what it says of the task its `DetailsView::lines`.
//!
//! While focus is here the details keep their task (`DetailsEvent::Show`), whatever the list
//! does: a change made here can move the list's row in hand off a task that leaves it (Mark
//! Done), and a second Ctrl+K must not complete a task nobody chose.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::{ActingOn, Asks, DetailsEvent, Offered};
use lumenna_ffi::ShellEvent;
use lumenna_records::fields::DetailsLine;
use lumenna_records::{ActionKind, Choice, FormField, TaskDetail, TaskFields};

use crate::actions;
use crate::tree::{Item, Tree};
use crate::window::App;


pub struct Detail {
    /// The pane: the placeholder or the form, whichever is showing.
    pub widget: gtk::Stack,
    title: gtk::Entry,
    due: gtk::Entry,
    repeat: gtk::Entry,
    priority: gtk::DropDown,
    estimate: gtk::Entry,
    project: gtk::DropDown,
    projects: gtk::StringList,
    /// What the core says of each of `projects`: how deep it sits, for indenting it in the
    /// open list, and its description.
    choices: Rc<RefCell<Vec<Choice>>>,
    labels: gtk::Entry,
    notes: gtk::TextView,
    waits: Rc<Tree>,
    waits_label: gtk::Label,
    add_wait: gtk::Button,
    stop_waiting: gtk::Button,
    /// What the core says of the task (`DetailsView::lines`), under About: each line's key,
    /// its label and its text.
    about: gtk::Box,
    lines: RefCell<Vec<(String, gtk::Label, gtk::TextView)>>,
    /// The task's primary actions, each a button after Save, as last drawn.
    buttons: gtk::Box,
    primary: RefCell<Vec<(gtk::Button, Offered)>>,
    /// Every action the task offers, as a menu.
    more: gtk::Button,
    shown: RefCell<Option<TaskDetail>>,
    /// Whether the fields are being filled from the app, not typed.
    filling: Cell<bool>,
    /// A task whose details are to take focus once they show it.
    focus_wanted: RefCell<Option<String>>,
}

/// The open project list's rows: each name, indented as deep as it sits in the tree, and
/// described as the core describes its choice, since Orca hears no indent. The names are
/// unique, and a row's name is what is read.
fn project_rows(choices: &Rc<RefCell<Vec<Choice>>>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
            item.set_child(Some(&gtk::Label::builder().xalign(0.0).build()));
        }
    });
    let choices = Rc::clone(choices);
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
        let (Some(label), Some(text)) =
            (item.child().and_downcast::<gtk::Label>(), item.item().and_downcast::<gtk::StringObject>())
        else {
            return;
        };
        let name = text.string();
        // Found by name: a search filters the list, so positions no longer match.
        let choice = choices.borrow().iter().find(|choice| choice.id == name).cloned().unwrap_or_default();
        label.set_label(&name);
        label.set_margin_start(i32::try_from(choice.depth * 16).unwrap_or(0));
        // The level in the core's words, which the indent alone never says.
        let level = (choice.depth > 0).then(|| lumenna_records::words::level(choice.depth));
        item.set_accessible_description(choice.reason.as_deref().or(choice.detail.as_deref()).or(level.as_deref()).unwrap_or_default());
    });
    factory
}

/// A label for `field`, whose mnemonic names it and reaches it.
fn label(text: &str, field: &impl IsA<gtk::Widget>) -> gtk::Label {
    let label = gtk::Label::builder().label(text).use_underline(true).xalign(0.0).build();
    label.set_mnemonic_widget(Some(field));
    label
}

fn describe(widget: &impl IsA<gtk::Accessible>, description: &str) {
    if !description.is_empty() {
        widget.update_property(&[gtk::accessible::Property::Description(description)]);
    }
}

/// The task form's field `key` as the core words it, its label marked at `mnemonic` — the
/// one thing about it that is this app's.
fn field(form: &[FormField], key: &str, mnemonic: char) -> (String, String) {
    let field = form.iter().find(|f| f.key == key);
    let label = field.map_or(key, |f| f.label.as_str());
    (lumenna_app::devices::marked(label, mnemonic, '_'), field.map(|f| f.hint.clone()).unwrap_or_default())
}

impl Detail {
    pub fn new() -> Rc<Self> {
        let form = lumenna_records::task_form();
        let words = |key: &str, mnemonic: char| field(&form, key, mnemonic);
        let title = gtk::Entry::new();
        let due = gtk::Entry::new();
        let repeat = gtk::Entry::new();
        let titles: Vec<String> = lumenna_records::priorities().into_iter().map(|p| p.title).collect();
        let priority = gtk::DropDown::from_strings(&titles.iter().map(String::as_str).collect::<Vec<_>>());
        let estimate = gtk::Entry::new();
        let projects = gtk::StringList::new(&[]);
        // Chosen from the projects there are, so a typo cannot make one; typing finds one.
        let project = gtk::DropDown::builder()
            .model(&projects)
            .enable_search(true)
            .search_match_mode(gtk::StringFilterMatchMode::Substring)
            .expression(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string"))
            .build();
        let choices = Rc::new(RefCell::new(Vec::new()));
        project.set_list_factory(Some(&project_rows(&choices)));
        let labels = gtk::Entry::new();
        // Tab leaves the notes rather than being typed into them, or a keyboard user could
        // not get out.
        let notes = gtk::TextView::builder().accepts_tab(false).wrap_mode(gtk::WrapMode::WordChar).build();
        crate::prompts::leaves_on_tab(&notes);
        let said = lumenna_records::fields::details_words(false);
        let waits = Tree::new(&said.waits);
        waits.fit(120);
        describe(&waits.view, &said.waits_hint);
        // Said in full on the buttons themselves: GTK names a button by its text, whatever
        // accessible label it is given.
        let add_wait = gtk::Button::builder().label(format!("{}…", said.add_wait)).action_name("win.wait-for").build();
        let stop_waiting = gtk::Button::builder().label(&said.stop_wait).build();

        let save = gtk::Button::with_mnemonic(&lumenna_app::devices::marked(&said.save, 'S', '_'));
        save.set_action_name(Some("win.save-task"));
        let more = gtk::Button::with_label(&format!("{}…", said.actions));

        // In reading order, which is also Tab's.
        let grid = gtk::Grid::builder().row_spacing(4).column_spacing(12).column_homogeneous(true).build();
        let mut row = 0;
        // A field's name above it, its hint as its description.
        let named = |field: &gtk::Widget, (text, hint): &(String, String)| {
            describe(field, hint);
            label(text, field)
        };
        let mut full = |grid: &gtk::Grid, words: (String, String), field: &gtk::Widget| {
            grid.attach(&named(field, &words), 0, row, 2, 1);
            grid.attach(field, 0, row + 1, 2, 1);
            row += 2;
        };
        full(&grid, words("title", 'i'), title.upcast_ref());
        let pair = |grid: &gtk::Grid, row: i32, left: ((String, String), &gtk::Widget), right: ((String, String), &gtk::Widget)| {
            grid.attach(&named(left.1, &left.0), 0, row, 1, 1);
            grid.attach(left.1, 0, row + 1, 1, 1);
            grid.attach(&named(right.1, &right.0), 1, row, 1, 1);
            grid.attach(right.1, 1, row + 1, 1, 1);
        };
        pair(&grid, 2, (words("due", 'u'), due.upcast_ref()), (words("repeat", 'p'), repeat.upcast_ref()));
        pair(&grid, 4, (words("priority", 'o'), priority.upcast_ref()), (words("estimate", 'm'), estimate.upcast_ref()));
        pair(&grid, 6, (words("project", 'j'), project.upcast_ref()), (words("labels", 'L'), labels.upcast_ref()));
        let notes_scroll = gtk::ScrolledWindow::builder()
            .child(&notes)
            .min_content_height(96)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .has_frame(true)
            .build();
        grid.attach(&named(notes.upcast_ref(), &words("notes", 'N')), 0, 8, 2, 1);
        grid.attach(&notes_scroll, 0, 9, 2, 1);
        let waits_label = label(&lumenna_app::devices::marked(&said.waits, 'W', '_'), &waits.view);
        grid.attach(&waits_label, 0, 10, 2, 1);
        grid.attach(&waits.widget, 0, 11, 2, 1);
        grid.attach(&add_wait, 0, 12, 1, 1);
        grid.attach(&stop_waiting, 1, 12, 1, 1);
        // What the core says of the task, a read-only line each, named by its label: Tab
        // reaches them, the arrows read a long one a line at a time.
        let about_heading = gtk::Label::builder()
            .label(&said.about)
            .xalign(0.0)
            .margin_top(6)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        about_heading.add_css_class("heading");
        let about = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();
        grid.attach(&about_heading, 0, 13, 2, 1);
        grid.attach(&about, 0, 14, 2, 1);
        let buttons = gtk::Box::builder().spacing(6).build();
        let bar = gtk::Box::builder().spacing(6).build();
        bar.append(&save);
        bar.append(&buttons);
        bar.append(&more);
        let form = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        form.append(&grid);
        form.append(&bar);
        // Named, so Orca says whose fields these are as focus comes in from the list.
        form.set_accessible_role(gtk::AccessibleRole::Group);
        form.update_property(&[gtk::accessible::Property::Label(&said.title)]);
        let scrolled = gtk::ScrolledWindow::builder().child(&form).hscrollbar_policy(gtk::PolicyType::Never).build();

        let placeholder = gtk::Label::builder().label(&said.none).build();
        let widget = gtk::Stack::new();
        widget.add_named(&placeholder, Some("none"));
        widget.add_named(&scrolled, Some("task"));
        widget.set_visible_child_name("none");

        let detail = Rc::new(Self {
            widget,
            title,
            due,
            repeat,
            priority,
            estimate,
            project,
            projects,
            choices,
            labels,
            notes,
            waits,
            waits_label,
            add_wait,
            stop_waiting,
            about,
            lines: RefCell::new(Vec::new()),
            buttons,
            primary: RefCell::new(Vec::new()),
            more,
            shown: RefCell::new(None),
            filling: Cell::new(false),
            focus_wanted: RefCell::new(None),
        });
        detail.connect(&form);
        detail
    }

    fn connect(self: &Rc<Self>, form: &gtk::Box) {
        // Enter in a one-line field saves, as a form's default button would.
        for entry in [&self.title, &self.due, &self.repeat, &self.estimate, &self.labels] {
            entry.connect_activate(|entry| {
                let _ = entry.activate_action("win.save-task", None);
            });
        }
        // Every action the task offers, as its row's menu offers them.
        let weak = Rc::downgrade(self);
        self.more.connect_clicked(move |button| {
            let (Some(detail), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(task) = detail.shown.borrow().clone() else { return };
            let (menu, group) = actions::menu(&task.offered, &ActingOn::Details);
            app.popup(&menu, button.upcast_ref(), None, Some(&group));
        });
        let weak = Rc::downgrade(self);
        self.stop_waiting.connect_clicked(move |_| {
            let (Some(detail), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(task) = detail.shown.borrow().clone() else { return };
            let Some(other) = detail.waits.selected().and_then(|i| task.depends.get(i)) else { return };
            let stop = task.offered.iter().find(|a| a.kind == ActionKind::StopWaiting && a.other.as_deref() == Some(&other.id));
            if let Some(action) = stop {
                app.model.act_on(lumenna_app::ActingOn::Details, action.clone());
            }
        });
        // Each field typed or chosen goes to the app, which keeps it until saved.
        let typed = |detail: &Rc<Self>, key: &'static str| {
            let weak = Rc::downgrade(detail);
            move || {
                let (Some(detail), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
                if detail.filling.get() || detail.shown.borrow().is_none() {
                    return;
                }
                let fields = detail.read();
                let value = match key {
                    "title" => fields.title,
                    "due" => fields.due,
                    "repeat" => fields.repeat,
                    "priority" => fields.priority.to_string(),
                    "estimate" => fields.estimate,
                    "project" => fields.project,
                    "labels" => fields.labels,
                    _ => fields.notes,
                };
                app.model.send(ShellEvent::Details(DetailsEvent::Field { key: key.to_owned(), value }));
            }
        };
        for (entry, key) in [(&self.title, "title"), (&self.due, "due"), (&self.repeat, "repeat"), (&self.estimate, "estimate"), (&self.labels, "labels")] {
            let send = typed(self, key);
            entry.connect_changed(move |_| send());
        }
        let send = typed(self, "priority");
        self.priority.connect_selected_notify(move |_| send());
        let send = typed(self, "project");
        self.project.connect_selected_notify(move |_| send());
        let send = typed(self, "notes");
        self.notes.buffer().connect_changed(move |_| send());
        // Focus coming in keeps this task shown, whatever the list does meanwhile.
        let entering = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        entering.connect_enter(move |_| {
            let (Some(detail), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            if let Some(id) = detail.task_id() {
                app.model.send(ShellEvent::Details(DetailsEvent::Show(id)));
            }
        });
        form.add_controller(entering);
        // Escape goes back to the list, from anywhere in the form.
        let escape = gtk::EventControllerKey::new();
        escape.connect_key_pressed(|_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(app) = crate::window::app() {
                    app.focus_content();
                    return glib::Propagation::Stop;
                }
            glib::Propagation::Proceed
        });
        form.add_controller(escape);
    }

    /// The task shown, if any.
    pub fn task_id(&self) -> Option<String> {
        self.shown.borrow().as_ref().map(|task| task.id.clone())
    }

    /// Whether focus is somewhere in the details.
    pub fn has_focus(&self) -> bool {
        self.widget.root().and_then(|root| root.focus()).is_some_and(|focus| focus.is_ancestor(&self.widget))
    }

    /// Moves focus to the first field; nowhere while it shows nothing. Returns whether it did.
    pub fn focus(&self) -> bool {
        self.shown.borrow().is_some() && self.title.grab_focus()
    }

    /// Shows a task's details, whatever is in hand, and puts focus in them once they do: a
    /// task opened from a row, a reminder.
    pub fn open(&self, app: &App, id: &str) {
        if self.focus_when_shown(id) {
            app.model.send(ShellEvent::Details(DetailsEvent::Show(id.to_owned())));
        }
    }

    /// Puts focus in the details once they show task `id`, now if they do: whether they do not
    /// yet, and something must have the app show it.
    pub fn focus_when_shown(&self, id: &str) -> bool {
        if self.task_id().as_deref() == Some(id) {
            self.focus();
            return false;
        }
        *self.focus_wanted.borrow_mut() = Some(id.to_owned());
        true
    }

    /// Draws the details the app last left. A field is refilled only where it differs from
    /// what it shows and focus is not in it: the one being typed in already holds the newest.
    pub fn draw(&self, app: &App) {
        let view = app.model.view();
        let details = view.details.as_ref().filter(|_| view.ready);
        self.widget.set_visible_child_name(if details.is_some() { "task" } else { "none" });
        let Some(details) = details else {
            *self.shown.borrow_mut() = None;
            return;
        };
        let task = &details.task;
        let fields = &details.fields;
        self.filling.set(true);
        let names: Vec<&str> = details.projects.iter().map(|o| o.id.as_str()).collect();
        let listed: Vec<String> = (0..self.projects.n_items()).filter_map(|i| self.projects.string(i).map(|s| s.to_string())).collect();
        // Kept before the list is spliced, which binds its rows again.
        self.choices.borrow_mut().clone_from(&details.projects);
        if listed != names {
            self.projects.splice(0, self.projects.n_items(), &names);
        }
        let refill = |entry: &gtk::Entry, text: &str| {
            if entry.text() != text && !within(entry) {
                entry.set_text(text);
            }
        };
        refill(&self.title, &fields.title);
        refill(&self.due, &fields.due);
        refill(&self.repeat, &fields.repeat);
        refill(&self.estimate, &fields.estimate);
        refill(&self.labels, &fields.labels);
        let position = lumenna_records::priorities().iter().position(|p| p.id == fields.priority.to_string()).unwrap_or(0) as u32;
        if self.priority.selected() != position && !within(&self.priority) {
            self.priority.set_selected(position);
        }
        if self.projects.string(self.project.selected()).is_none_or(|s| s != fields.project.as_str()) && !within(&self.project) {
            self.select_project(&fields.project);
        }
        let buffer = self.notes.buffer();
        if buffer.text(&buffer.start_iter(), &buffer.end_iter(), false) != fields.notes.as_str() && !within(&self.notes) {
            buffer.set_text(&fields.notes);
        }
        self.filling.set(false);
        let same = self.shown.borrow().as_ref().is_some_and(|shown| shown.depends == task.depends);
        if !same {
            let items =
                task.depends.iter().map(|other| Item { key: other.id.clone(), text: other.title.clone(), depth: 0, ..Default::default() }).collect();
            if self.waits.set(items) {
                self.waits.select_key_or_near(None, Some(0));
            }
        }
        // An empty list is not worth a stop: Wait for Another Task says what to do.
        self.waits_label.set_visible(!task.depends.is_empty());
        self.waits.widget.set_visible(!task.depends.is_empty());
        // Wait for Another Task while the task offers it; the last wait stopped from Stop
        // Waiting, focus goes beside it, else to the title, never nowhere.
        let waits_for = task.offered.iter().any(|action| action.kind == ActionKind::WaitFor);
        crate::prompts::set_sensitive(&self.add_wait, waits_for, &[self.title.upcast_ref()]);
        crate::prompts::set_sensitive(&self.stop_waiting, !task.depends.is_empty(), &[self.add_wait.upcast_ref(), self.title.upcast_ref()]);
        self.draw_lines(&details.lines);
        self.draw_buttons(&task.offered);
        *self.shown.borrow_mut() = Some(task.clone());
        let wanted = self.focus_wanted.borrow().clone();
        if wanted.as_deref() == Some(task.id.as_str()) {
            self.focus_wanted.take();
            self.focus();
        }
    }

    /// What the core says of the task, each line kept where its label is the same, so a line
    /// being read is not made again under the reader.
    fn draw_lines(&self, said: &[DetailsLine]) {
        let same = {
            let lines = self.lines.borrow();
            lines.len() == said.len() && lines.iter().zip(said).all(|((key, label, _), line)| *key == line.key && label.label() == line.label.as_str())
        };
        if !same {
            while let Some(child) = self.about.first_child() {
                self.about.remove(&child);
            }
            let made = said
                .iter()
                .map(|line| {
                    let label = gtk::Label::builder().label(&line.label).xalign(0.0).build();
                    let text = gtk::TextView::builder().editable(false).cursor_visible(false).wrap_mode(gtk::WrapMode::WordChar).build();
                    text.update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
                    crate::prompts::leaves_on_tab(&text);
                    self.about.append(&label);
                    self.about.append(&text);
                    (line.key.clone(), label, text)
                })
                .collect();
            *self.lines.borrow_mut() = made;
        }
        for ((_, _, text), line) in self.lines.borrow().iter().zip(said) {
            let buffer = text.buffer();
            if buffer.text(&buffer.start_iter(), &buffer.end_iter(), false) != line.value.as_str() {
                buffer.set_text(&line.value);
            }
        }
    }

    /// The task's primary actions as buttons, each named as the core titles it now: Mark Done
    /// is Mark Not Done on a done task, the same button. Waits For's own are beside its list.
    fn draw_buttons(&self, offered: &[Offered]) {
        let primary: Vec<Offered> = offered
            .iter()
            .filter(|action| action.primary && !matches!(action.kind, ActionKind::WaitFor | ActionKind::StopWaiting))
            .cloned()
            .collect();
        let label = |action: &Offered| if action.asks == Asks::Nothing { action.title.clone() } else { format!("{}…", action.title) };
        let mut shown = self.primary.borrow_mut();
        if shown.len() == primary.len() {
            for ((button, held), action) in shown.iter_mut().zip(&primary) {
                if button.label().is_none_or(|text| text != label(action).as_str()) {
                    button.set_label(&label(action));
                }
                set_destructive(button, action.destructive);
                *held = action.clone();
            }
            return;
        }
        // Made again: focus on one that goes moves to the first that comes, else the title.
        let had_focus = shown.iter().any(|(button, _)| within(button));
        while let Some(child) = self.buttons.first_child() {
            self.buttons.remove(&child);
        }
        *shown = primary
            .into_iter()
            .map(|action| {
                let button = gtk::Button::with_label(&label(&action));
                set_destructive(&button, action.destructive);
                let index = self.buttons.observe_children().n_items() as usize;
                button.connect_clicked(move |_| {
                    let Some(app) = crate::window::app() else { return };
                    let held = app.detail.primary.borrow().get(index).map(|(_, action)| action.clone());
                    if let Some(action) = held {
                        app.model.act_on(ActingOn::Details, action);
                    }
                });
                self.buttons.append(&button);
                (button, action)
            })
            .collect();
        if had_focus && !shown.first().is_some_and(|(button, _)| button.grab_focus()) {
            self.title.grab_focus();
        }
    }

    /// Selects a project by name, adding it to the list if missing.
    fn select_project(&self, name: &str) {
        let position = (0..self.projects.n_items()).find(|&i| self.projects.string(i).is_some_and(|s| s == name));
        let position = position.unwrap_or_else(|| {
            self.projects.append(name);
            self.projects.n_items() - 1
        });
        self.project.set_selected(position);
    }

    /// The fields as they are now.
    fn read(&self) -> TaskFields {
        let buffer = self.notes.buffer();
        TaskFields {
            title: self.title.text().to_string(),
            due: self.due.text().to_string(),
            repeat: self.repeat.text().to_string(),
            priority: lumenna_records::priorities()
                .get(self.priority.selected() as usize)
                .and_then(|p| p.id.parse().ok())
                .unwrap_or(4),
            estimate: self.estimate.text().to_string(),
            project: self.projects.string(self.project.selected()).map(|s| s.to_string()).unwrap_or_default(),
            labels: self.labels.text().to_string(),
            notes: buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string(),
        }
    }

    /// Saves what changed: the app's to work out, and to say.
    pub fn save(&self, app: &App) {
        if self.shown.borrow().is_some() {
            app.model.send(ShellEvent::Details(DetailsEvent::Save));
        }
    }
}

/// Draws `button` as libadwaita draws what removes something, or not.
fn set_destructive(button: &gtk::Button, destructive: bool) {
    if destructive {
        button.add_css_class("destructive-action");
    } else {
        button.remove_css_class("destructive-action");
    }
}

/// Whether focus is in `widget`, or in what it holds.
fn within(widget: &impl IsA<gtk::Widget>) -> bool {
    widget.root().and_then(|root| root.focus()).is_some_and(|focus| &focus == widget.as_ref() || focus.is_ancestor(widget))
}
