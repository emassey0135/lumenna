//! The screen's own actions (`ViewModel::screen_actions`), drawn as a toolbar over the middle
//! pane: New Task, the filter, the day's moves, Undo and Redo, whatever the place shown offers
//! that is no row's, in the core's order and names.
//!
//! A box with the toolbar role, named by the core (`ShellWords::screen_actions`), so Orca says
//! "Commands toolbar" going in. **It is one Tab stop**, as the ARIA toolbar pattern has it: only
//! one button is focusable, the last one used, and Left, Right, Home and End move among the
//! rest. Each button is sent back as `Key(id)`; one that is the shell's own (`shell`: the filter
//! field, Key Help) runs as this window's command. An action that does nothing now is drawn
//! insensitive, and one that asks something first ends in "…". The menu bar still has every
//! command; this is the place's own, a click away.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::ScreenAction;
use lumenna_ffi::ShellEvent;

use crate::window::App;

pub struct Toolbar {
    pub widget: gtk::Box,
    /// The actions as last drawn, each beside its button.
    buttons: RefCell<Vec<(ScreenAction, gtk::Button)>>,
    /// The id of the button that is the toolbar's Tab stop.
    current: RefCell<String>,
}

impl Toolbar {
    pub fn new() -> Rc<Self> {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .margin_top(8)
            .margin_start(8)
            .margin_end(8)
            .accessible_role(gtk::AccessibleRole::Toolbar)
            .css_classes(["toolbar"])
            .build();
        widget.update_property(&[gtk::accessible::Property::Label(&crate::words().screen_actions)]);
        let toolbar = Rc::new(Self { widget, buttons: RefCell::new(Vec::new()), current: RefCell::new(String::new()) });
        let arrows = gtk::EventControllerKey::new();
        // Before a button or the window takes an arrow to move focus out of the toolbar.
        arrows.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&toolbar);
        arrows.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(toolbar) = weak.upgrade() else { return glib::Propagation::Proceed };
            if !modifiers.is_empty() {
                return glib::Propagation::Proceed;
            }
            let moved = match key {
                gdk::Key::Right | gdk::Key::KP_Right => toolbar.step(1),
                gdk::Key::Left | gdk::Key::KP_Left => toolbar.step(-1),
                gdk::Key::Home | gdk::Key::KP_Home => toolbar.step(isize::MIN),
                gdk::Key::End | gdk::Key::KP_End => toolbar.step(isize::MAX),
                _ => return glib::Propagation::Proceed,
            };
            if moved {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        toolbar.widget.add_controller(arrows);
        toolbar
    }

    /// Draws the screen's actions as the app has them now: the buttons made again when the
    /// actions change, else only their names and whether each does anything now.
    pub fn draw(self: &Rc<Self>, app: &App) {
        let actions = app.model.view().screen_actions.clone();
        let same = {
            let buttons = self.buttons.borrow();
            buttons.len() == actions.len() && buttons.iter().zip(&actions).all(|((drawn, _), action)| drawn.id == action.id)
        };
        if same {
            if self.buttons.borrow().iter().zip(&actions).all(|((drawn, _), action)| drawn == action) {
                return;
            }
            self.settle(&actions);
            let current = self.button(&self.current_id());
            let drawn: Vec<gtk::Button> = self.buttons.borrow().iter().map(|(_, button)| button.clone()).collect();
            *self.buttons.borrow_mut() = actions.iter().cloned().zip(drawn.iter().cloned()).collect();
            let then: Vec<&gtk::Widget> = current.iter().map(|current| current.upcast_ref()).collect();
            for (button, action) in drawn.iter().zip(&actions) {
                button.set_label(&label(action));
                crate::prompts::set_sensitive(button, action.enabled, &then);
            }
            return;
        }
        let had_focus = holds_focus(&self.widget);
        for (_, button) in self.buttons.take() {
            self.widget.remove(&button);
        }
        let buttons: Vec<_> = actions
            .iter()
            .map(|action| {
                let button = gtk::Button::builder().label(label(action)).sensitive(action.enabled).build();
                let (id, shell) = (action.id.clone(), action.shell);
                button.connect_clicked(move |_| {
                    let Some(app) = crate::window::app() else { return };
                    if shell {
                        app.command(&id);
                    } else {
                        app.model.send(ShellEvent::Key(id.clone()));
                    }
                });
                // A button clicked, or reached any other way, is the Tab stop from then on.
                let focus = gtk::EventControllerFocus::new();
                let (weak, id) = (Rc::downgrade(self), action.id.clone());
                focus.connect_enter(move |_| {
                    if let Some(toolbar) = weak.upgrade() {
                        toolbar.current.replace(id.clone());
                        toolbar.rove();
                    }
                });
                button.add_controller(focus);
                self.widget.append(&button);
                (action.clone(), button)
            })
            .collect();
        *self.buttons.borrow_mut() = buttons;
        self.settle(&actions);
        self.widget.set_visible(!actions.is_empty());
        let current = self.button(&self.current_id());
        if had_focus && let Some(button) = current {
            button.grab_focus();
        }
    }

    /// Keeps the Tab stop on the button last used while it does anything, else on the first that
    /// does.
    fn settle(&self, actions: &[ScreenAction]) {
        let current = self.current_id();
        let keep = actions.iter().any(|action| action.id == current && action.enabled);
        if !keep {
            let first = actions.iter().find(|action| action.enabled).map(|action| action.id.clone()).unwrap_or_default();
            self.current.replace(first);
        }
        self.rove();
    }

    /// Only the current button is a Tab stop.
    fn rove(&self) {
        let current = self.current_id();
        let buttons: Vec<(String, gtk::Button)> =
            self.buttons.borrow().iter().map(|(action, button)| (action.id.clone(), button.clone())).collect();
        for (id, button) in buttons {
            button.set_focusable(id == current);
        }
    }

    /// The id of the button that is the Tab stop. Cloned out, since a button taking focus sets it.
    fn current_id(&self) -> String {
        self.current.borrow().clone()
    }

    /// The button drawn for `id`.
    fn button(&self, id: &str) -> Option<gtk::Button> {
        self.buttons.borrow().iter().find(|(action, _)| action.id == id).map(|(_, button)| button.clone())
    }

    /// Moves focus `by` buttons among those that do anything now, to the first or last for
    /// `isize::MIN` or `MAX`. False where there is nowhere to go.
    fn step(&self, by: isize) -> bool {
        let usable: Vec<(String, gtk::Button)> = self
            .buttons
            .borrow()
            .iter()
            .filter(|(_, button)| button.is_sensitive())
            .map(|(action, button)| (action.id.clone(), button.clone()))
            .collect();
        if usable.is_empty() {
            return false;
        }
        let current = self.current_id();
        let here = usable.iter().position(|(id, _)| *id == current).unwrap_or(0);
        let last = usable.len() - 1;
        let next = match by {
            isize::MIN => 0,
            isize::MAX => last,
            by => here.saturating_add_signed(by).min(last),
        };
        let (id, button) = &usable[next];
        self.current.replace(id.clone());
        self.rove();
        button.grab_focus();
        true
    }

    /// Whether focus is on one of its buttons.
    pub fn has_focus(&self) -> bool {
        holds_focus(&self.widget)
    }
}

/// A button's words: the core's title, and "…" after one that asks something first.
fn label(action: &ScreenAction) -> String {
    if action.asks { format!("{}…", action.title) } else { action.title.clone() }
}

/// Whether focus is on `widget` or inside it.
fn holds_focus(widget: &impl IsA<gtk::Widget>) -> bool {
    let widget = widget.upcast_ref::<gtk::Widget>();
    widget.root().and_then(|root| root.focus()).is_some_and(|focus| focus == *widget || focus.is_ancestor(widget))
}
