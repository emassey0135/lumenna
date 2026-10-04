//! What one command produced, as `--json` and `lum rpc` carry it (§12, §15).
//!
//! The operations and the records they return are `lumenna_surface`'s — the same ones the
//! phone apps get through UniFFI. What is here is the envelope a terminal and a pipe need on
//! top: the contract version, a `result` tag a reader can dispatch on, and whether text mode
//! should stay quiet. Every payload carries its own `announcement` and `notices`, so a
//! [`Response`] has nothing to compose.
//!
//! The sync payloads are here rather than in the surface because syncing is the CLI's and the
//! daemon's for now: it drives an Iroh endpoint, not an operation on the store.
//!
//! `--json` is a compatibility contract (§15). Reshaping anything here, or in the surface's
//! records, is a breaking change, and [`VERSION`] says which shape a reader is looking at.

use lumenna_core::edit::Edit;
use lumenna_surface::{
    Announced, BackupDone, Change, Completions, Exported, Filters, ImportDone, Imported, Plan,
    Preview, RestoreDone, Rows, SettingList, TaskShown, Timer, announced,
};
use serde::{Deserialize, Serialize};

/// The contract version, carried on every response.
pub const VERSION: u32 = lumenna_surface::CONTRACT;

/// What one command produced.
#[derive(Debug, Serialize)]
pub struct Response {
    /// The contract version.
    pub version: u32,
    /// The payload, with its announcement and notices.
    #[serde(flatten)]
    pub outcome: Outcome,
    /// Whether text mode should print nothing — `lum task add --quiet`. A structured reader
    /// gets the same response either way: suppressing output is a terminal convenience, not
    /// a smaller result.
    #[serde(skip)]
    pub silent: bool,
}

impl Response {
    /// A response carrying this payload.
    pub fn new(outcome: impl Into<Outcome>) -> Self {
        Self { version: VERSION, outcome: outcome.into(), silent: false }
    }

    /// A mutation that changed something, announced as core described it.
    pub fn changed(edit: &Edit) -> Self {
        Self::new(Change::of(edit))
    }

    /// A mutation that turned out to have nothing to do.
    pub fn unchanged(announcement: impl Into<String>) -> Self {
        Self::new(Change::unchanged(announcement))
    }

    /// A change made outside the documents — a service installed, say.
    pub fn touched(announcement: impl Into<String>) -> Self {
        Self::new(Change::local(announcement))
    }

    /// Suppresses text output without changing the response.
    #[must_use]
    pub const fn quietly(mut self) -> Self {
        self.silent = true;
        self
    }

    /// Adds something worth saying that is not the answer.
    #[must_use]
    pub fn note(mut self, notice: impl Into<String>) -> Self {
        self.outcome.notices_mut().push(notice.into());
        self
    }

    /// What happened, in a sentence.
    #[must_use]
    pub fn announcement(&self) -> &str {
        self.outcome.announcement()
    }

    /// Things worth saying that are not the answer.
    #[must_use]
    pub fn notices(&self) -> &[String] {
        self.outcome.notices()
    }
}

/// The payload of a [`Response`], tagged so a reader can dispatch on it.
#[derive(Debug, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Outcome {
    /// A mutation.
    Change(Change),
    /// A listing of rows, of one kind.
    Rows(Rows),
    /// Everything about one task.
    Task(TaskShown),
    /// A day's blocks and what is assigned to them.
    Plan(Plan),
    /// Saved filters.
    Filters(Filters),
    /// Settings.
    Settings(SettingList),
    /// A timer stopped, or minutes logged.
    Timer(Timer),
    /// What could be typed next (§6.3). Reachable over `lum rpc` only: completion is a
    /// keystroke-rate question, and a process per keystroke is not an answer.
    Completions(Completions),
    /// What a quick-add line would produce, without producing it (§6.1).
    Preview(Preview),
    /// What this server is, for a client checking it can talk to it.
    Server(ServerInfo),
    /// A backup was written.
    Backup(BackupDone),
    /// A backup was merged in.
    Restore(RestoreDone),
    /// The current state was exported.
    Export(Exported),
    /// An export was read in.
    Import(ImportDone),
    /// A pairing finished (§7).
    Paired(PairedWith),
    /// A sync round with every paired device.
    Synced(SyncReport),
    /// How sync is going (§9).
    SyncStatus(SyncStatus),
    /// The paired devices.
    Devices(DeviceList),
}

macro_rules! outcomes {
    ($($variant:ident($type:ty)),* $(,)?) => {
        impl Outcome {
            /// What happened, in a sentence.
            #[must_use]
            pub fn announcement(&self) -> &str {
                match self { $(Self::$variant(x) => x.announcement(),)* }
            }
            /// Things worth saying that are not the answer.
            #[must_use]
            pub fn notices(&self) -> &[String] {
                match self { $(Self::$variant(x) => x.notices(),)* }
            }
            fn notices_mut(&mut self) -> &mut Vec<String> {
                match self { $(Self::$variant(x) => x.notices_mut(),)* }
            }
        }
        $(
            impl From<$type> for Outcome {
                fn from(payload: $type) -> Self {
                    Self::$variant(payload)
                }
            }
        )*
    };
}

outcomes!(
    Change(Change),
    Rows(Rows),
    Task(TaskShown),
    Plan(Plan),
    Filters(Filters),
    Settings(SettingList),
    Timer(Timer),
    Completions(Completions),
    Preview(Preview),
    Server(ServerInfo),
    Backup(BackupDone),
    Restore(RestoreDone),
    Export(Exported),
    Import(ImportDone),
    Paired(PairedWith),
    Synced(SyncReport),
    SyncStatus(SyncStatus),
    Devices(DeviceList),
);

impl From<Imported> for Outcome {
    fn from(imported: Imported) -> Self {
        match imported {
            Imported::Export { done } => Self::Import(done),
            Imported::Backup { done } => Self::Restore(done),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Reachable over RPC only
// ---------------------------------------------------------------------------------------

/// What this server is.
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    /// Always "Lumenna".
    pub announcement: String,
    /// Nothing, as a rule.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Always `lumenna`.
    pub name: &'static str,
    /// The binary's version.
    pub version: &'static str,
    /// The version of the shapes on the wire — §15's compatibility contract. A client that
    /// does not know this number should refuse to guess.
    pub contract: u32,
    /// Every method this server answers, so a client can find out rather than assume.
    pub methods: Vec<&'static str>,
}

// ---------------------------------------------------------------------------------------
// Sync (§7, §9)
// ---------------------------------------------------------------------------------------

/// The device a pairing joined.
#[derive(Debug, Serialize)]
pub struct PairedWith {
    /// Who, and how much came across.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// What it is called.
    pub name: String,
    /// What it runs.
    pub platform: String,
    /// Its device key.
    pub node_id: String,
}

/// How one device went in a sync round.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PeerSync {
    /// What it is called.
    pub name: String,
    /// Its device key.
    pub node_id: String,
    /// Whether the sync completed.
    pub synced: bool,
    /// The documents that changed on this side.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    /// Why it did not complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A sync round.
#[derive(Debug, Serialize)]
pub struct SyncReport {
    /// How many were reached.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Each paired device, and how it went.
    pub peers: Vec<PeerSync>,
}

/// One paired device, with how syncing with it last went.
#[derive(Debug, Serialize)]
pub struct DeviceView {
    /// What it is called.
    pub name: String,
    /// What it runs.
    pub platform: String,
    /// Its device key.
    pub node_id: String,
    /// Whether it is the device answering.
    pub this_device: bool,
    /// When it was paired.
    pub paired_at: String,
    /// When this device last tried to sync with it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempt: Option<String>,
    /// When that last worked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<String>,
    /// What went wrong, if the last attempt failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// §9's `sync_status()`.
#[derive(Debug, Serialize)]
pub struct SyncStatus {
    /// Whether it is running, and with how many devices.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Whether a process on this device is holding the endpoint — the daemon, usually.
    pub running: bool,
    /// This device's key.
    pub this_device: String,
    /// Every paired device, this one first.
    pub devices: Vec<DeviceView>,
}

/// The paired devices.
#[derive(Debug, Serialize)]
pub struct DeviceList {
    /// How many.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// This one first, then by name.
    pub devices: Vec<DeviceView>,
}

announced!(ServerInfo, PairedWith, SyncReport, SyncStatus, DeviceList);
