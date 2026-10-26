//! Pairing this computer with another of the person's devices: the app's pairing screen
//! (`DevicesView::pairing`), drawn as a dialog over Settings while the app has it open.
//!
//! On one network the two find each other: both wait, and DNS-SD does the rest. Anywhere
//! else, one shows a code and the other enters it. Either way both show the same three words,
//! and only if the person says they match on both does anything get paired — the one
//! mechanism for every platform. Running it, the code shown, the words and how it ended are the
//! app's; this asks the words and sends what the person does.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::devices::marked;
use lumenna_app::{DevicesEvent, PairEvent};
use lumenna_ffi::ShellEvent;
use lumenna_records::PairingWords;

use crate::prompts;
use crate::window::{App, spawn};

fn send(event: PairEvent) {
    if let Some(app) = crate::window::app() {
        app.model.send(ShellEvent::Devices(DevicesEvent::Pair(event)));
    }
}

struct Dialog {
    window: gtk::Window,
    wait: gtk::Button,
    my_code_label: gtk::Label,
    my_code: gtk::Entry,
    copy_code: gtk::Button,
    their_code: gtk::Entry,
    with_code: gtk::Button,
    cancel: gtk::Button,
    status: gtk::Label,
    /// Whether the person gave up while a pairing ran: the screen closes once it has stopped.
    closing: Cell<bool>,
    /// The words last asked about, so each set is asked once.
    asked: RefCell<Option<Vec<String>>>,
    /// The problem last drawn, so the need for a code is told once.
    problem: RefCell<Option<String>>,
    /// Whether the code shown was drawn yet, so focus goes to it once.
    shown_code: Cell<bool>,
    /// Whether a pairing was running when last drawn.
    running: Cell<bool>,
    /// The screen's words, the app's, as this computer is called in them.
    words: PairingWords,
}

thread_local! {
    static OPEN: RefCell<Option<Rc<Dialog>>> = const { RefCell::new(None) };
}

/// Says `text` on the dialog's status line, if it is open: whether it was.
pub fn say(text: &str) -> bool {
    let Some(dialog) = OPEN.with_borrow(Clone::clone) else { return false };
    // The need for a code is told in an alert of its own.
    if text == dialog.words.need_code {
        return true;
    }
    dialog.status.set_label(text);
    crate::window::announce(&dialog.status, text);
    true
}

/// Draws the pairing screen the app has open, over `parent`.
pub fn draw(app: &App, view: Option<&lumenna_app::network::PairingView>, words: &PairingWords, parent: &gtk::Window) {
    let open = OPEN.with_borrow(Clone::clone);
    match (view, open) {
        (None, None) => {}
        (None, Some(dialog)) => {
            OPEN.set(None);
            dialog.window.destroy();
        }
        (Some(view), open) => {
            let dialog = open.unwrap_or_else(|| {
                let dialog = build(parent, words.clone());
                OPEN.set(Some(Rc::clone(&dialog)));
                dialog.window.present();
                prompts::focus_on(&dialog.wait);
                dialog
            });
            dialog.show(app, view);
        }
    }
}

impl Dialog {
    fn show(self: &Rc<Self>, _app: &App, view: &lumenna_app::network::PairingView) {
        let was_running = self.running.replace(view.running);
        if view.running && !was_running {
            // Focus somewhere that stays: a disabled button that had it leaves it nowhere.
            self.cancel.grab_focus();
        }
        prompts::set_sensitive(&self.wait, !view.running, &[self.cancel.upcast_ref()]);
        // While waiting, a code can still be entered: it gives the wait up and joins instead,
        // and the button pressed to join goes off with focus on it.
        prompts::set_sensitive(&self.with_code, !view.running || view.waiting, &[self.cancel.upcast_ref()]);
        let code = view.mine.clone().unwrap_or_default();
        if self.my_code.text() != code {
            self.my_code.set_text(&code);
        }
        let showing = view.mine.is_some();
        self.my_code_label.set_visible(showing);
        self.my_code.set_visible(showing);
        self.copy_code.set_visible(showing);
        if showing && !self.shown_code.replace(true) {
            self.my_code.grab_focus();
        }
        if !showing {
            self.shown_code.set(false);
        }
        if !view.running && was_running {
            if self.closing.take() {
                send(PairEvent::Cancel);
                return;
            }
            self.wait.grab_focus();
        }
        // The need for a code is told, then the person is back at the field for it.
        let need = self.words.need_code.clone();
        let told = self.problem.replace(view.problem.clone());
        if view.problem.as_deref() == Some(need.as_str()) && told.as_deref() != Some(need.as_str()) {
            let dialog = Rc::clone(self);
            spawn(async move {
                prompts::tell(&dialog.window, &need).await;
                prompts::focus_on(&dialog.their_code);
            });
        }
        // The words, asked once each time they come: whether they match on the other device.
        let asked = self.asked.replace(view.words.clone());
        if let Some(three) = &view.words
            && asked.as_ref() != Some(three)
        {
            let dialog = Rc::clone(self);
            let three = three.clone();
            spawn(async move {
                let said = &dialog.words;
                let detail = format!("{} {}.", said.match_message, three.join(", "));
                let matched = prompts::yes_or_no(&dialog.window, &said.match_title, &detail, &said.match_no, &said.match_yes).await;
                send(PairEvent::Confirm(matched));
            });
        }
    }

    /// Gives up: the pairing under way, and then the screen.
    fn give_up(&self) {
        if self.running.get() {
            self.closing.set(true);
        }
        send(PairEvent::Cancel);
    }
}

fn build(parent: &gtk::Window, words: PairingWords) -> Rc<Dialog> {
    let intro = gtk::Label::builder().label(&words.intro).wrap(true).xalign(0.0).build();
    let wait = gtk::Button::with_mnemonic(&marked(&words.wait, 'W', '_'));
    let my_code = gtk::Entry::builder().editable(false).visible(false).build();
    let my_code_label = gtk::Label::builder().label(marked(&words.my_code, 'c', '_')).use_underline(true).xalign(0.0).visible(false).build();
    my_code_label.set_mnemonic_widget(Some(&my_code));
    let copy_code = gtk::Button::builder().label(&words.copy_code).visible(false).halign(gtk::Align::Start).build();
    let their_code = gtk::Entry::builder().activates_default(false).build();
    let their_label = gtk::Label::builder().label(marked(&words.their_code, 'o', '_')).use_underline(true).xalign(0.0).build();
    their_label.set_mnemonic_widget(Some(&their_code));
    let with_code = gtk::Button::with_mnemonic(&marked(&words.join, 'P', '_'));
    let status = gtk::Label::builder().wrap(true).xalign(0.0).build();
    let cancel = gtk::Button::with_mnemonic("_Cancel");
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    for widget in [
        intro.upcast_ref::<gtk::Widget>(),
        wait.upcast_ref(),
        my_code_label.upcast_ref(),
        my_code.upcast_ref(),
        copy_code.upcast_ref(),
        their_label.upcast_ref(),
        their_code.upcast_ref(),
        with_code.upcast_ref(),
        status.upcast_ref(),
    ] {
        body.append(widget);
    }
    cancel.set_halign(gtk::Align::End);
    body.append(&cancel);
    wait.set_halign(gtk::Align::Start);
    with_code.set_halign(gtk::Align::Start);
    let window = gtk::Window::builder()
        .title(&words.title)
        .modal(true)
        .transient_for(parent)
        .destroy_with_parent(true)
        .default_width(480)
        .child(&body)
        .build();
    let dialog = Rc::new(Dialog {
        window: window.clone(),
        wait: wait.clone(),
        my_code_label,
        my_code: my_code.clone(),
        copy_code: copy_code.clone(),
        their_code: their_code.clone(),
        with_code: with_code.clone(),
        cancel: cancel.clone(),
        status,
        closing: Cell::new(false),
        asked: RefCell::new(None),
        problem: RefCell::new(None),
        shown_code: Cell::new(false),
        running: Cell::new(false),
        words,
    });
    wait.connect_clicked(|_| send(PairEvent::Wait));
    {
        let weak = Rc::downgrade(&dialog);
        copy_code.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.my_code.clipboard().set_text(&dialog.my_code.text());
                say(&dialog.words.copied);
            }
        });
    }
    // The code typed or pasted is the app's to keep; the clipboard is never read unasked:
    // nobody could tell it was, and it would send whatever happened to be copied.
    their_code.connect_changed(|entry| send(PairEvent::Code(entry.text().to_string())));
    with_code.connect_clicked(|_| send(PairEvent::Join));
    their_code.connect_activate(|_| send(PairEvent::Join));
    {
        let weak = Rc::downgrade(&dialog);
        cancel.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.give_up();
            }
        });
    }
    {
        let weak = Rc::downgrade(&dialog);
        window.connect_close_request(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.give_up();
            }
            glib::Propagation::Stop
        });
    }
    let escape = gtk::EventControllerKey::new();
    {
        let weak = Rc::downgrade(&dialog);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(dialog) = weak.upgrade() {
                    dialog.give_up();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(escape);
    dialog
}
