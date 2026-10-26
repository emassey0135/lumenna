//! The question open, and running a row's action: what it asks, what an answer does, what a
//! write leaves in hand and where it leads.

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::places::Place;
use lumenna_records::{Action, ActionKind, Answer, Change, Choices, LumennaError, Offered, Question, Subject, not_offered};

use crate::host::{Arrival, HostRequest, Writing};
use crate::model::Screen;
use crate::network::NetworkRequest;
use crate::{BlockEvent, Effect, Event, Lumenna, blockform, host, network_request, proposal, say, spoken, write};

/// What a question open is for.
#[derive(Debug, Clone, PartialEq)]
// One at a time, in the model: the size of the largest is no cost worth a box.
#[allow(clippy::large_enum_variant)]
pub enum Purpose {
    /// A row's action asking before it runs.
    Action(Action),
    /// Which day to go to.
    GoToDay,
    /// A new saved filter: its name first, then its query.
    NewFilter(Option<String>),
    /// Which occurrences of a repeating block a change is for, before its form opens: the
    /// block, and its day as it is said ("Tomorrow").
    EditWhich(lumenna_records::types::PlanBlock, String),
    /// Which day's blocks a pick is to offer, its Another Day chosen: the action picking.
    PickDay(Action),
}

/// What a pick's Another Day is picked by: Put in a Block's, offering a further day's blocks.
pub const ANOTHER_DAY: &str = "another-day";


/// A question open.
#[derive(Debug, Clone, PartialEq)]
pub struct Open {
    /// What it is for.
    pub purpose: Purpose,
    /// What a pick offers, once the host has said.
    pub choices: Option<Choices>,
    /// Why the last answer was refused: the question stays open with it.
    pub refusal: Option<String>,
    /// What was picked, for a pick that asks a second question once one is: how long a
    /// sitting is meant to take.
    pub picked: Option<String>,
    /// The answer last given, a line typed or a choice's identifier: where a place renamed or
    /// made is to be found.
    pub answered: Option<String>,
    /// The write the answer went out as, until it is answered: only that write's answer closes
    /// the question, or keeps it open with why.
    pub writing: Option<u64>,
    /// Which question this is among those the screen has asked, from one; none until it is shown.
    pub number: u32,
    /// The line last typed in answer, kept for the field when the answer is refused.
    pub typed: Option<String>,
}


impl Open {
    pub(crate) fn new(purpose: Purpose) -> Self {
        Self { purpose, choices: None, refusal: None, picked: None, answered: None, writing: None, number: 0, typed: None }
    }

    /// The action asking, if it is one.
    pub(crate) fn action(&self) -> Option<&Action> {
        match &self.purpose {
            Purpose::Action(action) | Purpose::PickDay(action) => Some(action),
            Purpose::GoToDay | Purpose::EditWhich(..) | Purpose::NewFilter(_) => None,
        }
    }

    /// What to ask now, a question of the app's own in sentence case where `sentence`.
    pub(crate) fn question(&self, sentence: bool) -> Question {
        match &self.purpose {
            Purpose::Action(_) if self.picked.is_some() => lumenna_records::length_question(),
            Purpose::Action(action) => action.question.clone(),
            Purpose::GoToDay | Purpose::PickDay(_) => lumenna_records::go_to_day_question(),
            Purpose::NewFilter(name) => {
                let mut steps = lumenna_records::new_filter_questions();
                if name.is_some() { steps.swap_remove(1) } else { steps.swap_remove(0) }
            }
            Purpose::EditWhich(block, day) => lumenna_records::fields::edit_which_question(&block.title, day, sentence),
        }
    }
}


/// Whether `place` is a project, label or saved filter the store no longer has.
fn gone(place: &Place, store: &Arrival) -> bool {
    let snapshot = &store.snapshot;
    match place {
        Place::Project(name) => !snapshot.projects.values().any(|p| p.deleted_at.is_none() && &p.name == name),
        Place::Label(name) => !snapshot.labels.values().any(|l| l.deleted_at.is_none() && &l.name == name),
        Place::Filter { name, .. } => !snapshot.saved_filters.values().any(|f| f.deleted_at.is_none() && &f.name == name),
        _ => false,
    }
}

/// Where a place renamed, made or removed is now, from the question that did it, or the
/// action that did it unasked; `shown` is the place shown. A place deleted goes to the tasks
/// only when it was the one shown, or one the place shown was inside.
fn after_place(open: &Open, store: &Arrival, shown: &Place) -> Option<Place> {
    let answered = open.answered.clone();
    let filter_named = |name: &str| {
        store.snapshot.saved_filters.values().find(|f| f.deleted_at.is_none() && f.name == name).map(|f| f.query.clone())
    };
    match &open.purpose {
        Purpose::NewFilter(Some(name)) => Some(Place::Filter { name: name.clone(), query: answered? }),
        Purpose::Action(action) => match (action.subject, action.kind) {
            (Subject::Project | Subject::Label | Subject::Filter, ActionKind::Delete) => gone(shown, store).then_some(Place::Tasks),
            (Subject::Project, ActionKind::New | ActionKind::NewInside | ActionKind::Rename) => Some(Place::Project(answered?)),
            (Subject::Label, ActionKind::New | ActionKind::Rename | ActionKind::MergeInto) => Some(Place::Label(answered?)),
            (Subject::Filter, ActionKind::Rename) => {
                let name = answered?;
                Some(Place::Filter { query: filter_named(&name)?, name })
            }
            (Subject::Filter, ActionKind::ChangeQuery) => Some(Place::Filter { name: action.target.clone(), query: answered? }),
            _ => None,
        },
        _ => None,
    }
}

impl Lumenna {
    pub(crate) fn perform(&self, action: Action, model: &mut Screen) -> Command<Effect, Event> {
        match action.kind {
            // A sidebar row folds in the sidebar: its key, not a row of the list.
            ActionKind::Expand | ActionKind::Collapse if action.target.starts_with("place:") || action.target.starts_with("group:") => {
                let expand = action.kind == ActionKind::Expand;
                if expand {
                    model.sidebar_folded.remove(&action.target);
                } else {
                    model.sidebar_folded.insert(action.target.clone());
                }
                let said = lumenna_records::words::folded_said(expand);
                return render().and(say(said));
            }
            // Folding is the app's: nothing stored changes.
            ActionKind::Expand | ActionKind::Collapse => {
                let expand = action.kind == ActionKind::Expand;
                if expand {
                    model.folded.remove(&action.target);
                } else {
                    model.folded.insert(action.target.clone());
                }
                let said = lumenna_records::words::folded_said(expand);
                model.focus = Some(action.target);
                model.settle();
                return render().and(say(said));
            }
            _ => {}
        }
        // The block form is the app's: changing a block, and a new one in free time.
        if action.question == Question::Form {
            match (action.subject, action.kind) {
                (Subject::Block, ActionKind::Edit) => return self.edit_block(&action, model),
                (Subject::Series, ActionKind::Edit) => return self.series_form(&action.target, model),
                // A saved filter's form is two questions, its name then its query.
                (Subject::Filter, ActionKind::New) => {
                    model.open = Some(Open::new(Purpose::NewFilter(None)));
                    return render();
                }
                // A task's or block's reminders, a list of the app's.
                (_, ActionKind::Reminders) => {
                    model.reminders = Some(action.target.clone());
                    model.reminders_held = crate::held::Held::default();
                    // Its name is the list's own; what else it says (a task with no due date
                    // takes a moment of its own) is said as it opens.
                    let notices = model
                        .store
                        .as_ref()
                        .and_then(|store| lumenna_records::reminders::reminders_of(&store.snapshot, &action.target).ok())
                        .map(|list| list.notices)
                        .unwrap_or_default();
                    return if notices.is_empty() { render() } else { render().and(say(notices.join(" "))) };
                }
                // The day's two proposals are the app's screens over the host's proposing.
                (Subject::Day, ActionKind::Replan) => return self.propose(proposal::Kind::Replan, model),
                (Subject::Day, ActionKind::PlanDay) => return self.propose(proposal::Kind::PlanDay(action.target.clone()), model),
                (Subject::FreeTime, ActionKind::AddBlock) => {
                    let minutes = model.day_view().and_then(|view| {
                        view.items.into_iter().find(|i| i.actions.iter().any(|a| a.kind == action.kind && a.target == action.target))?.free
                    });
                    let start = action.other.clone().or_else(|| minutes.as_ref().map(|f| f.start.clone()));
                    return self.block(BlockEvent::New { start, minutes: minutes.map(|f| f.minutes) }, model);
                }
                _ => {}
            }
        }
        match &action.question {
            Question::Immediate => {
                let unasked = Open::new(Purpose::Action(action.clone()));
                write(model, HostRequest::Act { action, answer: Answer::Yes }, Some(unasked)).1
            }
            Question::Pick { .. } => {
                let Some(store) = &model.store else { return Command::done() };
                match lumenna_records::choices::offered(&store.snapshot, &action, &store.now) {
                    // Nothing to pick: the question never opens, and why is all that is said.
                    Ok(choices) if choices.choices.is_empty() => say(choices.announcement),
                    Ok(choices) => {
                        let said = choices.announcement.clone();
                        let mut open = Open::new(Purpose::Action(action));
                        open.choices = Some(choices);
                        model.open = Some(open);
                        render().and(say(said))
                    }
                    Err(error) => say(error.message()),
                }
            }
            Question::Form | Question::Confirm { .. } | Question::Text { .. } | Question::Choose { .. } => {
                model.open = Some(Open::new(Purpose::Action(action)));
                render()
            }
        }
    }

    pub(crate) fn pick_from(&self, day: &str, model: &mut Screen) -> Command<Effect, Event> {
        let Some(store) = &model.store else { return Command::done() };
        let Some(open) = &mut model.open else { return Command::done() };
        let Some(task) = open.action().filter(|action| action.kind == ActionKind::PutInBlock).map(|action| action.target.clone()) else {
            return Command::done();
        };
        match lumenna_records::choices::blocks_for(&store.snapshot, Some(&task), Some(day), Some(1), &store.now) {
            Ok(choices) => {
                let said = choices.announcement.clone();
                open.refusal = None;
                open.choices = Some(choices);
                // Back to the pick, which now offers that day's blocks: a question of its own
                // again, so a shell that has answered the day's asks it.
                if let Purpose::PickDay(action) = &open.purpose {
                    open.purpose = Purpose::Action(action.clone());
                }
                open.number = 0;
                render().and(say(said))
            }
            // A day that does not read stays a question, with why.
            Err(error) => {
                open.refusal = Some(error.message().to_owned());
                render()
            }
        }
    }

    pub(crate) fn answer(&self, answer: Answer, model: &mut Screen) -> Command<Effect, Event> {
        let Some(open) = &mut model.open else { return Command::done() };
        if let Answer::Text { text } = &answer {
            open.typed = Some(text.clone());
        }
        // What was wrong with the last answer is gone with a new one: a refusal from here on is
        // this answer's, so a shell asks again only when it comes.
        open.refusal = None;
        match open.purpose.clone() {
            Purpose::GoToDay => {
                let Answer::Text { text } = answer else { return Command::done() };
                let Some(store) = &model.store else { return Command::done() };
                match lumenna_records::resolve::date(Some(&text), &store.now, store.snapshot.settings.week_start) {
                    Ok(date) => {
                        let today = date == store.now.date();
                        model.show_day((!today).then_some(date))
                    }
                    Err(error) => {
                        open.refusal = Some(error.message().to_owned());
                        render()
                    }
                }
            }
            Purpose::NewFilter(None) => {
                let Answer::Text { text } = answer else { return Command::done() };
                if text.trim().is_empty() {
                    open.refusal = Some(lumenna_records::words::filter_needs_name());
                    return render();
                }
                *open = Open::new(Purpose::NewFilter(Some(text.trim().to_owned())));
                render()
            }
            Purpose::NewFilter(Some(name)) => {
                let Answer::Text { text } = answer else { return Command::done() };
                open.answered = Some(text.clone());
                let asked = open.clone();
                let (number, sent) = write(model, HostRequest::AddFilter { name, query: text }, Some(asked));
                if let Some(open) = &mut model.open {
                    open.writing = Some(number);
                }
                sent
            }
            Purpose::EditWhich(block, _) => {
                let Answer::Picked { id, .. } = answer else { return Command::done() };
                model.open = None;
                if id == "day" {
                    let date = block.id.split('@').nth(1).unwrap_or_default().to_owned();
                    model.block_form = Some(blockform::Form::change(
                        blockform::Purpose::Occurrence { series: block.series.clone(), date },
                        block.title.clone(),
                        lumenna_records::fields::BlockFormScope::Day,
                        lumenna_records::day_block_fields(block),
                        None,
                    ));
                    render()
                } else {
                    self.series_form(&block.series, model)
                }
            }
            // Another day, as a phrase: the pick offers that day's blocks.
            Purpose::PickDay(_) => {
                let Answer::Text { text } = answer else { return Command::done() };
                self.pick_from(&text, model)
            }
            Purpose::Action(_) if matches!(&answer, Answer::Picked { id, .. } if id == ANOTHER_DAY) => {
                open.purpose = Purpose::PickDay(open.action().cloned().expect("an action's pick"));
                open.refusal = None;
                open.number = 0;
                render()
            }
            Purpose::Action(action) => {
                // A pick that asks how long: the length is asked once one is picked, and both
                // go together.
                if let (Question::Pick { length: Some(_), .. }, Answer::Picked { id, length: None }, None) =
                    (&action.question, &answer, &open.picked)
                {
                    open.picked = Some(id.clone());
                    open.refusal = None;
                    // How long is a question of its own.
                    open.number = 0;
                    return render();
                }
                open.answered = match &answer {
                    Answer::Text { text } => Some(text.trim().to_owned()),
                    Answer::Picked { id, .. } => Some(id.clone()),
                    Answer::Yes => None,
                };
                let answer = match (&open.picked, answer) {
                    (Some(id), Answer::Text { text }) => Answer::Picked { id: id.clone(), length: Some(text) },
                    (_, answer) => answer,
                };
                let asked = open.clone();
                let (number, sent) = write(model, HostRequest::Act { action, answer }, Some(asked));
                if let Some(open) = &mut model.open {
                    open.writing = Some(number);
                }
                sent
            }
        }
    }

    /// What the write `writing` did. Only what waits on that write takes its answer: the
    /// question it answered, the quick-add line, the details' save, the setting, the block form,
    /// the reminder dealt with. Whatever was opened or typed since stays as it is.
    pub(crate) fn wrote(&self, writing: &Writing, result: Result<Change, LumennaError>, store: Option<Arrival>, model: &mut Screen) -> Command<Effect, Event> {
        if let Some(store) = store {
            model.store = Some(store);
        }
        // Writes are numbered from one: an answer numbered none is nobody's.
        let number = writing.number;
        let mine = |waiting: Option<u64>| number != 0 && waiting == Some(number);
        let adding = mine(model.adding);
        if adding {
            model.adding = None;
        }
        let saving = model.details.saving.take_if(|saving| mine(Some(saving.number)));
        let setting = model.settings.as_mut().and_then(|s| s.setting.take_if(|(_, n)| mine(Some(*n)))).map(|(key, _)| key);
        let block_saving = model.block_form.as_mut().is_some_and(|form| form.saving.take_if(|n| mine(Some(*n))).is_some());
        let question = model.open.as_ref().is_some_and(|open| mine(open.writing));
        let dealt = model.dealing.iter().position(|(n, _)| mine(Some(*n))).map(|at| model.dealing.remove(at).1);
        match result {
            Ok(change) => {
                if question {
                    model.open = None;
                }
                // Dealt with: off the list now, and not before, so a failure leaves it there.
                if let Some(id) = dealt {
                    model.due.retain(|due| due.id != id);
                }
                // A place renamed, made or removed: go to where it now is.
                let deleted = writing.asked.as_ref().and_then(Open::action).is_some_and(|action| action.kind == ActionKind::Delete);
                let shown = model.place();
                let after = writing.asked.as_ref().and_then(|open| after_place(open, model.store.as_ref()?, &shown));
                if change.changed && let Some(place) = after {
                    // From the sidebar, its row in hand follows the place it acted on to where it
                    // now is; a place deleted leaves the row now in its place in hand instead.
                    if model.acting == Some(crate::Side::Sidebar) && !deleted {
                        let key = lumenna_records::places::SidebarEntry::key_of(&lumenna_records::places::SidebarKind::Place(place.clone()));
                        let keys = model.side_keys(crate::Side::Sidebar);
                        model.sidebar_held.hold(&key, &keys);
                    }
                    model.go(place);
                }
                if let Some(saving) = saving {
                    model.saved_details(saving);
                }
                if let (Some(key), Some(settings)) = (&setting, &mut model.settings) {
                    settings.problems.remove(key);
                    settings.typed.remove(key);
                    return render().and(say(spoken(&change))).and(host(HostRequest::Settings));
                }
                if adding {
                    model.line = None;
                }
                if block_saving {
                    model.block_form = None;
                    // What it made, or changed, is in hand: a new series under its first day.
                    if let Some(series) = change.affected.blocks.first() {
                        let key = model.rows().and_then(|rows| {
                            rows.shown.into_iter().map(|(key, _)| key).find(|key| {
                                key.strip_prefix("block:").unwrap_or(key).split('@').next() == Some(series.as_str())
                            })
                        });
                        if let Some(key) = key {
                            model.hold(&key);
                        }
                    }
                }
                // What it made is what the person works on next.
                if let Some(task) = &change.task {
                    model.hold(&task.id);
                }
                model.settle();
                render().and(say(spoken(&change)))
            }
            Err(error) => {
                model.settle();
                if adding {
                    model.add_refused = Some(error.message().to_owned());
                }
                // A line refused stays as typed, beside what was wrong with it.
                if let (Some(key), Some(settings)) = (&setting, &mut model.settings) {
                    settings.problems.insert(key.clone(), error.message().to_owned());
                    return render().and(say(error.message()));
                }
                if block_saving && let Some(form) = &mut model.block_form {
                    form.refusal = Some(error.message().to_owned());
                    return render();
                }
                // A refused answer stays in its question, with the reason.
                if question && let Some(open) = &mut model.open {
                    open.refusal = Some(error.message().to_owned());
                    open.writing = None;
                    return render();
                }
                render().and(say(error.message()))
            }
        }
    }

    pub(crate) fn arrived(&self, store: Arrival, model: &mut Screen) -> Command<Effect, Event> {
        model.store = Some(store);
        // Dismissed or snoozed elsewhere — another device, a notification's own button — is
        // dealt with here too.
        model.drop_acknowledged();
        model.settle();
        let mut more = render();
        // A setting synced from elsewhere is shown as it now is; so is a device, a round's
        // outcome included.
        if model.settings.is_some() {
            more = more.and(host(HostRequest::Settings));
        }
        if model.devices.is_some() {
            more = more.and(host(HostRequest::Devices)).and(network_request(NetworkRequest::Status, Event::SyncStatus));
        }
        more
    }

    pub(crate) fn filter(&self, text: String, model: &mut Screen) -> Command<Effect, Event> {
        model.filter = (!text.trim().is_empty()).then_some(text);
        let reads = model.store.as_ref().is_some_and(|store| {
            lumenna_records::resolve::list_query(&store.snapshot, &model.query()).is_ok()
        });
        if reads {
            model.read = model.filter.clone();
        }
        model.settle();
        render()
    }

    /// The first of `kinds` among `actions` run, or why there is none, said of `subject`.
    pub(crate) fn run_among(&self, actions: &[Action], kinds: &[ActionKind], subject: Subject, model: &mut Screen) -> Command<Effect, Event> {
        match (kinds.iter().find_map(|kind| actions.iter().find(|a| a.kind == *kind)), kinds.first()) {
            (Some(action), _) => self.perform(action.clone(), model),
            (None, Some(kind)) => say(not_offered(*kind, subject, false)),
            (None, None) => Command::done(),
        }
    }

    /// Runs the action `offered` names in short on the row `on` names, which `turn` has put in
    /// hand: found again there, whole. A row gone since says so; one that no longer offers it is
    /// refused as a key it lacks would be.
    pub(crate) fn act_on(&self, on: crate::ActingOn, offered: &Offered, model: &mut Screen) -> Command<Effect, Event> {
        use crate::ActingOn;
        let actions = match &on {
            ActingOn::Row(key) => model.actions_in_hand().filter(|_| model.focus.as_ref() == Some(key)),
            ActingOn::Held(side, key) => {
                if model.side_keys(*side).contains(key) { model.side_actions(*side) } else { None }
            }
            ActingOn::Day => model.day_view().map(|day| day.actions),
            ActingOn::Details => model.details_shown().map(|task| task.actions),
            ActingOn::Reminders => model.store.as_ref().zip(model.reminders.as_ref()).and_then(|(store, target)| {
                lumenna_records::reminders::reminders_of(&store.snapshot, target).ok()
            }).map(|list| list.actions),
            ActingOn::Settings => model.settings.as_ref().map(|_| lumenna_records::actions::store_actions()),
        };
        let Some(actions) = actions else { return say(lumenna_records::no_longer_here()) };
        match actions.iter().find(|action| offered.is(action)) {
            Some(action) => self.perform(action.clone(), model),
            None if actions.is_empty() => say(not_offered(offered.kind, offered.subject, false)),
            None => refuse(offered.kind, &actions, model),
        }
    }

    pub(crate) fn run(&self, kinds: Vec<ActionKind>, model: &mut Screen) -> Command<Effect, Event> {
        // A side list's row while the person acts from it, else the row in hand: never the main
        // list's row for a key pressed in a side list, which would act on what is not there.
        let actions = match model.acting {
            Some(side) => model.side_actions(side),
            None => model.actions_in_hand(),
        };
        let Some(actions) = actions else { return Command::done() };
        let found = kinds.iter().find_map(|kind| actions.iter().find(|a| a.kind == *kind));
        match (found, kinds.first()) {
            (Some(action), _) => self.perform(action.clone(), model),
            (None, Some(kind)) => refuse(*kind, &actions, model),
            (None, None) => Command::done(),
        }
    }
}

/// Why the row in hand, offering `actions`, does not offer `kind`, in the core's words.
fn refuse(kind: ActionKind, actions: &[Action], model: &Screen) -> Command<Effect, Event> {
    // The day's heading offers the day's own actions, and says the day of one it lacks.
    let heading = model.place() == Place::Today && model.focus.as_deref() == Some("summary");
    let subject = actions.first().map_or(if heading { Subject::Day } else { Subject::Task }, |a| a.subject);
    let this_device = model.acting == Some(crate::Side::Devices) && model.holds_this_device();
    say(not_offered(kind, subject, this_device))
}
