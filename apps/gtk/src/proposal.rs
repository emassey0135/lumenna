//! Replan from Now and Plan the Day: a proposal the core makes, each part of it a check box,
//! made only as far as the person accepts it.
//!
//! Every word is the core's (`proposal_words`, the proposal's own sentences). The proposal is
//! said when the screen opens, as one sentence; making the parts checked can leave the day
//! needing another look, so the core proposes again around what was left unchecked, and the
//! screen shows that until nothing is left to propose.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_desktop::speech;
use lumenna_surface::actions::ActionKind;
use lumenna_surface::{DayPlan, ProposalWords, Replan, proposal_words};

use crate::core::sentence;
use crate::prompts;
use crate::window::{App, spawn};

/// What is proposed.
#[derive(Clone)]
enum Proposal {
    Replan(Replan),
    Day(DayPlan),
}

impl Proposal {
    fn announcement(&self) -> String {
        match self {
            Self::Replan(replan) => speech::announcement(&replan.announcement, &replan.notices),
            Self::Day(plan) => speech::announcement(&plan.announcement, &plan.notices),
        }
    }

    /// Each part, as its check box says it, and why it is proposed, if the core says.
    fn parts(&self) -> Vec<(String, String)> {
        match self {
            Self::Replan(replan) => replan.changes.iter().map(|c| (c.sentence.clone(), String::new())).collect(),
            Self::Day(plan) => plan.sittings.iter().map(|s| (s.sentence.clone(), s.reason.clone().unwrap_or_default())).collect(),
        }
    }

    /// What does not fit anywhere, as lines.
    fn unfit(&self) -> &[String] {
        match self {
            Self::Replan(replan) => &replan.unplaceable,
            Self::Day(plan) => &plan.unfit,
        }
    }
}

/// The screen.
struct Screen {
    words: ProposalWords,
    window: gtk::Window,
    status: gtk::Label,
    parts: gtk::Box,
    checks: RefCell<Vec<gtk::CheckButton>>,
    unfit_heading: gtk::Label,
    unfit: gtk::TextView,
    hint: gtk::Label,
    /// Replan's other options, each a button named by its sentence, under their heading.
    others_heading: gtk::Label,
    others: gtk::Box,
    apply: gtk::Button,
    cancel: gtk::Button,
    proposal: RefCell<Proposal>,
}

impl Screen {
    fn say(&self, text: &str) {
        let text = speech::sentence(text);
        self.status.set_label(&text);
        crate::window::announce(&self.status, &text);
    }

    /// Shows `proposal`: a check box for each part, all checked, what does not fit, and
    /// Replan's other options.
    fn show(self: &Rc<Self>, proposal: Proposal) {
        while let Some(child) = self.others.first_child() {
            self.others.remove(&child);
        }
        let alternatives = match &proposal {
            Proposal::Replan(replan) => replan.alternatives.clone(),
            Proposal::Day(_) => Vec::new(),
        };
        for alternative in &alternatives {
            let label = gtk::Label::builder().label(&alternative.sentence).wrap(true).xalign(0.0).build();
            let button = gtk::Button::builder().child(&label).build();
            button.update_property(&[gtk::accessible::Property::Label(&alternative.sentence)]);
            let (weak, pins) = (Rc::downgrade(self), alternative.pins.clone());
            button.connect_clicked(move |_| {
                let (Some(screen), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
                screen.take(&app, pins.clone());
            });
            self.others.append(&button);
        }
        self.others_heading.set_visible(!alternatives.is_empty() && !self.words.others.is_empty());
        while let Some(child) = self.parts.first_child() {
            self.parts.remove(&child);
        }
        let mut checks = Vec::new();
        for (text, reason) in proposal.parts() {
            // A label without a mnemonic: the core's sentence is shown as it is.
            let check = gtk::CheckButton::with_label(&text);
            check.set_active(true);
            if !reason.is_empty() {
                check.update_property(&[gtk::accessible::Property::Description(&reason)]);
                let line = gtk::Label::builder().label(&reason).wrap(true).xalign(0.0).margin_start(28).build();
                line.add_css_class("dim-label");
                self.parts.append(&check);
                self.parts.append(&line);
            } else {
                self.parts.append(&check);
            }
            checks.push(check);
        }
        let any = !checks.is_empty();
        *self.checks.borrow_mut() = checks;
        self.status.set_label(&if any { proposal.announcement() } else { self.words.nothing.clone() });
        let unfit = proposal.unfit();
        self.unfit.buffer().set_text(&unfit.join("\n"));
        self.unfit_heading.set_visible(!unfit.is_empty());
        self.unfit.set_visible(!unfit.is_empty());
        self.apply.set_visible(any);
        self.hint.set_visible(any);
        *self.proposal.borrow_mut() = proposal;
    }

    /// Focus on the first part, or on Cancel with nothing to accept.
    fn land(&self) {
        match self.checks.borrow().first() {
            Some(check) => check.grab_focus(),
            None => self.cancel.grab_focus(),
        };
    }

    /// Takes one of the other options: Replan again with its pins, shown in place.
    fn take(self: &Rc<Self>, app: &App, pins: lumenna_surface::ReplanPins) {
        match app.core.lumenna.replan(app.clock.style(), pins, crate::clock::pinned_now()) {
            Ok(replan) => {
                self.show(Proposal::Replan(replan));
                self.land();
                self.say(&self.status.label());
            }
            Err(error) => prompts::fail(&self.window, &sentence(&error)),
        }
    }

    /// Makes the parts checked, then shows what the core proposes next, or closes.
    fn apply(self: &Rc<Self>, app: &Rc<App>) {
        let checked: Vec<bool> = self.checks.borrow().iter().map(gtk::CheckButton::is_active).collect();
        let proposal = self.proposal.borrow().clone();
        let lumenna = &app.core.lumenna;
        let made = match &proposal {
            Proposal::Replan(replan) => {
                let accepted = replan.changes.iter().zip(&checked).filter(|(_, c)| **c).map(|(p, _)| p.clone()).collect();
                lumenna.apply_replan(replan.clone(), accepted, crate::clock::pinned_now()).map(|done| {
                    (speech::announcement(&done.announcement, &done.notices), Proposal::Replan(done.next))
                })
            }
            Proposal::Day(plan) => {
                let accepted = plan.sittings.iter().zip(&checked).filter(|(_, c)| **c).map(|(p, _)| p.clone()).collect();
                lumenna.apply_day_plan(plan.clone(), accepted, crate::clock::pinned_now()).map(|done| {
                    (speech::announcement(&done.announcement, &done.notices), Proposal::Day(done.next))
                })
            }
        };
        match made {
            Ok((said, next)) => {
                app.store_changed();
                if next.parts().is_empty() {
                    self.window.close();
                    app.say(&speech::sentence(&said));
                } else {
                    self.show(next);
                    self.land();
                    self.say(&said);
                }
            }
            // The day moved since: say so here, and propose again from the day as it is.
            Err(error) => {
                let screen = Rc::clone(self);
                let app = Rc::clone(app);
                spawn(async move {
                    prompts::tell(&screen.window, &sentence(&error)).await;
                    match ask(&app, &proposal) {
                        Ok(fresh) => {
                            screen.show(fresh);
                            screen.land();
                            screen.say(&screen.status.label());
                        }
                        Err(error) => {
                            screen.window.close();
                            app.fail(&sentence(&error));
                        }
                    }
                });
            }
        }
    }
}

/// Asks the core again for the same kind of proposal, keeping what was decided.
fn ask(app: &App, like: &Proposal) -> lumenna_surface::Result<Proposal> {
    let style = app.clock.style();
    match like {
        Proposal::Replan(replan) => app.core.lumenna.replan(style, replan.pins.clone(), crate::clock::pinned_now()).map(Proposal::Replan),
        Proposal::Day(plan) => app
            .core
            .lumenna
            .propose_day_plan(Some(plan.date.clone()), style, plan.pins.clone(), crate::clock::pinned_now())
            .map(Proposal::Day),
    }
}

/// Replan from Now: the rest of today. An overrun warning's Replan button runs it too
/// (`app.reminder-replan`), bringing the window up first.
pub fn replan(app: &Rc<App>) {
    match app.core.lumenna.replan(app.clock.style(), Default::default(), crate::clock::pinned_now()) {
        Ok(replan) => open(app, ActionKind::Replan, Proposal::Replan(replan)),
        Err(error) => app.fail(&sentence(&error)),
    }
}

/// Plan the Day: `date` (ISO, or a date phrase).
pub fn plan_day(app: &Rc<App>, date: &str) {
    match app.core.lumenna.propose_day_plan(Some(date.to_owned()), app.clock.style(), Default::default(), crate::clock::pinned_now()) {
        Ok(plan) => open(app, ActionKind::PlanDay, Proposal::Day(plan)),
        Err(error) => app.fail(&sentence(&error)),
    }
}

fn open(app: &Rc<App>, kind: ActionKind, proposal: Proposal) {
    let words = proposal_words(kind);
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
    body.append(&status);
    body.append(&parts);
    body.append(&unfit_heading);
    body.append(&unfit);
    body.append(&others_heading);
    body.append(&others);
    body.append(&hint);
    body.append(&buttons);
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
    let screen = Rc::new(Screen {
        words,
        window: window.clone(),
        status,
        parts,
        checks: RefCell::new(Vec::new()),
        unfit_heading,
        unfit,
        hint,
        others_heading,
        others,
        apply: apply.clone(),
        cancel: cancel.clone(),
        proposal: RefCell::new(proposal.clone()),
    });
    screen.show(proposal);
    {
        let weak = Rc::downgrade(&screen);
        apply.connect_clicked(move |_| {
            if let (Some(screen), Some(app)) = (weak.upgrade(), crate::window::app()) {
                screen.apply(&app);
            }
        });
    }
    {
        let window = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        });
    }
    let escape = gtk::EventControllerKey::new();
    {
        let window = window.downgrade();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(window) = window.upgrade()
            {
                window.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(escape);
    // The proposal, said once the window is in front and Orca has read its title and focus,
    // which would cut it off.
    {
        let weak = Rc::downgrade(&screen);
        let said = Cell::new(false);
        window.connect_is_active_notify(move |window| {
            if !window.is_active() || said.replace(true) {
                return;
            }
            let weak = weak.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
                if let Some(screen) = weak.upgrade() {
                    screen.say(&screen.status.label());
                }
            });
        });
    }
    window.present();
    screen.land();
    // The screen lives as long as its window.
    window.connect_destroy(move |_| {
        let _ = &screen;
    });
}
