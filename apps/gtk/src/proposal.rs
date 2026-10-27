//! Replan from Now and Plan the Day: the app's proposal screen (`ViewModel::proposal`), drawn
//! as a window of its own while the app has it open.
//!
//! Every word is the core's, and what is checked, what applying makes and what is proposed
//! next are the app's: each check box sends its change's state, Apply and the other options
//! go to the app, and a new proposal (`shown` moving) is a new list whose first check box takes
//! focus. What the app says while the screen is open is said from it, on its own status line:
//! the main window is behind it. The first, the proposal itself, waits until the window is in
//! front, or Orca reading its title and focus would cut it off.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::{ProposalEvent, ProposalView};
use lumenna_ffi::ShellEvent;

use crate::prompts;
use crate::window::App;

/// The screen open.
struct Screen {
    window: gtk::Window,
    status: gtk::Label,
    parts: gtk::Box,
    checks: RefCell<Vec<gtk::CheckButton>>,
    unfit_heading: gtk::Label,
    unfit: gtk::TextView,
    hint: gtk::Label,
    others_heading: gtk::Label,
    others: gtk::Box,
    apply: gtk::Button,
    cancel: gtk::Button,
    /// The proposal last drawn, by the app's count.
    shown: Cell<Option<u32>>,
    /// The other options last drawn.
    offered: RefCell<Vec<String>>,
    /// Whether the check boxes are being set from the app, not toggled.
    filling: Rc<Cell<bool>>,
    /// What to say once the window is in front, until it is.
    waiting: RefCell<Option<Vec<String>>>,
}

thread_local! {
    static OPEN: RefCell<Option<Rc<Screen>>> = const { RefCell::new(None) };
}

fn send(event: ProposalEvent) {
    if let Some(app) = crate::window::app() {
        app.model.send(ShellEvent::Proposal(event));
    }
}

/// Draws the screen the app has open: opens its window when it opens, shows each proposal
/// as it comes, closes it when the app closes it.
pub fn draw(app: &App) {
    let view = app.model.view().proposal.clone();
    let open = OPEN.with_borrow(Clone::clone);
    match (view, open) {
        (None, None) => {}
        (None, Some(screen)) => {
            OPEN.set(None);
            screen.window.destroy();
        }
        (Some(view), Some(screen)) => screen.show(&view),
        (Some(view), None) => {
            let screen = build(app, &view);
            OPEN.set(Some(Rc::clone(&screen)));
            screen.show(&view);
            screen.window.present();
        }
    }
}

/// Says `text` on the screen, if it is open: whether it was.
pub fn say(text: &str) -> bool {
    let Some(screen) = OPEN.with_borrow(Clone::clone) else { return false };
    if let Some(waiting) = screen.waiting.borrow_mut().as_mut() {
        waiting.push(text.to_owned());
        return true;
    }
    screen.status.set_label(text);
    crate::window::announce(&screen.status, text);
    true
}

impl Screen {
    /// Shows what the app says: a new list for a new proposal, else the checks as they are.
    fn show(&self, view: &ProposalView) {
        let new = self.shown.replace(Some(view.shown)) != Some(view.shown);
        self.filling.set(true);
        if new {
            while let Some(child) = self.parts.first_child() {
                self.parts.remove(&child);
            }
            let mut checks = Vec::new();
            for (index, change) in view.changes.iter().enumerate() {
                // A label without a mnemonic: the core's sentence is shown as it is.
                let check = gtk::CheckButton::with_label(&change.sentence);
                check.set_active(change.checked);
                let filling = Rc::clone(&self.filling);
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                check.connect_toggled(move |check| {
                    if !filling.get() {
                        send(ProposalEvent::Check { index, on: check.is_active() });
                    }
                });
                self.parts.append(&check);
                // Why, for a sitting: said after its name, and shown beneath it.
                if let Some(reason) = change.reason.as_ref().filter(|reason| !reason.is_empty()) {
                    check.update_property(&[gtk::accessible::Property::Description(reason)]);
                    let line = gtk::Label::builder().label(reason).wrap(true).xalign(0.0).margin_start(28).build();
                    line.add_css_class("dim-label");
                    self.parts.append(&line);
                }
                checks.push(check);
            }
            *self.checks.borrow_mut() = checks;
        } else {
            for (check, change) in self.checks.borrow().iter().zip(&view.changes) {
                if check.is_active() != change.checked {
                    check.set_active(change.checked);
                }
            }
        }
        self.filling.set(false);
        if *self.offered.borrow() != view.others {
            while let Some(child) = self.others.first_child() {
                self.others.remove(&child);
            }
            for (index, sentence) in view.others.iter().enumerate() {
                let label = gtk::Label::builder().label(sentence).wrap(true).xalign(0.0).build();
                let button = gtk::Button::builder().child(&label).build();
                button.update_property(&[gtk::accessible::Property::Label(sentence)]);
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                button.connect_clicked(move |_| send(ProposalEvent::Take(index)));
                self.others.append(&button);
            }
            *self.offered.borrow_mut() = view.others.clone();
        }
        let any = !view.changes.is_empty();
        let status = if any || !view.ready { view.announcement.clone() } else { view.words.nothing.clone() };
        if view.ready && self.status.label() != status.as_str() {
            self.status.set_label(&status);
        }
        self.unfit.buffer().set_text(&view.unfit.join("\n"));
        self.unfit_heading.set_visible(!view.unfit.is_empty());
        self.unfit.set_visible(!view.unfit.is_empty());
        self.others_heading.set_visible(!view.others.is_empty() && !view.words.others.is_empty());
        self.apply.set_visible(any);
        self.hint.set_visible(any);
        // Cancel stays while the store works, so focus waits there rather than nowhere.
        prompts::set_sensitive(&self.apply, !view.busy, &[self.cancel.upcast_ref()]);
        prompts::set_sensitive(&self.others, !view.busy, &[self.cancel.upcast_ref()]);
        if new && view.ready {
            match self.checks.borrow().first() {
                Some(check) => prompts::focus_on(check),
                None => prompts::focus_on(&self.cancel),
            }
        }
    }
}

fn build(app: &App, view: &ProposalView) -> Rc<Screen> {
    let words = &view.words;
    let status = gtk::Label::builder().xalign(0.0).wrap(true).build();
    let parts = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).build();
    // Named after the heading over it; a read-only text, so Tab reaches it and the arrows
    // read it line by line.
    let unfit_heading = gtk::Label::builder()
        .label(&words.unfit)
        .xalign(0.0)
        .margin_top(6)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    unfit_heading.add_css_class("heading");
    let unfit = gtk::TextView::builder().editable(false).cursor_visible(false).wrap_mode(gtk::WrapMode::WordChar).build();
    unfit.update_relation(&[gtk::accessible::Relation::LabelledBy(&[unfit_heading.upcast_ref()])]);
    prompts::leaves_on_tab(&unfit);
    let others_heading = gtk::Label::builder()
        .label(&words.others)
        .xalign(0.0)
        .margin_top(6)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    others_heading.add_css_class("heading");
    let others = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).build();
    let hint = gtk::Label::builder().label(&words.hint).wrap(true).xalign(0.0).build();
    hint.add_css_class("dim-label");
    let cancel = gtk::Button::with_label(&words.cancel);
    let apply = gtk::Button::with_label(&words.apply);
    apply.add_css_class("suggested-action");
    apply.update_property(&[gtk::accessible::Property::Description(&words.hint)]);
    let buttons = gtk::Box::builder().spacing(8).halign(gtk::Align::End).margin_top(6).build();
    buttons.append(&cancel);
    buttons.append(&apply);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    for widget in [
        status.upcast_ref::<gtk::Widget>(),
        parts.upcast_ref(),
        unfit_heading.upcast_ref(),
        unfit.upcast_ref(),
        others_heading.upcast_ref(),
        others.upcast_ref(),
        hint.upcast_ref(),
        buttons.upcast_ref(),
    ] {
        body.append(widget);
    }
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&body)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(600)
        .build();
    let window = gtk::Window::builder()
        .title(&words.title)
        .modal(true)
        .transient_for(&app.window)
        .destroy_with_parent(true)
        .default_width(520)
        .child(&scrolled)
        .build();
    apply.connect_clicked(|_| send(ProposalEvent::Apply));
    cancel.connect_clicked(|_| send(ProposalEvent::Close));
    window.connect_close_request(|_| {
        send(ProposalEvent::Close);
        glib::Propagation::Stop
    });
    let escape = gtk::EventControllerKey::new();
    escape.connect_key_pressed(|_, key, _, _| {
        if key == gdk::Key::Escape {
            send(ProposalEvent::Close);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(escape);
    let screen = Rc::new(Screen {
        window: window.clone(),
        status,
        parts,
        checks: RefCell::new(Vec::new()),
        unfit_heading,
        unfit,
        hint,
        others_heading,
        others,
        apply,
        cancel,
        shown: Cell::new(None),
        offered: RefCell::new(Vec::new()),
        filling: Rc::new(Cell::new(false)),
        waiting: RefCell::new(Some(Vec::new())),
    });
    // What was said as it opened, once the window is in front and Orca has read its title and
    // focus, which would cut it off.
    let weak = Rc::downgrade(&screen);
    let said = Cell::new(false);
    window.connect_is_active_notify(move |window| {
        if !window.is_active() || said.replace(true) {
            return;
        }
        let weak = weak.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
            let Some(screen) = weak.upgrade() else { return };
            let waiting = screen.waiting.take().unwrap_or_default();
            for text in waiting {
                say(&text);
            }
        });
    });
    screen
}
