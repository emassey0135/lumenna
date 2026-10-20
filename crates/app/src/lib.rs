//! Lumenna's screens as a Crux app.
//!
//! What every app decided on its own before — which row is in hand after a change, what is
//! folded, which question an action is asking and what it refused, what to say and when —
//! is decided here once, as a pure function of events: [`update`](crux_core::App::update)
//! changes the [`Model`] and returns effects, and [`view`](crux_core::App::view) reads the
//! model into a [`ViewModel`] each app draws with its own controls.
//!
//! The model is a set of screens ([`Screen`]: a window, an Emacs buffer, a tab), each with its
//! own place, row in hand, folding and question open, over what the device holds alike: the
//! store, the clock, reminders due, syncing. An event goes to one screen ([`Event::On`]; screen
//! 0 without one), and the answers to what it asked come back to that screen.
//!
//! The app holds no store. It knows the store as the snapshot the host last sent
//! ([`host::Watch`]), and every listing is worked out from that snapshot by
//! `lumenna-records`, the same functions `lum` lists with. Every write is an effect
//! ([`host::HostRequest`]) whose answer comes back as an event. What a shell does itself is
//! drawing ([`Effect::Render`]) and saying ([`Effect::Announce`]), each its platform's way.
//!
//! [`update`](crux_core::App::update) only dispatches. [`Model`] and [`Screen`] are `model.rs`; a question and
//! running an action are `asking.rs`; each screen's events, state and view are its own module
//! (`day`, `details`, `typing`, `blockform`, `proposal`, `settings`, `network`, `due`), and the
//! view model is assembled in `view.rs`.

mod asking;
pub mod blockform;
pub mod capitals;
pub mod held;
pub mod keys;
pub mod lines;
pub mod day;
pub mod details;
pub mod devices;
pub mod due;
pub mod host;
pub mod list;
mod model;
pub mod network;
pub mod outline;
pub mod proposal;
pub mod settings;
pub mod shell;
pub mod speech;
pub mod typing;
mod view;


use crux_core::render::render;
use crux_core::{App, Command};
use lumenna_records::places::Place;
use lumenna_records::{Action, ActionKind, Answer, Change};
use serde::{Deserialize, Serialize};

pub use blockform::{BlockEvent, BlockFormView};
pub use capitals::Capitals;
pub use held::{HeldView, Side};
pub use shell::ShellEvent;
pub use day::{DayItem, DayKind, DayView};
pub use details::{DetailsEvent, DetailsView};
pub use typing::{LineView, Typing};
pub use host::{Arrival, HostAnswer, HostRequest, Watch};
pub use list::Item;
pub use proposal::{ProposalEvent, ProposalView};
pub use settings::{SaveFile, SettingsEvent, SettingsView};
pub use due::{Due, DueEvent, Notice};
pub use network::{DevicesEvent, DevicesView, NetworkAnswer, NetworkRequest, PairEvent};
pub use view::{Asking, ListView, ViewModel};
pub use asking::{ANOTHER_DAY, Open, Purpose};
pub use model::{Model, Screen, ScreenId};

/// Something to say, once the shell has moved focus where the view model puts it: said
/// before, it is cut off by the focus change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announce {
    /// The sentence.
    pub text: String,
    /// Say the day's heading instead — "<day>. <summary>", the day in the shell's own words —
    /// `text` being the summary: what a new day shown says.
    pub heading: bool,
    /// The screen that says it: where focus is about to land.
    pub screen: ScreenId,
}

impl crux_core::capability::Operation for Announce {
    type Output = ();
}

pub use effects::Effect;

// The macro writes items of its own beside the enum, undocumented.
#[allow(missing_docs)]
mod effects {
    use crux_core::macros::effect;
    use crux_core::render::RenderOperation;

    use crate::{Announce, HostRequest, NetworkRequest, Notice, SaveFile, Watch};

    /// What the app asks for.
    #[effect]
    #[derive(Debug)]
    pub enum Effect {
        /// Draw the view model again.
        Render(RenderOperation),
        /// Say something.
        Announce(Announce),
        /// The store as it changes: the host's.
        Watch(Watch),
        /// A write: the host's.
        Host(HostRequest),
        /// Save a file the person keeps: a backup, an export.
        Save(SaveFile),
        /// Reach other devices: the platform's runner answers, when it can.
        Network(NetworkRequest),
        /// Show a notification, as the platform shows one.
        Notify(Notice),
    }
}

impl Effect {
    /// The same effect, anything it says said on `screen`, and any file it saves saved from it.
    fn said_on(self, screen: ScreenId) -> Self {
        match self {
            Effect::Announce(mut request) => {
                request.operation.screen = screen;
                Effect::Announce(request)
            }
            Effect::Save(mut request) => {
                request.operation.screen = screen;
                Effect::Save(request)
            }
            effect => effect,
        }
    }
}

/// Moving through the days.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DayMove {
    /// The day before the one shown.
    Previous,
    /// The day after.
    Next,
    /// Today, which then follows the clock.
    Today,
    /// Today, with now in hand.
    Now,
    /// Ask which day, as a phrase: Go to Day.
    Ask,
}

/// What happens: what the person does, and what the host answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// An event on a screen other than the first, or the answer to what one asked. An event
    /// without one is screen 0's.
    On(ScreenId, Box<Event>),
    /// The screen is closed: a window shut, a buffer killed. What it had asked for that comes
    /// back afterwards is dropped.
    CloseScreen(ScreenId),
    /// The app has started: watch the store.
    Start,
    /// Go to a place.
    Open(Place),
    /// The person moved to a row.
    Focus(String),
    /// The person moved to a row of a list beside the main one: the sidebar, a reminder list,
    /// the devices. A change made there lands there.
    Hold(Side, String),
    /// The filter typed above the list, in place of the place's own query; empty for the
    /// place's own again.
    Filter(String),
    /// Run an action a row offered: from its menu, a button, a swipe.
    Perform(Action),
    /// Run the first of these kinds the row in hand offers, for a key that means one thing on
    /// every row: Space is Mark Done, Mark Not Done or Restore. One it offers none of says why,
    /// in the words for the first.
    Run(Vec<ActionKind>),
    /// The answer to the question open.
    Answer(Answer),
    /// The open pick offers the work blocks of the day `day` names ("friday", "2026-11-03")
    /// instead of the week's: Put in a Block, for a block further off.
    PickFrom(String),
    /// Leave the question open unanswered.
    Cancel,
    /// Move through the days, going to the day if another place is shown.
    Day(DayMove),
    /// A block's day, the block in hand: where a reminder of it opens.
    ShowBlock {
        /// Its series: a reminder's `target`.
        series: String,
        /// Its day, `YYYY-MM-DD`: a reminder's `occurrence`.
        date: String,
    },
    /// Something done in the details of the task in hand.
    Details(DetailsEvent),
    /// Something done in quick add.
    Typing(Typing),
    /// Something done in the block form.
    Block(BlockEvent),
    /// Something done on a proposal's screen.
    Proposal(ProposalEvent),
    /// Close the list of a task's or block's reminders.
    CloseReminders,
    /// Something done in Settings.
    Settings(SettingsEvent),
    /// Something done on the Devices page, or while pairing.
    Devices(DevicesEvent),
    /// Reminders due: the shell's look, or a button.
    Due(DueEvent),

    /// Syncing started, or why not.
    #[serde(skip)]
    SyncStarted(NetworkAnswer),
    /// A round's report.
    #[serde(skip)]
    Synced(NetworkAnswer),
    /// How syncing is going.
    #[serde(skip)]
    SyncStatus(NetworkAnswer),
    /// What a pairing under way says.
    #[serde(skip)]
    Pairing(NetworkAnswer),
    /// The clock this shell says times in, for the sentences the core composes whole.
    Clock(lumenna_records::proposals::TimeStyle),
    /// How this shell writes its buttons, headings and questions.
    Capitals(capitals::Capitals),
    /// What this shell is called in pairing's sentences ("this computer", "this watch"), and
    /// whether it finds devices on its own network or pairs only by code: for pairing's words.
    PairingAs {
        /// What it is called.
        called: String,
        /// Whether it finds devices nearby by itself.
        nearby: bool,
    },
    /// The moment the day's proposals are made against, `YYYY-MM-DDTHH:MM` in local time: a
    /// shell whose clock is pinned, for tests. None for now.
    PlanFrom(Option<String>),
    /// Add a task from a quick-add line.
    QuickAdd(String),
    /// Undo the last change.
    Undo,
    /// Redo what was undone.
    Redo,

    /// The store as it now is.
    #[serde(skip)]
    Arrived(Arrival),
    /// What a write did.
    #[serde(skip)]
    Answered(HostAnswer),
}

/// The app.
#[derive(Debug, Default)]
pub struct Lumenna;


pub(crate) fn say(text: impl Into<String>) -> Command<Effect, Event> {
    let text = text.into();
    if text.is_empty() {
        return Command::done();
    }
    Command::notify_shell(Announce { text, heading: false, screen: 0 }).into()
}

pub(crate) fn host(request: HostRequest) -> Command<Effect, Event> {
    Command::request_from_shell(request).then_send(Event::Answered)
}

/// A request of the network, its answer sent as `then`.
pub(crate) fn network_request(request: NetworkRequest, then: fn(NetworkAnswer) -> Event) -> Command<Effect, Event> {
    Command::request_from_shell(request).then_send(then)
}

/// Something told the network that needs no answer: the words' answer, giving up.
pub(crate) fn network_tell(request: NetworkRequest) -> Command<Effect, Event> {
    Command::request_from_shell(request).then_send(|_| Event::Pairing(NetworkAnswer::Done))
}

/// What a change says: its sentence, then anything else worth saying.
pub(crate) fn spoken(change: &Change) -> String {
    lumenna_records::words::spoken(&change.announcement, &change.notices)
}

impl Event {
    /// The screen it is for, and the event itself.
    fn addressed(self) -> (ScreenId, Event) {
        match self {
            Event::On(screen, event) => (screen, event.addressed().1),
            event => (0, event),
        }
    }

    /// What every screen takes in alike: the store, the clock and the capitals the shell says
    /// things in, the moment proposals are made against.
    fn is_everyones(&self) -> bool {
        matches!(self, Event::Arrived(_) | Event::Clock(_) | Event::Capitals(_) | Event::PairingAs { .. } | Event::PlanFrom(_))
    }

    /// Whether the person did it, rather than the store, the network or the shell's own clock.
    fn is_the_persons(&self) -> bool {
        !matches!(
            self,
            Event::Start
                | Event::Arrived(_)
                | Event::Answered(_)
                | Event::SyncStarted(_)
                | Event::Synced(_)
                | Event::SyncStatus(_)
                | Event::Pairing(_)
                | Event::Clock(_)
                | Event::Capitals(_)
                | Event::PairingAs { .. }
                | Event::PlanFrom(_)
                | Event::Due(DueEvent::Look { .. })
        )
    }
}

/// Every screen's view, by screen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Views(pub std::collections::BTreeMap<ScreenId, ViewModel>);

impl Views {
    /// What screen `screen` shows; nothing, before it is opened or once it is closed.
    #[must_use]
    pub fn screen(mut self, screen: ScreenId) -> ViewModel {
        self.0.remove(&screen).unwrap_or_default()
    }
}

impl App for Lumenna {
    type Event = Event;
    type Model = Model;
    type ViewModel = Views;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        let (screen, event) = event.addressed();
        if event.is_everyones() {
            let screens: Vec<ScreenId> = model.screens.keys().copied().collect();
            return Command::all(screens.into_iter().map(|screen| self.on(screen, event.clone(), model)));
        }
        if let Event::CloseScreen(closed) = event {
            model.screens.remove(&closed);
            return render();
        }
        // A screen opens with the person's first event on it; an answer for one closed since
        // goes nowhere.
        if !model.screens.contains_key(&screen) && !event.is_the_persons() {
            return Command::done();
        }
        self.on(screen, event, model)
    }

    fn view(&self, model: &Model) -> Views {
        Views(model.screens.iter().map(|(id, screen)| (*id, view::view(screen))).collect())
    }
}

impl Lumenna {
    /// `event` on screen `id`: what it asks for comes back to the same screen, and what it
    /// says is that screen's to say. What the device holds alike goes to every other screen.
    fn on(&self, id: ScreenId, event: Event, model: &mut Model) -> Command<Effect, Event> {
        let command = self.turn(event, model.screen(id));
        model.share(id);
        command
            .map_event(move |event| Event::On(id, Box::new(event)))
            .map_effect(move |effect| effect.said_on(id))
    }

    /// `event` on one screen.
    fn turn(&self, event: Event, model: &mut Screen) -> Command<Effect, Event> {
        // Where focus goes is the app's to say; the shell moves it only when this moves. Not when
        // the person reports moving, or a late answer would pull them back; always after a
        // write, whose rows a shell redraws.
        let before = model.focus.clone();
        let place = model.place.clone();
        let reported = matches!(event, Event::Focus(_));
        let wrote = matches!(
            event,
            Event::Answered(HostAnswer::Wrote(..) | HostAnswer::Applied(..) | HostAnswer::Imported(..))
        );
        // The store answers off the shell's thread, so the person can move on before a write's
        // answer comes back: then the row they moved to stays in hand, not the one the write
        // would have put there.
        let moved_since = wrote && model.moves > model.acted_at;
        // A side list is where a change lands while the person goes on acting from it — a row's
        // action, the question it asks, its answer — and anything else they do hands landing
        // back to the main list.
        match &event {
            Event::Hold(side, _) => model.acting = Some(*side),
            Event::Perform(_) | Event::Answer(_) | Event::Cancel | Event::Run(_) => {}
            other if other.is_the_persons() => model.acting = None,
            _ => {}
        }
        match &event {
            Event::Focus(_) => model.moves += 1,
            Event::Hold(..) => {}
            Event::Arrived(_)
            | Event::Answered(_)
            | Event::SyncStarted(_)
            | Event::Synced(_)
            | Event::SyncStatus(_)
            | Event::Pairing(_)
            | Event::Clock(_)
            | Event::Capitals(_)
            | Event::PairingAs { .. }
            | Event::PlanFrom(_) => {}
            _ => model.acted_at = model.moves,
        }
        let command = self.dispatch(event, model);
        if moved_since && model.place == place && before.is_some() {
            model.focus = before.clone();
            model.settle();
        }
        model.settle_sides(wrote);
        // A change made in a side list lands there; the main list's row moves on quietly.
        if !reported && model.acting.is_none() && (wrote || model.focus != before) {
            model.landing += 1;
        }
        command
    }

    fn dispatch(&self, event: Event, model: &mut Screen) -> Command<Effect, Event> {
        match event {
            // Taken apart before a screen's turn.
            Event::On(..) | Event::CloseScreen(_) => Command::done(),
            Event::Start => Command::stream_from_shell(Watch).then_send(Event::Arrived).and(host(HostRequest::Devices)),
            Event::Arrived(store) => self.arrived(store, model),
            Event::Open(place) => {
                model.open = None;
                model.go(place);
                render()
            }
            Event::Focus(key) => {
                model.hold(&key);
                model.details.shown = None;
                render()
            }
            Event::Hold(side, key) => {
                let keys = model.side_keys(side);
                model.held_mut(side).hold(&key, &keys);
                render()
            }
            Event::Filter(text) => self.filter(text, model),
            Event::Perform(action) => self.perform(action, model),
            Event::Run(kinds) => self.run(kinds, model),
            Event::Answer(answer) => self.answer(answer, model),
            Event::PickFrom(day) => self.pick_from(&day, model),
            Event::Cancel => {
                model.open = None;
                render()
            }
            Event::Day(step) => self.day_move(step, model),
            Event::ShowBlock { series, date } => self.show_block(&series, &date, model),
            Event::Details(event) => self.details(event, model),
            Event::Typing(event) => self.typing(event, model),
            Event::Block(event) => self.block(event, model),
            Event::Proposal(event) => self.proposal(event, model),
            Event::Settings(event) => self.settings(event, model),
            Event::Devices(event) => self.devices(event, model),
            Event::Due(event) => self.due(event, model),
            Event::SyncStarted(answer) => self.sync_started(answer, model),
            Event::Synced(answer) => self.synced(answer),
            Event::SyncStatus(answer) => self.sync_status(answer, model),
            Event::Pairing(answer) => self.pairing(answer, model),
            Event::Answered(HostAnswer::Devices(result)) => self.devices_listed(result, model),
            Event::Answered(HostAnswer::Found(result)) => self.found(result, model),
            Event::Answered(HostAnswer::Listed(result)) => self.listed(result, model),
            Event::Answered(HostAnswer::Saved(result)) => self.saved(*result),
            Event::Answered(HostAnswer::Imported(result, store)) => self.imported(*result, store, model),
            Event::Answered(HostAnswer::Applied(result, store)) => self.applied(*result, store, model),
            Event::CloseReminders => {
                model.reminders = None;
                render()
            }
            Event::PlanFrom(moment) => {
                model.plan_from = moment;
                Command::done()
            }
            Event::Clock(style) => {
                model.style = Some(style);
                Command::done()
            }
            Event::Capitals(capitals) => {
                model.capitals = capitals;
                render()
            }
            Event::PairingAs { called, nearby } => {
                model.pairing_as = Some((called, nearby));
                render()
            }
            Event::QuickAdd(text) => {
                model.adding = true;
                model.add_refused = None;
                host(HostRequest::AddTask { text })
            }
            Event::Undo => host(HostRequest::Undo),
            Event::Redo => host(HostRequest::Redo),
            Event::Answered(HostAnswer::Wrote(result, store)) => self.wrote(*result, store, model),
        }
    }
}
