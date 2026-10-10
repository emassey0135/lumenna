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

use crate::tree::{Item, Tree};

/// Says something went wrong, with an OK button.
pub fn fail(parent: &impl IsA<gtk::Window>, message: &str) {
    gtk::AlertDialog::builder().message(message).modal(true).build().show(Some(parent));
}

/// Says something went wrong, and waits for OK.
pub async fn tell(parent: &impl IsA<gtk::Window>, message: &str) {
    let dialog = gtk::AlertDialog::builder().message(message).buttons(["OK"]).modal(true).build();
    let _ = dialog.choose_future(Some(parent)).await;
}

/// Asks whether to go ahead with something worth asking about first. Cancel is the default.
pub async fn confirm(parent: &impl IsA<gtk::Window>, heading: &str, detail: &str, action: &str) -> bool {
    yes_or_no(parent, heading, detail, "Cancel", action).await
}

/// Asks a question answered yes or no, `no` first and the default, as GNOME puts the safe
/// answer: on the left, and what Escape and Enter give.
pub async fn yes_or_no(parent: &impl IsA<gtk::Window>, heading: &str, detail: &str, no: &str, yes: &str) -> bool {
    let dialog = gtk::AlertDialog::builder()
        .message(heading)
        .detail(detail)
        .buttons([no, yes])
        .cancel_button(0)
        .default_button(0)
        .modal(true)
        .build();
    dialog.choose_future(Some(parent)).await == Ok(1)
}

/// Asks which of `options` to go ahead with, or none: Cancel first, as GNOME orders a
/// dialog's buttons, and the default; then a button each.
pub async fn choose(parent: &impl IsA<gtk::Window>, heading: &str, detail: &str, options: &[&str]) -> Option<usize> {
    let mut buttons = vec!["Cancel"];
    buttons.extend_from_slice(options);
    let dialog = gtk::AlertDialog::builder()
        .message(heading)
        .detail(detail)
        .buttons(buttons)
        .cancel_button(0)
        .default_button(0)
        .modal(true)
        .build();
    let chosen = dialog.choose_future(Some(parent)).await.ok()?;
    usize::try_from(chosen).ok().and_then(|index| index.checked_sub(1))
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
    let ok = gtk::Button::with_mnemonic(&with_mnemonic(confirm_label));
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

/// A button's words with its first letter as its mnemonic, unless that is Cancel's C: Enter
/// answers anyway, as the default.
fn with_mnemonic(text: &str) -> String {
    let escaped = text.replace('_', "__");
    match text.chars().next() {
        Some(first) if !first.eq_ignore_ascii_case(&'c') => format!("_{escaped}"),
        _ => escaped,
    }
}

/// Makes `widget` the focus of the window it is in. Not `grab_focus`: a window just presented
/// may not be mapped yet, and a popover menu closing in the main window — the usual way here —
/// moves focus around as it goes, so focus grabbed now was found lost.
pub fn focus_on(widget: &impl IsA<gtk::Widget>) {
    if let Some(window) = widget.root().and_downcast::<gtk::Window>() {
        gtk::prelude::GtkWindowExt::set_focus(&window, Some(widget));
    }
}

/// Gives Tab and Shift+Tab back from a text view, to move between fields: GTK keeps them in
/// one even when it does not accept tabs, and a keyboard user could not get out.
pub fn leaves_on_tab(view: &gtk::TextView) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = view.downgrade();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
        let direction = match key {
            gdk::Key::Tab | gdk::Key::KP_Tab if !modifiers.contains(gdk::ModifierType::SHIFT_MASK) => {
                gtk::DirectionType::TabForward
            }
            gdk::Key::Tab | gdk::Key::KP_Tab | gdk::Key::ISO_Left_Tab => gtk::DirectionType::TabBackward,
            _ => return glib::Propagation::Proceed,
        };
        // A text view with focus keeps it when asked to move on, so it steps out of the focus
        // chain while the window finds the next field.
        if let Some(root) = view.root() {
            view.set_focusable(false);
            root.child_focus(direction);
            view.set_focusable(true);
        }
        glib::Propagation::Stop
    });
    view.add_controller(keys);
}

/// Text to read and not change, which Tab reaches and leaves like a field: a read-only entry,
/// which Orca reads whole, rather than a label, which named by the label above it is read as
/// that name.
pub fn read_only_text() -> gtk::Entry {
    gtk::Entry::builder().editable(false).build()
}

/// A check box with a mnemonic. Named in so many words, since GTK leaves the underscore that
/// marks the mnemonic in a check box's accessible name, and Orca reads it out.
pub fn check(text: &str) -> gtk::CheckButton {
    let check = gtk::CheckButton::with_mnemonic(text);
    check.update_property(&[gtk::accessible::Property::Label(&text.replace('_', ""))]);
    check
}

/// A label for `field`, with a mnemonic: the field is named by it.
fn label_for(text: &str, field: &impl IsA<gtk::Widget>) -> gtk::Label {
    let label = gtk::Label::builder().label(text).use_underline(true).xalign(0.0).build();
    label.set_mnemonic_widget(Some(field));
    label
}

/// Asks for one line of text. `message` says what is wanted, beneath the field.
/// `yes` is the button that answers, a verb as GNOME has it ("Rename", "Add"), not OK.
pub async fn ask(parent: &impl IsA<gtk::Window>, title: &str, label: &str, message: &str, initial: &str, yes: &str) -> Option<String> {
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
    let answer = open(parent, title, content.upcast_ref(), move || Some(field.text().to_string()), yes);
    focus_on(&entry);
    answer.await.ok().flatten()
}

/// Asks for one of `options`, each a line and how deep it sits, from a list. Enter or a double
/// click on one chooses it.
///
/// The list is the app's tree, as every list is, so it reads and moves as they do: the arrows
/// move between the options, Tab goes on to the buttons, and a project tree nests.
pub async fn pick(parent: &impl IsA<gtk::Window>, title: &str, label: &str, options: &[(String, u32)], yes: &str) -> Option<usize> {
    let name = label.replace('_', "");
    let list = Tree::new(name.trim_end_matches(':'));
    list.widget.set_min_content_height(240);
    let items = options
        .iter()
        .enumerate()
        .map(|(index, (text, depth))| Item { key: index.to_string(), text: text.clone(), depth: *depth })
        .collect();
    if list.set(items) {
        list.select_key_or_near(None, Some(0));
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&label_for(label, &list.view));
    content.append(&list.widget);
    let chosen = Rc::clone(&list);
    let answer = open(parent, title, content.upcast_ref(), move || chosen.selected(), yes);
    // Enter on a row is OK, as in any list chooser.
    let view = list.view.downgrade();
    list.connect_activate(move |_| {
        if let Some(window) = view.upgrade().and_then(|view| view.root()).and_downcast::<gtk::Window>()
            && let Some(default) = window.default_widget()
        {
            default.activate();
        }
    });
    list.focus();
    answer.await.ok().flatten()
}
