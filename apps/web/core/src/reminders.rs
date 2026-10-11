//! Reminders, for the browser: what came due since the tab last looked, a task's or block's
//! own list, and a notification's two buttons. Each export is one surface call.
//!
//! A browser has no lock to take (`take_reminders` is native only): the one tab that owns the
//! store is the one that gives this browser's reminders, keeping its own "since" in memory.

use lumenna_surface::{Change, DueReminders, ReminderList};
use wasm_bindgen::prelude::*;

use crate::{Core, Out, out};

#[wasm_bindgen]
impl Core {
    /// The reminders that came due after `since` up to `now` (both RFC 3339): what the tab
    /// asks each time it looks. Older ones come as `missed`, for one summary.
    #[wasm_bindgen(js_name = remindersDue)]
    pub fn reminders_due(&self, since: String, now: Option<String>) -> Out<DueReminders> {
        out(self.lumenna.reminders_due(since, now))
    }

    /// A task's or block's reminders: its own, or the defaults it takes.
    #[wasm_bindgen(js_name = remindersOf)]
    pub fn reminders_of(&self, target: String) -> Out<ReminderList> {
        out(self.lumenna.reminders_of(target))
    }

    /// A notification's Dismiss: this firing, on every device.
    #[wasm_bindgen(js_name = dismissReminder)]
    pub fn dismiss_reminder(&self, key: String, occurrence: String) -> Out<Change> {
        out(self.lumenna.dismiss_reminder(key, occurrence))
    }

    /// A notification's Snooze, for the core's ten minutes.
    #[wasm_bindgen(js_name = snoozeReminder)]
    pub fn snooze_reminder(&self, key: String, occurrence: String) -> Out<Change> {
        out(self.lumenna.snooze_reminder(key, occurrence, None))
    }
}
