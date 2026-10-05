//! Adding or changing a block (§16.1: block editor; §13: creating is a form).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures_channel::oneshot;
use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_surface::{BlockEdit, BlockScope, Change, Lumenna, NewBlock};

use crate::core::sentence;
use crate::prompts;

const KINDS: [(&str, &str); 3] = [("work", "Work, takes tasks"), ("break", "Break"), ("event", "Event")];

/// What the form is for.
pub enum Purpose {
    /// A new block, starting on this date phrase.
    Add { date: String },
    /// Every occurrence of a series.
    Series { id: String },
    /// One day of a repeating series alone.
    Occurrence { series: String, date: String },
}

/// A block's fields as the form shows them.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Fields {
    pub title: String,
    pub start: String,
    pub minutes: String,
    pub kind: String,
    pub repeat: String,
    /// The RFC 5545 rule it repeats by. When `repeat` is empty, it is a rule the repetition
    /// words cannot say: the field starts empty, the rule is said beside it, and it is left
    /// alone unless something is typed there.
    pub rule: Option<String>,
}

struct Form {
    title: gtk::Entry,
    start: gtk::Entry,
    minutes: gtk::Entry,
    kind: gtk::DropDown,
    repeat: gtk::Entry,
    date: gtk::Entry,
}

impl Form {
    fn read(&self, initial: &Fields) -> Fields {
        Fields {
            title: self.title.text().trim().to_owned(),
            start: self.start.text().trim().to_owned(),
            minutes: self.minutes.text().trim().to_owned(),
            kind: KINDS.get(self.kind.selected() as usize).map_or("work", |k| k.0).to_owned(),
            repeat: self.repeat.text().trim().to_owned(),
            rule: initial.rule.clone(),
        }
    }

    fn save(&self, lumenna: &Lumenna, purpose: &Purpose, initial: &Fields) -> Result<Change, String> {
        let fields = self.read(initial);
        let minutes: u32 = fields.minutes.parse().map_err(|_| "Minutes has to be a whole number.".to_owned())?;
        let changed = |now: &String, was: &String| (now != was).then(|| now.clone());
        let result = match purpose {
            Purpose::Add { .. } => lumenna.add_block(NewBlock {
                title: fields.title.clone(),
                at: fields.start.clone(),
                minutes,
                date: Some(self.date.text().trim().to_owned()).filter(|d| !d.is_empty()),
                kind: fields.kind.clone(),
                repeat: Some(fields.repeat.clone()).filter(|r| !r.is_empty()),
            }),
            // Only what changed, so a concurrent edit to another field elsewhere stands.
            Purpose::Series { id } => lumenna.edit_block(
                id,
                BlockEdit {
                    title: changed(&fields.title, &initial.title),
                    at: changed(&fields.start, &initial.start),
                    minutes: (fields.minutes != initial.minutes).then_some(minutes),
                    kind: changed(&fields.kind, &initial.kind),
                    repeat: changed(&fields.repeat, &initial.repeat)
                        .map(|r| if r.is_empty() { "none".to_owned() } else { r }),
                },
                BlockScope::Series,
            ),
            Purpose::Occurrence { series, date } => lumenna.edit_block(
                series,
                BlockEdit {
                    title: changed(&fields.title, &initial.title),
                    at: changed(&fields.start, &initial.start),
                    minutes: (fields.minutes != initial.minutes).then_some(minutes),
                    kind: changed(&fields.kind, &initial.kind),
                    repeat: None,
                },
                BlockScope::Occurrence { date: date.clone() },
            ),
        };
        result.map_err(|error| sentence(&error))
    }
}

/// Runs the form, and returns what saving it did.
pub async fn run(parent: &gtk::Window, lumenna: Arc<Lumenna>, purpose: Purpose, initial: Fields) -> Option<Change> {
    let heading = match &purpose {
        Purpose::Add { .. } => "New Block".to_owned(),
        Purpose::Series { .. } => format!("Change {}, Every Occurrence", initial.title),
        Purpose::Occurrence { .. } => format!("Change {}, This Day Only", initial.title),
    };
    let adding = matches!(purpose, Purpose::Add { .. });
    let once = matches!(purpose, Purpose::Occurrence { .. });
    let form = Rc::new(Form {
        title: gtk::Entry::builder().text(&initial.title).activates_default(true).build(),
        start: gtk::Entry::builder().text(&initial.start).activates_default(true).build(),
        minutes: gtk::Entry::builder().text(&initial.minutes).activates_default(true).input_purpose(gtk::InputPurpose::Digits).build(),
        kind: gtk::DropDown::from_strings(&KINDS.map(|k| k.1)),
        repeat: gtk::Entry::builder().text(&initial.repeat).activates_default(true).build(),
        date: gtk::Entry::builder().activates_default(true).build(),
    });
    form.kind.set_selected(KINDS.iter().position(|(k, _)| *k == initial.kind).unwrap_or(0) as u32);
    if let Purpose::Add { date } = &purpose {
        form.date.set_text(date);
    }

    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let field = |text: &str, widget: &gtk::Widget| {
        let label = gtk::Label::builder().label(text).use_underline(true).xalign(0.0).margin_top(6).build();
        label.set_mnemonic_widget(Some(widget));
        body.append(&label);
        body.append(widget);
    };
    field("_Title", form.title.upcast_ref());
    field("Starts _at", form.start.upcast_ref());
    form.start.update_property(&[gtk::accessible::Property::Description("Such as 9am, or 14:30.")]);
    field("_Minutes", form.minutes.upcast_ref());
    field("_Kind", form.kind.upcast_ref());
    if !once {
        field("_Repeats", form.repeat.upcast_ref());
        form.repeat.update_property(&[gtk::accessible::Property::Description(
            "Such as every weekday. Empty for once.",
        )]);
        if let Some(rule) = initial.rule.as_ref().filter(|_| initial.repeat.is_empty()) {
            let note = format!(
                "It repeats by the rule {rule}, which the repetition words cannot say. Leave Repeats empty to keep it."
            );
            let note = gtk::Label::builder().label(&note).wrap(true).xalign(0.0).build();
            form.repeat.update_relation(&[gtk::accessible::Relation::DescribedBy(&[note.upcast_ref()])]);
            body.append(&note);
        }
    }
    if adding {
        field("Starts _on", form.date.upcast_ref());
    }
    let ok = gtk::Button::with_mnemonic(if adding { "A_dd" } else { "_Save" });
    ok.add_css_class("suggested-action");
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    let buttons = gtk::Box::builder().spacing(8).halign(gtk::Align::End).margin_top(12).build();
    buttons.append(&cancel);
    buttons.append(&ok);
    body.append(&buttons);

    let window = gtk::Window::builder()
        .title(&heading)
        .modal(true)
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(420)
        .child(&body)
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
            Ok(change) => finish(Some(change)),
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
    form.title.grab_focus();
    receiver.await.ok().flatten()
}
