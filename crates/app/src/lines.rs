//! A view model's lines in this device's clock: every row, the day, what a pick offers, the
//! devices, the quick-add offer. The app sends components; a shell that shows lines asks here
//! rather than joining them itself, so the web and Android read every line alike.

use crate::{DayKind, ViewModel};
use lumenna_records::places::Place;

use crate::speech::{self, Clock, Line};

/// What a view reads as, line by line, in the order its records come.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Lines {
    /// Each of the list's rows.
    pub list: Vec<Line>,
    /// What the list says of itself: the query described, its count, its notices.
    pub readback: String,
    /// The day shown, as a person says it: "Today", "Monday 5 October".
    pub day_title: String,
    /// Each of the day's rows, its heading first: "Today. 2 blocks, 3 hours of work".
    pub day: Vec<Line>,
    /// Each choice the open question offers.
    pub choices: Vec<Line>,
    /// Each paired device.
    pub devices: Vec<Line>,
    /// Each reminder of the open reminder list: when it fires, and whether it is a default.
    pub reminders: Vec<Line>,
    /// Each completion the quick-add line offers: "Work, project".
    pub candidates: Vec<String>,
    /// The task in hand's state, for its details.
    pub task_state: String,
}

/// How a list's query was understood and what it found, or why it is not one yet: the
/// refusal, the query described, the count (or, with nothing listed, the listing's own words
/// for that), then the notices, which name what the query could not find in the core's words.
/// Each is a sentence, joined as the core joins an announcement and its notices.
fn readback(list: &crate::ListView, trash: bool) -> String {
    let query = list.query.as_ref();
    let found = if list.items.is_empty() && !list.empty.is_empty() { &list.empty } else { &list.announcement };
    let parts: Vec<String> = list
        .refusal
        .iter()
        .cloned()
        .chain(query.filter(|_| !trash).map(|query| query.description.clone()))
        .chain([found.clone()])
        .chain(list.notices.iter().cloned())
        .filter(|part| !part.trim().is_empty())
        .map(|part| {
            let part = speech::sentence(part.trim());
            if part.ends_with(['.', '?', '!']) { part } else { format!("{part}.") }
        })
        .collect();
    lumenna_records::words::spoken("", &parts)
}

/// `view`'s lines, in `clock`.
pub fn lines(view: &ViewModel, clock: &dyn Clock) -> Lines {
    let trash = view.place == Some(Place::Trash);
    let list = view.list.as_ref();
    Lines {
        list: list
            .map(|list| {
                list.items
                    .iter()
                    .map(|item| if trash { speech::trashed_line(&item.row, clock) } else { speech::row_line(&item.row, view.checkboxes, clock) })
                    .collect()
            })
            .unwrap_or_default(),
        readback: list.map(|list| readback(list, trash)).unwrap_or_default(),
        day_title: view.day.as_ref().map(|day| clock.day(&day.date)).unwrap_or_default(),
        day: view
            .day
            .as_ref()
            .map(|day| {
                day.items
                    .iter()
                    .map(|item| match item.kind {
                        DayKind::Summary => Line {
                            braille: speech::summary(&day.date, &day.summary_braille, clock),
                            ..Line::of(speech::summary(&day.date, &day.summary, clock), Vec::new())
                        },
                        DayKind::Block => item.block.as_ref().map(|b| speech::block_line(b, clock)).unwrap_or_default(),
                        DayKind::Sitting => item.sitting.as_ref().map(speech::sitting_line).unwrap_or_default(),
                        DayKind::Free => item
                            .free
                            .as_ref()
                            .map(|free| speech::free_line_brailled(free, clock))
                            .unwrap_or_default(),
                        DayKind::Now => item
                            .now
                            .as_ref()
                            .map(|now| Line::of(format!("{}, {}", now.title, clock.time(&now.time)), Vec::new()))
                            .unwrap_or_default(),
                        DayKind::Cancelled => item.cancelled.as_ref().map(|c| speech::cancelled_line(c, clock)).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        choices: view
            .asking
            .as_ref()
            .and_then(|asking| asking.choices.as_ref())
            .map(|offered| offered.choices.iter().map(|choice| speech::choice_line(choice, clock)).collect())
            .unwrap_or_default(),
        devices: view
            .devices
            .as_ref()
            .and_then(|devices| devices.list.as_ref())
            .map(|list| list.devices.iter().map(crate::devices::parts).collect())
            .unwrap_or_default(),
        reminders: view
            .reminders
            .as_ref()
            .map(|list| {
                list.reminders
                    .iter()
                    .map(|reminder| {
                        // A moment of its own is said in this device's clock.
                        let words = match (&reminder.day, &reminder.time) {
                            (Some(day), Some(time)) => format!("{} at {}", clock.day(day), clock.time(time)),
                            _ => speech::sentence(&reminder.words),
                        };
                        let rest = if reminder.own { vec![reminder.delivery.clone()] } else { vec!["a default".to_owned()] };
                        Line::of(words, rest.into_iter().filter(|part| !part.is_empty()).collect())
                    })
                    .collect()
            })
            .unwrap_or_default(),
        candidates: view
            .line
            .as_ref()
            .map(|line| line.offer.candidates.iter().map(speech::candidate).collect())
            .unwrap_or_default(),
        task_state: view.details.as_ref().map(|details| speech::task_state(&details.task)).unwrap_or_default(),
    }
}
