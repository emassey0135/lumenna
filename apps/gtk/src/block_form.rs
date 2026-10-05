//! Adding or changing a block (§16.1: block editor; §13: creating is a form).
//!
//! What a form starts from and what saving it sends are the surface's (`block_fields`,
//! `day_block_fields`, `block_edit`, `new_block`), as a task's are, so every client changes a
//! block by the same rules. A new block or every occurrence takes every setting; one day of a
//! repeating block takes only what a day can differ in — its time, length, title, kind and
//! three flags.
//!
//! The three flags start from the kind, and go back to the new kind's when it is changed.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures_channel::oneshot;
use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_surface::{BlockFields, BlockScope, Change, Lumenna, block_defaults, block_edit, new_block};

use crate::core::sentence;
use crate::prompts;

const KINDS: [(&str, &str); 3] = [("work", "Work"), ("break", "Break"), ("event", "Event")];

/// What the form is for.
pub enum Purpose {
    /// A new block, starting on this date phrase.
    Add { date: String },
    /// Every occurrence of a series.
    Series { id: String },
    /// One day of a repeating series alone.
    Occurrence { series: String, date: String },
}

/// A new block's fields: a work block from `start`, `minutes` long, with a work block's flags.
pub fn new_fields(start: &str, minutes: u32) -> BlockFields {
    let mut fields = BlockFields {
        start: start.to_owned(),
        minutes: minutes.to_string(),
        kind: "work".to_owned(),
        ..BlockFields::default()
    };
    if let Some(defaults) = block_defaults(fields.kind.clone()) {
        fields.accepts_tasks = defaults.accepts_tasks;
        fields.counts_capacity = defaults.counts_capacity;
        fields.anchored = defaults.anchored;
    }
    fields
}

struct Form {
    title: gtk::Entry,
    start: gtk::Entry,
    minutes: gtk::Entry,
    kind: gtk::DropDown,
    accepts_tasks: gtk::CheckButton,
    counts_capacity: gtk::CheckButton,
    anchored: gtk::CheckButton,
    repeat: gtk::Entry,
    until: gtk::Entry,
    min_minutes: gtk::Entry,
    task_filter: gtk::Entry,
    colour: gtk::Entry,
    notes: gtk::TextView,
    date: gtk::Entry,
}

impl Form {
    fn read(&self, initial: &BlockFields, once: bool) -> BlockFields {
        let text = |entry: &gtk::Entry| entry.text().trim().to_owned();
        let buffer = self.notes.buffer();
        let fields = BlockFields {
            title: text(&self.title),
            start: text(&self.start),
            minutes: text(&self.minutes),
            kind: KINDS.get(self.kind.selected() as usize).map_or("work", |k| k.0).to_owned(),
            accepts_tasks: self.accepts_tasks.is_active(),
            counts_capacity: self.counts_capacity.is_active(),
            anchored: self.anchored.is_active(),
            repeat: text(&self.repeat),
            until: text(&self.until),
            min_minutes: text(&self.min_minutes),
            task_filter: text(&self.task_filter),
            colour: text(&self.colour),
            notes: buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string(),
        };
        if once {
            // A day holds none of the rest: they stay as they were, so are not sent.
            BlockFields {
                repeat: initial.repeat.clone(),
                until: initial.until.clone(),
                min_minutes: initial.min_minutes.clone(),
                task_filter: initial.task_filter.clone(),
                colour: initial.colour.clone(),
                notes: initial.notes.clone(),
                ..fields
            }
        } else {
            fields
        }
    }

    /// Saves the form. `Ok(None)` is nothing changed.
    fn save(&self, lumenna: &Lumenna, purpose: &Purpose, initial: &BlockFields) -> Result<Option<Change>, String> {
        let once = matches!(purpose, Purpose::Occurrence { .. });
        let fields = self.read(initial, once);
        let result = match purpose {
            Purpose::Add { .. } => {
                let date = Some(self.date.text().trim().to_owned());
                new_block(fields, date).and_then(|block| lumenna.add_block(block)).map(Some)
            }
            Purpose::Series { id } => match block_edit(initial.clone(), fields) {
                Ok(Some(edit)) => lumenna.edit_block(id, edit, BlockScope::Series).map(Some),
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            },
            Purpose::Occurrence { series, date } => match block_edit(initial.clone(), fields) {
                Ok(Some(edit)) => {
                    lumenna.edit_block(series, edit, BlockScope::Occurrence { date: date.clone() }).map(Some)
                }
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            },
        };
        result.map_err(|error| sentence(&error))
    }
}

/// Runs the form, and returns what saving it did. `rule` is the RFC 5545 rule the block
/// repeats by, when the repetition words cannot say it: it is said beside an empty Repeats.
pub async fn run(
    parent: &gtk::Window,
    lumenna: Arc<Lumenna>,
    purpose: Purpose,
    initial: BlockFields,
    rule: Option<String>,
) -> Option<Change> {
    let heading = match &purpose {
        Purpose::Add { .. } => "New Block".to_owned(),
        Purpose::Series { .. } => format!("Change {}, Every Occurrence", initial.title),
        Purpose::Occurrence { .. } => format!("Change {}, This Day Only", initial.title),
    };
    let adding = matches!(purpose, Purpose::Add { .. });
    let once = matches!(purpose, Purpose::Occurrence { .. });
    let entry = |text: &str| gtk::Entry::builder().text(text).activates_default(true).build();
    let form = Rc::new(Form {
        title: entry(&initial.title),
        start: entry(&initial.start),
        minutes: entry(&initial.minutes),
        kind: gtk::DropDown::from_strings(&KINDS.map(|k| k.1)),
        accepts_tasks: prompts::check("Tasks can _go here"),
        counts_capacity: prompts::check("Counts toward the _hours for work"),
        anchored: prompts::check("_Fixed in time, never moved when the day slips"),
        repeat: entry(&initial.repeat),
        until: entry(&initial.until),
        min_minutes: entry(&initial.min_minutes),
        task_filter: entry(&initial.task_filter),
        colour: entry(&initial.colour),
        notes: gtk::TextView::builder().accepts_tab(false).wrap_mode(gtk::WrapMode::WordChar).build(),
        date: entry(""),
    });
    form.kind.set_selected(KINDS.iter().position(|(k, _)| *k == initial.kind).unwrap_or(0) as u32);
    form.accepts_tasks.set_active(initial.accepts_tasks);
    form.counts_capacity.set_active(initial.counts_capacity);
    form.anchored.set_active(initial.anchored);
    form.notes.buffer().set_text(&initial.notes);
    prompts::leaves_on_tab(&form.notes);
    if let Purpose::Add { date } = &purpose {
        form.date.set_text(date);
    }
    // A new kind brings its own flags, as saving will.
    {
        let weak = Rc::downgrade(&form);
        form.kind.connect_selected_notify(move |dropdown| {
            let Some(form) = weak.upgrade() else { return };
            let word = KINDS.get(dropdown.selected() as usize).map_or("work", |k| k.0);
            if let Some(defaults) = block_defaults(word.to_owned()) {
                form.accepts_tasks.set_active(defaults.accepts_tasks);
                form.counts_capacity.set_active(defaults.counts_capacity);
                form.anchored.set_active(defaults.anchored);
            }
        });
    }

    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let field = |text: &str, widget: &gtk::Widget, description: &str| {
        let label = gtk::Label::builder().label(text).use_underline(true).xalign(0.0).margin_top(6).build();
        label.set_mnemonic_widget(Some(widget));
        if !description.is_empty() {
            widget.update_property(&[gtk::accessible::Property::Description(description)]);
        }
        body.append(&label);
        body.append(widget);
    };
    field("_Title", form.title.upcast_ref(), "");
    field("Starts _at", form.start.upcast_ref(), "Such as 9am, or 14:30.");
    field("_Minutes", form.minutes.upcast_ref(), "");
    field("_Kind", form.kind.upcast_ref(), "Changing it sets the three choices after it to the kind's own.");
    for check in [&form.accepts_tasks, &form.counts_capacity, &form.anchored] {
        check.set_margin_top(4);
        body.append(check);
    }
    if !once {
        field("_Repeats", form.repeat.upcast_ref(), "Such as every weekday. Empty for once.");
        if let Some(rule) = rule.filter(|_| initial.repeat.is_empty()) {
            let note = format!(
                "It repeats by the rule {rule}, which the repetition words cannot say. Leave Repeats empty to keep it."
            );
            let note = gtk::Label::builder().label(&note).wrap(true).xalign(0.0).build();
            form.repeat.update_relation(&[gtk::accessible::Relation::DescribedBy(&[note.upcast_ref()])]);
            body.append(&note);
        }
        field(
            "Repeats _until",
            form.until.upcast_ref(),
            "The last day it happens, such as 31 december. Empty for for good.",
        );
        field(
            "Shortest l_ength, in minutes",
            form.min_minutes.upcast_ref(),
            "How short it may become when the day slips. Empty for its kind's own.",
        );
        field("Ta_sk filter", form.task_filter.upcast_ref(), "Which tasks it is offered, such as #Work. Empty for all.");
        field("_Colour", form.colour.upcast_ref(), "A colour name, such as teal. Empty for none.");
        let notes = gtk::ScrolledWindow::builder()
            .child(&form.notes)
            .min_content_height(64)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .has_frame(true)
            .build();
        let label = gtk::Label::builder().label("_Notes").use_underline(true).xalign(0.0).margin_top(6).build();
        label.set_mnemonic_widget(Some(&form.notes));
        body.append(&label);
        body.append(&notes);
    }
    if adding {
        field("Starts _on", form.date.upcast_ref(), "");
    }
    let ok = gtk::Button::with_mnemonic(if adding { "Add _Block" } else { "Sa_ve" });
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
        .title(&heading)
        .modal(true)
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(440)
        .child(&scrolled)
        .build();
    window.set_default_widget(Some(&ok));

    let (sender, receiver) = oneshot::channel::<Option<Change>>();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let finish = {
        let sender = Rc::clone(&sender);
        let window = window.clone();
        Rc::new(move |change: Option<Change>| {
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(change);
            }
            window.destroy();
        })
    };
    {
        let finish = Rc::clone(&finish);
        let form = Rc::clone(&form);
        let window = window.clone();
        ok.connect_clicked(move |_| match form.save(&lumenna, &purpose, &initial) {
            Ok(change) => finish(change),
            // The form stays, with what was typed, so it can be put right.
            Err(message) => prompts::fail(&window, &message),
        });
    }
    {
        let finish = Rc::clone(&finish);
        cancel.connect_clicked(move |_| finish(None));
    }
    {
        let sender = Rc::clone(&sender);
        window.connect_close_request(move |_| {
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(None);
            }
            glib::Propagation::Proceed
        });
    }
    let escape = gtk::EventControllerKey::new();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            finish(None);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(escape);
    window.present();
    prompts::focus_on(&form.title);
    receiver.await.ok().flatten()
}
