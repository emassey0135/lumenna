//! What the app asks of the host: the store as it changes, and every write.
//!
//! These never reach a shell. The bridge hands them to the host (`lumenna-host`), as Crux
//! middleware on a native platform and from the web worker's own loop, and resolves each with
//! the host's answer; the app sees only the answer, as an event. So the app holds no store and
//! calls nothing: what it knows of the store is the snapshot the last answer carried.

use std::sync::Arc;

use crux_core::capability::Operation;
use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use lumenna_records::{Action, Answer, Change, LumennaError};

/// The store as it stands, and the moment it was read at.
#[derive(Debug, Clone)]
pub struct Arrival {
    /// The repaired snapshot every listing reads.
    pub snapshot: Arc<Snapshot>,
    /// Now, as the device's clock and zone have it.
    pub now: Zoned,
}

impl PartialEq for Arrival {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.snapshot, &other.snapshot) && self.now == other.now
    }
}

/// Every change to the store from here on, whoever made it — this app, `lum`, a sync — and a
/// tick each minute so that "now" moves: a stream, answered first with the store as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch;

impl Operation for Watch {
    type Output = Arrival;
}

/// A write, or a question only the host can answer yet.
#[derive(Debug, Clone, PartialEq)]
pub enum HostRequest {
    /// Run a row's action with the answer to its question (`Lumenna::act`).
    Act {
        /// The action, as the row offered it.
        action: Action,
        /// The answer to its question.
        answer: Answer,
    },
    /// Add a task from a quick-add line.
    AddTask {
        /// The line as typed, a place's own start included.
        text: String,
    },
    /// Save a task's details: only what changed.
    EditTask {
        /// The task.
        id: String,
        /// What changed.
        edit: lumenna_records::types::TaskEdit,
    },
    /// Add a block from its form.
    AddBlock(lumenna_records::types::NewBlock),
    /// Save what changed in a block's form, to every occurrence or one day's.
    EditBlock {
        /// The series.
        series: String,
        /// What changed.
        edit: lumenna_records::types::BlockEdit,
        /// To which occurrences.
        scope: lumenna_records::types::BlockScope,
    },
    /// Make the changes accepted, by position, out of a proposal.
    Apply {
        /// The proposal, whole, as it came.
        proposal: crate::proposal::Proposal,
        /// The changes accepted, by position.
        accepted: Vec<usize>,
        /// The moment it was proposed against, when the shell pinned it.
        now: Option<String>,
    },
    /// A saved filter, from its two questions.
    AddFilter {
        /// Its name.
        name: String,
        /// Its query.
        query: String,
    },
    /// Every setting, each describing itself.
    Settings,
    /// Apply a setting.
    SetSetting {
        /// The setting.
        key: String,
        /// Its new value.
        value: String,
    },
    /// A backup taken now, as a file to save.
    Backup,
    /// What is there now, in a format, as a file to save.
    Export(lumenna_records::types::ExportFormat),
    /// Read a JSON export or a backup.
    Import {
        /// The file's name.
        name: String,
        /// Its contents.
        bytes: Vec<u8>,
    },
    /// The paired devices, this one first.
    Devices,
    /// What came due after `since` (RFC 3339) until now.
    RemindersDue {
        /// When the shell last looked.
        since: String,
        /// The moment looked at, for tests; now when absent.
        now: Option<String>,
    },
    /// Dismiss these firings (key, occurrence), on every device.
    Dismiss(Vec<(String, String)>),
    /// Snooze these firings for the core's ten minutes.
    Snooze(Vec<(String, String)>),
    /// Carry the rest of a block's short work forward, and dismiss the firings that offered it.
    CarryForward {
        /// `<series>@<date>`.
        block: String,
        /// The firings, dismissed once it is carried.
        firings: Vec<(String, String)>,
    },
    /// Undo the last change made on this device.
    Undo,
    /// Redo the last change undone.
    Redo,
}

/// What the host answered.
#[derive(Debug, Clone)]
pub enum HostAnswer {
    /// A write: what it did, and the store just after it, so the app moves focus to what it
    /// made against the list that holds it.
    Wrote(Box<Result<Change, LumennaError>>, Option<Arrival>),
    /// The paired devices.
    Devices(Result<lumenna_records::types::DeviceList, LumennaError>),
    /// What came due.
    Found(Result<lumenna_records::reminders::DueReminders, LumennaError>),
    /// Every setting.
    Listed(Result<lumenna_records::types::SettingList, LumennaError>),
    /// A file to save, and what to say about it.
    Saved(Box<Result<(crate::SaveFile, String), LumennaError>>),
    /// What an import did, as said, and the store just after it.
    Imported(Box<Result<String, LumennaError>>, Option<Arrival>),
    /// What applying a proposal made, and the store just after it.
    Applied(Box<Result<crate::proposal::Applied, LumennaError>>, Option<Arrival>),
}

impl Operation for HostRequest {
    type Output = HostAnswer;
}

/// What a write was sent for, carried back with its answer (`Event::Wrote`): only what is still
/// waiting on that write takes it, so the answer to Mark Done never closes a question opened
/// since, nor takes the quick-add line typed meanwhile.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Writing {
    /// Its number among the screen's writes: what the question, the line, the form or the setting
    /// waiting on it holds.
    pub number: u64,
    /// The question it answered as it was then, or the action it ran without asking: where a
    /// place it renamed, made or removed is now.
    pub asked: Option<crate::asking::Open>,
}


impl PartialEq for HostAnswer {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Wrote(a, x), Self::Wrote(b, y)) => a == b && x == y,
            (Self::Applied(a, x), Self::Applied(b, y)) => a == b && x == y,
            (Self::Listed(a), Self::Listed(b)) => a == b,
            (Self::Saved(a), Self::Saved(b)) => a == b,
            (Self::Imported(a, x), Self::Imported(b, y)) => a == b && x == y,
            (Self::Devices(a), Self::Devices(b)) => a == b,
            (Self::Found(a), Self::Found(b)) => a == b,
            _ => false,
        }
    }
}
