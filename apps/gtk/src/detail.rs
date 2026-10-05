//! The selected task's details (§16.1: task detail / edit), in the right-hand pane.
//!
//! Stock fields, each named by the label above it through the label's mnemonic, which is also
//! the field's Alt shortcut. The mnemonics avoid the menu bar's letters (F E V T D H). What a
//! field takes is its accessible description. Saving sends only the fields that changed (the
//! surface's `task_edit`), and the buttons are the Task menu's own actions (`task_actions`).
//!
//! The fields follow the store while nobody is editing them: a change from another device or
//! another process refills them, unless they hold typing not yet saved, which it would lose.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_desktop::speech;
use lumenna_surface::{TaskDetail, TaskFields, task_edit, task_fields};

use crate::task_actions::{self, Command};
use crate::window::App;

const PRIORITIES: [&str; 4] = ["Priority 1, highest", "Priority 2", "Priority 3", "Priority 4, none"];

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
    labels: gtk::Entry,
    notes: gtk::TextView,
    waits: gtk::ListBox,
    stop_waiting: gtk::Button,
    state: gtk::Entry,
    mark_done: gtk::Button,
    move_to_top: gtk::Button,
    shown: RefCell<Option<TaskDetail>>,
}

/// A label for `field`, whose mnemonic names it and reaches it.
fn label(text: &str, field: &impl IsA<gtk::Widget>) -> gtk::Label {
    let label = gtk::Label::builder().label(text).use_underline(true).xalign(0.0).build();
    label.set_mnemonic_widget(Some(field));
    label
}

fn describe(widget: &impl IsA<gtk::Accessible>, description: &str) {
    widget.update_property(&[gtk::accessible::Property::Description(description)]);
}

impl Detail {
    pub fn new() -> Rc<Self> {
        let title = gtk::Entry::new();
        let due = gtk::Entry::new();
        describe(&due, "A date, such as tomorrow or next Friday. Empty for none. A new date keeps how it repeats.");
        let repeat = gtk::Entry::new();
        describe(&repeat, "Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition.");
        let priority = gtk::DropDown::from_strings(&PRIORITIES);
        let estimate = gtk::Entry::new();
        describe(&estimate, "Such as 45m or 1h30m. Empty for none.");
        let projects = gtk::StringList::new(&[]);
        let project = gtk::DropDown::builder().model(&projects).build();
        let labels = gtk::Entry::new();
        describe(&labels, "Names separated by commas. A new name becomes a label.");
        // Tab leaves the notes rather than being typed into them, or a keyboard user could
        // not get out (§6.4's rule, for a different field).
        let notes = gtk::TextView::builder().accepts_tab(false).wrap_mode(gtk::WrapMode::WordChar).build();
        let waits = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Browse).build();
        describe(&waits, "The tasks this one waits for. It is blocked until they are done.");
        let add_wait = gtk::Button::builder().label("Add…").action_name("win.wait-for").build();
        add_wait.update_property(&[gtk::accessible::Property::Label("Add something it waits for")]);
        let stop_waiting = gtk::Button::builder().label("Stop Waiting").build();
        stop_waiting.update_property(&[gtk::accessible::Property::Label("Stop waiting for the selected task")]);
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
        let mut full = |grid: &gtk::Grid, text: &str, field: &gtk::Widget| {
            grid.attach(&label(text, field), 0, row, 2, 1);
            grid.attach(field, 0, row + 1, 2, 1);
            row += 2;
        };
        full(&grid, "T_itle", title.upcast_ref());
        let pair = |grid: &gtk::Grid, row: i32, left: (&str, &gtk::Widget), right: (&str, &gtk::Widget)| {
            grid.attach(&label(left.0, left.1), 0, row, 1, 1);
            grid.attach(left.1, 0, row + 1, 1, 1);
            grid.attach(&label(right.0, right.1), 1, row, 1, 1);
            grid.attach(right.1, 1, row + 1, 1, 1);
        };
        pair(&grid, 2, ("D_ue", due.upcast_ref()), ("Re_peats", repeat.upcast_ref()));
        pair(&grid, 4, ("Pri_ority", priority.upcast_ref()), ("Esti_mate", estimate.upcast_ref()));
        pair(&grid, 6, ("Pro_ject", project.upcast_ref()), ("_Labels", labels.upcast_ref()));
        let notes_scroll = gtk::ScrolledWindow::builder()
            .child(&notes)
            .min_content_height(96)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .has_frame(true)
            .build();
        grid.attach(&label("_Notes", &notes), 0, 8, 2, 1);
        grid.attach(&notes_scroll, 0, 9, 2, 1);
        let waits_scroll = gtk::ScrolledWindow::builder()
            .child(&waits)
            .min_content_height(64)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .has_frame(true)
            .build();
        grid.attach(&label("_Waits for", &waits), 0, 10, 2, 1);
        grid.attach(&waits_scroll, 0, 11, 2, 1);
        grid.attach(&add_wait, 0, 12, 1, 1);
        grid.attach(&stop_waiting, 1, 12, 1, 1);
        let state_label = gtk::Label::builder().label("State").xalign(0.0).build();
        state.update_relation(&[gtk::accessible::Relation::LabelledBy(&[state_label.upcast_ref()])]);
        grid.attach(&state_label, 0, 13, 2, 1);
        grid.attach(&state, 0, 14, 2, 1);
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
            labels,
            notes,
            waits,
            stop_waiting,
            state,
            mark_done,
            move_to_top,
            shown: RefCell::new(None),
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
            let index = detail.waits.selected_row().and_then(|row| usize::try_from(row.index()).ok());
            if let Some(other) = index.and_then(|i| task.depends.get(i)) {
                task_actions::run(&app, Command::StopWaiting(other.id.clone()), &task.id);
            }
        });
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

    /// Shows a task, or none. The same task stays as it is, so typing not yet saved is kept
    /// when the list reloads around a change made here.
    pub fn show(&self, app: &App, id: Option<&str>) {
        if self.task_id().as_deref() == id {
            return;
        }
        match id {
            Some(id) => self.load(app, id),
            None => self.fill(None),
        }
    }

    /// Reads the task again, and refills the fields with it.
    fn load(&self, app: &App, id: &str) {
        let projects: Vec<String> =
            app.core.lumenna.list_projects().map(|r| r.rows.into_iter().map(|p| p.title).collect()).unwrap_or_default();
        let names: Vec<&str> = projects.iter().map(String::as_str).collect();
        self.projects.splice(0, self.projects.n_items(), &names);
        match app.core.lumenna.show_task(id) {
            Ok(shown) => self.fill(Some(shown.task)),
            Err(_) => self.fill(None),
        }
    }

    fn fill(&self, task: Option<TaskDetail>) {
        self.widget.set_visible_child_name(if task.is_some() { "task" } else { "none" });
        if let Some(task) = &task {
            let fields = task_fields(task.clone());
            self.title.set_text(&fields.title);
            self.due.set_text(&fields.due);
            self.repeat.set_text(&fields.repeat);
            self.priority.set_selected(u32::from(fields.priority.clamp(1, 4) - 1));
            self.estimate.set_text(&fields.estimate);
            self.select_project(&fields.project);
            self.labels.set_text(&fields.labels);
            self.notes.buffer().set_text(&fields.notes);
            while let Some(row) = self.waits.row_at_index(0) {
                self.waits.remove(&row);
            }
            for other in &task.depends {
                let label = gtk::Label::builder().label(&other.title).xalign(0.0).margin_start(6).build();
                self.waits.append(&label);
            }
            if let Some(first) = self.waits.row_at_index(0) {
                self.waits.select_row(Some(&first));
            }
            self.stop_waiting.set_sensitive(!task.depends.is_empty());
            self.state.set_text(&speech::task_state(task));
            let completed = task.state.iter().any(|s| s == "completed");
            self.mark_done.set_label(if completed { "Mark Not Done" } else { "Mark Done" });
            // Only a subtask has a top level to move to.
            self.move_to_top.set_sensitive(task.parent.is_some());
        }
        *self.shown.borrow_mut() = task;
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
            priority: u8::try_from(self.priority.selected() + 1).unwrap_or(4).clamp(1, 4),
            estimate: self.estimate.text().to_string(),
            project: self.projects.string(self.project.selected()).map(|s| s.to_string()).unwrap_or_default(),
            labels: self.labels.text().to_string(),
            notes: buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string(),
        }
    }

    /// Whether the fields hold typing not yet saved.
    pub fn has_changes(&self) -> bool {
        self.shown.borrow().as_ref().is_some_and(|task| task_edit(task.clone(), self.read()).is_some())
    }

    /// The store changed: the fields follow unless someone is part way through editing them.
    pub fn reload(&self, app: &App) {
        if let Some(id) = self.task_id()
            && !self.has_changes()
        {
            self.load(app, &id);
        }
    }

    /// Saves what changed, and says what that did.
    pub fn save(&self, app: &App) {
        let Some(task) = self.shown.borrow().clone() else { return };
        let Some(edit) = task_edit(task.clone(), self.read()) else {
            app.say("Nothing changed");
            return;
        };
        if let Some(change) = app.perform(|lumenna| lumenna.edit_task(&task.id, edit)) {
            self.load(app, &task.id);
            app.say_change(&change);
        }
    }
}
