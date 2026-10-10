//! Doing what a row offers. Which actions a row has, what each is called and what each asks
//! first are the core's (`Action`, on every record a list shows); this offers them — a row's
//! menu, the keys, the menu bar — asks each question in GTK's own dialogs, and hands the
//! answer back to `Lumenna::act`.
//!
//! The only questions answered here are forms (`Question::Form`): a task's details, a block's
//! form, and a new saved filter's name and query.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::gio;
use lumenna_desktop::speech::{self, Clock};
use lumenna_surface::actions::{Action, ActionKind, Answer, Question, Subject};
use lumenna_surface::places::Place;
use lumenna_surface::{Change, block_fields, day_block_fields};

use crate::block_form::{self, Purpose};
use crate::core::sentence;
use crate::prompts;
use crate::window::{App, spawn};

/// What a key does on whatever row has focus: one meaning everywhere.
pub const SPACE: &[ActionKind] = &[
    ActionKind::MarkDone,
    ActionKind::MarkNotDone,
    ActionKind::StartTimer,
    ActionKind::PauseTimer,
    ActionKind::ResumeTimer,
    ActionKind::Restore,
];
/// What Delete does: whatever removes the row.
pub const DELETE: &[ActionKind] = &[ActionKind::Delete, ActionKind::DeleteForGood, ActionKind::Unassign, ActionKind::Unpair];
/// What Enter does: open it, or what free time and a cancelled day are for.
pub const ENTER: &[ActionKind] = &[ActionKind::Edit, ActionKind::EditTask, ActionKind::AddBlock, ActionKind::RestoreDay];

/// The first of `actions` of one of `kinds`.
pub fn find<'a>(actions: &'a [Action], kinds: &[ActionKind]) -> Option<&'a Action> {
    kinds.iter().find_map(|kind| actions.iter().find(|action| action.kind == *kind))
}

/// What runs once an action's change is in: where the view puts the selection, say.
pub type After = Rc<dyn Fn(&Rc<App>, &Action, &Answer)>;

/// A menu of `actions`, each running its own when chosen. Those that remove something come
/// last, apart.
pub fn menu(actions: &[Action], after: Option<After>) -> (gio::Menu, gio::SimpleActionGroup) {
    let menu = gio::Menu::new();
    let (keeping, removing) = (gio::Menu::new(), gio::Menu::new());
    let group = gio::SimpleActionGroup::new();
    for (index, action) in actions.iter().enumerate() {
        let name = format!("a{index}");
        // The core's titles, as spoken; an underscore in one is the title's, not a mnemonic.
        let title = action.title.replace('_', "__");
        let title = if asks(action) { format!("{title}…") } else { title };
        let section = if action.destructive { &removing } else { &keeping };
        section.append(Some(&title), Some(&format!("row.{name}")));
        let simple = gio::SimpleAction::new(&name, None);
        let (action, after) = (action.clone(), after.clone());
        simple.connect_activate(move |_, _| {
            if let Some(app) = crate::window::app() {
                run(&app, action.clone(), after.clone());
            }
        });
        group.add_action(&simple);
    }
    menu.append_section(None, &keeping);
    menu.append_section(None, &removing);
    (menu, group)
}

/// Whether an action asks something first, which a menu says with an ellipsis.
fn asks(action: &Action) -> bool {
    !matches!(action.question, Question::Immediate)
}

/// What says a result in place of the main window's announcement.
pub type Say = Rc<dyn Fn(&str)>;

/// Where an action is asked and answered from: the window its questions sit over, and what
/// says the result. The main window says it as an announcement; a settings page on its own
/// status line, since a window in front hides the main one's.
#[derive(Clone)]
pub struct Asking {
    pub window: gtk::Window,
    pub say: Option<Say>,
}

impl Asking {
    fn say(&self, app: &App, text: &str) {
        match &self.say {
            Some(say) => say(text),
            None => app.say(text),
        }
    }
}

/// Runs an action: asks its question, then hands the answer to the core and says what it did.
pub fn run(app: &Rc<App>, action: Action, after: Option<After>) {
    let from = Asking { window: app.window.clone().upcast(), say: None };
    run_from(app, action, after, from);
}

/// Runs an action asked over another window than the main one.
pub fn run_from(app: &Rc<App>, action: Action, after: Option<After>, from: Asking) {
    let app = Rc::clone(app);
    spawn(async move {
        if action.question == Question::Form {
            return form(&app, action).await;
        }
        let mut typed = None;
        loop {
            let Some(answer) = answer(&app, &from, &action, typed.take()).await else { return };
            match app.core.lumenna.act(action.clone(), answer.clone()) {
                Ok(change) => {
                    app.store_changed();
                    if let Some(after) = &after {
                        after(&app, &action, &answer);
                    }
                    from.say(&app, &speech::sentence(&speech::announcement(&change.announcement, &change.notices)));
                    return;
                }
                Err(error) => {
                    prompts::tell(&from.window, &sentence(&error)).await;
                    // A refused line is asked for again, with what was typed.
                    match (&action.question, answer) {
                        (Question::Text { .. }, Answer::Text { text }) => typed = Some(text),
                        _ => return,
                    }
                }
            }
        }
    });
}

/// Asks an action's question, and returns the answer, or `None` if the person gave up.
async fn answer(app: &App, from: &Asking, action: &Action, typed: Option<String>) -> Option<Answer> {
    let window = &from.window;
    match &action.question {
        Question::Immediate => Some(Answer::Yes),
        Question::Form => None,
        Question::Confirm { title, message, yes } => {
            prompts::confirm(window, title, message, yes).await.then_some(Answer::Yes)
        }
        Question::Text { title, label, initial, hint, .. } => {
            let start = typed.as_deref().unwrap_or(initial);
            let text = prompts::ask(window, title, &format!("{label}:"), hint, start).await?;
            Some(Answer::Text { text })
        }
        Question::Pick { title, length } => {
            let choices = match app.core.lumenna.choices(action.clone()) {
                Ok(choices) => choices,
                Err(error) => {
                    prompts::fail(window, &sentence(&error));
                    return None;
                }
            };
            if choices.choices.is_empty() {
                // Nothing to choose from: the core says why, and that is all.
                from.say(app, &choices.announcement);
                return None;
            }
            let options: Vec<(String, u32)> =
                choices.choices.iter().map(|choice| (choice_text(choice, &app.clock), choice.depth)).collect();
            let index = prompts::pick(window, title, "_Choices:", &options).await?;
            let id = choices.choices.get(index)?.id.clone();
            let length = match length {
                Some(hint) => Some(prompts::ask(window, &action.title, "_Length:", hint, "").await?),
                None => None,
            };
            Some(Answer::Picked { id, length })
        }
        Question::Choose { title, message, answers } => {
            let titles: Vec<&str> = answers.iter().map(|choice| choice.title.as_str()).collect();
            let index = prompts::choose(window, title, message, &titles).await?;
            Some(Answer::Picked { id: answers.get(index)?.id.clone(), length: None })
        }
    }
}

/// A choice as its row reads: its title, then what tells it apart. A block is said by its day
/// and times in this desktop's own words.
fn choice_text(choice: &lumenna_surface::actions::Choice, clock: &dyn Clock) -> String {
    let mut parts = Vec::new();
    if let Some(date) = &choice.date {
        parts.push(clock.day(date));
    }
    if let (Some(start), Some(end)) = (&choice.start, &choice.end) {
        parts.push(format!("{} to {}", clock.time(start), clock.time(end)));
    }
    parts.push(choice.title.clone());
    parts.extend(choice.detail.clone());
    parts.join(", ")
}

// ---------------------------------------------------------------------------------------
// The forms: the questions the app answers itself
// ---------------------------------------------------------------------------------------

async fn form(app: &Rc<App>, action: Action) {
    match (action.subject, action.kind) {
        (Subject::Task, ActionKind::Edit | ActionKind::EditTask) => {
            app.detail.show(app, Some(&action.target));
            app.open_detail();
        }
        (Subject::Block, ActionKind::Edit) => edit_day_block(app, &action.target).await,
        (Subject::Series, ActionKind::Edit) => edit_series(app, &action.target).await,
        (Subject::FreeTime, ActionKind::AddBlock) => {
            let start = action.other.clone().unwrap_or_else(|| "09:00".to_owned());
            add_block(app, &action.target, &start, 60).await;
        }
        (Subject::Filter, ActionKind::New) => new_filter(app).await,
        _ => {}
    }
}

/// A new block from `start` on `date`, `minutes` long: the block form, then the day lands
/// on it.
pub async fn add_block(app: &Rc<App>, date: &str, start: &str, minutes: u32) {
    let window = app.window.clone().upcast::<gtk::Window>();
    let fields = block_form::new_fields(start, minutes);
    let purpose = Purpose::Add { date: date.to_owned() };
    let Some(change) = block_form::run(&window, app.core.lumenna.clone(), purpose, fields, None).await else { return };
    app.store_changed();
    if let Some(series) = change.affected.blocks.first() {
        app.land_on_block(series);
    }
    app.say_change(&change);
}

/// Changes one day's block, asking first, of a repeating one, whether that day alone or every
/// day: a change's reach is never guessed.
async fn edit_day_block(app: &Rc<App>, id: &str) {
    let Some((series, date)) = id.split_once('@') else { return };
    let block = app.core.lumenna.plan(Some(date.to_owned())).ok().and_then(|plan| plan.blocks.into_iter().find(|b| b.id == id));
    let Some(block) = block else { return };
    if !block.repeats {
        return edit_series(app, series).await;
    }
    let on = format!("{} Only", app.clock.day(date));
    let heading = format!("Change {}", block.title);
    match prompts::choose(&app.window, &heading, "Which occurrences?", &[&on, "Every Occurrence"]).await {
        Some(0) => {
            let window = app.window.clone().upcast::<gtk::Window>();
            let purpose = Purpose::Occurrence { series: series.to_owned(), date: date.to_owned() };
            let fields = day_block_fields(block);
            if let Some(change) = block_form::run(&window, app.core.lumenna.clone(), purpose, fields, None).await {
                finish(app, &change);
            }
        }
        Some(_) => edit_series(app, series).await,
        None => {}
    }
}

/// Changes every occurrence of a series.
async fn edit_series(app: &Rc<App>, series: &str) {
    let shown = match app.core.lumenna.show_block(series) {
        Ok(shown) => shown,
        Err(error) => return app.fail(&sentence(&error)),
    };
    let rule = shown.rrule.clone().filter(|_| shown.repeats);
    let purpose = Purpose::Series { id: shown.id.clone() };
    let fields = block_fields(shown);
    let window = app.window.clone().upcast::<gtk::Window>();
    if let Some(change) = block_form::run(&window, app.core.lumenna.clone(), purpose, fields, rule).await {
        finish(app, &change);
    }
}

fn finish(app: &App, change: &Change) {
    app.store_changed();
    app.say_change(change);
}

/// A new saved filter: its name, then its query.
async fn new_filter(app: &Rc<App>) {
    let Some(name) = prompts::ask(&app.window, "New Saved Filter", "_Name:", "", "").await else { return };
    let hint = lumenna_surface::actions::QUERY;
    let mut query = String::new();
    loop {
        let Some(typed) = prompts::ask(&app.window, &format!("Query for {name}"), "_Query:", hint, &query).await else {
            return;
        };
        match app.core.lumenna.add_filter(&name, &typed) {
            Ok(change) => {
                app.store_changed();
                app.go(Place::Filter { name: name.clone(), query: typed }, false);
                return app.say_change(&change);
            }
            Err(error) => {
                prompts::tell(&app.window, &sentence(&error)).await;
                query = typed;
            }
        }
    }
}
