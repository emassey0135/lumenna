//! The row in hand in each list beside the main one — the sidebar, a reminder list, the
//! devices — kept as the main list's is: the same row after a change if it is still shown, else
//! whatever now holds its place, and a landing count a shell moves focus by.

use serde::{Deserialize, Serialize};

use crate::Screen;

/// A list beside the main one.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// The places.
    Sidebar,
    /// A task's or block's reminders, while their list is open.
    Reminders,
    /// The paired devices, while the Devices page is shown.
    Devices,
    /// The reminders due, by `Due::id`: after a button, what took the reminder's place is in hand.
    Due,
}

/// The row an action handed back in short is for (`Event::ActOn`).
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActingOn {
    /// A row of the place shown, by key: a list's row, a row of the day.
    Row(String),
    /// A row of a list beside the main one, by its key there.
    Held(Side, String),
    /// The day shown, its own actions: Plan the Day, Replan from Now, ….
    Day,
    /// The task the details show.
    Details,
    /// The reminder list open, its own actions: Add Reminder.
    Reminders,
    /// Settings open, the store's own actions: Erase Everything Deleted.
    Settings,
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

impl Screen {
    /// The actions of a side list's row in hand, as the view offers them: what a key pressed
    /// there runs (`Event::Run`).
    pub(crate) fn side_actions(&self, side: Side) -> Option<Vec<lumenna_records::Action>> {
        let store = self.store.as_ref()?;
        match side {
            Side::Sidebar => {
                let key = self.sidebar_held.key.as_ref()?;
                let entries = crate::view::folded_sidebar(lumenna_records::places::sidebar(&store.snapshot, &store.now), &self.sidebar_folded);
                entries.into_iter().find(|entry| &entry.key == key).map(|entry| entry.actions)
            }
            Side::Reminders => {
                let id = self.reminders_held.key.as_ref()?;
                let list = lumenna_records::reminders::reminders_of(&store.snapshot, self.reminders.as_ref()?).ok()?;
                list.reminders.into_iter().find(|reminder| &reminder.id == id).map(|reminder| reminder.actions)
            }
            // A reminder's buttons are its `DueEvent`s, no row action.
            Side::Due => None,
            Side::Devices => {
                let id = self.devices_held.key.as_ref()?;
                let list = self.devices.as_ref()?.list.as_ref()?;
                list.devices.iter().find(|device| &device.node_id == id).map(|device| device.actions.clone())
            }
        }
    }

    /// Whether the devices' row in hand is this device's own, which cannot unpair itself.
    pub(crate) fn holds_this_device(&self) -> bool {
        let Some(id) = &self.devices_held.key else { return false };
        self.devices.as_ref().and_then(|d| d.list.as_ref()).is_some_and(|list| list.devices.iter().any(|d| &d.node_id == id && d.this_device))
    }

    pub(crate) fn held_mut(&mut self, side: Side) -> &mut Held {
        match side {
            Side::Sidebar => &mut self.sidebar_held,
            Side::Reminders => &mut self.reminders_held,
            Side::Devices => &mut self.devices_held,
            Side::Due => &mut self.due_held,
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
            Side::Due => self.due.iter().map(|due| due.id.clone()).collect(),
            Side::Devices => self
                .devices
                .as_ref()
                .and_then(|devices| devices.list.as_ref())
                .map(|list| list.devices.iter().map(|d| d.node_id.clone()).collect())
                .unwrap_or_default(),
        }
    }

    /// Keeps every side list's row in hand where it belongs after a change; the list acted on
    /// last moves focus there. A write made from a side list holds what it made there, else
    /// the row in hand, else the list's first: `before` is the list's rows before the write,
    /// for telling what it made.
    pub(crate) fn settle_sides(&mut self, wrote: bool, before: Option<&[String]>) {
        for side in [Side::Sidebar, Side::Reminders, Side::Devices, Side::Due] {
            let acted_here = self.acting == Some(side);
            if self.held_mut(side).key.is_none() && !(acted_here && wrote) {
                continue;
            }
            let keys = self.side_keys(side);
            let made = before.filter(|_| acted_here && wrote).and_then(|before| keys.iter().find(|key| !before.contains(key)).cloned());
            let held = self.held_mut(side);
            let was = held.key.clone();
            match made {
                Some(key) => held.hold(&key, &keys),
                None if held.key.is_none() => held.key = keys.first().cloned(),
                None => held.settle(&keys),
            }
            if acted_here && (wrote || held.key != was) {
                held.landing += 1;
            }
        }
    }
}
