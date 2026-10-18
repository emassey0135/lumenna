//! Everything the screens hold, and how the rows of whatever is shown are found again.

use std::collections::BTreeSet;

use crux_core::Command;
use crux_core::render::render;
use jiff::civil::Date;
use lumenna_records::Action;
use lumenna_records::places::Place;

use crate::asking::Open;
use crate::host::Arrival;
use crate::{Announce, DayView, Effect, Event, Item, blockform, day, details, due, list, network, proposal, settings, typing, view};

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
    /// The details of the task in hand.
    pub details: details::Details,
    /// The quick-add line, while quick add is open.
    pub line: Option<typing::Line>,
    /// The block form, while it is open.
    pub block_form: Option<blockform::Form>,
    /// A proposal's screen, while it is open.
    pub proposal: Option<proposal::Screen>,
    /// The clock the shell says times in.
    pub style: Option<lumenna_records::proposals::TimeStyle>,
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
    /// Reminders due, not yet dealt with.
    pub due: Vec<due::Due>,
    /// The moment proposals are made against, when the shell pins it (`Event::PlanFrom`).
    pub plan_from: Option<String>,
    /// Whether a look for reminders due is with the host now.
    pub looking: bool,
    /// The looks asked for meanwhile, as one: the earliest `since`, and the latest moment.
    pub look_next: Option<(String, Option<String>)>,
    /// An overrun warning whose Replan from Now is open: dismissed once something is replanned.
    pub replanning: Option<String>,
    /// Counted: each time the app moves focus itself, which is when a shell moves it.
    pub landing: u32,
    /// How many times the person has moved the row in hand themselves (`Event::Focus`).
    pub moves: u64,
    /// How many times they had moved it when they last did something: a write answered after
    /// a later move leaves the row in hand where they moved it.
    pub acted_at: u64,
    /// Sidebar rows folded away, by their key: a heading, or a project with projects inside.
    pub sidebar_folded: BTreeSet<String>,
}


/// The rows a place shows, as its keys and their actions.
pub(crate) struct Rows {
    /// The rows shown, folded ones left out: their keys and actions.
    pub(crate) shown: Vec<(String, Vec<Action>)>,
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
    pub(crate) fn day_shown(&self) -> Option<Date> {
        let store = self.store.as_ref()?;
        Some(self.day.unwrap_or_else(|| store.now.date()))
    }

    /// The rows of the list shown, the task list's or a task place's.
    pub(crate) fn items(&self) -> Option<Vec<Item>> {
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

    /// Goes to `place`, its rows from the top; Today opens on now the first time, after that
    /// on its summary, as any day.
    pub(crate) fn go(&mut self, place: Place) {
        let first_today = place == Place::Today && !self.day_opened;
        self.place = Some(place);
        self.filter = None;
        self.read = None;
        self.focus = None;
        self.position = 0;
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
        render().and(Command::notify_shell(Announce { text: summary, heading: true }).into())
    }
}
