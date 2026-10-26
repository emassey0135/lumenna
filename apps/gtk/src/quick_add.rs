//! Adding a task the way it would be said: one line, read back as it is typed. The app's
//! quick-add line (`ViewModel::line`), drawn as a window of its own while the app has it open.
//!
//! The readback is what a sighted user gets from inline highlighting: what will be saved, with
//! the date resolved. It is a read-only field after the line, not spoken as it changes — speech
//! on every keystroke buries the typing — but Tab reaches it, and the app says it when the task
//! is added. The line, what it reads as, what fits at the cursor and why a line was refused are
//! the app's: each keystroke goes to it (`Typing::Text`), a completion chosen is spliced in by it
//! (`Typing::Choose`), and Add sends the line (`QuickAdd`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::Typing;
use lumenna_ffi::ShellEvent;

use crate::prompts;
use crate::window::App;

struct Dialog {
    window: gtk::Window,
    line: gtk::Entry,
    shown: gtk::Entry,
    add: gtk::Button,
    /// Whether the line is being set from the app, not typed.
    filling: Rc<Cell<bool>>,
    /// The refusal last told, so each is told once.
    refused: RefCell<Option<String>>,
}

thread_local! {
    static OPEN: RefCell<Option<Rc<Dialog>>> = const { RefCell::new(None) };
    /// Whether the next line opened stands on its own, from anywhere, starting empty.
    static ANYWHERE: Cell<bool> = const { Cell::new(false) };
}

fn send(event: ShellEvent) {
    if let Some(app) = crate::window::app() {
        app.model.send(event);
    }
}

/// Quick add, from the menu or from anywhere: from the menu it starts with what the place
/// shown adds, so a task added while looking at a project lands in it; from anywhere it starts
/// empty, and stands on its own unless the window is up.
pub fn open(app: &App, from_anywhere: bool) {
    if let Some(dialog) = OPEN.with_borrow(Clone::clone) {
        dialog.window.present();
        return;
    }
    ANYWHERE.set(from_anywhere);
    app.model.send(ShellEvent::Typing(Typing::Open));
    if from_anywhere {
        app.model.send(ShellEvent::Typing(Typing::Text { text: String::new(), cursor: 0 }));
    }
}

/// Draws the line the app has open: opens its window when it opens, closes it when the app
/// closes it — on adding, or on giving up.
pub fn draw(app: &App) {
    let view = app.model.view();
    let open = OPEN.with_borrow(Clone::clone);
    match (view.line.clone(), open) {
        (None, None) => {}
        (None, Some(dialog)) => {
            OPEN.set(None);
            dialog.window.destroy();
        }
        (Some(line), open) => {
            let dialog = open.unwrap_or_else(|| {
                let dialog = build(app);
                OPEN.set(Some(Rc::clone(&dialog)));
                dialog
            });
            dialog.show(&line, view.add_refused.as_deref());
        }
    }
}

/// Whether `text` is what the line open was refused for: told in an alert of its own, so not
/// said again.
pub fn say(text: &str) -> bool {
    OPEN.with_borrow(Clone::clone).is_some_and(|dialog| dialog.refused.borrow().as_deref() == Some(text))
}

impl Dialog {
    fn show(self: &Rc<Self>, line: &lumenna_app::LineView, refused: Option<&str>) {
        let first = !self.window.is_visible();
        if self.line.text() != line.text {
            self.filling.set(true);
            self.line.set_text(&line.text);
            let chars = line.text.get(..line.cursor as usize).map_or(0, |before| before.chars().count());
            self.line.set_position(i32::try_from(chars).unwrap_or(-1));
            self.filling.set(false);
        }
        if self.shown.text() != line.readback {
            self.shown.set_text(&line.readback);
        }
        self.add.set_sensitive(!line.text.trim().is_empty());
        if first {
            self.window.present();
            prompts::focus_on(&self.line);
            // The cursor after the prefix, so typing carries on from `#Work `.
            self.line.set_position(-1);
        }
        // A line refused stays as typed, and why is told once.
        let told = self.refused.replace(refused.map(str::to_owned));
        if let Some(why) = refused
            && told.as_deref() != Some(why)
        {
            prompts::fail(&self.window, why);
        }
    }
}

fn build(app: &App) -> Rc<Dialog> {
    let anywhere = ANYWHERE.take();
    // Every label and button the core's; the mnemonics this app's.
    let said = lumenna_records::fields::quick_add_words(false);
    let window = gtk::Window::builder().title(&said.title).default_width(480).application(&app.application).build();
    if !anywhere || app.window.is_visible() {
        window.set_modal(true);
        window.set_transient_for(Some(&app.window));
    }
    let marked = |text: &str, letter: char| lumenna_app::devices::marked(text, letter, '_');
    let line = gtk::Entry::builder().activates_default(true).placeholder_text(&said.example).build();
    let line_label = gtk::Label::builder().label(marked(&said.label, 'T')).use_underline(true).xalign(0.0).build();
    line_label.set_mnemonic_widget(Some(&line));
    line.update_property(&[gtk::accessible::Property::Description(&said.hint)]);
    let hint = gtk::Label::builder().label(&said.hint).wrap(true).xalign(0.0).build();
    let shown = prompts::read_only_text();
    let shown_label = gtk::Label::builder().label(marked(&said.will_add, 'W')).use_underline(true).xalign(0.0).build();
    shown_label.set_mnemonic_widget(Some(&shown));
    let add = gtk::Button::with_mnemonic(&marked(&said.add, 'A'));
    add.add_css_class("suggested-action");
    let cancel = gtk::Button::with_mnemonic(&marked(&said.cancel, 'C'));
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
    let filling = Rc::new(Cell::new(false));
    {
        let filling = Rc::clone(&filling);
        line.connect_changed(move |line| {
            if filling.get() {
                return;
            }
            let text = line.text().to_string();
            let chars = usize::try_from(line.position()).unwrap_or(0);
            let cursor = text.char_indices().nth(chars).map_or(text.len(), |(at, _)| at);
            send(ShellEvent::Typing(Typing::Text { text, cursor: u32::try_from(cursor).unwrap_or(u32::MAX) }));
        });
    }
    crate::completion::attach(&line, crate::completion::Source::QuickAdd);
    {
        let line = line.clone();
        add.connect_clicked(move |_| {
            let text = line.text().to_string();
            if !text.trim().is_empty() {
                send(ShellEvent::QuickAdd(text));
            }
        });
    }
    cancel.connect_clicked(|_| send(ShellEvent::Typing(Typing::Close)));
    window.connect_close_request(|_| {
        send(ShellEvent::Typing(Typing::Close));
        glib::Propagation::Stop
    });
    let escape = gtk::EventControllerKey::new();
    escape.connect_key_pressed(|_, key, _, _| {
        if key == gdk::Key::Escape {
            send(ShellEvent::Typing(Typing::Close));
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(escape);
    Rc::new(Dialog { window, line, shown, add, filling, refused: RefCell::new(None) })
}
