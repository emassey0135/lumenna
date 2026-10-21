//! Adding or changing a block, in a form: the app's block form (`ViewModel::block_form`),
//! drawn as a window of its own while the app has it open.
//!
//! What the form holds, which fields it shows, what a kind brings with it and what saving
//! sends are the app's: each field typed or chosen goes to it (`BlockEvent::Field`), and the
//! fields are refilled from what it says — the three flags when the kind changes, Starts at when
//! a new block's day does — except the one being typed in, which holds the newest already. A
//! field the form does not show now (Until while nothing repeats) is hidden, not removed.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::{BlockEvent, BlockFormView};
use lumenna_ffi::ShellEvent;
use lumenna_records::{BlockFields, FieldKind};

use crate::prompts;
use crate::window::App;

/// Each field's mnemonic, none used twice in the form nor by its Save button.
fn mnemonic(key: &str) -> char {
    match key {
        "title" => 'm',
        "date" => 'D',
        "start" => 'a',
        "minutes" => 'L',
        "kind" => 'K',
        "accepts_tasks" => 'T',
        "counts_capacity" => 'h',
        "anchored" => 'w',
        "repeat" => 'R',
        "until" => 'U',
        "min_minutes" => 'e',
        "task_filter" => 'f',
        "colour" => 'C',
        "notes" => 'N',
        _ => '\0',
    }
}

/// A field's control.
enum Control {
    Line(gtk::Entry),
    Kind(gtk::DropDown, Vec<String>),
    Flag(gtk::CheckButton),
    Notes(gtk::TextView),
}

impl Control {
    fn widget(&self) -> gtk::Widget {
        match self {
            Self::Line(entry) => entry.clone().upcast(),
            Self::Kind(kind, _) => kind.clone().upcast(),
            Self::Flag(check) => check.clone().upcast(),
            Self::Notes(notes) => notes.clone().upcast(),
        }
    }

    /// What it holds, as the app takes a field: a flag as `true` or `false`.
    fn value(&self) -> String {
        match self {
            Self::Line(entry) => entry.text().to_string(),
            Self::Kind(kind, ids) => ids.get(kind.selected() as usize).cloned().unwrap_or_default(),
            Self::Flag(check) => check.is_active().to_string(),
            Self::Notes(notes) => {
                let buffer = notes.buffer();
                buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
            }
        }
    }

    /// Shows `value`, unless focus is in it: what is typed there is the newest.
    fn fill(&self, value: &str) {
        let widget = self.widget();
        let typing = widget.root().and_then(|root| root.focus()).is_some_and(|focus| focus == widget || focus.is_ancestor(&widget));
        if self.value() == value || (typing && !matches!(self, Self::Flag(_))) {
            return;
        }
        match self {
            Self::Line(entry) => entry.set_text(value),
            Self::Kind(kind, ids) => kind.set_selected(ids.iter().position(|id| id == value).unwrap_or(0) as u32),
            Self::Flag(check) => check.set_active(value == "true"),
            Self::Notes(notes) => notes.buffer().set_text(value),
        }
    }
}

/// A field's description: the core's hint, and for a line in the filter language how to have
/// it completed.
fn described(field: &lumenna_records::FormField) -> String {
    match field.syntax {
        Some(lumenna_records::Syntax::Filter) => format!("{} {}", field.hint, crate::completion::HINT).trim().to_owned(),
        _ => field.hint.clone(),
    }
}

/// What a field holds in the app's values, by its key.
fn value_of(values: &BlockFields, date: &str, key: &str) -> String {
    match key {
        "title" => values.title.clone(),
        "date" => date.to_owned(),
        "start" => values.start.clone(),
        "minutes" => values.minutes.clone(),
        "kind" => values.kind.clone(),
        "accepts_tasks" => values.accepts_tasks.to_string(),
        "counts_capacity" => values.counts_capacity.to_string(),
        "anchored" => values.anchored.to_string(),
        "repeat" => values.repeat.clone(),
        "until" => values.until.clone(),
        "min_minutes" => values.min_minutes.clone(),
        "task_filter" => values.task_filter.clone(),
        "colour" => values.colour.clone(),
        "notes" => values.notes.clone(),
        _ => String::new(),
    }
}

/// The form open.
struct Open {
    window: gtk::Window,
    heading: String,
    /// Each field's control, and what shows or hides with it (its label, its scroller).
    fields: HashMap<String, (Control, Vec<gtk::Widget>)>,
    /// Whether the controls are being filled from the app, not typed.
    filling: Rc<Cell<bool>>,
    /// The refusal last told, so one is told once.
    refusal: RefCell<Option<String>>,
}

thread_local! {
    static OPEN: RefCell<Option<Rc<Open>>> = const { RefCell::new(None) };
}

/// Draws the form the app has open: opens its window when it opens, refills it as the app
/// changes it, closes it when the app closes it.
pub fn draw(app: &App) {
    let view = app.model.view().block_form.clone();
    let open = OPEN.with_borrow(Clone::clone);
    match (view, open) {
        (None, None) => {}
        (None, Some(open)) => {
            OPEN.set(None);
            open.window.destroy();
        }
        (Some(view), Some(open)) if open.heading == view.heading => fill(&open, &view, app),
        (Some(view), open) => {
            if let Some(open) = open {
                open.window.destroy();
            }
            let open = build(app, &view);
            OPEN.set(Some(Rc::clone(&open)));
            fill(&open, &view, app);
            open.window.present();
            if let Some((control, _)) = open.fields.get("title") {
                prompts::focus_on(&control.widget());
            }
        }
    }
}

fn fill(open: &Rc<Open>, view: &BlockFormView, app: &App) {
    open.filling.set(true);
    for (key, (control, with)) in &open.fields {
        let shown = view.fields.iter().any(|field| &field.key == key);
        control.widget().set_visible(shown);
        for widget in with {
            widget.set_visible(shown);
        }
        // A hint the app changed: the repetition kept by a rule the words cannot say.
        if let Some(field) = view.fields.iter().find(|field| &field.key == key)
            && !field.hint.is_empty()
        {
            control.widget().update_property(&[gtk::accessible::Property::Description(&described(field))]);
        }
        control.fill(&value_of(&view.values, &view.date, key));
    }
    open.filling.set(false);
    // A refusal is told once, and the form stays with what was typed so it can be put right.
    let told = open.refusal.replace(view.refusal.clone());
    if let Some(refusal) = &view.refusal
        && told.as_ref() != Some(refusal)
    {
        prompts::fail(&open.window, refusal);
    }
    let _ = app;
}

fn build(app: &App, view: &BlockFormView) -> Rc<Open> {
    let adding = view.fields.iter().any(|field| field.key == "date");
    let form = lumenna_records::block_form();
    let marked = |key: &str, label: &str| lumenna_app::devices::marked(label, mnemonic(key), '_');
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let filling = Rc::new(Cell::new(false));
    let mut fields = HashMap::new();
    // Every field the core's form has, in its order, the ones this form never shows left out:
    // a new block's day only when adding, and one day's change only what one day can change.
    let once = !view.fields.iter().any(|field| field.key == "notes");
    for described in form.iter().filter(|field| (field.key != "date" || adding) && (!once || field.one_day)) {
        let key = described.key.clone();
        let mut with: Vec<gtk::Widget> = Vec::new();
        let control = match key.as_str() {
            "kind" => {
                let ids: Vec<String> = described.options.iter().map(|option| option.id.clone()).collect();
                let titles: Vec<&str> = described.options.iter().map(|option| option.title.as_str()).collect();
                Control::Kind(gtk::DropDown::from_strings(&titles), ids)
            }
            "accepts_tasks" | "counts_capacity" | "anchored" => {
                let check = prompts::check(&marked(&key, &described.label));
                check.set_margin_top(4);
                Control::Flag(check)
            }
            "notes" => Control::Notes(gtk::TextView::builder().accepts_tab(false).wrap_mode(gtk::WrapMode::WordChar).build()),
            _ => Control::Line(gtk::Entry::builder().activates_default(true).build()),
        };
        let widget = control.widget();
        if !matches!(control, Control::Flag(_)) {
            let label = gtk::Label::builder().label(marked(&key, &described.label)).use_underline(true).xalign(0.0).margin_top(6).build();
            label.set_mnemonic_widget(Some(&widget));
            body.append(&label);
            with.push(label.upcast());
        }
        if !described.hint.is_empty() {
            widget.update_property(&[gtk::accessible::Property::Description(&self::described(described))]);
        }
        // Tasks from is the filter language: it completes as the list's filter does.
        if let (Control::Line(entry), Some(lumenna_records::Syntax::Filter)) = (&control, &described.syntax) {
            crate::completion::attach(entry, crate::completion::Source::Filter);
        }
        match &control {
            Control::Notes(notes) => {
                prompts::leaves_on_tab(notes);
                let scroller = gtk::ScrolledWindow::builder()
                    .child(notes)
                    .min_content_height(64)
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .has_frame(true)
                    .build();
                body.append(&scroller);
                with.push(scroller.upcast());
            }
            Control::Line(entry) => {
                body.append(&widget);
                // A day or a time has GTK's own picker beneath its line, filling it.
                let picker: Option<gtk::Widget> = match described.kind {
                    FieldKind::Date => Some(crate::pickers::calendar(entry, None).upcast()),
                    FieldKind::Time => Some(crate::pickers::clock(entry, None, None).upcast()),
                    _ => None,
                };
                if let Some(picker) = picker {
                    body.append(&picker);
                    with.push(picker);
                }
            }
            _ => body.append(&widget),
        }
        // Each field typed or chosen goes to the app.
        let send = {
            let filling = Rc::clone(&filling);
            let key = key.clone();
            move |value: String| {
                if filling.get() {
                    return;
                }
                if let Some(app) = crate::window::app() {
                    app.model.send(ShellEvent::Block(BlockEvent::Field { key: key.clone(), value }));
                }
            }
        };
        match &control {
            Control::Line(entry) => {
                entry.connect_changed(move |entry| send(entry.text().to_string()));
            }
            Control::Kind(kind, ids) => {
                let ids = ids.clone();
                kind.connect_selected_notify(move |kind| send(ids.get(kind.selected() as usize).cloned().unwrap_or_default()));
            }
            Control::Flag(check) => {
                check.connect_toggled(move |check| send(check.is_active().to_string()));
            }
            Control::Notes(notes) => {
                notes.buffer().connect_changed(move |buffer| {
                    send(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string());
                });
            }
        }
        fields.insert(key, (control, with));
    }
    // The app's word for the button; its mnemonic this app's, clear of every field's.
    let yes = if view.yes == "Save" { "Sa_ve".to_owned() } else { view.yes.replace('_', "__") };
    let ok = gtk::Button::with_mnemonic(&yes);
    ok.add_css_class("suggested-action");
    let cancel = gtk::Button::with_label("Cancel");
    let buttons = gtk::Box::builder().spacing(8).halign(gtk::Align::End).margin_top(12).build();
    buttons.append(&cancel);
    buttons.append(&ok);
    body.append(&buttons);
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&body)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(640)
        .build();
    let window = gtk::Window::builder()
        .title(&view.heading)
        .modal(true)
        .transient_for(&app.window)
        .destroy_with_parent(true)
        .default_width(440)
        .child(&scrolled)
        .build();
    window.set_default_widget(Some(&ok));
    let send = |event: BlockEvent| {
        if let Some(app) = crate::window::app() {
            app.model.send(ShellEvent::Block(event));
        }
    };
    ok.connect_clicked(move |_| send(BlockEvent::Save));
    cancel.connect_clicked(move |_| send(BlockEvent::Cancel));
    window.connect_close_request(move |_| {
        send(BlockEvent::Cancel);
        glib::Propagation::Stop
    });
    let escape = gtk::EventControllerKey::new();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            send(BlockEvent::Cancel);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(escape);
    Rc::new(Open { window, heading: view.heading.clone(), fields, filling, refusal: RefCell::new(None) })
}

/// A new block, on the day shown or today, from the menu or the Add Block button.
pub fn add(app: &App) {
    app.model.send(ShellEvent::Block(BlockEvent::New { start: None, minutes: None }));
}
