//! The selected task's details, to read and edit, in the right-hand pane.
//!
//! Stock fields, each named by the label above it through the label's mnemonic, which is also
//! the field's Alt shortcut. The mnemonics avoid the menu bar's letters (F E V T D H). What a
//! field takes is its accessible description. Saving sends only the fields that changed (the
//! surface's `task_edit`), and the buttons run the task's own actions (`TaskDetail::actions`), as the Task menu
//! does: each is there only while the task offers it.
//!
//! The fields follow the store while nobody is editing them: a change from another device or
//! another process refills them, unless they hold typing not yet saved, which it would lose.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_desktop::speech;
use lumenna_surface::{FormField, TaskDetail, TaskFields, task_edit, task_fields};

use lumenna_surface::actions::{Action, ActionKind};

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
    /// Each button and the kinds of action it runs.
    buttons: Vec<(gtk::Button, &'static [ActionKind])>,
    shown: RefCell<Option<TaskDetail>>,
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
    (lumenna_desktop::devices::marked(label, mnemonic, '_'), field.map(|f| f.hint.clone()).unwrap_or_default())
}

impl Detail {
    pub fn new() -> Rc<Self> {
        let form = lumenna_surface::task_form();
        let words = |key: &str, mnemonic: char| field(&form, key, mnemonic);
        let title = gtk::Entry::new();
        let due = gtk::Entry::new();
        let repeat = gtk::Entry::new();
        let titles: Vec<String> = lumenna_surface::priorities().into_iter().map(|p| p.title).collect();
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
            buttons: vec![
                (add_wait.clone(), &[ActionKind::WaitFor]),
                (mark_done.clone(), &[ActionKind::MarkDone, ActionKind::MarkNotDone]),
                (put_in_block.clone(), &[ActionKind::PutInBlock]),
                (make_subtask.clone(), &[ActionKind::MakeSubtaskOf]),
                (move_to_top.clone(), &[ActionKind::MoveToTopLevel]),
                (trash.clone(), &[ActionKind::Delete]),
            ],
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
            let Some(other) = detail.waits.selected().and_then(|i| task.depends.get(i)) else { return };
            let stop = task.actions.iter().find(|a| a.kind == ActionKind::StopWaiting && a.other.as_deref() == Some(&other.id));
            if let Some(action) = stop {
                actions::run(&app, action.clone(), None);
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

    /// What can be done to the task shown: what the Task menu runs while focus is here.
    pub fn actions(&self) -> Vec<Action> {
        self.shown.borrow().as_ref().map(|task| task.actions.clone()).unwrap_or_default()
    }

    /// Whether focus is somewhere in the details.
    pub fn has_focus(&self) -> bool {
        self.widget.root().and_then(|root| root.focus()).is_some_and(|focus| focus.is_ancestor(&self.widget))
    }

    /// Moves focus to the first field; nowhere while it shows nothing. Returns whether it did.
    pub fn focus(&self) -> bool {
        self.shown.borrow().is_some() && self.title.grab_focus()
    }

    /// Follows the selection in the list beside it — unless focus is here, where the person
    /// is working on the task shown: a change made here moves the list's selection off a
    /// task that leaves it (Mark Done), and the details must not follow, or a second Ctrl+K
    /// would complete a task nobody chose.
    pub fn follow(&self, app: &App, id: Option<&str>) {
        if !self.has_focus() {
            self.show(app, id);
        }
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
        let options = app.core.lumenna.project_options().unwrap_or_default();
        let names: Vec<&str> = options.iter().map(|o| o.id.as_str()).collect();
        self.projects.splice(0, self.projects.n_items(), &names);
        *self.depths.borrow_mut() = options.iter().map(|o| (o.id.clone(), o.depth)).collect();
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
            let position = lumenna_surface::priorities().iter().position(|p| p.id == fields.priority.to_string());
            self.priority.set_selected(position.unwrap_or(0) as u32);
            self.estimate.set_text(&fields.estimate);
            self.select_project(&fields.project);
            self.labels.set_text(&fields.labels);
            self.notes.buffer().set_text(&fields.notes);
            let items =
                task.depends.iter().map(|other| Item { key: other.id.clone(), text: other.title.clone(), depth: 0 }).collect();
            if self.waits.set(items) {
                self.waits.select_key_or_near(None, Some(0));
            }
            // An empty list is not worth a stop: Wait for Another Task says what to do.
            self.waits_label.set_visible(!task.depends.is_empty());
            self.waits.widget.set_visible(!task.depends.is_empty());
            self.stop_waiting.set_sensitive(!task.depends.is_empty());
            self.state.set_text(&speech::task_state(task));
            // A button is there while the task offers its action, and says it as the core
            // titles it: Mark Done is Mark Not Done on a done task.
            for (button, kinds) in &self.buttons {
                let action = actions::find(&task.actions, kinds);
                button.set_sensitive(action.is_some());
                if let Some(action) = action {
                    let asks = !matches!(action.question, lumenna_surface::Question::Immediate);
                    button.set_label(&if asks { format!("{}…", action.title) } else { action.title.clone() });
                }
            }
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
            priority: lumenna_surface::priorities()
                .get(self.priority.selected() as usize)
                .and_then(|p| p.id.parse().ok())
                .unwrap_or(4),
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
