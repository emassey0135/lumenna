//! Adding a task the way it would be said: one line, read back as it is typed.
//!
//! The readback is what a sighted user gets from inline highlighting: what will be saved,
//! with the date resolved. It is a read-only field after the line, not spoken as it changes —
//! speech on every keystroke buries the typing — but Tab reaches it, and it is what is
//! announced when the task is added.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures_channel::oneshot;
use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_surface::{Change, Lumenna};

use crate::core::sentence;
use crate::prompts;

/// What the line would add, said in full: the readback, then anything wrong with it — there
/// is no squiggle under the text, so this is the only channel.
fn readback(lumenna: &Lumenna, text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    match lumenna.preview_task(text) {
        Ok(preview) => std::iter::once(preview.announcement)
            .chain(preview.diagnostics.into_iter().map(|d| d.message))
            .collect::<Vec<_>>()
            .join(". "),
        Err(error) => sentence(&error),
    }
}

/// Asks for a task in quick-add words, starting from `initial`, and adds it. `parent` is the
/// main window when it is showing; from anywhere, the dialog stands on its own.
pub async fn run(parent: Option<&gtk::Window>, application: &gtk::Application, lumenna: Arc<Lumenna>, initial: &str) -> Option<Change> {
    let window = gtk::Window::builder().title("New Task").modal(parent.is_some()).default_width(480).application(application).build();
    if let Some(parent) = parent {
        window.set_transient_for(Some(parent));
    }
    let line = gtk::Entry::builder().text(initial).activates_default(true).build();
    let line_label = gtk::Label::builder().label("_Task:").use_underline(true).xalign(0.0).build();
    line_label.set_mnemonic_widget(Some(&line));
    let hint = "Such as: write the chapter tomorrow p1 #Work. Down arrow offers what could come next.";
    line.update_property(&[gtk::accessible::Property::Description(hint)]);
    let hint = gtk::Label::builder().label(hint).wrap(true).xalign(0.0).build();
    let shown = prompts::read_only_text();
    let shown_label = gtk::Label::builder().label("_Will add:").use_underline(true).xalign(0.0).build();
    shown_label.set_mnemonic_widget(Some(&shown));
    let add = gtk::Button::with_mnemonic("_Add");
    add.add_css_class("suggested-action");
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    let buttons = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).halign(gtk::Align::End).build();
    buttons.append(&cancel);
    buttons.append(&add);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    for widget in [line_label.upcast_ref::<gtk::Widget>(), line.upcast_ref(), hint.upcast_ref(), shown_label.upcast_ref(), shown.upcast_ref(), buttons.upcast_ref()] {
        body.append(widget);
    }
    window.set_child(Some(&body));
    window.set_default_widget(Some(&add));

    let update = {
        let lumenna = Arc::clone(&lumenna);
        let shown = shown.clone();
        let add = add.clone();
        move |line: &gtk::Entry| {
            let text = line.text();
            shown.set_text(&readback(&lumenna, &text));
            add.set_sensitive(!text.trim().is_empty());
        }
    };
    update(&line);
    line.connect_changed(update);
    crate::completion::attach(&line, Arc::clone(&lumenna), lumenna_surface::Syntax::QuickAdd);

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
        let line = line.clone();
        let window = window.clone();
        add.connect_clicked(move |_| {
            let text = line.text();
            if text.trim().is_empty() {
                return;
            }
            match lumenna.add_task(&text) {
                Ok(change) => finish(Some(change)),
                Err(error) => prompts::fail(&window, &sentence(&error)),
            }
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
    {
        let finish = Rc::clone(&finish);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                finish(None);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(escape);
    window.present();
    prompts::focus_on(&line);
    // The cursor after the prefix, so typing carries on from `#Work `.
    line.set_position(-1);
    receiver.await.ok().flatten()
}
