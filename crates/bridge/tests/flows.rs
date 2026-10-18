//! Whole flows through the app and a real store: events in, what is shown and said out.

use lumenna_app::{BlockEvent, DayMove, DetailsEvent, Event, Item, ProposalEvent, SettingsEvent, Typing, ViewModel};
use lumenna_records::places::Place;
use lumenna_bridge::{Session, Shell};
use lumenna_host::Lumenna;
use lumenna_records::{ActionKind, Answer};

struct App {
    session: Session,
    _dir: tempfile::TempDir,
    profile: std::path::PathBuf,
}

impl App {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let lumenna = Lumenna::open(profile.to_str().unwrap()).unwrap();
        let (session, shell) = Session::new(lumenna);
        assert_eq!(shell.first(), Some(&Shell::Render));
        Self { session, _dir: dir, profile }
    }

    fn send(&mut self, event: Event) -> Vec<Shell> {
        self.session.send(event)
    }

    fn view(&self) -> ViewModel {
        self.session.view()
    }

    fn items(&self) -> Vec<Item> {
        self.view().list.unwrap().items
    }

    /// The titles of the rows shown, folded rows left out.
    fn titles(&self) -> Vec<String> {
        self.items().into_iter().filter(|item| !item.hidden).map(|item| item.row.title).collect()
    }

    fn id_of(&self, title: &str) -> String {
        self.items().into_iter().find(|item| item.row.title == title).unwrap().row.id
    }

    fn add(&mut self, line: &str) -> Vec<Shell> {
        self.send(Event::QuickAdd(line.to_owned()))
    }
}

fn said(shell: &[Shell]) -> Vec<&str> {
    shell
        .iter()
        .filter_map(|s| match s {
            Shell::Announce(text) | Shell::Heading(text) => Some(text.as_str()),
            Shell::Render | Shell::Save(_) => None,
        })
        .collect()
}

#[test]
fn a_fresh_store_lists_no_open_tasks_and_says_so() {
    let app = App::new();
    let view = app.view();
    assert!(view.ready);
    let list = view.list.unwrap();
    assert!(list.items.is_empty());
    assert_eq!(list.empty, "No open tasks.");
    assert_eq!(view.focus, None);
}

#[test]
fn a_task_added_is_listed_in_hand_and_its_readback_said() {
    let mut app = App::new();
    let shell = app.add("Buy milk");
    assert_eq!(shell.first(), Some(&Shell::Render), "drawn before anything is said");
    assert!(said(&shell).iter().any(|s| s.contains("Buy milk")), "{shell:?}");
    assert_eq!(app.titles(), ["Buy milk"]);
    assert_eq!(app.view().focus, Some(app.id_of("Buy milk")));
}

#[test]
fn marking_the_task_in_hand_done_puts_the_row_that_took_its_place_in_hand() {
    let mut app = App::new();
    app.add("Write the report");
    app.add("Buy milk");
    let first = app.items()[0].row.clone();
    let second = app.items()[1].row.id.clone();
    app.send(Event::Focus(first.id.clone()));

    let shell = app.send(Event::Run(vec![ActionKind::MarkDone]));

    assert!(!said(&shell).is_empty(), "the change is said");
    assert!(!app.titles().contains(&first.title));
    assert_eq!(app.view().focus, Some(second));
}

#[test]
fn a_key_the_row_does_not_offer_says_why_and_changes_nothing() {
    let mut app = App::new();
    app.add("Buy milk");
    let shell = app.send(Event::Run(vec![ActionKind::Restore]));
    assert_eq!(said(&shell), ["That does not apply here."]);
    assert_eq!(app.titles(), ["Buy milk"]);
}

#[test]
fn a_pick_offers_what_the_host_says_and_a_refused_answer_stays_in_its_question() {
    let mut app = App::new();
    Lumenna::open(app.profile.to_str().unwrap()).unwrap().add_project("Work", None).unwrap();
    app.add("Buy milk");
    let shell = app.send(Event::Run(vec![ActionKind::MoveToProject]));
    let asking = app.view().asking.expect("the pick is open");
    assert_eq!(asking.action.map(|a| a.kind), Some(ActionKind::MoveToProject));
    let choices = asking.choices.expect("what it offers came back");
    assert!(said(&shell).contains(&choices.announcement.as_str()));

    app.send(Event::Answer(Answer::Picked { id: "Nowhere".to_owned(), length: None }));
    let asking = app.view().asking.expect("still asking");
    assert!(asking.refusal.is_some_and(|r| r.contains("Nowhere")));

    app.send(Event::Cancel);
    assert_eq!(app.view().asking, None);
}

#[test]
fn collapsing_a_task_hides_its_subtasks_and_keeps_it_in_hand() {
    let mut app = App::new();
    app.add("Plan the trip");
    app.add("Book the train");
    let parent = app.id_of("Plan the trip");
    app.send(Event::Focus(app.id_of("Book the train")));
    app.send(Event::Run(vec![ActionKind::MakeSubtaskOf]));
    app.send(Event::Answer(Answer::Picked { id: parent.clone(), length: None }));
    assert_eq!(app.view().asking, None, "the subtask is made");
    let items = app.items();
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].level.as_deref(), Some("level 2"));

    app.send(Event::Focus(parent.clone()));
    let shell = app.send(Event::Run(vec![ActionKind::Collapse]));
    assert_eq!(said(&shell), ["Collapsed"]);
    assert_eq!(app.titles(), ["Plan the trip"]);
    assert_eq!(app.items()[0].row.expanded, Some(false));
    assert_eq!(app.view().focus, Some(parent));
    assert!(app.items()[1].hidden, "still there, for a tree that folds by itself");

    app.send(Event::Run(vec![ActionKind::Expand]));
    assert_eq!(app.titles().len(), 2);
}

#[test]
fn a_change_made_elsewhere_arrives_at_the_next_tick() {
    let mut app = App::new();
    assert!(app.session.tick().is_empty(), "nothing moved");
    let elsewhere = Lumenna::open(app.profile.to_str().unwrap()).unwrap();
    elsewhere.add_task("Call the bank").unwrap();
    let shell = app.session.tick();
    assert_eq!(shell, [Shell::Render]);
    assert_eq!(app.titles(), ["Call the bank"]);
}

#[test]
fn undo_takes_back_the_last_change_and_says_so() {
    let mut app = App::new();
    app.add("Buy milk");
    let shell = app.send(Event::Undo);
    assert!(!said(&shell).is_empty());
    assert!(app.titles().is_empty());
    assert_eq!(app.view().focus, None);
}

#[test]
fn opening_a_place_lists_what_it_holds_with_nothing_in_hand() {
    let mut app = App::new();
    app.add("Buy milk");
    app.send(Event::Open(lumenna_records::places::Place::Trash));
    let view = app.view();
    assert_eq!(view.title, "Trash");
    assert!(view.list.unwrap().items.is_empty());
    assert_eq!(view.focus, None);
}

#[test]
fn putting_a_task_in_a_block_asks_how_long_once_the_block_is_picked() {
    let mut app = App::new();
    let elsewhere = Lumenna::open(app.profile.to_str().unwrap()).unwrap();
    elsewhere
        .add_block(lumenna_host::NewBlock {
            title: "Deep work".to_owned(),
            at: "09:00".to_owned(),
            minutes: 120,
            date: Some("tomorrow".to_owned()),
            kind: "work".to_owned(),
            ..lumenna_host::NewBlock::default()
        })
        .unwrap();
    app.add("Write the report");

    app.send(Event::Run(vec![ActionKind::PutInBlock]));
    let asking = app.view().asking.unwrap();
    assert!(matches!(asking.question, lumenna_records::Question::Pick { length: Some(_), .. }));
    let block = asking.choices.unwrap().choices[0].id.clone();

    app.send(Event::Answer(Answer::Picked { id: block, length: None }));
    let asking = app.view().asking.expect("the length is asked next");
    assert!(matches!(asking.question, lumenna_records::Question::Text { .. }));

    let shell = app.send(Event::Answer(Answer::Text { text: "45m".to_owned() }));
    assert_eq!(app.view().asking, None);
    assert!(said(&shell).iter().any(|s| s.contains("Deep work")), "{shell:?}");
}

#[test]
fn a_pick_with_nothing_to_offer_closes_and_says_why() {
    let mut app = App::new();
    app.add("Write the report");
    let shell = app.send(Event::Run(vec![ActionKind::PutInBlock]));
    assert_eq!(app.view().asking, None);
    assert_eq!(said(&shell).len(), 1, "{shell:?}");
    assert!(said(&shell)[0].contains("no work blocks"), "{shell:?}");
}

#[test]
fn a_filter_typed_replaces_the_places_query_until_the_person_goes_elsewhere() {
    let mut app = App::new();
    app.add("Buy milk");
    app.add("Call the bank p1");
    app.send(Event::Filter("p1".to_owned()));
    assert_eq!(app.titles(), ["Call the bank"]);
    assert_eq!(app.view().focus, Some(app.id_of("Call the bank")));
    app.send(Event::Filter("due before".to_owned()));
    let list = app.view().list.unwrap();
    assert!(list.refusal.is_some(), "a filter still being typed says why it does not read");
    assert_eq!(app.titles(), ["Call the bank"], "and the rows of the last that did stay");
    app.send(Event::Open(lumenna_records::places::Place::Tasks));
    assert_eq!(app.titles().len(), 2);
    assert_eq!(app.view().query, "");
}

#[test]
fn a_tick_after_a_change_has_been_taken_in_finds_nothing_new() {
    let mut app = App::new();
    let elsewhere = Lumenna::open(app.profile.to_str().unwrap()).unwrap();
    elsewhere
        .add_block(lumenna_host::NewBlock {
            title: "Commute".to_owned(),
            at: "08:00".to_owned(),
            minutes: 60,
            date: Some("today".to_owned()),
            kind: "break".to_owned(),
            ..lumenna_host::NewBlock::default()
        })
        .unwrap();
    assert_eq!(app.session.tick(), [Shell::Render]);
    assert!(app.session.tick().is_empty());
    assert!(app.session.tick().is_empty());
}

fn block(app: &mut App, title: &str, at: &str, date: &str, repeat: Option<&str>) {
    Lumenna::open(app.profile.to_str().unwrap())
        .unwrap()
        .add_block(lumenna_host::NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes: 60,
            date: Some(date.to_owned()),
            kind: "work".to_owned(),
            repeat: repeat.map(str::to_owned),
            ..lumenna_host::NewBlock::default()
        })
        .unwrap();
    app.session.tick();
}

fn day_keys(app: &App) -> Vec<String> {
    app.view().day.unwrap().items.into_iter().filter(|i| !i.hidden).map(|i| i.key).collect()
}

#[test]
fn today_opens_on_now_the_first_time() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.send(Event::Open(Place::Today));
    let view = app.view();
    let day = view.day.unwrap();
    assert!(day.today);
    let opening = day.items.iter().find(|i| i.kind == lumenna_app::DayKind::Now || i.block.as_ref().is_some_and(|b| b.when == "now"));
    assert_eq!(view.focus, Some(opening.map_or_else(|| "summary".to_owned(), |i| i.key.clone())));
    assert!(view.list.is_none());
}

#[test]
fn the_next_day_is_shown_with_its_summary_in_hand_and_its_heading_said() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.send(Event::Open(Place::Today));
    let shell = app.send(Event::Day(DayMove::Next));
    let day = app.view().day.unwrap();
    assert!(!day.today);
    assert!(shell.iter().any(|s| matches!(s, Shell::Heading(summary) if summary.starts_with("1 block"))), "{shell:?}");
    assert!(day_keys(&app).iter().any(|k| k.starts_with("block:")));
    assert_eq!(app.view().focus.as_deref(), Some("summary"));
    app.send(Event::Day(DayMove::Today));
    assert!(app.view().day.unwrap().today);
}

#[test]
fn go_to_day_asks_for_a_day_and_keeps_one_it_cannot_read_in_the_question() {
    let mut app = App::new();
    app.send(Event::Open(Place::Tasks));
    app.send(Event::Day(DayMove::Ask));
    let asking = app.view().asking.unwrap();
    assert!(matches!(asking.question, lumenna_records::Question::Text { .. }));
    app.send(Event::Answer(Answer::Text { text: "the twelfth of never".to_owned() }));
    assert!(app.view().asking.unwrap().refusal.is_some());
    app.send(Event::Answer(Answer::Text { text: "tomorrow".to_owned() }));
    let view = app.view();
    assert_eq!(view.asking, None);
    assert_eq!(view.place, Some(Place::Today));
    assert!(!view.day.unwrap().today);
}

#[test]
fn a_block_with_a_sitting_folds_it_away() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.add("Write the report");
    app.send(Event::Run(vec![ActionKind::PutInBlock]));
    let id = app.view().asking.unwrap().choices.unwrap().choices[0].id.clone();
    app.send(Event::Answer(Answer::Picked { id, length: None }));
    app.send(Event::Answer(Answer::Text { text: String::new() }));
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Next));
    let keys = day_keys(&app);
    let block = keys.iter().find(|k| k.starts_with("block:")).unwrap().clone();
    assert!(keys.iter().any(|k| k.starts_with("sitting:")));
    app.send(Event::Focus(block.clone()));
    app.send(Event::Run(vec![ActionKind::Collapse]));
    assert!(!day_keys(&app).iter().any(|k| k.starts_with("sitting:")));
    assert_eq!(app.view().focus, Some(block));
}

#[test]
fn the_blocks_list_every_series_with_its_actions() {
    let mut app = App::new();
    block(&mut app, "Gym", "07:00", "tomorrow", Some("every weekday"));
    app.send(Event::Open(Place::Blocks));
    let items = app.items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].row.title, "Gym");
    assert!(items[0].row.actions.iter().any(|a| a.kind == ActionKind::Delete));
    assert_eq!(app.view().focus, Some(items[0].row.id.clone()));
}

#[test]
fn the_details_follow_the_task_in_hand_and_keep_typing_through_a_change_elsewhere() {
    let mut app = App::new();
    app.add("Write report");
    let id = app.id_of("Write report");
    let details = app.view().details.expect("the task in hand's details");
    assert_eq!(details.fields.title, "Write report");
    assert!(!details.dirty);

    app.send(Event::Details(DetailsEvent::Field { key: "title".to_owned(), value: "Write the report".to_owned() }));
    assert!(app.view().details.unwrap().dirty);
    // Another device sets its priority meanwhile.
    let elsewhere = Lumenna::open(app.profile.to_str().unwrap()).unwrap();
    elsewhere.edit_task(&id, lumenna_records::types::TaskEdit { priority: Some(1), ..Default::default() }).unwrap();
    app.session.tick();
    let details = app.view().details.unwrap();
    assert_eq!(details.fields.title, "Write the report", "the typing is kept");

    let shell = app.send(Event::Details(DetailsEvent::Save));
    assert!(!said(&shell).is_empty());
    let details = app.view().details.unwrap();
    assert!(!details.dirty);
    assert_eq!(details.task.title, "Write the report");
    assert_eq!(details.fields.priority, 1, "the edit elsewhere holds: only the title was sent");
}

#[test]
fn saving_details_with_nothing_changed_says_so() {
    let mut app = App::new();
    app.add("Write report");
    let shell = app.send(Event::Details(DetailsEvent::Save));
    assert_eq!(said(&shell), ["Nothing changed"]);
}

#[test]
fn quick_add_reads_back_the_line_and_puts_a_completion_in_at_the_cursor() {
    let mut app = App::new();
    app.send(Event::Typing(Typing::Open));
    assert_eq!(app.view().line.unwrap().text, "");
    app.send(Event::Typing(Typing::Text { text: "Call Sam tomorrow p1".to_owned(), cursor: 20 }));
    let line = app.view().line.unwrap();
    assert!(line.readback.contains("Call Sam"), "{}", line.readback);
    assert!(!line.has_errors);

    app.send(Event::Typing(Typing::Text { text: "Call #Inb tomorrow".to_owned(), cursor: 9 }));
    let line = app.view().line.unwrap();
    let at = line.offer.candidates.iter().position(|c| c.text.contains("Inbox")).expect("the Inbox is offered");
    app.send(Event::Typing(Typing::Choose(u32::try_from(at).unwrap())));
    let line = app.view().line.unwrap();
    assert!(line.text.starts_with("Call #Inbox"), "{}", line.text);
    assert!(line.text.ends_with("tomorrow"));
}

fn field(app: &mut App, key: &str, value: &str) {
    app.send(Event::Block(BlockEvent::Field { key: key.to_owned(), value: value.to_owned() }));
}

#[test]
fn a_new_block_is_added_from_its_form_and_is_in_hand_in_the_blocks_list() {
    let mut app = App::new();
    app.send(Event::Open(Place::Blocks));
    app.send(Event::Block(BlockEvent::New { start: None, minutes: None }));
    let form = app.view().block_form.expect("the form is open");
    assert_eq!(form.heading, "New block");
    assert_eq!(form.yes, "Add");
    assert!(form.fields.iter().any(|f| f.key == "date"));
    field(&mut app, "title", "Gym");
    field(&mut app, "start", "07:00");
    field(&mut app, "date", "tomorrow");
    let shell = app.send(Event::Block(BlockEvent::Save));
    assert!(app.view().block_form.is_none());
    assert!(said(&shell).iter().any(|s| s.contains("Gym")), "{shell:?}");
    assert_eq!(app.titles(), ["Gym"]);
    assert_eq!(app.view().focus, Some(app.items()[0].row.id.clone()));
}

#[test]
fn a_blocks_flags_follow_its_kind_and_a_length_that_does_not_read_is_refused_in_the_form() {
    let mut app = App::new();
    app.send(Event::Block(BlockEvent::New { start: None, minutes: None }));
    assert!(app.view().block_form.unwrap().values.accepts_tasks);
    field(&mut app, "kind", "break");
    let values = app.view().block_form.unwrap().values;
    assert!(!values.accepts_tasks);
    assert_eq!(values.kind, "break");
    field(&mut app, "title", "Lunch");
    field(&mut app, "minutes", "soon");
    app.send(Event::Block(BlockEvent::Save));
    let form = app.view().block_form.expect("still open");
    assert!(form.refusal.is_some());
}

#[test]
fn a_new_blocks_start_follows_the_day_typed_until_the_person_sets_one() {
    let mut app = App::new();
    app.send(Event::Block(BlockEvent::New { start: None, minutes: None }));
    field(&mut app, "date", "tomorrow");
    assert_eq!(app.view().block_form.unwrap().values.start, "08:00");
    field(&mut app, "start", "18:15");
    field(&mut app, "date", "next week");
    assert_eq!(app.view().block_form.unwrap().values.start, "18:15");
}

#[test]
fn a_repeating_block_asks_which_occurrences_and_one_days_form_changes_that_day_alone() {
    let mut app = App::new();
    block(&mut app, "Standup", "09:00", "tomorrow", Some("every day"));
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Next));
    let key = day_keys(&app).into_iter().find(|k| k.starts_with("block:")).unwrap();
    app.send(Event::Focus(key.clone()));
    app.send(Event::Run(vec![ActionKind::Edit]));
    let asking = app.view().asking.expect("which occurrences");
    let lumenna_records::Question::Choose { answers, .. } = asking.question else { panic!() };
    assert_eq!(answers[0].title, "Tomorrow only");
    app.send(Event::Answer(Answer::Picked { id: "day".to_owned(), length: None }));
    let form = app.view().block_form.expect("one day's form");
    assert_eq!(form.heading, "Change Standup, this day only");
    assert!(!form.fields.iter().any(|f| f.key == "repeat"));
    assert_eq!(form.second_column, None);
    field(&mut app, "minutes", "15");
    app.send(Event::Block(BlockEvent::Save));
    assert!(app.view().block_form.is_none());
    let day = app.view().day.unwrap();
    let block = day.items.iter().find(|i| i.key == key).and_then(|i| i.block.clone()).unwrap();
    assert_eq!(block.duration_mins, 15);
    assert!(block.changed_for_this_day);
    assert_eq!(app.view().focus, Some(key));
}

#[test]
fn plan_the_day_proposes_each_task_checked_and_a_partial_apply_proposes_again() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.add("Write report p1");
    app.add("Buy milk");
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Next));
    let plan_day = app.view().day.unwrap().actions.into_iter().find(|a| a.kind == ActionKind::PlanDay).expect("Plan the Day");
    let shell = app.send(Event::Perform(plan_day));
    let proposal = app.view().proposal.expect("the screen is open");
    assert!(proposal.ready);
    assert_eq!(proposal.changes.len(), 2, "{proposal:?}");
    assert!(proposal.changes.iter().all(|c| c.checked));
    assert!(said(&shell).contains(&proposal.announcement.as_str()), "the whole proposal is said");
    let milk = proposal.changes.iter().position(|c| c.sentence.contains("Buy milk")).unwrap();

    app.send(Event::Proposal(ProposalEvent::Check { index: u32::try_from(milk).unwrap(), on: false }));
    assert!(!app.view().proposal.unwrap().changes[milk].checked);
    let shell = app.send(Event::Proposal(ProposalEvent::Apply));
    assert!(!said(&shell).is_empty());
    // Buy milk turned down for that block: nothing else to propose, so the screen closes.
    assert!(app.view().proposal.is_none(), "{:?}", app.view().proposal);
    let sittings: Vec<_> = app.view().day.unwrap().items.into_iter().filter_map(|i| i.sitting).map(|s| s.title).collect();
    assert_eq!(sittings, ["Write report"]);
}

#[test]
fn closing_a_proposal_makes_nothing() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.add("Write report");
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Next));
    let plan_day = app.view().day.unwrap().actions.into_iter().find(|a| a.kind == ActionKind::PlanDay).unwrap();
    app.send(Event::Perform(plan_day));
    app.send(Event::Proposal(ProposalEvent::Close));
    assert!(app.view().proposal.is_none());
    assert!(app.view().day.unwrap().items.iter().all(|i| i.sitting.is_none()));
}

fn sidebar_action(app: &App, place: &str, kind: ActionKind) -> lumenna_records::Action {
    app.view()
        .sidebar
        .into_iter()
        .find(|e| e.text.starts_with(place))
        .and_then(|e| e.actions.into_iter().find(|a| a.kind == kind))
        .expect("the place's action")
}

#[test]
fn renaming_a_project_goes_to_it_and_deleting_it_goes_to_tasks() {
    let mut app = App::new();
    Lumenna::open(app.profile.to_str().unwrap()).unwrap().add_project("Work", None).unwrap();
    app.session.tick();
    app.send(Event::Perform(sidebar_action(&app, "Work", ActionKind::Rename)));
    app.send(Event::Answer(Answer::Text { text: "Office".to_owned() }));
    assert_eq!(app.view().place, Some(Place::Project("Office".to_owned())));
    app.send(Event::Perform(sidebar_action(&app, "Office", ActionKind::Delete)));
    app.send(Event::Answer(Answer::Picked { id: "keep".to_owned(), length: None }));
    assert_eq!(app.view().place, Some(Place::Tasks));
}

#[test]
fn a_new_saved_filter_asks_its_name_then_its_query_and_goes_to_it() {
    let mut app = App::new();
    let new = app
        .view()
        .sidebar
        .into_iter()
        .flat_map(|e| e.actions)
        .find(|a| a.kind == ActionKind::New && a.subject == lumenna_records::Subject::Filter)
        .unwrap();
    app.send(Event::Perform(new));
    let lumenna_records::Question::Text { label, .. } = app.view().asking.unwrap().question else { panic!() };
    assert_eq!(label, "Name");
    app.send(Event::Answer(Answer::Text { text: "Urgent".to_owned() }));
    let lumenna_records::Question::Text { label, .. } = app.view().asking.unwrap().question else { panic!() };
    assert_eq!(label, "Query");
    app.send(Event::Answer(Answer::Text { text: "p1".to_owned() }));
    assert_eq!(app.view().asking, None);
    assert_eq!(app.view().place, Some(Place::Filter { name: "Urgent".to_owned(), query: "p1".to_owned() }));
}

#[test]
fn a_blocks_reminders_list_keeps_a_phrase_it_cannot_read_in_its_question() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.send(Event::Open(Place::Blocks));
    app.send(Event::Run(vec![ActionKind::Reminders]));
    let list = app.view().reminders.expect("the list is open");
    assert_eq!(list.announcement, "1 reminder, the defaults");
    let add = list.actions[0].clone();
    app.send(Event::Perform(add));
    app.send(Event::Answer(Answer::Text { text: "when it feels right".to_owned() }));
    assert!(app.view().asking.unwrap().refusal.unwrap().contains("is not a reminder"));
    app.send(Event::Answer(Answer::Text { text: "10m after end".to_owned() }));
    assert_eq!(app.view().asking, None);
    assert_eq!(app.view().reminders.unwrap().announcement, "1 reminder");
    app.send(Event::CloseReminders);
    assert!(app.view().reminders.is_none());
}

fn setting(app: &App, key: &str) -> lumenna_app::settings::SettingView {
    app.view().settings.unwrap().settings.into_iter().find(|s| s.setting.key == key).unwrap()
}

#[test]
fn a_setting_applies_and_one_refused_stays_as_typed_beside_why() {
    let mut app = App::new();
    app.send(Event::Settings(SettingsEvent::Open));
    assert_eq!(setting(&app, "day-start").typed, "08:00");
    app.send(Event::Settings(SettingsEvent::Type { key: "day-start".to_owned(), value: "07:".to_owned() }));
    assert_eq!(setting(&app, "day-start").typed, "07:");
    let shell = app.send(Event::Settings(SettingsEvent::Set { key: "day-start".to_owned(), value: "07:00".to_owned() }));
    assert!(!said(&shell).is_empty());
    let day_start = setting(&app, "day-start");
    assert_eq!(day_start.setting.value, "07:00");
    assert_eq!(day_start.problem, None);
    app.send(Event::Settings(SettingsEvent::Set { key: "day-end".to_owned(), value: "teatime".to_owned() }));
    let day_end = setting(&app, "day-end");
    assert_eq!(day_end.typed, "teatime");
    assert!(day_end.problem.is_some());
}

#[test]
fn a_backup_and_an_export_are_files_to_save_and_an_export_imports_into_another_store() {
    let mut app = App::new();
    app.add("Write report");
    app.send(Event::Settings(SettingsEvent::Open));
    let saved = |shell: Vec<Shell>| {
        shell.into_iter().find_map(|s| match s {
            Shell::Save(file) => Some(file),
            _ => None,
        })
    };
    let backup = saved(app.send(Event::Settings(SettingsEvent::Backup))).expect("a backup to save");
    assert!(backup.name.ends_with(".lumbak"));
    assert!(!backup.bytes.is_empty());
    let json = saved(app.send(Event::Settings(SettingsEvent::Export(lumenna_records::types::ExportFormat::Json)))).unwrap();
    assert!(json.name.ends_with(".json"));

    let mut other = App::new();
    let shell = other.send(Event::Settings(SettingsEvent::Import { name: json.name, bytes: json.bytes }));
    assert!(!said(&shell).is_empty());
    assert_eq!(other.titles(), ["Write report"]);
}
