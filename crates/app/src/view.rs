//! What each app draws: the model read into records.

use lumenna_records::reminders::ReminderList;
use lumenna_records::Syntax;
use lumenna_records::Offered;
use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use lumenna_records::places::{Place, SidebarEntry, sidebar};
use lumenna_records::{Action, Choices, LumennaError, Query, Question, Rows, lists};
use serde::{Deserialize, Serialize};

use crate::list::{self, Item};
use crate::{DayView, DetailsView, LineView, Screen};

/// Everything on screen.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewModel {
    /// Whether the store has arrived: until it has there is nothing to show.
    pub ready: bool,
    /// The places, as a sidebar lists them: a heading or a project with projects inside has
    /// Expand or Collapse among its actions.
    pub sidebar: Vec<SidebarEntry>,
    /// The sidebar's rows folded away, by key; a tree control folds them itself, a flat list
    /// leaves out what is under them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidebar_folded: Vec<String>,
    /// Where the person is.
    pub place: Option<Place>,
    /// What the window and the list are called.
    pub title: String,
    /// The list shown, for a place that lists tasks or blocks.
    pub list: Option<ListView>,
    /// The day shown, on Today.
    pub day: Option<DayView>,
    /// The row in hand, by identifier.
    pub focus: Option<String>,
    /// The sidebar's row in hand.
    pub sidebar_held: crate::HeldView,
    /// The open reminder list's row in hand.
    pub reminders_held: crate::HeldView,
    /// The devices list's row in hand.
    pub devices_held: crate::HeldView,
    /// Counted: when this moves, the shell puts focus on `focus` — if focus is in that list, or
    /// lost, or coming back to it from a dialog. It does not move for the person's own moves.
    pub landing: u32,
    /// The question an action is asking.
    pub asking: Option<Asking>,
    /// What a new task typed here starts with: `#Work ` in the Work project.
    pub quick_add: String,
    /// The filter typed above the list, or the place's own query.
    pub query: String,
    /// Why the last quick-add line was refused, for the line to stay with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add_refused: Option<String>,
    /// The details of the task in hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<DetailsView>,
    /// The quick-add line, while quick add is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineView>,
    /// The block form, while it is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_form: Option<crate::BlockFormView>,
    /// A proposal's screen, while it is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<crate::ProposalView>,
    /// Whether the proposal's screen last closed had made anything.
    pub proposal_made: bool,
    /// Settings, while open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<crate::SettingsView>,
    /// The Devices page, while shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<crate::DevicesView>,
    /// Reminders due, not yet dealt with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub due: Vec<crate::Due>,
    /// A task's or block's reminders, while their list is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminders: Option<ReminderList>,
}

/// A list of tasks as shown.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListView {
    /// Every row, those folded away marked hidden.
    pub items: Vec<Item>,
    /// The actions the rows offer, each once, in short: each row names its own by place here
    /// (`Item::actions`), and a shell hands one back with `Event::ActOn`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<Offered>,
    /// How many there are, in a sentence: "3 tasks".
    pub announcement: String,
    /// What the list says when it has none.
    pub empty: String,
    /// How the query was understood, for a filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<Query>,
    /// Anything wrong with the query: a project it names that does not exist.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Why the filter typed does not read yet: the rows are the last that did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// A question open.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asking {
    /// The action asking, when it is a row's action that asks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// What to ask now: the action's question, or, once a pick that asks how long has its
    /// choice, how long.
    pub question: Question,
    /// What a pick offers, once known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Choices>,
    /// Why the last answer was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// When the question asks for a day (Go to Day, Another Day's), the day, ISO, it starts
    /// from: the day shown, else today. A shell offers its own calendar beside the line, open
    /// on it, a day picked there answered as its ISO date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<String>,
    /// When the answer is written in the filter language (a saved filter's query), which a
    /// shell offers completion in, as it does in the list's filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<Syntax>,
}

/// The rows a place lists under `query`: tasks, or the block series; `None` for the day.
pub fn listing(snapshot: &Snapshot, place: &Place, query: &str, now: &Zoned) -> Option<Result<Rows, LumennaError>> {
    match place {
        Place::Today => None,
        Place::Blocks => Some(lumenna_records::plan::blocks(snapshot, now.date())),
        Place::History => Some(lumenna_records::lists::history(snapshot, now)),
        _ => Some(lists::tasks(snapshot, query, now)),
    }
}

/// The sidebar with Expand or Collapse on each row that has rows under it.
fn folded_sidebar(mut entries: Vec<SidebarEntry>, folded: &std::collections::BTreeSet<String>) -> Vec<SidebarEntry> {
    let depths: Vec<u32> = entries.iter().map(|e| e.depth).collect();
    for (at, entry) in entries.iter_mut().enumerate() {
        if depths.get(at + 1).is_some_and(|next| *next > entry.depth) {
            let key = entry.key();
            let (kind, title) = if folded.contains(&key) {
                (lumenna_records::ActionKind::Expand, "Expand")
            } else {
                (lumenna_records::ActionKind::Collapse, "Collapse")
            };
            let subject = entry.actions.first().map_or(lumenna_records::Subject::Project, |a| a.subject);
            let mut fold = Action::new(kind, subject, title, key);
            fold.question = Question::Immediate;
            entry.actions.push(fold);
        }
    }
    entries
}

pub fn view(model: &Screen) -> ViewModel {
    let mut view = whole(model);
    offer(&mut view);
    view
}

/// The view with every row's actions whole: where the app finds an action handed back in short.
pub(crate) fn whole(model: &Screen) -> ViewModel {
    let mut view = shown(model);
    if model.capitals == crate::Capitals::Sentence {
        crate::capitals::in_sentence_case(&mut view);
    }
    view
}

/// Every row's actions as the view offers them, in short (`Offered`): a shell hands one back
/// with `Event::ActOn`, naming its row, and the app runs that row's action whole as it has it.
/// A list's rows name theirs by place in the list (`ListView::offered`, made with the list).
/// Every row's actions in short, and none carried whole: a shell hands one back with `ActOn`,
/// and the app runs that row's own as it has it then (`whole`). The reminder list's own Add
/// Reminder stays whole beside its short form, its question's words being the field a shell
/// draws for it.
fn offer(view: &mut ViewModel) {
    let short = |actions: &mut Vec<lumenna_records::Action>| std::mem::take(actions).iter().map(Offered::of).collect::<Vec<_>>();
    for entry in &mut view.sidebar {
        entry.offered = short(&mut entry.actions);
    }
    if let Some(day) = &mut view.day {
        day.offered = short(&mut day.actions);
        for item in &mut day.items {
            item.offered = short(&mut item.actions);
            if let Some(block) = &mut item.block {
                block.actions.clear();
            }
            if let Some(sitting) = &mut item.sitting {
                sitting.actions.clear();
            }
        }
    }
    if let Some(details) = &mut view.details {
        details.task.offered = short(&mut details.task.actions);
    }
    if let Some(reminders) = &mut view.reminders {
        reminders.offered = reminders.actions.iter().map(Offered::of).collect();
        for reminder in &mut reminders.reminders {
            reminder.offered = short(&mut reminder.actions);
        }
    }
    if let Some(list) = view.devices.as_mut().and_then(|devices| devices.list.as_mut()) {
        for device in &mut list.devices {
            device.offered = short(&mut device.actions);
        }
    }
}

fn shown(model: &Screen) -> ViewModel {
    let place = model.place();
    let Some(store) = &model.store else {
        return ViewModel { place: Some(place.clone()), title: place.title(), ..ViewModel::default() };
    };
    // The rows of the last query that read; why the one typed does not, if it does not.
    let refusal = (place != Place::Blocks)
        .then(|| lumenna_records::resolve::list_query(&store.snapshot, &model.query()).err().map(|e| e.message().to_owned()))
        .flatten();
    let list = listing(&store.snapshot, &place, &model.shown_query(), &store.now).map(|found| match found {
        Ok(rows) => {
            let mut items = list::items(&rows, &model.folded);
            let offered = list::offer(&mut items);
            // Named in short, each once: the rows carry none whole, which the app finds again
            // when `Event::ActOn` names the row (a list of 2,000 tasks carried 15,372).
            for item in &mut items {
                item.row.actions.clear();
            }
            ListView {
            items,
            offered,
            announcement: rows.announcement.clone(),
            empty: rows.empty.clone(),
            query: rows.query.clone(),
            notices: rows.notices.clone(),
            refusal,
        }
        }
        Err(error) => ListView { refusal: Some(error.message().to_owned()), ..ListView::default() },
    });
    ViewModel {
        ready: true,
        sidebar: folded_sidebar(sidebar(&store.snapshot, &store.now), &model.sidebar_folded),
        sidebar_folded: model.sidebar_folded.iter().cloned().collect(),
        title: place.title(),
        quick_add: place.quick_add_prefix(),
        query: model.query(),
        add_refused: model.add_refused.clone(),
        day: (place == Place::Today).then(|| model.day_view()).flatten(),
        details: model.details_shown().map(|task| {
            let draft = model.details.draft.as_ref().filter(|d| d.task == task.id);
            DetailsView {
                fields: draft.map_or_else(|| lumenna_records::task_fields(task.clone()), |d| d.fields.clone()),
                dirty: draft.is_some_and(|d| d.fields != lumenna_records::task_fields(d.original.clone())),
                projects: projects_for(&task, &store.snapshot),
                task,
                form: lumenna_records::task_form(),
            }
        }),
        block_form: model.block_form.as_ref().map(crate::blockform::Form::view),
        proposal: model.proposal.as_ref().map(crate::proposal::view),
        proposal_made: model.proposal_made,
        settings: model.settings.as_ref().map(|s| crate::settings::view(s, model.background, store.now.date())),
        devices: model.devices.as_ref().map(|d| crate::DevicesView {
            words: model.pairing_words(),
            list: d.list.clone(),
            status: d.status.clone().unwrap_or_default(),
            syncing: model.syncing,
            pairing: model.pairing.as_ref().map(crate::network::Pairing::view),
        }),
        due: model.due.clone(),
        reminders: model
            .reminders
            .as_ref()
            .and_then(|target| lumenna_records::reminders::reminders_of(&store.snapshot, target).ok()),
        line: model.line.as_ref().map(|line| {
            let style = model.style.unwrap_or(crate::proposal::default_style());
            let preview = lumenna_records::text::preview_in(&store.snapshot, &line.text, &store.now, style);
            crate::typing::LineView {
                text: line.text.clone(),
                cursor: line.cursor,
                readback: if line.text.trim().is_empty() { String::new() } else { crate::typing::readback(&preview) },
                has_errors: line.text.trim().is_empty() || preview.has_errors,
                offer: lumenna_records::text::completions(
                    &store.snapshot,
                    &line.text,
                    line.cursor,
                    lumenna_records::types::Syntax::QuickAdd,
                ),
            }
        }),
        place: Some(place),
        list,
        focus: model.focus.clone(),
        sidebar_held: model.sidebar_held.view(),
        reminders_held: model.reminders_held.view(),
        devices_held: model.devices_held.view(),
        landing: model.landing,
        asking: model.open.as_ref().map(|open| asking(open, model)),
    }
}

/// The question open, as shown. Put in a Block's pick ends in Another Day, the app's own choice:
/// picked, it asks which day, and offers that day's blocks.
fn asking(open: &crate::Open, model: &Screen) -> Asking {
    let mut choices = open.choices.clone();
    let picking = matches!(open.purpose, crate::Purpose::Action(_)) && open.picked.is_none();
    if picking
        && open.action().is_some_and(|action| action.kind == lumenna_records::ActionKind::PutInBlock)
        && let Some(choices) = &mut choices
    {
        choices.choices.push(lumenna_records::Choice {
            id: crate::asking::ANOTHER_DAY.to_owned(),
            title: "Another Day".to_owned(),
            ..lumenna_records::Choice::default()
        });
    }
    let day = matches!(open.purpose, crate::Purpose::GoToDay | crate::Purpose::PickDay(_))
        .then(|| model.day_shown())
        .flatten()
        .map(|day| day.to_string());
    // A saved filter's query, new or changed, is the filter language.
    let query = match &open.purpose {
        crate::Purpose::NewFilter(Some(_)) => true,
        _ => open.action().is_some_and(|a| a.kind == lumenna_records::ActionKind::ChangeQuery),
    };
    let syntax = query.then_some(Syntax::Filter);
    Asking { action: open.action().cloned(), question: open.question(), choices, refusal: open.refusal.clone(), day, syntax }
}

/// The projects a task's Project field offers: every one not archived, in tree order, and the
/// task's own when it is archived, so the field does not lose it.
fn projects_for(task: &lumenna_records::types::TaskDetail, snapshot: &lumenna_core::snapshot::Snapshot) -> Vec<lumenna_records::Choice> {
    let mut projects = lumenna_records::tree::project_options(snapshot);
    if let Some(own) = &task.project
        && !projects.iter().any(|project| &project.id == own)
    {
        projects.push(lumenna_records::Choice { id: own.clone(), title: own.clone(), ..lumenna_records::Choice::default() });
    }
    projects
}
