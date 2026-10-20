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
use lumenna_desktop::speech::{self};
use lumenna_host::actions::{Action, ActionKind, Answer, Question, Subject};

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

/// A menu of `actions` asked over another window than the main one, when `from` says which.
pub fn menu_from(actions: &[Action], after: Option<After>, from: Option<Asking>) -> (gio::Menu, gio::SimpleActionGroup) {
    menu_running(
        actions,
        Rc::new(move |action| {
            if let Some(app) = crate::window::app() {
                match &from {
                    Some(from) => run_from(&app, action, after.clone(), from.clone()),
                    None => run(&app, action, after.clone()),
                }
            }
        }),
    )
}

/// A menu of `actions`, each handed to `run` when chosen: the app's rows hand theirs to the
/// app. Those that remove something come last, apart.
pub fn menu_running(actions: &[Action], run: Rc<dyn Fn(Action)>) -> (gio::Menu, gio::SimpleActionGroup) {
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
        let (action, run) = (action.clone(), Rc::clone(&run));
        simple.connect_activate(move |_, _| run(action.clone()));
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
            prompts::confirm(window, title, message, yes, action.destructive).await.then_some(Answer::Yes)
        }
        Question::Text { .. } => Some(Answer::Text { text: ask_text(window, &action.question, typed.as_deref()).await? }),
        Question::Pick { title, length, yes } => {
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
            // Why a choice is offered is its description, never its name.
            let options: Vec<(String, u32, String)> = choices
                .choices
                .iter()
                .map(|choice| (speech::choice(choice, &app.clock), choice.depth, choice.reason.clone().unwrap_or_default()))
                .collect();
            let index = prompts::pick(window, title, "C_hoices", &options, yes).await?;
            let id = choices.choices.get(index)?.id.clone();
            let length = match length {
                Some(_) => Some(ask_text(window, &lumenna_host::length_question(), None).await?),
                None => None,
            };
            Some(Answer::Picked { id, length })
        }
        Question::Choose { title, message, answers } => {
            let titles: Vec<&str> = answers.iter().map(|choice| choice.title.as_str()).collect();
            let index = prompts::choose(window, title, message, &titles, action.destructive).await?;
            Some(Answer::Picked { id: answers.get(index)?.id.clone(), length: None })
        }
    }
}

/// Asks a line of text as the core words the question, starting from `typed` if given:
/// what was typed before, when the answer was refused.
pub async fn ask_text(window: &gtk::Window, question: &Question, typed: Option<&str>) -> Option<String> {
    let Question::Text { title, label, initial, hint, yes, .. } = question else { return None };
    prompts::ask(window, title, label, hint, typed.unwrap_or(initial), yes).await
}

// ---------------------------------------------------------------------------------------
// The forms: the questions the app answers itself
// ---------------------------------------------------------------------------------------

/// Opens what a form action asks with, the app's own forms aside: a task's details, the
/// block form and the rest not yet drawn from the app.
pub fn open_form(app: &Rc<App>, action: Action) {
    let app = Rc::clone(app);
    spawn(async move { form(&app, action).await });
}

async fn form(app: &Rc<App>, action: Action) {
    match (action.subject, action.kind) {
        (Subject::Task, ActionKind::Edit | ActionKind::EditTask) => app.detail.open(app, &action.target),
        (_, ActionKind::Reminders) => crate::reminders::show(app, &action),
        _ => {}
    }
}
