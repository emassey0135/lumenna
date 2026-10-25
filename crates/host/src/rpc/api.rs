//! What one command produced, as `lum --json` and every RPC server carry it.
//!
//! The operations and the records they return are `lumenna_host`'s — the same ones the
//! apps get through BoltFFI. What is here is the envelope a terminal and a pipe need on
//! top: the contract version, a `result` tag a reader can dispatch on, and whether text mode
//! should stay quiet. Every payload carries its own `announcement` and `notices`, so a
//! [`Response`] has nothing to compose.
//!
//! `--json` is a compatibility contract. Reshaping anything here, or in the surface's
//! records, is a breaking change, and [`VERSION`] says which shape a reader is looking at.

use crate::{
    Announced, BackupDone, Choices, BlockShown, Change, Completions, DeviceList, Exported, Filters, ImportDone,
    Imported, PairedWith, Plan, RestoreDone, Rows, SettingList, SyncReport, SyncStatus,
    TaskShown, Timer, announced,
};
use serde::Serialize;

/// The contract version, carried on every response.
pub const VERSION: u32 = crate::CONTRACT;

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
    /// What `lum daemon status` found of the sync daemon's service, for a client that turns it
    /// on and off (Emacs's Sync in the background); absent from every other response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceState>,
}

/// Whether the sync daemon is installed as the platform's service, and whether anything holds
/// the device's sync endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ServiceState {
    /// Installed as a service, so it starts at login.
    pub installed: bool,
    /// Something on this device holds the sync endpoint: the daemon, or an app.
    pub running: bool,
}

impl Response {
    /// A response carrying this payload.
    pub fn new(outcome: impl Into<Outcome>) -> Self {
        Self { version: VERSION, outcome: outcome.into(), silent: false, service: None }
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

    /// Carries what was found of the sync daemon's service.
    #[must_use]
    pub const fn with_service(mut self, service: ServiceState) -> Self {
        self.service = Some(service);
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
    /// What a picker offers.
    Choices(Choices),
    /// Reminders to give.
    DueReminders(crate::DueReminders),
    /// A task's or block's reminders.
    Reminders(crate::ReminderList),
    /// A task's history and urgency.
    History(crate::TaskHistory),
    /// What replanning the rest of today proposes.
    Replan(crate::Replan),
    /// What planning a whole day proposes.
    DayPlan(crate::DayPlan),
    /// A replan's changes made, and the next proposal.
    Replanned(crate::Replanned),
    /// A day plan's sittings made, and the next plan.
    DayPlanned(crate::DayPlanned),
    /// Every operation an assistant can do, as `lum assist --list` writes them.
    Operations(Operations),
    /// What could come next in what is being typed.
    Completions(Completions),
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
    Server(ServerInfo),
    Backup(BackupDone),
    Restore(RestoreDone),
    Export(Exported),
    Import(ImportDone),
    Paired(PairedWith),
    Synced(SyncReport),
    SyncStatus(SyncStatus),
    Devices(DeviceList),
    Choices(Choices),
    DueReminders(crate::DueReminders),
    Reminders(crate::ReminderList),
    History(crate::TaskHistory),
    Replan(crate::Replan),
    DayPlan(crate::DayPlan),
    Replanned(crate::Replanned),
    DayPlanned(crate::DayPlanned),
    Operations(Operations),
    Completions(Completions),
);

impl From<lumenna_records::assistant::Assisted> for Outcome {
    fn from(assisted: lumenna_records::assistant::Assisted) -> Self {
        use lumenna_records::assistant::Assisted as A;
        match assisted {
            A::Change(x) => Self::Change(x),
            A::Rows(x) => Self::Rows(x),
            A::Task(x) => Self::Task(x),
            A::Plan(x) => Self::Plan(x),
            A::Filters(x) => Self::Filters(x),
            A::Choices(x) => Self::Choices(x),
            A::Replan(x) => Self::Replan(x),
            A::DayPlan(x) => Self::DayPlan(x),
            A::Timer(x) => Self::Timer(x),
            A::Replanned(x) => Self::Replanned(x),
            A::DayPlanned(x) => Self::DayPlanned(x),
        }
    }
}

impl From<Imported> for Outcome {
    fn from(imported: Imported) -> Self {
        match imported {
            Imported::Export { done } => Self::Import(done),
            Imported::Backup { done } => Self::Restore(done),
        }
    }
}

/// The assistant table (`lumenna_records::assistant::operations`), for a script that builds its
/// own commands from it: the PowerShell module's cmdlets, and the test that keeps them whole.
#[derive(Debug, Serialize)]
pub struct Operations {
    /// How many there are.
    pub announcement: String,
    /// Nothing, as a rule.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Every operation, in the table's order.
    pub operations: Vec<lumenna_records::assistant::AssistantOperation>,
}

impl Operations {
    /// The whole table.
    #[must_use]
    pub fn all() -> Self {
        let operations = lumenna_records::assistant::operations();
        Self { announcement: crate::words::count_line(operations.len(), "operation"), notices: Vec::new(), operations }
    }
}

announced!(Operations);

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
    /// What is answering: `daemon` for `lum`'s, an app's name for an app holding this
    /// device's endpoint, or `lum rpc` for a server of a client's own.
    pub process: String,
    /// The version of the shapes on the wire, the compatibility contract. A client that
    /// does not know this number should refuse to guess.
    pub contract: u32,
    /// Every method this server answers, so a client can find out rather than assume.
    pub methods: Vec<&'static str>,
}

announced!(ServerInfo);

