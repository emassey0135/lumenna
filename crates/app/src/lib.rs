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

pub mod day;
pub mod host;
pub mod list;
mod view;

use std::collections::BTreeSet;

use crux_core::render::render;
use crux_core::{App, Command};
use jiff::civil::Date;
use lumenna_records::places::Place;
use lumenna_records::{Action, ActionKind, Answer, Change, Choices, LumennaError, Question, Subject, not_offered};
use serde::{Deserialize, Serialize};

pub use day::{DayItem, DayKind, DayView};
pub use host::{Arrival, HostAnswer, HostRequest, Watch};
pub use list::Item;
pub use view::{Asking, ListView, ViewModel};

/// Something to say, once the shell has moved focus where the view model puts it: said
/// before, it is cut off by the focus change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announce {
    /// The sentence.
    pub text: String,
    /// Say the day's heading instead — "<day>. <summary>", the day in the shell's own words —
    /// `text` being the summary: what a new day shown says.
    pub heading: bool,
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

/// Moving through the days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
    /// Move through the days, going to the day if another place is shown.
    Day(DayMove),
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

/// What a question open is for.
#[derive(Debug, Clone, PartialEq)]
// One at a time, in the model: the size of the largest is no cost worth a box.
#[allow(clippy::large_enum_variant)]
pub enum Purpose {
    /// A row's action asking before it runs.
    Action(Action),
    /// Which day to go to.
    GoToDay,
}

/// A question open.
#[derive(Debug, Clone, PartialEq)]
pub struct Open {
    /// What it is for.
    pub purpose: Purpose,
    /// What a pick offers, once the host has said.
    pub choices: Option<Choices>,
    /// Why the last answer was refused: the question stays open with it.
    pub refusal: Option<String>,
    /// What was picked, for a pick that asks a second question once one is: how long a
    /// sitting is meant to take.
    pub picked: Option<String>,
}

impl Open {
    fn new(purpose: Purpose) -> Self {
        Self { purpose, choices: None, refusal: None, picked: None }
    }

    /// The action asking, if it is one.
    pub(crate) fn action(&self) -> Option<&Action> {
        match &self.purpose {
            Purpose::Action(action) => Some(action),
            Purpose::GoToDay => None,
        }
    }

    /// What to ask now.
    pub(crate) fn question(&self) -> Question {
        match &self.purpose {
            Purpose::Action(_) if self.picked.is_some() => lumenna_records::length_question(),
            Purpose::Action(action) => action.question.clone(),
            Purpose::GoToDay => lumenna_records::go_to_day_question(),
        }
    }
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
    /// The day shown, when it is not today; today follows the clock.
    pub day: Option<Date>,
    /// Whether today has been shown yet: the first time, it opens on now.
    pub day_opened: bool,
}

/// The rows a place shows, as its keys and their actions.
struct Rows {
    /// The rows shown, folded ones left out: their keys and actions.
    shown: Vec<(String, Vec<Action>)>,
}

impl Model {
    pub(crate) fn place(&self) -> Place {
        self.place.clone().unwrap_or(Place::Tasks)
    }

    /// The query typed: the filter, or the place's own.
    pub(crate) fn query(&self) -> String {
        self.filter.clone().unwrap_or_else(|| self.place().query())
    }

    /// The query whose rows are shown: the last that read.
    pub(crate) fn shown_query(&self) -> String {
        self.read.clone().unwrap_or_else(|| self.place().query())
    }

    /// The day shown on Today, at the moment the store was read.
    fn day_shown(&self) -> Option<Date> {
        let store = self.store.as_ref()?;
        Some(self.day.unwrap_or_else(|| store.now.date()))
    }

    /// The rows of the list shown, the task list's or a task place's.
    fn items(&self) -> Option<Vec<Item>> {
        let store = self.store.as_ref()?;
        let rows = view::listing(&store.snapshot, &self.place(), &self.shown_query(), &store.now)?.ok()?;
        Some(list::items(&rows, &self.folded))
    }

    /// The day shown, as rows.
    pub(crate) fn day_view(&self) -> Option<DayView> {
        let store = self.store.as_ref()?;
        let date = self.day_shown()?;
        let plan = lumenna_records::plan::plan(&store.snapshot, date, &store.now).ok()?;
        Some(day::day(&plan, date == store.now.date(), &self.folded))
    }

    /// The rows of whatever the place shows.
    fn rows(&self) -> Option<Rows> {
        let shown = if self.place() == Place::Today {
            self.day_view()?.items.into_iter().filter(|i| !i.hidden).map(|i| (i.key, i.actions)).collect()
        } else {
            self.items()?.into_iter().filter(|i| !i.hidden).map(|i| (i.row.id, i.row.actions)).collect()
        };
        Some(Rows { shown })
    }

    /// Keeps the row in hand where the person left it, after the list changed under it.
    fn settle(&mut self) {
        let Some(rows) = self.rows() else { return };
        let keys: Vec<&String> = rows.shown.iter().map(|(key, _)| key).collect();
        if let Some(at) = self.focus.as_ref().and_then(|focus| keys.iter().position(|key| *key == focus)) {
            self.position = at;
            return;
        }
        match keys.len().checked_sub(1) {
            Some(last) => {
                self.position = self.position.min(last);
                self.focus = Some(keys[self.position].clone());
            }
            None => self.focus = None,
        }
    }

    /// Puts `key` in hand, if it is shown.
    fn hold(&mut self, key: &str) -> bool {
        let Some(rows) = self.rows() else { return false };
        match rows.shown.iter().position(|(k, _)| k == key) {
            Some(at) => {
                self.focus = Some(key.to_owned());
                self.position = at;
                true
            }
            None => false,
        }
    }

    /// The actions of the row in hand.
    fn actions_in_hand(&self) -> Option<Vec<Action>> {
        let focus = self.focus.as_ref()?;
        self.rows()?.shown.into_iter().find(|(key, _)| key == focus).map(|(_, actions)| actions)
    }

    /// Shows a day: Today shows today, and a new day opens on its summary, which it says.
    fn show_day(&mut self, day: Option<Date>) -> Command<Effect, Event> {
        let changed = self.place() != Place::Today || self.day != day;
        self.place = Some(Place::Today);
        self.day = day;
        self.open = None;
        if !changed {
            return render();
        }
        self.focus = None;
        self.position = 0;
        self.settle();
        let summary = self.day_view().map(|view| view.summary).unwrap_or_default();
        render().and(Command::notify_shell(Announce { text: summary, heading: true }).into())
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
    Command::notify_shell(Announce { text, heading: false }).into()
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
                model.open = Some(Open::new(Purpose::Action(action.clone())));
                render().and(host(HostRequest::Choices { action }))
            }
            Question::Form | Question::Confirm { .. } | Question::Text { .. } | Question::Choose { .. } => {
                model.open = Some(Open::new(Purpose::Action(action)));
                render()
            }
        }
    }

    fn answer(&self, answer: Answer, model: &mut Model) -> Command<Effect, Event> {
        let Some(open) = &mut model.open else { return Command::done() };
        match open.purpose.clone() {
            Purpose::GoToDay => {
                let Answer::Text { text } = answer else { return Command::done() };
                let Some(store) = &model.store else { return Command::done() };
                match lumenna_records::resolve::date(Some(&text), &store.now) {
                    Ok(date) => {
                        let today = date == store.now.date();
                        model.show_day((!today).then_some(date))
                    }
                    Err(error) => {
                        open.refusal = Some(error.message().to_owned());
                        render()
                    }
                }
            }
            Purpose::Action(action) => {
                // A pick that asks how long: the length is asked once one is picked, and both
                // go together.
                if let (Question::Pick { length: Some(_), .. }, Answer::Picked { id, length: None }, None) =
                    (&action.question, &answer, &open.picked)
                {
                    open.picked = Some(id.clone());
                    open.refusal = None;
                    return render();
                }
                let answer = match (&open.picked, answer) {
                    (Some(id), Answer::Text { text }) => Answer::Picked { id: id.clone(), length: Some(text) },
                    (_, answer) => answer,
                };
                host(HostRequest::Act { action, answer })
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
                    model.hold(&task.id);
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

    fn arrived(&self, store: Arrival, model: &mut Model) -> Command<Effect, Event> {
        model.store = Some(store);
        model.settle();
        render()
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
            Event::Arrived(store) => self.arrived(store, model),
            Event::Open(place) => {
                let first_today = place == Place::Today && !model.day_opened;
                model.place = Some(place);
                model.filter = None;
                model.read = None;
                model.focus = None;
                model.position = 0;
                model.open = None;
                model.day = None;
                // Today opens on now the first time; after that on its summary, as any day.
                if first_today && let Some(view) = model.day_view() {
                    model.day_opened = true;
                    if let Some(key) = day::opening(&view) {
                        model.hold(&key);
                    }
                }
                model.settle();
                render()
            }
            Event::Focus(key) => {
                model.hold(&key);
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
                let Some(actions) = model.actions_in_hand() else { return Command::done() };
                let found = kinds.iter().find_map(|kind| actions.iter().find(|a| a.kind == *kind));
                match (found, kinds.first()) {
                    (Some(action), _) => self.perform(action.clone(), model),
                    (None, Some(kind)) => {
                        let subject = actions.first().map_or(Subject::Task, |a| a.subject);
                        say(not_offered(*kind, subject, false))
                    }
                    (None, None) => Command::done(),
                }
            }
            Event::Answer(answer) => self.answer(answer, model),
            Event::Cancel => {
                model.open = None;
                render()
            }
            Event::Day(step) => {
                let Some(store) = &model.store else { return Command::done() };
                let today = store.now.date();
                let shown = if model.place() == Place::Today { model.day.unwrap_or(today) } else { today };
                match step {
                    DayMove::Previous | DayMove::Next => {
                        let days = if step == DayMove::Next { 1 } else { -1 };
                        let Ok(to) = shown.checked_add(jiff::Span::new().days(days)) else { return Command::done() };
                        model.show_day((to != today).then_some(to))
                    }
                    DayMove::Today => model.show_day(None),
                    DayMove::Now => {
                        let shown = model.show_day(None);
                        if let Some(key) = model.day_view().as_ref().and_then(day::opening) {
                            model.hold(&key);
                        }
                        shown
                    }
                    DayMove::Ask => {
                        model.open = Some(Open::new(Purpose::GoToDay));
                        render()
                    }
                }
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
