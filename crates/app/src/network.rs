//! What reaches other devices: syncing, and pairing a new one.
//!
//! These are asynchronous everywhere — a round waits on the network, a pairing on another
//! device and on the person — so the bridge does not answer them as it answers the store's
//! requests. It hands each to the platform's runner, which answers it when it can: a pairing
//! more than once, as it shows a code, then words to compare, then how it ended.

use crux_core::capability::Operation;
use crux_core::Command;
use crux_core::render::render;
use lumenna_records::{LumennaError, PairingWords};
use lumenna_records::types::DeviceList;
use serde::{Deserialize, Serialize};

use crate::host::HostRequest;
use crate::model::Screen;
use crate::{Effect, Event, Lumenna, host, network_request, network_tell, say};

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
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DevicesEvent {
    /// The page is shown: list the devices, and how syncing is going.
    Show,
    /// Sync now.
    SyncNow,
    /// Something done while pairing.
    Pair(PairEvent),
}

/// What the person does while pairing.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicesView {
    /// Every sentence and button of pairing, in the shell's capitals.
    pub words: PairingWords,
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


/// Starts a pairing: waiting to be dialled without `code`, else dialling it.
fn start_pairing(pairing: &mut Pairing, code: Option<String>, words: &lumenna_records::PairingWords) -> Command<Effect, Event> {
    pairing.problem = None;
    pairing.running = true;
    pairing.waiting = code.is_none();
    let said = if code.is_some() { words.connecting.clone() } else { words.opening.clone() };
    let name = pairing.name.trim().to_owned();
    let pair: Command<Effect, Event> = Command::stream_from_shell(NetworkRequest::Pair { code, name }).then_send(Event::Pairing);
    render().and(say(said)).and(pair)
}

impl Lumenna {
    pub(crate) fn devices(&self, event: DevicesEvent, model: &mut Screen) -> Command<Effect, Event> {
        match event {
            DevicesEvent::Show => {
                model.sync_failed = false;
                model.devices = Some(Devices::default());
                model.devices_held = crate::held::Held::default();
                render().and(host(HostRequest::Devices)).and(network_request(NetworkRequest::Status, Event::SyncStatus))
            }
            DevicesEvent::SyncNow => say(lumenna_records::words::syncing()).and(network_request(NetworkRequest::SyncNow, Event::Synced)),
            DevicesEvent::Pair(event) => self.pair(event, model),
        }
    }

    pub(crate) fn pair(&self, event: PairEvent, model: &mut Screen) -> Command<Effect, Event> {
        if let PairEvent::Open(name) = event {
            model.pairing = Some(Pairing { name, ..Pairing::default() });
            return render();
        }
        let words = model.pairing_words();
        let Some(pairing) = &mut model.pairing else { return Command::done() };
        match event {
            PairEvent::Open(_) => Command::done(),
            PairEvent::Name(name) => {
                pairing.name = name;
                render()
            }
            PairEvent::Code(code) => {
                pairing.theirs = code;
                render()
            }
            // One pairing at a time: waiting while one is under way does nothing.
            PairEvent::Wait if pairing.running => Command::done(),
            PairEvent::Wait => start_pairing(pairing, None, &words),
            PairEvent::Join => {
                let code = pairing.theirs.trim().to_owned();
                if code.is_empty() {
                    pairing.problem = Some(words.need_code.clone());
                    return render().and(say(words.need_code));
                }
                // One pairing at a time: a code entered while waiting gives up the wait first.
                if pairing.running {
                    pairing.then = Some(code);
                    return render().and(say(words.switching)).and(network_tell(NetworkRequest::CancelPairing));
                }
                start_pairing(pairing, Some(code), &words)
            }
            PairEvent::Confirm(matched) => {
                pairing.words = None;
                let said = if matched { words.finishing } else { words.refusing };
                render().and(say(said)).and(network_tell(NetworkRequest::Confirm(matched)))
            }
            PairEvent::Cancel => {
                if pairing.running {
                    return network_tell(NetworkRequest::CancelPairing);
                }
                model.pairing = None;
                render()
            }
        }
    }

    pub(crate) fn pairing(&self, answer: NetworkAnswer, model: &mut Screen) -> Command<Effect, Event> {
        let words = model.pairing_words();
        let Some(pairing) = &mut model.pairing else { return Command::done() };
        match answer {
            NetworkAnswer::Done | NetworkAnswer::Said(_) => Command::done(),
            NetworkAnswer::Code(code) => {
                pairing.mine = Some(code);
                render().and(say(words.waiting))
            }
            NetworkAnswer::Words(three) => {
                pairing.words = Some(three);
                render()
            }
            NetworkAnswer::Paired(Ok(said)) => {
                model.pairing = None;
                model.sync_failed = false;
                render().and(say(said)).and(host(HostRequest::Devices))
            }
            NetworkAnswer::Paired(Err(why)) => {
                pairing.running = false;
                pairing.mine = None;
                pairing.words = None;
                if let Some(next) = pairing.then.take() {
                    return start_pairing(pairing, Some(next), &words);
                }
                pairing.problem = Some(why.clone());
                render().and(say(why))
            }
        }
    }

    pub(crate) fn sync_started(&self, answer: NetworkAnswer, model: &mut Screen) -> Command<Effect, Event> {
        match answer {
        // Asked again when the person next shows the devices or pairs one, not at every refresh.
        NetworkAnswer::Said(why) => {
            model.sync_asked = false;
            model.sync_failed = true;
            say(lumenna_records::words::sync_not_started(&why))
        }
        _ => {
            model.syncing = true;
            if model.devices.is_some() {
                return render().and(network_request(NetworkRequest::Status, Event::SyncStatus));
            }
            render()
        }
    }
    }

    pub(crate) fn synced(&self, answer: NetworkAnswer) -> Command<Effect, Event> {
        let said = match answer {
            NetworkAnswer::Said(said) => said,
            _ => String::new(),
        };
        say(said).and(host(HostRequest::Devices))
    }

    pub(crate) fn sync_status(&self, answer: NetworkAnswer, model: &mut Screen) -> Command<Effect, Event> {
        if let (NetworkAnswer::Said(status), Some(devices)) = (answer, &mut model.devices) {
            devices.status = Some(status);
        }
        render()
    }

    pub(crate) fn devices_listed(&self, result: Result<lumenna_records::types::DeviceList, LumennaError>, model: &mut Screen) -> Command<Effect, Event> {
        let Ok(list) = result else { return Command::done() };
        // Another device paired: keep in sync with it while the app runs.
        let start = !model.sync_asked && !model.sync_failed && list.devices.iter().any(|d| !d.this_device);
        if let Some(devices) = &mut model.devices {
            devices.list = Some(list);
        }
        if start {
            model.sync_asked = true;
            return render().and(network_request(NetworkRequest::StartSync, Event::SyncStarted));
        }
        render()
    }
}
