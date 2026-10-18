//! Whole flows through the app and a real store: events in, what is shown and said out.

use lumenna_app::{DayMove, DetailsEvent, Event, Item, Typing, ViewModel};
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
            Shell::Render => None,
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
