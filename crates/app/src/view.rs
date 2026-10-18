//! What each app draws: the model read into records.

use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use lumenna_records::places::{Place, SidebarEntry, sidebar};
use lumenna_records::{Action, Choices, LumennaError, Query, Question, Rows, lists};
use serde::{Deserialize, Serialize};

use crate::list::{self, Item};
use crate::{DayView, DetailsView, LineView, Model};

/// Everything on screen.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ViewModel {
    /// Whether the store has arrived: until it has there is nothing to show.
    pub ready: bool,
    /// The places, as a sidebar lists them.
    pub sidebar: Vec<SidebarEntry>,
    /// Where the person is.
    pub place: Option<Place>,
    /// What the window and the list are called.
    pub title: String,
    /// The list shown, for a place that lists tasks or blocks.
    pub list: Option<ListView>,
    /// The day shown, on Today.
    pub day: Option<DayView>,
    /// The row in hand, by identifier: where the shell puts focus when it changes.
    pub focus: Option<String>,
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
}

/// A list of tasks as shown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ListView {
    /// Every row, those folded away marked hidden.
    pub items: Vec<Item>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
}

/// The rows a place lists under `query`: tasks, or the block series; `None` for the day.
pub fn listing(snapshot: &Snapshot, place: &Place, query: &str, now: &Zoned) -> Option<Result<Rows, LumennaError>> {
    match place {
        Place::Today => None,
        Place::Blocks => Some(lumenna_records::plan::blocks(snapshot, now.date())),
        _ => Some(lists::tasks(snapshot, query, now)),
    }
}

pub fn view(model: &Model) -> ViewModel {
    let place = model.place();
    let Some(store) = &model.store else {
        return ViewModel { place: Some(place.clone()), title: place.title(), ..ViewModel::default() };
    };
    // The rows of the last query that read; why the one typed does not, if it does not.
    let refusal = (place != Place::Blocks)
        .then(|| lumenna_records::resolve::list_query(&store.snapshot, &model.query()).err().map(|e| e.message().to_owned()))
        .flatten();
    let list = listing(&store.snapshot, &place, &model.shown_query(), &store.now).map(|found| match found {
        Ok(rows) => ListView {
            items: list::items(&rows, &model.folded),
            announcement: rows.announcement.clone(),
            empty: rows.empty.clone(),
            query: rows.query.clone(),
            notices: rows.notices.clone(),
            refusal,
        },
        Err(error) => ListView { refusal: Some(error.message().to_owned()), ..ListView::default() },
    });
    ViewModel {
        ready: true,
        sidebar: sidebar(&store.snapshot, &store.now),
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
                task,
                form: lumenna_records::task_form(),
                projects: lumenna_records::tree::project_options(&store.snapshot),
            }
        }),
        block_form: model.block_form.as_ref().map(crate::blockform::Form::view),
        proposal: model.proposal.as_ref().map(crate::proposal::view),
        proposal_made: model.proposal_made,
        line: model.line.as_ref().map(|line| {
            let preview = lumenna_records::text::preview(&store.snapshot, &line.text, &store.now);
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
        asking: model.open.as_ref().map(|open| Asking {
            action: open.action().cloned(),
            question: open.question(),
            choices: open.choices.clone(),
            refusal: open.refusal.clone(),
        }),
    }
}
