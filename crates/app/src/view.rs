//! What each app draws: the model read into records.

use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use lumenna_records::places::{Place, SidebarEntry, sidebar};
use lumenna_records::{Action, Choices, LumennaError, Query, Question, Rows, length_question, lists};
use serde::{Deserialize, Serialize};

use crate::list::{self, Item};
use crate::Model;

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
    /// The list shown, for a place that lists tasks.
    pub list: Option<ListView>,
    /// The row in hand, by identifier: where the shell puts focus when it changes.
    pub focus: Option<String>,
    /// The question an action is asking.
    pub asking: Option<Asking>,
    /// What a new task typed here starts with: `#Work ` in the Work project.
    pub quick_add: String,
}

/// A list of tasks as shown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ListView {
    /// The rows shown.
    pub items: Vec<Item>,
    /// What the list says when it has none.
    pub empty: String,
    /// How the query was understood, for a filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<Query>,
    /// Anything wrong with the query: a project it names that does not exist.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Why the query could not be read, in place of rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// An action's question, open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Asking {
    /// The action asking.
    pub action: Action,
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

/// The tasks a place lists, or `None` for a place that is not a list of tasks.
pub fn listing(snapshot: &Snapshot, place: &Place, now: &Zoned) -> Option<Result<Rows, LumennaError>> {
    match place {
        Place::Today | Place::Blocks => None,
        _ => Some(lists::tasks(snapshot, &place.query(), now)),
    }
}

pub fn view(model: &Model) -> ViewModel {
    let place = model.place();
    let Some(store) = &model.store else {
        return ViewModel { place: Some(place.clone()), title: place.title(), ..ViewModel::default() };
    };
    let list = listing(&store.snapshot, &place, &store.now).map(|found| match found {
        Ok(rows) => ListView {
            items: list::items(&rows, &model.folded),
            empty: rows.empty.clone(),
            query: rows.query.clone(),
            notices: rows.notices.clone(),
            refusal: None,
        },
        Err(error) => ListView { refusal: Some(error.message().to_owned()), ..ListView::default() },
    });
    ViewModel {
        ready: true,
        sidebar: sidebar(&store.snapshot, &store.now),
        title: place.title(),
        quick_add: place.quick_add_prefix(),
        place: Some(place),
        list,
        focus: model.focus.clone(),
        asking: model.open.as_ref().map(|open| Asking {
            action: open.action.clone(),
            question: if open.picked.is_some() { length_question() } else { open.action.question.clone() },
            choices: open.choices.clone(),
            refusal: open.refusal.clone(),
        }),
    }
}
