//! The question the app has open (`ViewModel::asking`), asked in GTK's dialogs and answered
//! back to the app: which question is open, its words, what a pick offers and why an answer
//! was refused are all the app's. A refused answer is said, then asked again with what was
//! typed.
//!
//! It asks only once the app is still: right after an answer the question stays open until the
//! store answers the write, and a turn drawn meanwhile would ask it again.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use lumenna_app::Asking;
use lumenna_ffi::ShellEvent;
use lumenna_records::{Action, Answer, Question};

use crate::prompts;
use crate::window::{App, spawn};

thread_local! {
    /// Whether a question is being asked and not yet answered.
    static ASKING: Cell<bool> = const { Cell::new(false) };
    /// Whether a look waits for the app to be still.
    static WAITING: Cell<bool> = const { Cell::new(false) };
    /// What was typed into the last line asked, and which question it was, for asking it again
    /// when refused.
    static TYPED: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
}

/// Asks what the app has open, unless something is being asked already.
pub fn update(app: &Rc<App>) {
    if ASKING.get() || app.model.view().asking.is_none() {
        return;
    }
    if !app.model.is_still() {
        if !WAITING.replace(true) {
            glib::timeout_add_local_once(std::time::Duration::from_millis(50), || {
                WAITING.set(false);
                if let Some(app) = crate::window::app() {
                    update(&app);
                }
            });
        }
        return;
    }
    let (view, lines) = app.model.current();
    let Some(open) = view.asking else { return };
    ask(app, open, lines.choices.into_iter().map(|line| line.text).collect());
}

fn ask(app: &Rc<App>, open: Asking, worded: Vec<String>) {
    let action = open.action.clone();
    // Asked over the window the question came from: Settings, when it is in front.
    let window: gtk::Window = crate::reminders::in_front()
        .or_else(crate::settings::in_front)
        .unwrap_or_else(|| app.window.clone().upcast());
    match open.question.clone() {
        Question::Immediate => {}
        Question::Form => {
            if let Some(action) = action {
                crate::actions::open_form(app, action);
            }
            app.model.send(ShellEvent::Cancel);
        }
        Question::Confirm { title, message, yes } => {
            let destructive = action.as_ref().is_some_and(|a| a.destructive);
            asking(app, async move |app: Rc<App>| {
                let yes = prompts::confirm(&window, &title, &message, &yes, destructive).await;
                answer(&app, yes.then_some(Answer::Yes));
            });
        }
        Question::Text { title, label, initial, hint, yes, .. } => {
            let which = format!("{} {title}", action.as_ref().map(|a| a.target.as_str()).unwrap_or_default());
            let start = match &open.refusal {
                Some(_) => TYPED.with_borrow(|typed| typed.as_ref().filter(|(q, _)| *q == which).map(|(_, t)| t.clone())),
                None => None,
            }
            .unwrap_or(initial);
            let refusal = open.refusal.clone();
            asking(app, async move |app: Rc<App>| {
                if let Some(refusal) = refusal {
                    prompts::tell(&window, &refusal).await;
                }
                let text = prompts::ask(&window, &title, &label, &hint, &start, &yes).await;
                TYPED.set(text.clone().map(|text| (which, text)));
                answer(&app, text.map(|text| Answer::Text { text }));
            });
        }
        Question::Pick { title, yes, .. } => {
            // What a pick offers comes with the question once the app has worked it out.
            let Some(offered) = open.choices.clone() else { return };
            let mut options: Vec<(String, u32, String)> = offered
                .choices
                .iter()
                .enumerate()
                .map(|(at, choice)| {
                    let text = worded.get(at).cloned().unwrap_or_else(|| choice.title.clone());
                    (text, choice.depth, choice.reason.clone().unwrap_or_default())
                })
                .collect();
            if let Some(another) = &open.another_day {
                options.push((another.clone(), 0, String::new()));
            }
            if options.is_empty() {
                app.model.send(ShellEvent::Cancel);
                return;
            }
            let refusal = open.refusal.clone();
            asking(app, async move |app: Rc<App>| {
                // A day for another day's blocks that did not read, said before the pick again.
                if let Some(refusal) = refusal {
                    prompts::tell(&window, &refusal).await;
                }
                let Some(index) = prompts::pick(&window, &title, "C_hoices", &options, &yes).await else {
                    return answer(&app, None);
                };
                match offered.choices.get(index) {
                    Some(choice) => answer(&app, Some(Answer::Picked { id: choice.id.clone(), length: None })),
                    None => another_day(&app, &window).await,
                }
            });
        }
        Question::Choose { title, message, answers } => {
            let destructive = action.as_ref().is_some_and(|a| a.destructive);
            asking(app, async move |app: Rc<App>| {
                let titles: Vec<&str> = answers.iter().map(|choice| choice.title.as_str()).collect();
                let index = prompts::choose(&window, &title, &message, &titles, destructive).await;
                let id = index.and_then(|index| answers.get(index)).map(|choice| choice.id.clone());
                answer(&app, id.map(|id| Answer::Picked { id, length: None }));
            });
        }
    }
}

/// Runs a dialog, marked as asking until it is answered.
fn asking(app: &Rc<App>, run: impl AsyncFnOnce(Rc<App>) + 'static) {
    ASKING.set(true);
    let app = Rc::clone(app);
    spawn(async move { run(app).await });
}

/// Hands the answer back, or that the question was left.
fn answer(app: &App, given: Option<Answer>) {
    ASKING.set(false);
    app.model.send(match given {
        Some(answer) => ShellEvent::Answer(answer),
        None => ShellEvent::Cancel,
    });
}

/// Which day's blocks a pick should offer instead, in the core's words for asking a day: the
/// pick stays open, offering that day's blocks, and is asked again as it comes.
async fn another_day(app: &App, window: &gtk::Window) {
    let question = lumenna_records::go_to_day_question();
    let day = crate::actions::ask_text(window, &question, None).await;
    ASKING.set(false);
    match day.map(|day| day.trim().to_owned()).filter(|day| !day.is_empty()) {
        Some(day) => app.model.send(ShellEvent::PickFrom(day)),
        None => app.model.send(ShellEvent::Cancel),
    }
}

/// Runs one of a row's actions through the app: the row in hand, then the action. A task's
/// details are opened from the app's question; a reminder list is named first, after what it
/// is for.
pub fn run(app: &Rc<App>, action: Action, on: &str) {
    if action.kind == lumenna_records::ActionKind::Reminders {
        return crate::reminders::show(app, &action, on);
    }
    app.model.run(action, on);
}
