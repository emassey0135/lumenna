//! What one command produced, as `--json` and `lum rpc` carry it.
//!
//! The operations and the records they return are `lumenna_surface`'s — the same ones the
//! phone apps get through UniFFI. What is here is the envelope a terminal and a pipe need on
//! top: the contract version, a `result` tag a reader can dispatch on, and whether text mode
//! should stay quiet. Every payload carries its own `announcement` and `notices`, so a
//! [`Response`] has nothing to compose.
//!
//! `--json` is a compatibility contract. Reshaping anything here, or in the surface's
//! records, is a breaking change, and [`VERSION`] says which shape a reader is looking at.

use lumenna_surface::{
    Announced, BackupDone, BlockShown, Change, Completions, DeviceList, Exported, Filters, ImportDone,
    Imported, PairedWith, Plan, Preview, RestoreDone, Rows, SettingList, SyncReport, SyncStatus,
    TaskShown, Timer, WorkBlocks, announced,
};
use serde::Serialize;

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
    /// One block series, as an editor starts from it.
    Block(BlockShown),
    /// Saved filters.
    Filters(Filters),
    /// Settings.
    Settings(SettingList),
    /// A timer stopped, or minutes logged.
    Timer(Timer),
    /// What could be typed next. Reachable over `lum rpc` only: completion is a
    /// keystroke-rate question, and a process per keystroke is not an answer.
    Completions(Completions),
    /// What a quick-add line would produce, without producing it.
    Preview(Preview),
    /// The work blocks a task could go in. Reachable over `lum rpc` only.
    WorkBlocks(WorkBlocks),
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
    /// A pairing finished.
    Paired(PairedWith),
    /// A sync round with every paired device.
    Synced(SyncReport),
    /// How sync is going.
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
    Block(BlockShown),
    Filters(Filters),
    Settings(SettingList),
    Timer(Timer),
    Completions(Completions),
    Preview(Preview),
    WorkBlocks(WorkBlocks),
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
    /// The version of the shapes on the wire, the compatibility contract. A client that
    /// does not know this number should refuse to guess.
    pub contract: u32,
    /// Every method this server answers, so a client can find out rather than assume.
    pub methods: Vec<&'static str>,
}

announced!(ServerInfo);
