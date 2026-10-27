//! Everything the screens hold, and how the rows of whatever is shown are found again.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use crux_core::Command;
use crux_core::render::render;
use jiff::civil::Date;
use lumenna_records::Action;
use lumenna_records::places::Place;

use crate::asking::Open;
use crate::host::Arrival;
use crate::{Announce, DayView, Effect, Event, Item, blockform, day, details, due, list, network, proposal, settings, typing, view};

/// A screen's identifier: the window, buffer or tab showing it. Screen 0 is every shell's first.
pub type ScreenId = u32;

/// Everything the app holds: its screens, each over what the device holds alike.
#[derive(Debug)]
pub struct Model {
    /// The screens open, by identifier; screen 0 from the start.
    pub screens: BTreeMap<ScreenId, Screen>,
}

impl Default for Model {
    fn default() -> Self {
        Self { screens: BTreeMap::from([(0, Screen::default())]) }
    }
}

impl Model {
    /// Screen `id`, opened if it is not open yet, over what the device holds as another screen
    /// has it.
    pub(crate) fn screen(&mut self, id: ScreenId) -> &mut Screen {
        if !self.screens.contains_key(&id) {
            let mut screen = Screen::default();
            if let Some(other) = self.screens.values().next() {
                screen.share(other);
            }
            self.screens.insert(id, screen);
        }
        self.screens.get_mut(&id).expect("just opened")
    }

    /// What the device holds alike, as screen `id` left it, given to every other screen, each
    /// keeping its rows in hand where they belong in the store as it now is: a task deleted on
    /// one screen is not left in hand on another.
    pub(crate) fn share(&mut self, id: ScreenId) {
        let Some(from) = self.screens.remove(&id) else { return };
        for screen in self.screens.values_mut() {
            let moved = screen.store != from.store;
            screen.share(&from);
            if moved {
                screen.settle();
                screen.settle_sides(false, None);
            }
        }
        self.screens.insert(id, from);
    }
}

/// Everything one screen holds: where the person is on it, and, alike on every screen, what
/// the device holds (`Screen::share`).
#[derive(Debug, Default)]
pub struct Screen {
    /// The store as last sent, once it has been. The device's.
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
    /// A quick-add line on its way to the host, by the write it went out as.
    pub adding: Option<u64>,
    /// How many writes this screen has sent: each is numbered, so its answer goes only to what
    /// waits on it.
    pub writes: u64,
    /// Reminders due being dealt with, by the write that deals with each: one leaves the list
    /// once that write is answered, and stays if it failed.
    pub dealing: Vec<(u64, String)>,
    /// Why the last quick-add line was refused: the line stays, to be put right.
    pub add_refused: Option<String>,
    /// What each place left held, put back when it opens again (`Screen::go`).
    pub left: HashMap<Place, Left>,
    /// The day shown, when it is not today; today follows the clock.
    pub day: Option<Date>,
    /// Whether today has been shown yet: the first time, it opens on now.
    pub day_opened: bool,
    /// The details of the task in hand.
    pub details: details::Details,
    /// The quick-add line, while quick add is open.
    pub line: Option<typing::Line>,
    /// The block form, while it is open.
    pub block_form: Option<blockform::Form>,
    /// A proposal's screen, while it is open.
    pub proposal: Option<proposal::Sheet>,
    /// The clock the shell says times in.
    pub style: Option<lumenna_records::proposals::TimeStyle>,
    /// How the shell writes its buttons, headings and questions.
    pub capitals: crate::capitals::Capitals,
    /// Whether the shell's list shows each task's checkbox.
    pub checkboxes: bool,
    /// What the shell is called in pairing's sentences, and whether it finds devices nearby.
    pub pairing_as: Option<(String, bool)>,
    /// What the shell keeps running in the background and whether it is on, where it offers it.
    pub background: Option<(lumenna_records::background::BackgroundKind, bool)>,
    /// What this screen asked the shell to turn background running to, until it answers.
    pub background_asked: Option<(lumenna_records::background::BackgroundKind, bool)>,
    /// Whether the proposal's screen last closed had made anything.
    pub proposal_made: bool,
    /// The task or block whose reminders are listed, while the list is open.
    pub reminders: Option<String>,
    /// Settings, while open.
    pub settings: Option<settings::Settings>,
    /// The Devices page, while shown.
    pub devices: Option<network::Devices>,
    /// A pairing, while its screen is open.
    pub pairing: Option<network::Pairing>,
    /// Whether this device keeps in sync now.
    pub syncing: bool,
    /// Whether keeping in sync has been asked for.
    pub sync_asked: bool,
    /// Whether keeping in sync failed to start: asked again once the person shows the devices or
    /// pairs one.
    pub sync_failed: bool,
    /// Reminders due, not yet dealt with.
    pub due: Vec<due::Due>,
    /// The moment proposals are made against, when the shell pins it (`Event::PlanFrom`).
    pub plan_from: Option<String>,
    /// Whether a look for reminders due is with the host now.
    pub looking: bool,
    /// The looks asked for meanwhile, as one: the earliest `since`, and the latest moment.
    pub look_next: Option<(String, Option<String>)>,
    /// An overrun warning whose Replan from Now is open, by its id, and the firing it is when the
    /// platform gave it itself: dismissed once something is replanned.
    pub replanning: Option<(String, Option<(String, String)>)>,
    /// Counted: each time the app moves focus itself, which is when a shell moves it.
    pub landing: u32,
    /// How many times the person has moved the row in hand themselves (`Event::Focus`).
    pub moves: u64,
    /// How many times they had moved it when they last did something: a write answered after
    /// a later move leaves the row in hand where they moved it.
    pub acted_at: u64,
    /// Sidebar rows folded away, by their key: a heading, or a project with projects inside.
    pub sidebar_folded: BTreeSet<String>,
    /// The sidebar's row in hand.
    pub sidebar_held: crate::held::Held,
    /// The open reminder list's row in hand.
    pub reminders_held: crate::held::Held,
    /// The devices list's row in hand.
    pub devices_held: crate::held::Held,
    /// The reminders due's row in hand.
    pub due_held: crate::held::Held,
    /// How many questions this screen has asked: each is numbered as it is shown.
    pub questions: u32,
    /// The side list the person last moved in, until they move in the main list or open a
    /// place: a change it makes lands there, not in the main list.
    pub acting: Option<crate::held::Side>,
    /// Whether this screen watches the store already: Start is taken once.
    pub watching: bool,
    /// What this screen read last from the store it holds: the listing and the day's plan.
    pub(crate) worked: Worked,
}

/// What a store was read for: the store itself (held, so no other store is ever taken for it),
/// the place, and the query or the day.
#[derive(Debug)]
pub(crate) struct Read {
    store: Arrival,
    place: Place,
    query: String,
}

impl Read {
    fn is(&self, store: &Arrival, place: &Place, query: &str) -> bool {
        &self.store == store && &self.place == place && self.query == query
    }
}

/// What a screen read last, kept until the store, the place or the query moves.
#[derive(Debug, Default)]
pub(crate) struct Worked {
    listing: Mutex<Option<ListingRead>>,
    plan: Mutex<Option<(Read, Arc<lumenna_records::types::Plan>)>>,
}

type ListingRead = (Read, Result<Arc<lumenna_records::Rows>, lumenna_records::LumennaError>);


/// The rows a place shows, as its keys and their actions.
pub(crate) struct Rows {
    /// The rows shown, folded ones left out: their keys and actions.
    pub(crate) shown: Vec<(String, Vec<Action>)>,
}


impl Screen {
    /// Takes what the device holds alike on every screen as `from` has it: the store, the
    /// shell's clock and capitals, what it keeps running in the background, the moment proposals
    /// are made against, reminders due and the look for them, and syncing.
    fn share(&mut self, from: &Screen) {
        self.store.clone_from(&from.store);
        self.style = from.style;
        self.capitals = from.capitals;
        self.checkboxes = from.checkboxes;
        self.pairing_as.clone_from(&from.pairing_as);
        self.background = from.background;
        self.plan_from.clone_from(&from.plan_from);
        self.due.clone_from(&from.due);
        self.looking = from.looking;
        self.look_next.clone_from(&from.look_next);
        self.syncing = from.syncing;
        self.sync_asked = from.sync_asked;
        self.sync_failed = from.sync_failed;
    }

    /// Pairing's words, as this shell is called in them.
    pub(crate) fn pairing_words(&self) -> lumenna_records::PairingWords {
        let (called, nearby) = self.pairing_as.clone().unwrap_or_else(|| ("this device".to_owned(), true));
        lumenna_records::pairing_words(called, nearby)
    }

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
    pub(crate) fn day_shown(&self) -> Option<Date> {
        let store = self.store.as_ref()?;
        Some(self.day.unwrap_or_else(|| store.now.date()))
    }

    /// The rows of the list shown, the task list's or a task place's.
    pub(crate) fn items(&self) -> Option<Vec<Item>> {
        let rows = self.listing()?.ok()?;
        Some(list::items(&rows, &self.folded))
    }

    /// The rows the place shown lists under the query shown, worked out once for each store
    /// that arrives: a turn asks for them several times, and a list of thousands of tasks costs
    /// a small device (the BTSpeak's) dearly each time. `None` for the day.
    pub(crate) fn listing(&self) -> Option<Result<Arc<lumenna_records::Rows>, lumenna_records::LumennaError>> {
        let store = self.store.as_ref()?;
        let place = self.place();
        let query = self.shown_query();
        let mut cached = self.worked.listing.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((read, rows)) = cached.as_ref()
            && read.is(store, &place, &query)
        {
            return Some(rows.clone());
        }
        let rows = view::listing(&store.snapshot, &place, &query, &store.now)?.map(Arc::new);
        *cached = Some((Read { store: store.clone(), place, query }, rows.clone()));
        Some(rows)
    }

    /// The plan of the day shown, worked out once for each store that arrives.
    fn plan(&self) -> Option<Arc<lumenna_records::types::Plan>> {
        let store = self.store.as_ref()?;
        let date = self.day_shown()?.to_string();
        let mut cached = self.worked.plan.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((read, plan)) = cached.as_ref()
            && read.is(store, &Place::Today, &date)
        {
            return Some(Arc::clone(plan));
        }
        let plan = Arc::new(lumenna_records::plan::plan(&store.snapshot, self.day_shown()?, &store.now).ok()?);
        *cached = Some((Read { store: store.clone(), place: Place::Today, query: date }, Arc::clone(&plan)));
        Some(plan)
    }

    /// The day shown, as rows.
    pub(crate) fn day_view(&self) -> Option<DayView> {
        let store = self.store.as_ref()?;
        let date = self.day_shown()?;
        let plan = self.plan()?;
        Some(day::day(&plan, date == store.now.date(), &self.folded))
    }

    /// The rows of whatever the place shows.
    pub(crate) fn rows(&self) -> Option<Rows> {
        let shown = if self.place() == Place::Today {
            self.day_view()?.items.into_iter().filter(|i| !i.hidden).map(|i| (i.key, i.actions)).collect()
        } else {
            self.items()?.into_iter().filter(|i| !i.hidden).map(|i| (i.row.id, i.row.actions)).collect()
        };
        Some(Rows { shown })
    }

    /// Keeps the row in hand where the person left it, after the list changed under it.
    pub(crate) fn settle(&mut self) {
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

    /// Goes to `place`. One left before opens again as it was left, its row in hand and its
    /// filter put back, so going to Today and back to a project returns to the same task;
    /// otherwise its rows from the top. Today opens on now the first time, after that on the row
    /// it was left on, or its summary, as any day, and always on today. Going to the place
    /// shown keeps its row in hand and its filter.
    pub(crate) fn go(&mut self, place: Place) {
        if let Some(from) = self.place.clone() {
            let left = Left { focus: self.focus.clone(), position: self.position, filter: self.filter.clone(), read: self.read.clone() };
            self.left.insert(from, left);
        }
        let kept = self.left.remove(&place).unwrap_or_default();
        let first_today = place == Place::Today && !self.day_opened;
        self.place = Some(place);
        self.filter = kept.filter;
        self.read = kept.read;
        self.focus = kept.focus;
        self.position = kept.position;
        self.day = None;
        self.details.shown = None;
        if first_today && let Some(view) = self.day_view() {
            self.day_opened = true;
            if let Some(key) = day::opening(&view) {
                self.hold(&key);
            }
        }
        self.settle();
    }

    /// Puts `key` in hand, if it is shown.
    pub(crate) fn hold(&mut self, key: &str) -> bool {
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

    /// The task the details show: one asked for, else the task in hand — a list's row, or on
    /// the day a sitting's task.
    pub(crate) fn details_task(&self) -> Option<String> {
        if let Some(task) = &self.details.shown {
            return Some(task.clone());
        }
        let focus = self.focus.as_ref()?;
        match self.place() {
            Place::Blocks => None,
            Place::Today => {
                let view = self.day_view()?;
                view.items.into_iter().find(|i| &i.key == focus)?.sitting.map(|s| s.task)
            }
            _ => Some(focus.clone()),
        }
    }

    /// The task the details show, as the store has it.
    pub(crate) fn details_shown(&self) -> Option<lumenna_records::types::TaskDetail> {
        let store = self.store.as_ref()?;
        let id = self.details_task()?;
        lumenna_records::plan::task_shown(&store.snapshot, &id, &store.now).ok().map(|shown| shown.task)
    }

    /// The actions of the row in hand.
    pub(crate) fn actions_in_hand(&self) -> Option<Vec<Action>> {
        let focus = self.focus.as_ref()?;
        self.rows()?.shown.into_iter().find(|(key, _)| key == focus).map(|(_, actions)| actions)
    }

    /// Shows a day: Today shows today, and a new day opens on its summary, which it says.
    pub(crate) fn show_day(&mut self, day: Option<Date>) -> Command<Effect, Event> {
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
        render().and(Command::notify_shell(Announce { text: summary, heading: true, screen: 0 }).into())
    }
}

/// What a place held when the person left it: the row in hand, where it was, and the filter.
#[derive(Debug, Default, Clone)]
pub struct Left {
    /// The row in hand.
    pub focus: Option<String>,
    /// Where it was in the rows shown, for whatever takes its place if it has gone since.
    pub position: usize,
    /// The filter typed above the list.
    pub filter: Option<String>,
    /// The last filter typed that read.
    pub read: Option<String>,
}
