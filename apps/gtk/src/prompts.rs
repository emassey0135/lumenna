//! Asking and telling: the few dialogs the app puts up, all modal to the main window.
//!
//! GTK 4 has no blocking `run`, so each is a future, awaited from a task spawned on the main
//! context (`window::spawn`). Confirmations and failures are `GtkAlertDialog`, the platform's
//! own. Choosing from a list and typing a value are small windows of stock widgets, with the
//! label and the field joined by a mnemonic, so the field is named by the label beside it.

use std::cell::RefCell;
use std::rc::Rc;

use futures_channel::oneshot;
use gtk::prelude::*;
use gtk::{gdk, glib};

/// Says something went wrong, with an OK button.
pub fn fail(parent: &impl IsA<gtk::Window>, message: &str) {
    gtk::AlertDialog::builder().message(message).modal(true).build().show(Some(parent));
}

/// Says something went wrong, and waits for OK.
pub async fn tell(parent: &impl IsA<gtk::Window>, message: &str) {
    let dialog = gtk::AlertDialog::builder().message(message).buttons(["OK"]).modal(true).build();
    let _ = dialog.choose_future(Some(parent)).await;
}

/// Asks whether to go ahead with something that cannot be undone. Cancel is the default.
pub async fn confirm(parent: &impl IsA<gtk::Window>, heading: &str, detail: &str, action: &str) -> bool {
    let dialog = gtk::AlertDialog::builder()
        .message(heading)
        .detail(detail)
        .buttons(["Cancel", action])
        .cancel_button(0)
        .default_button(0)
        .modal(true)
        .build();
    dialog.choose_future(Some(parent)).await == Ok(1)
}

/// Asks which of `options` to go ahead with, or none: a button each, then Cancel, which is
/// the default.
pub async fn choose(parent: &impl IsA<gtk::Window>, heading: &str, detail: &str, options: &[&str]) -> Option<usize> {
    let mut buttons: Vec<&str> = options.to_vec();
    buttons.push("Cancel");
    let cancel = i32::try_from(options.len()).unwrap_or(i32::MAX);
    let dialog = gtk::AlertDialog::builder()
        .message(heading)
        .detail(detail)
        .buttons(buttons)
        .cancel_button(cancel)
        .default_button(cancel)
        .modal(true)
        .build();
    let chosen = dialog.choose_future(Some(parent)).await.ok()?;
    usize::try_from(chosen).ok().filter(|&index| index < options.len())
}

/// Opens a modal window holding `content` above Cancel and OK, which answers with what
/// `answer` makes of it when OK is chosen, and `None` when cancelled or closed. The window is
/// up when this returns, so the caller can put focus in it before awaiting the answer.
fn open<T: 'static>(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    content: &gtk::Widget,
    answer: impl Fn() -> Option<T> + 'static,
    confirm_label: &str,
) -> oneshot::Receiver<Option<T>> {
    let window = gtk::Window::builder()
        .title(title)
        .modal(true)
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(420)
        .build();
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    let ok = gtk::Button::with_mnemonic(confirm_label);
    ok.add_css_class("suggested-action");
    let buttons = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).halign(gtk::Align::End).build();
    buttons.append(&cancel);
    buttons.append(&ok);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    body.append(content);
    body.append(&buttons);
    window.set_child(Some(&body));
    window.set_default_widget(Some(&ok));

    let (sender, receiver) = oneshot::channel::<Option<T>>();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let finish = {
        let sender = Rc::clone(&sender);
        let window = window.clone();
        Rc::new(move |value: Option<T>| {
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(value);
            }
            window.destroy();
        })
    };
    {
        let finish = Rc::clone(&finish);
        ok.connect_clicked(move |_| {
            if let Some(value) = answer() {
                finish(Some(value));
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
    receiver
}

/// A label for `field`, with a mnemonic: the field is named by it.
fn label_for(text: &str, field: &impl IsA<gtk::Widget>) -> gtk::Label {
    let label = gtk::Label::builder().label(text).use_underline(true).xalign(0.0).build();
    label.set_mnemonic_widget(Some(field));
    label
}

/// Asks for one line of text. `message` says what is wanted, beneath the field.
pub async fn ask(parent: &impl IsA<gtk::Window>, title: &str, label: &str, message: &str, initial: &str) -> Option<String> {
    let entry = gtk::Entry::builder().text(initial).activates_default(true).build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&label_for(label, &entry));
    content.append(&entry);
    if !message.is_empty() {
        let note = gtk::Label::builder().label(message).wrap(true).xalign(0.0).build();
        entry.update_relation(&[gtk::accessible::Relation::DescribedBy(&[note.upcast_ref()])]);
        content.append(&note);
    }
    let field = entry.clone();
    let answer = open(parent, title, content.upcast_ref(), move || Some(field.text().to_string()), "_OK");
    entry.grab_focus();
    answer.await.ok().flatten()
}

/// Asks for one of `options`, from a list. Enter or a double click on one chooses it.
pub async fn pick(parent: &impl IsA<gtk::Window>, title: &str, label: &str, options: &[String]) -> Option<usize> {
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Browse).build();
    for option in options {
        let row = gtk::ListBoxRow::builder()
            .child(&gtk::Label::builder().label(option).xalign(0.0).margin_top(4).margin_bottom(4).margin_start(6).build())
            .build();
        list.append(&row);
    }
    if let Some(first) = list.row_at_index(0) {
        list.select_row(Some(&first));
    }
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&list)
        .min_content_height(240)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .has_frame(true)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&label_for(label, &list));
    content.append(&scrolled);
    let chosen = list.clone();
    let answer = open(
        parent,
        title,
        content.upcast_ref(),
        move || chosen.selected_row().and_then(|row| usize::try_from(row.index()).ok()),
        "_OK",
    );
    // Enter on a row is OK, as in any list chooser.
    list.connect_row_activated(|list, _| {
        if let Some(window) = list.root().and_downcast::<gtk::Window>()
            && let Some(default) = window.default_widget() {
                default.activate();
            }
    });
    if let Some(first) = list.row_at_index(0) {
        first.grab_focus();
    }
    answer.await.ok().flatten()
}
