//! The task in hand's details, to read and edit, in the right-hand pane: the app's details
//! (`ViewModel::details`), drawn.
//!
//! Stock fields, each named by the label above it through the label's mnemonic, which is also
//! the field's Alt shortcut. The mnemonics avoid the menu bar's letters (F E V T D H). What a
//! field takes is its accessible description. Which task is shown, the typing not yet saved and
//! what saving sends are the app's: each field typed goes to it (`DetailsEvent::Field`), and it
//! keeps the draft through a change elsewhere and saves only what changed. The buttons run the
//! task's own actions, as the Task menu does: each is there only while the task offers it.
//!
//! While focus is here the details keep their task (`DetailsEvent::Show`), whatever the list
//! does: a change made here can move the list's row in hand off a task that leaves it (Mark
//! Done), and a second Ctrl+K must not complete a task nobody chose.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::DetailsEvent;
use lumenna_ffi::ShellEvent;
use lumenna_records::{ActionKind, FormField, TaskDetail, TaskFields};

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
    /// How deep each of `projects` sits, for indenting it in the open list.
    depths: Rc<RefCell<Vec<(String, u32)>>>,
    labels: gtk::Entry,
    notes: gtk::TextView,
    waits: Rc<Tree>,
    waits_label: gtk::Label,
    stop_waiting: gtk::Button,
    state: gtk::Entry,
    /// What has happened to it, under a History heading; hidden when nothing has.
    history_heading: gtk::Label,
    history: gtk::TextView,
    /// Each button and the kinds of action it runs.
    buttons: Vec<(gtk::Button, &'static [ActionKind])>,
    shown: RefCell<Option<TaskDetail>>,
    /// Whether the fields are being filled from the app, not typed.
    filling: Cell<bool>,
    /// A task whose details are to take focus once they show it.
    focus_wanted: RefCell<Option<String>>,
}

/// The open project list's rows: each name, indented as deep as it sits in the tree. Indent
/// alone: the names are unique, and a row's name is what is read.
fn project_rows(depths: &Rc<RefCell<Vec<(String, u32)>>>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
            item.set_child(Some(&gtk::Label::builder().xalign(0.0).build()));
        }
    });
    let depths = Rc::clone(depths);
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
        let (Some(label), Some(text)) =
            (item.child().and_downcast::<gtk::Label>(), item.item().and_downcast::<gtk::StringObject>())
        else {
            return;
        };
        let name = text.string();
        // Found by name: a search filters the list, so positions no longer match.
        let depth = depths.borrow().iter().find(|(n, _)| *n == name).map_or(0, |(_, d)| *d);
        label.set_label(&name);
        label.set_margin_start(i32::try_from(depth * 16).unwrap_or(0));
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
        let depths = Rc::new(RefCell::new(Vec::new()));
        project.set_list_factory(Some(&project_rows(&depths)));
        let labels = gtk::Entry::new();
        // Tab leaves the notes rather than being typed into them, or a keyboard user could
        // not get out.
        let notes = gtk::TextView::builder().accepts_tab(false).wrap_mode(gtk::WrapMode::WordChar).build();
        crate::prompts::leaves_on_tab(&notes);
        let waits = Tree::new("Waits for");
        waits.fit(120);
        describe(&waits.view, "The tasks this one waits for. It is blocked until they are done.");
        // Said in full on the buttons themselves: GTK names a button by its text, whatever
        // accessible label it is given.
        let add_wait = gtk::Button::builder().label("Wait for Another Task…").action_name("win.wait-for").build();
        let stop_waiting = gtk::Button::builder().label("Stop Waiting for the Selected Task").build();
        let state = gtk::Entry::builder().editable(false).build();

        let save = gtk::Button::with_mnemonic("_Save");
        save.set_action_name(Some("win.save-task"));
        let mark_done = gtk::Button::builder().label("Mark Done").action_name("win.mark-done").build();
        let put_in_block = gtk::Button::builder().label("Put in a Block…").action_name("win.put-in-block").build();
        let make_subtask = gtk::Button::builder().label("Make Subtask Of…").action_name("win.make-subtask").build();
        let move_to_top = gtk::Button::builder().label("Move to Top Level").action_name("win.move-to-top").build();
        let trash = gtk::Button::builder().label("Move to Trash").action_name("win.trash-task").build();

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
        let waits_label = label("_Waits for", &waits.view);
        grid.attach(&waits_label, 0, 10, 2, 1);
        grid.attach(&waits.widget, 0, 11, 2, 1);
        grid.attach(&add_wait, 0, 12, 1, 1);
        grid.attach(&stop_waiting, 1, 12, 1, 1);
        let state_label = gtk::Label::builder().label("State").xalign(0.0).build();
        state.update_relation(&[gtk::accessible::Relation::LabelledBy(&[state_label.upcast_ref()])]);
        grid.attach(&state_label, 0, 13, 2, 1);
        grid.attach(&state, 0, 14, 2, 1);
        // Read-only lines, named by the heading over them: Tab reaches them, the arrows read
        // them a line at a time.
        let history_heading = gtk::Label::builder()
            .label("History")
            .xalign(0.0)
            .margin_top(6)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        history_heading.add_css_class("heading");
        let history = gtk::TextView::builder().editable(false).cursor_visible(false).wrap_mode(gtk::WrapMode::WordChar).build();
        history.update_relation(&[gtk::accessible::Relation::LabelledBy(&[history_heading.upcast_ref()])]);
        crate::prompts::leaves_on_tab(&history);
        grid.attach(&history_heading, 0, 15, 2, 1);
        grid.attach(&history, 0, 16, 2, 1);
        let buttons = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(2)
            .max_children_per_line(3)
            .homogeneous(true)
            .column_spacing(6)
            .row_spacing(6)
            .build();
        for button in [&save, &mark_done, &put_in_block, &make_subtask, &move_to_top, &trash] {
            // A flow box's children are focusable themselves; the button inside should be
            // the only stop.
            let child = gtk::FlowBoxChild::builder().child(button).focusable(false).build();
            buttons.append(&child);
        }
        let form = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        form.append(&grid);
        form.append(&buttons);
        let scrolled = gtk::ScrolledWindow::builder().child(&form).hscrollbar_policy(gtk::PolicyType::Never).build();

        let placeholder = gtk::Label::builder().label("No task selected").build();
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
            depths,
            labels,
            notes,
            waits,
            waits_label,
            stop_waiting,
            state,
            history_heading,
            history,
            buttons: vec![
                (add_wait.clone(), &[ActionKind::WaitFor]),
                (mark_done.clone(), &[ActionKind::MarkDone, ActionKind::MarkNotDone]),
                (put_in_block.clone(), &[ActionKind::PutInBlock]),
                (make_subtask.clone(), &[ActionKind::MakeSubtaskOf]),
                (move_to_top.clone(), &[ActionKind::MoveToTopLevel]),
                (trash.clone(), &[ActionKind::Delete]),
            ],
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
        if self.task_id().as_deref() == Some(id) {
            self.focus();
            return;
        }
        *self.focus_wanted.borrow_mut() = Some(id.to_owned());
        app.model.send(ShellEvent::Details(DetailsEvent::Show(id.to_owned())));
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
        if listed != names {
            self.projects.splice(0, self.projects.n_items(), &names);
            *self.depths.borrow_mut() = details.projects.iter().map(|o| (o.id.clone(), o.depth)).collect();
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
        self.stop_waiting.set_sensitive(!task.depends.is_empty());
        let state = app.model.lines().task_state.clone();
        if self.state.text() != state {
            self.state.set_text(&state);
        }
        let history = task.history.join("\n");
        let buffer = self.history.buffer();
        if buffer.text(&buffer.start_iter(), &buffer.end_iter(), false) != history.as_str() {
            buffer.set_text(&history);
        }
        self.history_heading.set_visible(!task.history.is_empty());
        self.history.set_visible(!task.history.is_empty());
        // A button is there while the task offers its action, and says it as the core titles
        // it: Mark Done is Mark Not Done on a done task.
        for (button, kinds) in &self.buttons {
            let action = actions::find(&task.offered, kinds);
            button.set_sensitive(action.is_some());
            if let Some(action) = action {
                let asks = action.asks != lumenna_app::Asks::Nothing;
                let label = if asks { format!("{}…", action.title) } else { action.title.clone() };
                if button.label().is_none_or(|shown| shown != label.as_str()) {
                    button.set_label(&label);
                }
            }
        }
        *self.shown.borrow_mut() = Some(task.clone());
        let wanted = self.focus_wanted.borrow().clone();
        if wanted.as_deref() == Some(task.id.as_str()) {
            self.focus_wanted.take();
            self.focus();
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

/// Whether focus is in `widget`, or in what it holds.
fn within(widget: &impl IsA<gtk::Widget>) -> bool {
    widget.root().and_then(|root| root.focus()).is_some_and(|focus| &focus == widget.as_ref() || focus.is_ancestor(widget))
}
