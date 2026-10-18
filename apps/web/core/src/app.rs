//! The app itself, beside the store: events in, the view model and what to say out.
//!
//! The page sends what the person did; the session runs it, answering whatever it asks of the
//! store here in the worker, and hands back what to draw and what to say. Nothing about which
//! row is in hand, what is folded or which question is open is the page's.

use std::cell::RefCell;

use lumenna_app::{Event, ViewModel};
use lumenna_bridge::{Session, Shell};
use serde::{Deserialize, Serialize};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{Core, Out, js};

/// What an event left for the page: the view model, and what to say once focus is where it
/// puts it.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
pub struct Turn {
    /// What to draw.
    pub view: ViewModel,
    /// What to say, in order.
    pub said: Vec<String>,
}

/// The app over the store this worker holds.
#[wasm_bindgen]
pub struct App {
    session: RefCell<Session>,
}

fn turn(session: &Session, shell: &[Shell]) -> Turn {
    let said = shell
        .iter()
        .filter_map(|s| match s {
            Shell::Announce(text) => Some(text.clone()),
            Shell::Render => None,
        })
        .collect();
    Turn { view: session.view(), said }
}

#[wasm_bindgen]
impl Core {
    /// The app, started over this store: it already has the store and shows the place it
    /// starts in.
    pub fn app(&self) -> App {
        let (session, _) = Session::new(std::sync::Arc::clone(&self.lumenna));
        App { session: RefCell::new(session) }
    }
}

#[wasm_bindgen]
impl App {
    /// Runs what the person did.
    pub fn send(&self, event: Ts<Event>) -> Out<Turn> {
        let event = event.to_rust()?;
        let mut session = self.session.borrow_mut();
        let shell = session.send(event);
        js(&turn(&session, &shell))
    }

    /// Takes in what changed elsewhere since: another tab, `lum`, a sync, a write made through
    /// the store's own exports, or a new minute. Nothing when nothing did.
    pub fn tick(&self) -> Result<Option<Ts<Turn>>, JsError> {
        let mut session = self.session.borrow_mut();
        let shell = session.tick();
        if shell.is_empty() {
            return Ok(None);
        }
        Ok(Some(js(&turn(&session, &shell))?))
    }

    /// What to draw now.
    pub fn view(&self) -> Out<ViewModel> {
        js(&self.session.borrow().view())
    }
}
