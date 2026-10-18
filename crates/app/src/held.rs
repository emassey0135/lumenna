//! The row in hand in each list beside the main one — the sidebar, a reminder list, the
//! devices — kept as the main list's is: the same row after a change if it is still shown, else
//! whatever now holds its place, and a landing count a shell moves focus by.

use serde::{Deserialize, Serialize};

use crate::Model;

/// A list beside the main one.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum Side {
    /// The places.
    Sidebar,
    /// A task's or block's reminders, while their list is open.
    Reminders,
    /// The paired devices, while the Devices page is shown.
    Devices,
}

/// One side list's row in hand, as the model keeps it.
#[derive(Debug, Default, Clone)]
pub struct Held {
    /// The row in hand, by key: a sidebar entry's `key`, a reminder's `id`, a device's `node_id`.
    pub key: Option<String>,
    /// Where it was among the rows shown, for when it goes.
    pub position: usize,
    /// Counted: each time the app moves the row itself.
    pub landing: u32,
}

/// One side list's row in hand, as the view shows it: a shell puts focus on `focus` when
/// `landing` goes up, and leaves it alone otherwise.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct HeldView {
    /// The row in hand, by key.
    pub focus: Option<String>,
    /// Counted: when this moves, the shell puts focus on `focus`.
    pub landing: u32,
}

impl Held {
    /// Puts `key` in hand, where it is among `keys`.
    pub(crate) fn hold(&mut self, key: &str, keys: &[String]) {
        self.key = Some(key.to_owned());
        if let Some(at) = keys.iter().position(|k| k == key) {
            self.position = at;
        }
    }

    /// Keeps the row in hand after `keys` changed: the same row, else the one now in its place.
    pub(crate) fn settle(&mut self, keys: &[String]) {
        if self.key.is_none() {
            return;
        }
        if let Some(at) = self.key.as_ref().and_then(|key| keys.iter().position(|k| k == key)) {
            self.position = at;
            return;
        }
        self.key = keys.get(self.position.min(keys.len().saturating_sub(1))).cloned();
    }

    pub(crate) fn view(&self) -> HeldView {
        HeldView { focus: self.key.clone(), landing: self.landing }
    }
}

impl Model {
    pub(crate) fn held_mut(&mut self, side: Side) -> &mut Held {
        match side {
            Side::Sidebar => &mut self.sidebar_held,
            Side::Reminders => &mut self.reminders_held,
            Side::Devices => &mut self.devices_held,
        }
    }

    /// The rows a side list shows, by key, folded rows left out.
    pub(crate) fn side_keys(&self, side: Side) -> Vec<String> {
        let Some(store) = &self.store else { return Vec::new() };
        match side {
            Side::Sidebar => {
                let mut keys = Vec::new();
                let mut under: Option<u32> = None;
                for entry in lumenna_records::places::sidebar(&store.snapshot, &store.now) {
                    if under.is_some_and(|depth| entry.depth > depth) {
                        continue;
                    }
                    under = None;
                    let key = entry.key();
                    if self.sidebar_folded.contains(&key) {
                        under = Some(entry.depth);
                    }
                    keys.push(key);
                }
                keys
            }
            Side::Reminders => self
                .reminders
                .as_ref()
                .and_then(|target| lumenna_records::reminders::reminders_of(&store.snapshot, target).ok())
                .map(|list| list.reminders.into_iter().map(|r| r.id).collect())
                .unwrap_or_default(),
            Side::Devices => self
                .devices
                .as_ref()
                .and_then(|devices| devices.list.as_ref())
                .map(|list| list.devices.iter().map(|d| d.node_id.clone()).collect())
                .unwrap_or_default(),
        }
    }

    /// Keeps every side list's row in hand where it belongs after a change; the list acted on
    /// last moves focus there.
    pub(crate) fn settle_sides(&mut self, wrote: bool) {
        for side in [Side::Sidebar, Side::Reminders, Side::Devices] {
            if self.held_mut(side).key.is_none() {
                continue;
            }
            let keys = self.side_keys(side);
            let acted_here = self.acting == Some(side);
            let held = self.held_mut(side);
            let before = held.key.clone();
            held.settle(&keys);
            if acted_here && (wrote || held.key != before) {
                held.landing += 1;
            }
        }
    }
}
