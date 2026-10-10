//! Pairing this computer with another of the person's devices.
//!
//! On one network the two find each other: both wait, and DNS-SD does the rest. Anywhere
//! else, one shows a code and the other enters it. Either way both show the same three words,
//! and only if the person says they match on both does anything get paired — the one
//! mechanism for every platform. The pairing runs on a thread of its own and asks its
//! questions back through this dialog, as the Mac's sheet and the Windows dialog do.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_desktop::speech;
use lumenna_surface::{Lumenna, LumennaError, PairedWith, PairingPrompt, PairingWords, Reach};

use crate::core::sentence;
use crate::prompts;

/// The pairing screen's words, the core's: how the device calls itself is the one thing here.
fn words() -> PairingWords {
    lumenna_surface::pairing_words("this computer".to_owned(), true)
}

/// What the pairing thread has to say to the dialog.
enum Message {
    Code(String),
    Words(Vec<String>, Sender<bool>),
    Done(Result<PairedWith, String>),
}

/// The pairing's questions, asked through the dialog while the pairing thread waits.
struct Prompt {
    messages: Mutex<mpsc::UnboundedSender<Message>>,
    cancelled: Arc<AtomicBool>,
}

impl Prompt {
    fn send(&self, message: Message) -> bool {
        self.messages.lock().is_ok_and(|messages| messages.unbounded_send(message).is_ok())
    }
}

impl PairingPrompt for Prompt {
    fn show_code(&self, code: String) {
        self.send(Message::Code(code));
    }

    fn confirm(&self, words: Vec<String>) -> bool {
        let (answer, answered) = channel();
        if !self.send(Message::Words(words, answer)) {
            return false;
        }
        // Waits for the person, but not past their giving up: a dialog closed meanwhile never
        // answers.
        loop {
            match answered.recv_timeout(Duration::from_millis(250)) {
                Ok(matched) => return matched,
                Err(RecvTimeoutError::Timeout) if !self.cancelled.load(Ordering::Relaxed) => {}
                Err(_) => return false,
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
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
    lumenna: Arc<Lumenna>,
    cancelled: RefCell<Option<Arc<AtomicBool>>>,
    running: Cell<bool>,
    /// Whether the pairing running is the wait to be found, which a code entered gives up.
    waiting: Cell<bool>,
    /// A code entered while waiting, to join with once the wait has ended.
    pending: RefCell<Option<String>>,
    finished: RefCell<Option<oneshot::Sender<Option<PairedWith>>>>,
}

impl Dialog {
    fn say(&self, text: &str) {
        self.status.set_label(text);
        crate::window::announce(&self.status, text);
    }

    fn finish(&self, paired: Option<PairedWith>) {
        if let Some(cancelled) = self.cancelled.borrow().as_ref() {
            cancelled.store(true, Ordering::Relaxed);
        }
        if let Some(finished) = self.finished.borrow_mut().take() {
            let _ = finished.send(paired);
        }
        self.window.destroy();
    }

    fn start(self: &Rc<Self>, code: Option<String>) {
        if self.running.replace(true) {
            return;
        }
        // Focus somewhere that stays: a disabled button that had it leaves it nowhere.
        self.cancel.grab_focus();
        self.wait.set_sensitive(false);
        // While waiting, a code can still be entered: it gives the wait up and joins instead.
        self.with_code.set_sensitive(code.is_none());
        self.waiting.set(code.is_none());
        let words = words();
        self.say(if code.is_none() { &words.opening } else { &words.connecting });
        let cancelled = Arc::new(AtomicBool::new(false));
        *self.cancelled.borrow_mut() = Some(Arc::clone(&cancelled));
        let (sender, mut receiver) = mpsc::unbounded();
        let prompt = Arc::new(Prompt { messages: Mutex::new(sender.clone()), cancelled });
        let lumenna = Arc::clone(&self.lumenna);
        let name = glib::host_name().to_string();
        std::thread::spawn(move || {
            let result = lumenna
                .pair(code, Reach::Internet, name, "linux".to_owned(), prompt)
                .map_err(|error: LumennaError| sentence(&error));
            let _ = sender.unbounded_send(Message::Done(result));
        });
        let dialog = Rc::clone(self);
        glib::spawn_future_local(async move {
            while let Some(message) = receiver.next().await {
                if !dialog.heard(message).await {
                    break;
                }
            }
        });
    }

    /// Takes one message from the pairing thread. Returns whether more are to come.
    async fn heard(self: &Rc<Self>, message: Message) -> bool {
        match message {
            Message::Code(code) => {
                self.my_code.set_text(&code);
                self.my_code_label.set_visible(true);
                self.my_code.set_visible(true);
                self.copy_code.set_visible(true);
                self.my_code.grab_focus();
                let words = words();
                self.say(&words.waiting);
                true
            }
            Message::Words(words, answer) => {
                let said = self::words();
                let detail = format!("{} {}.", said.match_message, words.join(", "));
                let matched = prompts::yes_or_no(&self.window, &said.match_title, &detail, &said.match_no, &said.match_yes).await;
                // What happens meanwhile, while the other device hears the answer.
                self.say(if matched { &said.finishing } else { &said.refusing });
                let _ = answer.send(matched);
                true
            }
            Message::Done(Ok(paired)) => {
                self.running.set(false);
                self.finish(Some(paired));
                false
            }
            Message::Done(Err(message)) => {
                self.running.set(false);
                // The wait was given up for a code entered meanwhile: join with it now. That
                // the wait was cancelled is not news.
                if let Some(code) = self.pending.borrow_mut().take() {
                    self.start(Some(code));
                    return false;
                }
                self.my_code_label.set_visible(false);
                self.my_code.set_visible(false);
                self.copy_code.set_visible(false);
                self.wait.set_sensitive(true);
                self.with_code.set_sensitive(true);
                self.wait.grab_focus();
                self.say(&message);
                false
            }
        }
    }

    /// Pairs with the code typed in — or, if nothing was typed, the one on the clipboard,
    /// which is how a code sent from the other device usually arrives.
    async fn with_code(self: &Rc<Self>) {
        let mut code = self.their_code.text().trim().to_owned();
        if code.is_empty() {
            let pasted = self.their_code.clipboard().read_text_future().await.ok().flatten();
            // Not this device's own code, which Copy Code may have put on the clipboard.
            let own = self.my_code.text();
            if let Some(pasted) = pasted.map(|p| p.trim().to_owned()).filter(|p| !p.is_empty() && *p != own) {
                self.their_code.set_text(&pasted);
                code = pasted;
            }
        }
        if code.is_empty() {
            prompts::tell(&self.window, &words().need_code).await;
            prompts::focus_on(&self.their_code);
            return;
        }
        if self.running.get() {
            if self.waiting.get() {
                // One pairing at a time: the wait ends, and the pairing with the code follows.
                *self.pending.borrow_mut() = Some(code);
                if let Some(cancelled) = self.cancelled.borrow().as_ref() {
                    cancelled.store(true, Ordering::Relaxed);
                }
                self.say(&words().switching);
            }
            return;
        }
        self.start(Some(code));
    }
}

/// Runs the pairing dialog, and returns what was said of the device paired, if one was.
pub async fn run(parent: &gtk::Window, lumenna: Arc<Lumenna>) -> Option<String> {
    let words = words();
    let marked = |text: &str, key: char| lumenna_desktop::devices::marked(text, key, '_');
    let intro = gtk::Label::builder().label(&words.intro).wrap(true).xalign(0.0).build();
    let wait = gtk::Button::with_mnemonic(&marked(&words.wait, 'W'));
    let my_code = gtk::Entry::builder().editable(false).visible(false).build();
    let my_code_label = gtk::Label::builder().label(marked(&words.my_code, 'c')).use_underline(true).xalign(0.0).visible(false).build();
    my_code_label.set_mnemonic_widget(Some(&my_code));
    let copy_code = gtk::Button::builder().label(&words.copy_code).visible(false).halign(gtk::Align::Start).build();
    let their_code = gtk::Entry::builder().activates_default(false).build();
    let their_label = gtk::Label::builder().label(marked(&words.their_code, 'o')).use_underline(true).xalign(0.0).build();
    their_label.set_mnemonic_widget(Some(&their_code));
    // Shown, and read with the field as its description: a placeholder would go as soon as
    // anything was typed.
    let clipboard = gtk::Label::builder().label(&words.empty_means).wrap(true).xalign(0.0).build();
    their_code.update_property(&[gtk::accessible::Property::Description(&words.empty_means)]);
    let with_code = gtk::Button::with_mnemonic(&marked(&words.join, 'P'));
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
        clipboard.upcast_ref(),
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
    let (finished, done) = oneshot::channel();
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
        lumenna,
        cancelled: RefCell::new(None),
        running: Cell::new(false),
        waiting: Cell::new(false),
        pending: RefCell::new(None),
        finished: RefCell::new(Some(finished)),
    });
    {
        let dialog = Rc::clone(&dialog);
        wait.connect_clicked(move |_| dialog.start(None));
    }
    {
        let dialog = Rc::clone(&dialog);
        copy_code.connect_clicked(move |_| {
            dialog.my_code.clipboard().set_text(&dialog.my_code.text());
            dialog.say(&self::words().copied);
        });
    }
    {
        let dialog = Rc::clone(&dialog);
        with_code.connect_clicked(move |_| {
            let dialog = Rc::clone(&dialog);
            glib::spawn_future_local(async move { dialog.with_code().await });
        });
    }
    {
        let dialog = Rc::clone(&dialog);
        their_code.connect_activate(move |_| {
            let dialog = Rc::clone(&dialog);
            glib::spawn_future_local(async move { dialog.with_code().await });
        });
    }
    {
        let dialog = Rc::clone(&dialog);
        cancel.connect_clicked(move |_| dialog.finish(None));
    }
    {
        let dialog = Rc::downgrade(&dialog);
        window.connect_close_request(move |_| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.finish(None);
            }
            glib::Propagation::Proceed
        });
    }
    let escape = gtk::EventControllerKey::new();
    {
        let dialog = Rc::downgrade(&dialog);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(dialog) = dialog.upgrade() {
                    dialog.finish(None);
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(escape);
    window.present();
    prompts::focus_on(&wait);
    let paired = done.await.ok().flatten()?;
    Some(speech::announcement(&paired.announcement, &paired.notices))
}
