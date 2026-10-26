//! The question the app has open (`ViewModel::asking`), asked in GTK's dialogs and answered
//! back to the app: which question is open, its words, what a pick offers and why an answer
//! was refused are all the app's. A refused answer is said, then asked again with what was
//! typed.
//!
//! A question is asked as soon as it comes: each has its number (`Asking::number`), so a new one
//! is told from one already answered. One answered and still open waits until the app is
//! still, since it stays open until the store answers the write; then, refused, it is asked
//! again, the line starting from what was typed (`Asking::typed`).

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use lumenna_app::Asking;
use lumenna_ffi::ShellEvent;
use lumenna_records::{Answer, Question};

use crate::prompts;
use crate::window::{App, spawn};

thread_local! {
    /// Whether a question is being asked and not yet answered.
    static ASKING: Cell<bool> = const { Cell::new(false) };
    /// Whether a look waits for the app to be still.
    static WAITING: Cell<bool> = const { Cell::new(false) };
    /// The number of the question last answered here, or left.
    static ANSWERED: Cell<u32> = const { Cell::new(0) };
}

/// Asks what the app has open, unless something is being asked already.
pub fn update(app: &Rc<App>) {
    if ASKING.get() {
        return;
    }
    let Some(number) = app.model.view().asking.as_ref().map(|asking| asking.number) else { return };
    if number == ANSWERED.get() && !app.model.is_still() {
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
    // Answered and still open, with the store done: asked again only if it was refused.
    if open.number == ANSWERED.get() && open.refusal.is_none() {
        return;
    }
    ask(app, open, lines.choices.into_iter().map(|line| line.text).collect());
}

fn ask(app: &Rc<App>, open: Asking, worded: Vec<String>) {
    let action = open.action.clone();
    let number = open.number;
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
            answer(app, number, None);
        }
        Question::Confirm { title, message, yes } => {
            let destructive = action.as_ref().is_some_and(|a| a.destructive);
            asking(app, async move |app: Rc<App>| {
                let yes = prompts::confirm(&window, &title, &message, &yes, destructive).await;
                answer(&app, number, yes.then_some(Answer::Yes));
            });
        }
        Question::Text { title, label, initial, hint, yes, .. } => {
            // Refused, the line starts from what was typed, which the app keeps.
            let start = open.refusal.as_ref().and(open.typed.clone()).unwrap_or(initial);
            let refusal = open.refusal.clone();
            // A question that asks for a day has GTK's calendar beneath its line.
            let day = open.day.clone();
            let filter = open.syntax == Some(lumenna_records::Syntax::Filter);
            asking(app, async move |app: Rc<App>| {
                if let Some(refusal) = refusal {
                    prompts::tell(&window, &refusal).await;
                }
                let text = prompts::ask(&window, &title, &label, &hint, &start, &yes, match (&day, filter) {
                    (Some(day), _) => prompts::Line::Day(day),
                    (None, true) => prompts::Line::Filter,
                    (None, false) => prompts::Line::Words,
                })
                .await;
                answer(&app, number, text.map(|text| Answer::Text { text }));
            });
        }
        Question::Pick { title, yes, .. } => {
            // What a pick offers comes with the question once the app has worked it out.
            let Some(offered) = open.choices.clone() else { return };
            // Put in a Block's ends in the app's Another Day, picked like any other.
            let options: Vec<(String, u32, String, Option<lumenna_records::types::Swatch>)> = offered
                .choices
                .iter()
                .enumerate()
                .map(|(at, choice)| {
                    let text = worded.get(at).cloned().unwrap_or_else(|| choice.title.clone());
                    (text, choice.depth, choice.reason.clone().unwrap_or_default(), choice.swatch.clone())
                })
                .collect();
            // Nothing to pick: said in the core's words, and the question left.
            if options.is_empty() {
                app.say(&lumenna_records::words::nothing_to_choose());
                answer(app, number, None);
                return;
            }
            let refusal = open.refusal.clone();
            asking(app, async move |app: Rc<App>| {
                if let Some(refusal) = refusal {
                    prompts::tell(&window, &refusal).await;
                }
                let picked = prompts::pick(&window, &title, "C_hoices", &options, &yes).await.and_then(|index| offered.choices.get(index));
                answer(&app, number, picked.map(|choice| Answer::Picked { id: choice.id.clone(), length: None }));
            });
        }
        Question::Choose { title, message, answers } => {
            let destructive = action.as_ref().is_some_and(|a| a.destructive);
            asking(app, async move |app: Rc<App>| {
                let titles: Vec<&str> = answers.iter().map(|choice| choice.title.as_str()).collect();
                let index = prompts::choose(&window, &title, &message, &titles, destructive).await;
                let id = index.and_then(|index| answers.get(index)).map(|choice| choice.id.clone());
                answer(&app, number, id.map(|id| Answer::Picked { id, length: None }));
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

/// Hands the answer to question `number` back, or that it was left.
fn answer(app: &App, number: u32, given: Option<Answer>) {
    ASKING.set(false);
    ANSWERED.set(number);
    app.model.send(match given {
        Some(answer) => ShellEvent::Answer(answer),
        None => ShellEvent::Cancel,
    });
}


