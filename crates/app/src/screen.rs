//! What is on a screen and what changed since the last turn, for a shell that speaks for itself:
//! the three braille apps, where no platform screen reader decides what to say as something
//! changes. The Orbit app hands this to Orbit's screen reader; the BrailleSense app speaks it and
//! puts it on the cells; the BTSpeak app takes what its dialogs lack.
//!
//! Every word is the core's. It is built from the view and its lines, each a function of what
//! the app holds and the device's clock, so it comes with each turn beside them (`Turn::described`
//! over the FFI) rather than on the view model: what a row's line says is in this device's clock.
//! Names are in sentence case: a braille device's capitals cost a cell.

use lumenna_records::{Offered, Question};

use crate::lines::Lines;
use crate::speech::Line;
use crate::{Asking, Side, ViewModel, braille};

/// What is shown now, and what changed since the turn before.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScreenView {
    /// What is on the screen: what a shell's review keys read.
    pub shown: Shown,
    /// What changed, in the order to say it: a place entered, then a move, a question opened, a
    /// refusal.
    pub changes: Vec<ScreenChange>,
}

/// What is on the screen, the topmost of it.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Shown {
    /// Nothing yet: the store has not arrived.
    #[default]
    Nothing,
    /// A list: a place's, the day, a reminder list, the devices.
    List(ListShown),
    /// A question open over a list.
    Question(QuestionShown),
    /// A screen the shell draws from the view: quick add, the block form, a proposal, settings.
    /// `name` is what it is called.
    Other {
        /// Its name.
        name: String,
    },
}

/// A list on the screen.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq)]
pub struct ListShown {
    /// What it is called: "Tasks", "Today", "Reminders for Pay rent".
    pub name: String,
    /// The list beside the main one it is, if one.
    pub side: Option<Side>,
    /// What the list says of itself: the query, its count, its notices; on the day, its summary.
    pub readback: String,
    /// What it says with nothing in it.
    pub empty: String,
    /// How many rows are shown, those folded away left out.
    pub count: u32,
    /// The row in hand; none on a list with nothing in it.
    pub row: Option<RowShown>,
}

/// A row, as a shell says it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowShown {
    /// Its key: a row's, a device's `node_id`, a reminder's id.
    pub key: String,
    /// Its line, what a list says and a display shows.
    pub line: Line,
    /// Where it is among the rows shown, from 0.
    pub index: u32,
    /// Where it is in words: "3 of 12".
    pub place: String,
    /// What can be done to it, what a row's context menu offers, in order.
    pub actions: Vec<ActionShown>,
}

/// One action a row offers, with the letter that runs it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionShown {
    /// What is handed back to run it (`Event::ActOn`).
    pub offered: Offered,
    /// What it is called, in sentence case.
    pub name: String,
    /// Its letter; empty for none (Erase Permanently, folding).
    pub letter: String,
}

/// A question open: what to read and what can be pressed.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionShown {
    /// Which question this is, from one: a new number is a new question, the same one still open.
    pub number: u32,
    /// What is asked, its parts joined: a title, what it takes, a field's name.
    pub prompt: String,
    /// What is typed: a line refused stays as typed.
    pub typed: String,
    /// Why the last answer was refused, if it was.
    pub refusal: Option<String>,
    /// What can be pressed, in order: `yes` and `cancel`, or a choice's own id, with its name.
    pub buttons: Vec<ButtonShown>,
    /// What the question offers to pick from, by the core's lines.
    pub choices: Vec<Line>,
    /// Whether it asks for a line of text.
    pub line: bool,
    /// Whether it is the shell's own form (a task's or block's), which it draws itself.
    pub form: bool,
}

/// One button of a question.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonShown {
    /// `yes`, `cancel`, or a choice's id.
    pub id: String,
    /// Its name.
    pub title: String,
}

/// What changed since the last turn.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq)]
pub enum ScreenChange {
    /// A place, or a screen, entered: what it is called and the row in hand.
    Entered {
        /// Its name.
        name: String,
        /// Its row in hand.
        row: Option<RowShown>,
    },
    /// The app moved the row in hand: only when `landing` rose, never for the person's own move.
    Moved(RowShown),
    /// A question opened, or a new one in its place.
    Asked(QuestionShown),
    /// Something was refused: a letter a row lacks, an answer the question will not take.
    Refused {
        /// Why, in the core's words.
        reason: String,
        /// Whether the same words come in the turn's `said` too, so a shell says them once.
        said: bool,
    },
}

fn prompt(parts: &[&str]) -> String {
    parts.iter().map(|part| part.trim().trim_end_matches('.')).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(". ")
}

fn action_name(offered: &Offered) -> String {
    if offered.sentence.is_empty() { offered.title.clone() } else { offered.sentence.clone() }
}

fn shown_actions(offered: &[Offered]) -> Vec<ActionShown> {
    offered
        .iter()
        .map(|offered| ActionShown {
            name: action_name(offered),
            letter: braille::action_letter(offered.kind).unwrap_or_default().to_owned(),
            offered: offered.clone(),
        })
        .collect()
}

/// The list row with `key` among `shown`, as a row said: its place counted among those shown.
fn shown_row(key: &str, shown: &[(String, Line, Vec<Offered>)]) -> Option<RowShown> {
    let at = shown.iter().position(|(k, _, _)| k == key)?;
    let (_, line, offered) = &shown[at];
    Some(RowShown {
        key: key.to_owned(),
        line: line.clone(),
        index: u32::try_from(at).unwrap_or(u32::MAX),
        place: lumenna_records::words::place_in_list(at + 1, shown.len()),
        actions: shown_actions(offered),
    })
}

fn list(name: String, side: Option<Side>, readback: String, empty: String, focus: Option<&str>, shown: &[(String, Line, Vec<Offered>)]) -> ListShown {
    ListShown {
        name,
        side,
        readback,
        empty,
        count: u32::try_from(shown.len()).unwrap_or(u32::MAX),
        row: focus.and_then(|key| shown_row(key, shown)).or_else(|| shown.first().and_then(|(key, _, _)| shown_row(key, shown))),
    }
}

/// What is beneath any question: the list or the screen.
fn beneath(view: &ViewModel, lines: &Lines) -> Shown {
    let other = |name: &str| Shown::Other { name: name.to_owned() };
    if let Some(form) = &view.block_form {
        return other(&form.heading);
    }
    if let Some(proposal) = &view.proposal {
        return other(&proposal.words.title);
    }
    if let Some(reminders) = &view.reminders {
        let shown: Vec<(String, Line, Vec<Offered>)> = reminders
            .reminders
            .iter()
            .zip(&lines.reminders)
            .map(|(reminder, line)| (reminder.id.clone(), line.clone(), reminder.offered.clone()))
            .collect();
        return Shown::List(list(
            view.reminders_title.clone().unwrap_or_default(),
            Some(Side::Reminders),
            reminders.announcement.clone(),
            String::new(),
            view.reminders_held.focus.as_deref(),
            &shown,
        ));
    }
    if view.line.is_some() {
        return other(&crate::keys::command_of("new-task").map_or_else(String::new, |command| lumenna_records::fields::sentence_case(command.title.to_owned())));
    }
    if let Some(devices) = &view.devices {
        let shown: Vec<(String, Line, Vec<Offered>)> = devices
            .list
            .iter()
            .flat_map(|list| list.devices.iter())
            .zip(&lines.devices)
            .map(|(device, line)| (device.node_id.clone(), line.clone(), device.offered.clone()))
            .collect();
        return Shown::List(list(devices.words.title.clone(), Some(Side::Devices), devices.status.clone(), String::new(), view.devices_held.focus.as_deref(), &shown));
    }
    if view.settings.is_some() {
        return other(&crate::keys::command_of("settings").map_or_else(String::new, |command| lumenna_records::fields::sentence_case(command.title.to_owned())));
    }
    let focus = view.focus.as_deref();
    if let Some(day) = &view.day {
        let shown: Vec<(String, Line, Vec<Offered>)> = day
            .items
            .iter()
            .zip(&lines.day)
            .filter(|(item, _)| !item.hidden)
            .map(|(item, line)| (item.key.clone(), line.clone(), item.offered.clone()))
            .collect();
        let summary = shown.iter().find(|(key, _, _)| key == "summary").map(|(_, line, _)| line.text.clone()).unwrap_or_default();
        return Shown::List(list(lines.day_title.clone(), None, summary, String::new(), focus, &shown));
    }
    if let Some(listed) = &view.list {
        let shown: Vec<(String, Line, Vec<Offered>)> = listed
            .items
            .iter()
            .zip(&lines.list)
            .filter(|(item, _)| !item.hidden)
            .map(|(item, line)| {
                let offered = item.actions.iter().filter_map(|at| listed.offered.get(*at as usize).cloned()).collect();
                (item.row.id.clone(), line.clone(), offered)
            })
            .collect();
        return Shown::List(list(view.title.clone(), None, lines.readback.clone(), listed.empty.clone(), focus, &shown));
    }
    other(&view.title)
}

fn question(asking: &Asking, lines: &Lines) -> QuestionShown {
    let cancel = || ButtonShown { id: "cancel".to_owned(), title: lumenna_records::words::cancel(true) };
    let yes = |title: &str| ButtonShown { id: "yes".to_owned(), title: title.to_owned() };
    let typed = |initial: &str| asking.typed.clone().unwrap_or_else(|| initial.to_owned());
    let mut shown = QuestionShown {
        number: asking.number,
        prompt: String::new(),
        typed: String::new(),
        refusal: asking.refusal.clone(),
        buttons: Vec::new(),
        choices: lines.choices.clone(),
        line: false,
        form: false,
    };
    match &asking.question {
        Question::Immediate => {}
        Question::Form => {
            shown.prompt = asking.action.as_ref().map_or_else(String::new, |action| if action.sentence.is_empty() { action.title.clone() } else { action.sentence.clone() });
            shown.form = true;
        }
        Question::Confirm { title, message, yes: button } => {
            shown.prompt = prompt(&[title, message]);
            shown.buttons = vec![cancel(), yes(button)];
        }
        Question::Text { title, label, initial, hint, yes: button, .. } => {
            shown.prompt = prompt(&[title, hint, label]);
            shown.typed = typed(initial);
            shown.line = true;
            shown.buttons = vec![yes(button), cancel()];
        }
        Question::Pick { title, yes: button, .. } => {
            shown.prompt = title.clone();
            shown.buttons = vec![yes(button), cancel()];
        }
        Question::Choose { title, message, answers } => {
            shown.prompt = prompt(&[title, message]);
            shown.buttons = answers.iter().map(|answer| ButtonShown { id: answer.id.clone(), title: answer.title.clone() }).chain([cancel()]).collect();
        }
    }
    shown
}

/// What is on the screen now: a question if one is open, else what it stands over.
#[must_use]
pub fn shown(view: &ViewModel, lines: &Lines) -> Shown {
    if !view.ready {
        return Shown::Nothing;
    }
    match &view.asking {
        Some(asking) => Shown::Question(question(asking, lines)),
        None => beneath(view, lines),
    }
}

/// What tells two views' places apart, a question left aside.
fn place_of(view: &ViewModel) -> (bool, Option<&lumenna_records::places::Place>, Option<&str>, Option<&str>, [bool; 5]) {
    (
        view.ready,
        view.place.as_ref(),
        view.day.as_ref().map(|day| day.date.as_str()),
        view.reminders_title.as_deref(),
        [view.block_form.is_some(), view.proposal.is_some(), view.line.is_some(), view.devices.is_some(), view.settings.is_some()],
    )
}

/// What the row in hand of the main list, or a side list open, is, if any.
fn row_in_hand(shown: &Shown) -> Option<RowShown> {
    match shown {
        Shown::List(list) => list.row.clone(),
        _ => None,
    }
}

/// `view`'s screen, and what changed since `before`: `refusals` are what this turn said that was
/// a refusal, carried beside `said`. With nothing before, the place is entered.
#[must_use]
pub fn screen_view(before: Option<&ViewModel>, view: &ViewModel, lines: &Lines, refusals: &[String]) -> ScreenView {
    let mut changes = Vec::new();
    let under = beneath(view, lines);
    let entered = before.is_none_or(|before| place_of(before) != place_of(view));
    if view.ready && entered {
        let name = match &under {
            Shown::List(list) => list.name.clone(),
            Shown::Other { name } => name.clone(),
            Shown::Question(_) | Shown::Nothing => String::new(),
        };
        changes.push(ScreenChange::Entered { name, row: row_in_hand(&under) });
    } else if let Some(before) = before
        && view.landing > before.landing
        && view.focus != before.focus
        && let Some(row) = row_in_hand(&under)
    {
        changes.push(ScreenChange::Moved(row));
    }
    if let Some(asking) = &view.asking
        && before.and_then(|before| before.asking.as_ref()).is_none_or(|was| was.number != asking.number)
    {
        changes.push(ScreenChange::Asked(question(asking, lines)));
    }
    for reason in refusals {
        changes.push(ScreenChange::Refused { reason: reason.clone(), said: true });
    }
    if let Some(reason) = view.asking.as_ref().and_then(|asking| asking.refusal.clone())
        && before.and_then(|before| before.asking.as_ref()).and_then(|asking| asking.refusal.as_ref()) != Some(&reason)
    {
        changes.push(ScreenChange::Refused { reason, said: false });
    }
    ScreenView { shown: shown(view, lines), changes }
}
