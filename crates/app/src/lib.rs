//! Lumenna's screens as a Crux app.
//!
//! What every app decided on its own before — which row is in hand after a change, what is
//! folded, which question an action is asking and what it refused, what to say and when —
//! is decided here once, as a pure function of events: [`update`](crux_core::App::update)
//! changes the [`Model`] and returns effects, and [`view`](crux_core::App::view) reads the
//! model into a [`ViewModel`] each app draws with its own controls.
//!
//! The app holds no store. It knows the store as the snapshot the host last sent
//! ([`host::Watch`]), and every listing is worked out from that snapshot by
//! `lumenna-records`, the same functions `lum` lists with. Every write is an effect
//! ([`host::HostRequest`]) whose answer comes back as an event. What a shell does itself is
//! drawing ([`Effect::Render`]) and saying ([`Effect::Announce`]), each its platform's way.

pub mod host;
pub mod list;
mod view;

use std::collections::BTreeSet;

use crux_core::render::render;
use crux_core::{App, Command};
use lumenna_records::places::Place;
use lumenna_records::{Action, ActionKind, Answer, Change, Choices, LumennaError, Question, not_offered};
use serde::{Deserialize, Serialize};

pub use host::{Arrival, HostAnswer, HostRequest, Watch};
pub use list::Item;
pub use view::{Asking, ListView, ViewModel};

/// Something to say, once the shell has moved focus where the view model puts it: said
/// before, it is cut off by the focus change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announce {
    /// The sentence.
    pub text: String,
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

    use crate::{Announce, HostRequest, Watch};

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
    }
}

/// What happens: what the person does, and what the host answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum Event {
    /// The app has started: watch the store.
    Start,
    /// Go to a place.
    Open(Place),
    /// The person moved to a row.
    Focus(String),
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
    /// Leave the question open unanswered.
    Cancel,
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

impl PartialEq for HostAnswer {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Wrote(a, x), Self::Wrote(b, y)) => a == b && x == y,
            (Self::Offered(a), Self::Offered(b)) => a == b,
            _ => false,
        }
    }
}

/// An action whose question is open.
#[derive(Debug, Clone, PartialEq)]
pub struct Open {
    /// The action asking.
    pub action: Action,
    /// What a pick offers, once the host has said.
    pub choices: Option<Choices>,
    /// Why the last answer was refused: the question stays open with it.
    pub refusal: Option<String>,
    /// What was picked, for a pick that asks a second question once one is: how long a
    /// sitting is meant to take.
    pub picked: Option<String>,
}

/// Everything the screens hold.
#[derive(Debug, Default)]
pub struct Model {
    /// The store as last sent, once it has been.
    pub store: Option<Arrival>,
    /// Where the person is.
    pub place: Option<Place>,
    /// The row in hand, by identifier.
    pub focus: Option<String>,
    /// Where it was in the rows shown, for whatever takes its place if it goes.
    pub position: usize,
    /// Rows folded away, by identifier.
    pub folded: BTreeSet<String>,
    /// The question open, if any.
    pub open: Option<Open>,
    /// The filter typed above the list, while the person stays in this place.
    pub filter: Option<String>,
    /// The last filter typed that read, whose rows stay shown while one being typed does not.
    pub read: Option<String>,
    /// A quick-add line on its way to the host.
    pub adding: bool,
    /// Why the last quick-add line was refused: the line stays, to be put right.
    pub add_refused: Option<String>,
}

impl Model {
    fn place(&self) -> Place {
        self.place.clone().unwrap_or(Place::Tasks)
    }

    /// The query typed: the filter, or the place's own.
    fn query(&self) -> String {
        self.filter.clone().unwrap_or_else(|| self.place().query())
    }

    /// The query whose rows are shown: the last that read.
    fn shown_query(&self) -> String {
        self.read.clone().unwrap_or_else(|| self.place().query())
    }

    /// The rows shown in the place open, if it is a list of tasks and the store has arrived.
    fn items(&self) -> Option<Vec<Item>> {
        let store = self.store.as_ref()?;
        let rows = view::listing(&store.snapshot, &self.place(), &self.shown_query(), &store.now)?.ok()?;
        Some(list::items(&rows, &self.folded))
    }

    /// Keeps the row in hand where the person left it, after the list changed under it.
    fn settle(&mut self) {
        let Some(items) = self.items() else { return };
        match list::settle(&items, self.focus.as_deref(), self.position) {
            Some((id, at)) => {
                self.focus = Some(id);
                self.position = at;
            }
            None => self.focus = None,
        }
    }
}

/// The app.
#[derive(Debug, Default)]
pub struct Lumenna;

fn say(text: impl Into<String>) -> Command<Effect, Event> {
    let text = text.into();
    if text.is_empty() {
        return Command::done();
    }
    Command::notify_shell(Announce { text }).into()
}

fn host(request: HostRequest) -> Command<Effect, Event> {
    Command::request_from_shell(request).then_send(Event::Answered)
}

/// What a change says: its sentence, then anything else worth saying.
fn spoken(change: &Change) -> String {
    std::iter::once(change.announcement.as_str())
        .chain(change.notices.iter().map(String::as_str))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl Lumenna {
    fn perform(&self, action: Action, model: &mut Model) -> Command<Effect, Event> {
        match action.kind {
            // Folding is the app's: nothing stored changes.
            ActionKind::Expand | ActionKind::Collapse => {
                let said = if action.kind == ActionKind::Expand {
                    model.folded.remove(&action.target);
                    "Expanded"
                } else {
                    model.folded.insert(action.target.clone());
                    "Collapsed"
                };
                model.focus = Some(action.target);
                model.settle();
                return render().and(say(said));
            }
            _ => {}
        }
        match &action.question {
            Question::Immediate => host(HostRequest::Act { action, answer: Answer::Yes }),
            Question::Pick { .. } => {
                model.open = Some(Open { action: action.clone(), choices: None, refusal: None, picked: None });
                render().and(host(HostRequest::Choices { action }))
            }
            Question::Form | Question::Confirm { .. } | Question::Text { .. } | Question::Choose { .. } => {
                model.open = Some(Open { action, choices: None, refusal: None, picked: None });
                render()
            }
        }
    }

    fn wrote(&self, result: Result<Change, LumennaError>, store: Option<Arrival>, model: &mut Model) -> Command<Effect, Event> {
        if let Some(store) = store {
            model.store = Some(store);
        }
        let adding = std::mem::take(&mut model.adding);
        match result {
            Ok(change) => {
                model.open = None;
                // What it made is what the person works on next.
                if let Some(task) = &change.task {
                    model.focus = Some(task.id.clone());
                }
                model.settle();
                render().and(say(spoken(&change)))
            }
            Err(error) => {
                model.settle();
                if adding {
                    model.add_refused = Some(error.message().to_owned());
                }
                // A refused answer stays in its question, with the reason.
                if let Some(open) = &mut model.open {
                    open.refusal = Some(error.message().to_owned());
                    return render();
                }
                render().and(say(error.message()))
            }
        }
    }
}

impl App for Lumenna {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Start => Command::stream_from_shell(Watch).then_send(Event::Arrived),
            Event::Arrived(store) => {
                model.store = Some(store);
                model.settle();
                render()
            }
            Event::Open(place) => {
                model.place = Some(place);
                model.filter = None;
                model.read = None;
                model.focus = None;
                model.position = 0;
                model.open = None;
                model.settle();
                render()
            }
            Event::Focus(id) => {
                if let Some(at) = model.items().and_then(|items| list::position(&items, &id)) {
                    model.focus = Some(id);
                    model.position = at;
                }
                Command::done()
            }
            Event::Filter(text) => {
                model.filter = (!text.trim().is_empty()).then_some(text);
                let reads = model.store.as_ref().is_some_and(|store| {
                    lumenna_records::resolve::list_query(&store.snapshot, &model.query()).is_ok()
                });
                if reads {
                    model.read = model.filter.clone();
                }
                model.settle();
                render()
            }
            Event::Perform(action) => self.perform(action, model),
            Event::Run(kinds) => {
                let items = model.items().unwrap_or_default();
                let Some(item) = model.focus.as_ref().and_then(|id| items.iter().find(|item| &item.row.id == id)) else {
                    return Command::done();
                };
                let found = kinds.iter().find_map(|kind| item.row.actions.iter().find(|a| a.kind == *kind));
                match (found, kinds.first()) {
                    (Some(action), _) => self.perform(action.clone(), model),
                    (None, Some(kind)) => {
                        let subject = item.row.actions.first().map_or(lumenna_records::Subject::Task, |a| a.subject);
                        say(not_offered(*kind, subject, false))
                    }
                    (None, None) => Command::done(),
                }
            }
            Event::Answer(answer) => {
                let Some(open) = &mut model.open else { return Command::done() };
                // A pick that asks how long: the length is asked once one is picked, and both
                // go together.
                if let (Question::Pick { length: Some(_), .. }, Answer::Picked { id, length: None }, None) =
                    (&open.action.question, &answer, &open.picked)
                {
                    open.picked = Some(id.clone());
                    open.refusal = None;
                    return render();
                }
                let answer = match (&open.picked, answer) {
                    (Some(id), Answer::Text { text }) => Answer::Picked { id: id.clone(), length: Some(text) },
                    (_, answer) => answer,
                };
                host(HostRequest::Act { action: open.action.clone(), answer })
            }
            Event::Cancel => {
                model.open = None;
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
            Event::Answered(HostAnswer::Offered(result)) => {
                let Some(open) = &mut model.open else { return Command::done() };
                match result {
                    // Nothing to pick: the question closes, and why is all that is said.
                    Ok(choices) if choices.choices.is_empty() => {
                        model.open = None;
                        render().and(say(choices.announcement))
                    }
                    Ok(choices) => {
                        let said = choices.announcement.clone();
                        open.choices = Some(choices);
                        render().and(say(said))
                    }
                    Err(error) => {
                        model.open = None;
                        render().and(say(error.message()))
                    }
                }
            }
        }
    }

    fn view(&self, model: &Model) -> ViewModel {
        view::view(model)
    }
}
