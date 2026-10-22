//! What a shell sends across BoltFFI (Android's, the web's): the app's events that a person's
//! doing makes.
//!
//! The app's [`Event`] also carries what its own effects answer — the store as it arrives, a
//! write's result, the network's word — which never crosses, and BoltFFI cannot leave a variant
//! out. So the shell's events are their own type here. Turning an [`Event`] back into one is an
//! exhaustive match, so a new event is sorted into one or the other where it is added.

use boltffi::data;
use crate::{ActingOn, BlockEvent, Capitals, DayMove, DetailsEvent, DevicesEvent, DueEvent, Event, ProposalEvent, SettingsEvent, Side, Typing};
use lumenna_records::places::Place;
use lumenna_records::proposals::TimeStyle;
use lumenna_records::Offered;
use lumenna_records::{ActionKind, Answer};

/// Something the person did, or the shell says of itself: as [`Event`], less what the app's
/// own effects answer.
#[data]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
// Sent once each, by value, across the boundary: boxing the large ones would only add an
// allocation to every event.
#[allow(clippy::large_enum_variant)]
pub enum ShellEvent {
    /// The app has started: watch the store.
    Start,
    /// Go to a place.
    Open(Place),
    /// The person moved to a row.
    Focus(String),
    /// The person moved to a row of a list beside the main one.
    Hold(Side, String),
    /// The filter typed above the list; empty for the place's own query again.
    Filter(String),
    /// Run an action a row offered, in short as the view gave it, on the row it names.
    ActOn(ActingOn, Offered),
    /// Run the first of these kinds the row in hand offers.
    Run(Vec<ActionKind>),
    /// The answer to the question open.
    Answer(Answer),
    /// Leave the question open unanswered.
    Cancel,
    /// Move through the days.
    Day(DayMove),
    /// Run the first of these kinds the day shown offers of its own (Plan the Day, Replan from
    /// Now, …), or say why it has none, whatever row is in hand.
    RunDay(Vec<ActionKind>),
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
    /// The clock this shell says times in.
    Clock(TimeStyle),
    /// How this shell writes its buttons, headings and questions.
    Capitals(Capitals),
    /// What this shell is called in pairing's sentences ("this computer", "this watch"), and
    /// whether it finds devices on its own network or pairs only by code: for pairing's words.
    PairingAs {
        /// What it is called.
        called: String,
        /// Whether it finds devices nearby by itself.
        nearby: bool,
    },
    /// The moment proposals are made against, `YYYY-MM-DDTHH:MM` local, when the shell's clock
    /// is pinned for tests; none for now.
    PlanFrom(Option<String>),
    /// Add a task from a quick-add line.
    QuickAdd(String),
    /// Undo the last change.
    Undo,
    /// Redo what was undone.
    Redo,
}

impl From<ShellEvent> for Event {
    fn from(event: ShellEvent) -> Self {
        match event {
            ShellEvent::Start => Event::Start,
            ShellEvent::Open(place) => Event::Open(place),
            ShellEvent::Focus(key) => Event::Focus(key),
            ShellEvent::Hold(side, key) => Event::Hold(side, key),
            ShellEvent::Filter(text) => Event::Filter(text),
            ShellEvent::ActOn(on, offered) => Event::ActOn(on, offered),
            ShellEvent::Run(kinds) => Event::Run(kinds),
            ShellEvent::Answer(answer) => Event::Answer(answer),
            ShellEvent::Cancel => Event::Cancel,
            ShellEvent::Day(step) => Event::Day(step),
            ShellEvent::RunDay(kinds) => Event::RunDay(kinds),
            ShellEvent::ShowBlock { series, date } => Event::ShowBlock { series, date },
            ShellEvent::Details(event) => Event::Details(event),
            ShellEvent::Typing(event) => Event::Typing(event),
            ShellEvent::Block(event) => Event::Block(event),
            ShellEvent::Proposal(event) => Event::Proposal(event),
            ShellEvent::CloseReminders => Event::CloseReminders,
            ShellEvent::Settings(event) => Event::Settings(event),
            ShellEvent::Devices(event) => Event::Devices(event),
            ShellEvent::Due(event) => Event::Due(event),
            ShellEvent::Clock(style) => Event::Clock(style),
            ShellEvent::Capitals(capitals) => Event::Capitals(capitals),
            ShellEvent::PairingAs { called, nearby } => Event::PairingAs { called, nearby },
            ShellEvent::PlanFrom(moment) => Event::PlanFrom(moment),
            ShellEvent::QuickAdd(line) => Event::QuickAdd(line),
            ShellEvent::Undo => Event::Undo,
            ShellEvent::Redo => Event::Redo,
        }
    }
}

impl TryFrom<Event> for ShellEvent {
    type Error = Event;

    /// The shell's event an [`Event`] is, or the event back when it is one of the app's own.
    fn try_from(event: Event) -> Result<Self, Event> {
        Ok(match event {
            Event::Start => ShellEvent::Start,
            Event::Open(place) => ShellEvent::Open(place),
            Event::Focus(key) => ShellEvent::Focus(key),
            Event::Hold(side, key) => ShellEvent::Hold(side, key),
            Event::Filter(text) => ShellEvent::Filter(text),
            Event::ActOn(on, offered) => ShellEvent::ActOn(on, offered),
            Event::Run(kinds) => ShellEvent::Run(kinds),
            Event::Answer(answer) => ShellEvent::Answer(answer),
            Event::Cancel => ShellEvent::Cancel,
            Event::Day(step) => ShellEvent::Day(step),
            Event::RunDay(kinds) => ShellEvent::RunDay(kinds),
            Event::ShowBlock { series, date } => ShellEvent::ShowBlock { series, date },
            Event::Details(event) => ShellEvent::Details(event),
            Event::Typing(event) => ShellEvent::Typing(event),
            Event::Block(event) => ShellEvent::Block(event),
            Event::Proposal(event) => ShellEvent::Proposal(event),
            Event::CloseReminders => ShellEvent::CloseReminders,
            Event::Settings(event) => ShellEvent::Settings(event),
            Event::Devices(event) => ShellEvent::Devices(event),
            Event::Due(event) => ShellEvent::Due(event),
            Event::Clock(style) => ShellEvent::Clock(style),
            Event::Capitals(capitals) => ShellEvent::Capitals(capitals),
            Event::PairingAs { called, nearby } => ShellEvent::PairingAs { called, nearby },
            Event::PlanFrom(moment) => ShellEvent::PlanFrom(moment),
            Event::QuickAdd(line) => ShellEvent::QuickAdd(line),
            Event::Undo => ShellEvent::Undo,
            Event::Redo => ShellEvent::Redo,
            // A shell's events are screen 0's for now; another screen is the app's own to address.
            Event::On(..)
            | Event::CloseScreen(_)
            | Event::SyncStarted(_)
            | Event::Synced(_)
            | Event::SyncStatus(_)
            | Event::Pairing(_)
            | Event::Arrived(_)
            | Event::Answered(_)
            // A whole action is the app's own to run; a shell names a row's in short.
            | Event::Perform(_) => return Err(event),
        })
    }
}
