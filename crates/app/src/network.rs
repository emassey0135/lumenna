//! What reaches other devices: syncing, and pairing a new one.
//!
//! These are asynchronous everywhere — a round waits on the network, a pairing on another
//! device and on the person — so the bridge does not answer them as it answers the store's
//! requests. It hands each to the platform's runner, which answers it when it can: a pairing
//! more than once, as it shows a code, then words to compare, then how it ended.

use crux_core::capability::Operation;
use lumenna_records::types::DeviceList;
use serde::{Deserialize, Serialize};

/// Something asked of the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkRequest {
    /// Keep this device in sync while the app runs.
    StartSync,
    /// Sync with every paired device now.
    SyncNow,
    /// How syncing is going, as one sentence.
    Status,
    /// Pair with another device: wait to be dialled when `code` is none, else dial it.
    Pair {
        /// The other device's code.
        code: Option<String>,
        /// What this device is called on the others.
        name: String,
    },
    /// Whether the words shown match on the other device.
    Confirm(bool),
    /// Give up the pairing under way.
    CancelPairing,
}

/// What the network answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkAnswer {
    /// Done, with nothing to say: syncing started, an answer taken.
    Done,
    /// Something to say: a round's report, syncing's status, a failure.
    Said(String),
    /// A pairing waiting to be dialled shows this code.
    Code(String),
    /// The words to compare on both devices.
    Words(Vec<String>),
    /// The pairing ended: what it made, or why not.
    Paired(Result<String, String>),
}

impl Operation for NetworkRequest {
    type Output = NetworkAnswer;
}

/// What the person does on the Devices page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum DevicesEvent {
    /// The page is shown: list the devices, and how syncing is going.
    Show,
    /// Sync now.
    SyncNow,
    /// Something done while pairing.
    Pair(PairEvent),
}

/// What the person does while pairing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum PairEvent {
    /// The pairing screen opens, this device's name to start from.
    Open(String),
    /// What this device is to be called.
    Name(String),
    /// The other device's code, as typed.
    Code(String),
    /// Wait to be dialled, showing a code.
    Wait,
    /// Dial the code typed; while waiting, the wait is given up first.
    Join,
    /// Whether the words match.
    Confirm(bool),
    /// Give up: the pairing under way, else the screen.
    Cancel,
}

/// A pairing, while its screen is open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pairing {
    /// What this device is to be called.
    pub name: String,
    /// The other device's code, as typed.
    pub theirs: String,
    /// This device's code, while it waits to be dialled.
    pub mine: Option<String>,
    /// Whether a pairing is under way.
    pub running: bool,
    /// Whether the pairing under way waits to be dialled, rather than dialling.
    pub waiting: bool,
    /// The words to compare, while they are asked about.
    pub words: Option<Vec<String>>,
    /// What went wrong.
    pub problem: Option<String>,
    /// A code entered while waiting: dialled once the wait has been given up.
    pub then: Option<String>,
}

/// The Devices page's state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Devices {
    /// The paired devices, once the host has said.
    pub list: Option<DeviceList>,
    /// How syncing is going, once the network has said.
    pub status: Option<String>,
}

/// The pairing screen as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PairingView {
    /// What this device is to be called.
    pub name: String,
    /// The other device's code, as typed.
    pub theirs: String,
    /// This device's code, while it waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mine: Option<String>,
    /// Whether a pairing is under way.
    pub running: bool,
    /// Whether it waits to be dialled.
    pub waiting: bool,
    /// The words to compare, while asked about: the question is the core's words'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<String>>,
    /// What went wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// The Devices page as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct DevicesView {
    /// The paired devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<DeviceList>,
    /// How syncing is going.
    pub status: String,
    /// Whether this device keeps in sync now.
    pub syncing: bool,
    /// The pairing screen, while open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairing: Option<PairingView>,
}

impl Pairing {
    /// As shown.
    #[must_use]
    pub fn view(&self) -> PairingView {
        PairingView {
            name: self.name.clone(),
            theirs: self.theirs.clone(),
            mine: self.mine.clone(),
            running: self.running,
            waiting: self.waiting,
            words: self.words.clone(),
            problem: self.problem.clone(),
        }
    }
}
