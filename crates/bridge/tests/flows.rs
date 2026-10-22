//! Whole flows through the app and a real store: events in, what is shown and said out.

use std::sync::{Arc, Mutex};

use crux_core::middleware::{EffectMiddleware, EffectResolver, Layer};
use lumenna_app::{
    Asks, BlockEvent, DayMove, DetailsEvent, Effect, Event, Item, NetworkAnswer, NetworkRequest, ProposalEvent,
    SettingsEvent, ScreenId, Typing, ViewModel,
};
use lumenna_bridge::{Shell, Stack, Store, shell, stack};
use lumenna_host::Lumenna;
use lumenna_records::places::Place;
use lumenna_records::{ActionKind, Answer, Question};

/// A request of the network: its id, what it asks, and the way to answer it until answered.
type Request = (u64, NetworkRequest, Option<EffectResolver<NetworkAnswer>>);

/// What the network has been asked.
#[derive(Clone, Default)]
struct Asked(Arc<Mutex<Vec<Request>>>);

/// The network, stood in for: it answers nothing until the test does.
struct StandIn(Asked);

impl EffectMiddleware for StandIn {
    type Op = NetworkRequest;

    fn try_process_effect(&self, request: NetworkRequest, resolver: EffectResolver<NetworkAnswer>) {
        let mut asked = self.0.0.lock().unwrap();
        let id = asked.len() as u64;
        asked.push((id, request, Some(resolver)));
    }
}

/// The app and its store, each call waiting until the app is still, so a test reads each event's
/// whole outcome in order: the store worker on a thread of its own.
struct Session<N: EffectMiddleware<Op = NetworkRequest> + 'static> {
    stack: Stack<N>,
    store: Store,
    /// What the app left for the shell after the call that led to it had returned.
    later: Arc<Mutex<Vec<Effect>>>,
}

impl<N: EffectMiddleware<Op = NetworkRequest> + 'static> Session<N> {
    /// The app over this store, started: it is watching the store, and has it.
    #[must_use]
    fn new(lumenna: Arc<Lumenna>, network: N) -> (Self, Vec<Shell>) {
        let (store, worker) = Store::new(lumenna);
        std::thread::spawn(move || futures::executor::block_on(worker.run()));
        let session = Self { stack: stack(&store, network), store, later: Arc::default() };
        let shell = session.send(Event::Start);
        (session, shell)
    }

    /// Runs an event, and says what it left for the shell once the app is still.
    fn send(&self, event: Event) -> Vec<Shell> {
        let now = self.stack.update(event, self.later());
        self.settled(now)
    }

    /// Runs an event on screen `screen`, and says what it left, once the app is still, for the
    /// shell showing screen `heard`.
    fn send_heard(&self, screen: ScreenId, event: Event, heard: ScreenId) -> Vec<Shell> {
        let mut effects = self.stack.update(Event::On(screen, Box::new(event)), self.later());
        self.store.busy().wait();
        effects.append(&mut self.later.lock().unwrap());
        lumenna_bridge::shell_on(effects, heard)
    }

    /// What screen `screen` shows.
    fn view_on(&self, screen: ScreenId) -> ViewModel {
        self.stack.view().screen(screen)
    }

    /// Looks for a change made elsewhere, or a new minute ([`Store::tick`]).
    fn tick(&self) -> Vec<Shell> {
        self.store.tick();
        self.settled(Vec::new())
    }

    /// What the app has left for the shell since, once it is still: after the network answered.
    fn settle(&self) -> Vec<Shell> {
        self.settled(Vec::new())
    }

    /// What to draw.
    #[must_use]
    fn view(&self) -> ViewModel {
        self.stack.view().screen(0)
    }

    fn later(&self) -> impl Fn(Vec<Effect>) + Send + Sync + 'static {
        let later = Arc::clone(&self.later);
        move |effects| later.lock().unwrap().extend(effects)
    }

    fn settled(&self, mut effects: Vec<Effect>) -> Vec<Shell> {
        self.store.busy().wait();
        effects.append(&mut self.later.lock().unwrap());
        shell(effects)
    }
}

struct App {
    session: Session<StandIn>,
    asked: Asked,
    /// How many of the network's requests the test has seen.
    seen: usize,
    _dir: tempfile::TempDir,
    profile: std::path::PathBuf,
}

impl App {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let lumenna = Lumenna::open(profile.to_str().unwrap()).unwrap();
        let asked = Asked::default();
        let (session, shell) = Session::new(lumenna, StandIn(asked.clone()));
        assert_eq!(shell.first(), Some(&Shell::Render));
        Self { session, asked, seen: 0, _dir: dir, profile }
    }

    /// What the network has been asked since the test last looked.
    fn network(&mut self) -> Vec<(u64, NetworkRequest)> {
        let asked = self.asked.0.lock().unwrap();
        let new = asked[self.seen..].iter().map(|(id, request, _)| (*id, request.clone())).collect();
        self.seen = asked.len();
        new
    }

    /// Answers the network's request `id`, as the platform's runner would; a pairing's stream
    /// stays open until it ends.
    fn answer_network(&mut self, id: u64, answer: NetworkAnswer) -> Vec<Shell> {
        let ends = !matches!(answer, NetworkAnswer::Code(_) | NetworkAnswer::Words(_));
        let taken = self.asked.0.lock().unwrap()[id as usize].2.take();
        let Some(mut resolver) = taken else { panic!("request {id} was answered already") };
        resolver.resolve(answer);
        if !ends {
            self.asked.0.lock().unwrap()[id as usize].2 = Some(resolver);
        }
        self.session.settle()
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

    /// Runs an event on screen `screen`, and says what that screen's shell is left.
    fn send_on(&mut self, screen: ScreenId, event: Event) -> Vec<Shell> {
        self.session.send_heard(screen, event, screen)
    }

    fn view_on(&self, screen: ScreenId) -> ViewModel {
        self.session.view_on(screen)
    }
}

fn said(shell: &[Shell]) -> Vec<&str> {
    shell
        .iter()
        .filter_map(|s| match s {
            Shell::Announce(text) | Shell::Heading(text) => Some(text.as_str()),
            Shell::Render | Shell::Save(_) | Shell::Notify(_) => None,
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
fn a_lists_rows_name_their_actions_in_short_each_offered_once() {
    let mut app = App::new();
    app.add("Write the report");
    app.add("Buy milk");
    let list = app.view().list.unwrap();
    let listed = Lumenna::open(app.profile.to_str().unwrap()).unwrap().list_tasks("").unwrap();
    for item in &list.items {
        assert!(item.row.actions.is_empty(), "none carried whole");
        let named: Vec<&str> = item.actions.iter().map(|at| list.offered[*at as usize].title.as_str()).collect();
        let row = listed.rows.iter().find(|row| row.id == item.row.id).unwrap();
        let whole: Vec<&str> = row.actions.iter().map(|action| action.title.as_str()).collect();
        assert_eq!(named, whole, "each row's own, in its order");
    }
    let every = list.items.iter().map(|item| item.actions.len()).sum::<usize>();
    assert!(list.offered.len() < every, "{} offered for {every} actions", list.offered.len());
    let put = list.offered.iter().find(|o| o.kind == ActionKind::PutInBlock).expect("Put in a Block is offered");
    assert_eq!(put.asks, Asks::Something, "it asks, worded with the task's name only when it does");
}

#[test]
fn an_action_handed_back_in_short_runs_on_the_row_in_hand_whole() {
    let mut app = App::new();
    Lumenna::open(app.profile.to_str().unwrap()).unwrap().add_project("Work", None).unwrap();
    app.add("Buy milk");
    let list = app.view().list.unwrap();
    let row = &list.items[0];
    let short = |kind| row.actions.iter().map(|at| list.offered[*at as usize].clone()).find(|o| o.kind == kind).unwrap();
    app.send(Event::Focus(row.row.id.clone()));

    app.send(Event::Act(short(ActionKind::MoveToProject)));
    let asking = app.view().asking.expect("its question is open");
    let Question::Pick { title, .. } = asking.question else { panic!("a pick: {:?}", asking.question) };
    assert!(title.contains("Buy milk"), "worded with the task's name, as the app has it: {title}");
    app.send(Event::Cancel);

    app.send(Event::Act(short(ActionKind::MarkDone)));
    assert!(app.titles().is_empty(), "done");
}

#[test]
fn an_action_the_row_in_hand_does_not_offer_handed_back_in_short_says_why() {
    let mut app = App::new();
    app.add("Buy milk");
    let list = app.view().list.unwrap();
    let mut restore = list.offered[0].clone();
    restore.kind = ActionKind::Restore;
    restore.title = "Restore".to_owned();
    let shell = app.send(Event::Act(restore));
    assert_eq!(said(&shell), ["That does not apply here."]);
    assert_eq!(app.titles(), ["Buy milk"]);
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
fn a_pick_offers_what_fits_and_a_refused_answer_stays_in_its_question() {
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
    let app = App::new();
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
fn a_place_gone_back_to_has_the_row_in_hand_and_the_filter_it_was_left_with() {
    use lumenna_records::places::Place;
    let mut app = App::new();
    app.add("Buy milk");
    app.add("Call the bank p1");
    app.add("Write to the council p1");
    app.send(Event::Open(Place::Tasks));
    app.send(Event::Filter("p1".to_owned()));
    let council = app.id_of("Write to the council");
    app.send(Event::Focus(council.clone()));
    app.send(Event::Open(Place::Today));
    assert_ne!(app.view().place, Some(Place::Tasks));
    app.send(Event::Open(Place::Tasks));
    assert_eq!(app.view().query, "p1", "the filter it was left with");
    assert_eq!(app.titles(), ["Call the bank", "Write to the council"]);
    assert_eq!(app.view().focus, Some(council), "and the row in hand");
    // Opening the place shown again, as a screen coming back to the front does, keeps both.
    app.send(Event::Open(Place::Tasks));
    assert_eq!(app.view().query, "p1");
}

#[test]
fn a_tick_after_a_change_has_been_taken_in_finds_nothing_new() {
    let app = App::new();
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
fn go_to_day_says_it_asks_for_a_day_and_which_a_calendar_opens_on() {
    let mut app = App::new();
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Next));
    app.send(Event::Day(DayMove::Ask));
    let tomorrow = jiff::Zoned::now().date().tomorrow().unwrap().to_string();
    assert_eq!(app.view().asking.unwrap().day, Some(tomorrow.clone()), "the day shown");
    // Picked from a calendar: its ISO date is the answer.
    app.send(Event::Answer(Answer::Text { text: tomorrow }));
    assert_eq!(app.view().asking, None);
    // Any other question asks for no day.
    app.send(Event::Open(Place::Tasks));
    app.send(Event::Perform(sidebar_action(&app, "Projects", ActionKind::New)));
    let asking = app.view().asking.expect("a new project's name");
    assert_eq!(asking.day, None);
}

#[test]
fn a_blocks_day_opens_from_anywhere_with_that_block_in_hand_and_lands_there() {
    let mut app = App::new();
    let date = jiff::Zoned::now().date().checked_add(jiff::Span::new().days(3)).unwrap().to_string();
    block(&mut app, "Deep work", "09:00", &date, None);
    block(&mut app, "Admin", "14:00", &date, None);
    app.send(Event::Day(DayMove::Ask));
    app.send(Event::Answer(Answer::Text { text: date.clone() }));
    let admin = app.view().day.unwrap().items.into_iter().find(|i| i.block.as_ref().is_some_and(|b| b.title == "Admin")).unwrap();
    let (key, series) = (admin.key, admin.block.unwrap().series);
    app.send(Event::Open(Place::Tasks));
    let landing = app.view().landing;
    // As a reminder names it: the series and the occurrence's day.
    let shell = app.send(Event::ShowBlock { series, date });
    let view = app.view();
    assert_eq!(view.place, Some(Place::Today));
    assert!(!view.day.unwrap().today);
    assert_eq!(view.focus, Some(key));
    assert!(view.landing > landing);
    assert!(shell.iter().any(|s| matches!(s, Shell::Heading(summary) if summary.starts_with("2 blocks"))), "{shell:?}");
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
    let list = app.view().list.unwrap();
    assert!(items[0].actions.iter().any(|at| list.offered[*at as usize].kind == ActionKind::Delete));
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

#[test]
fn quick_add_makes_a_block_from_a_line_that_starts_with_block_reading_it_back_first() {
    let mut app = App::new();
    app.send(Event::Typing(Typing::Open));
    let line = "block \"Deep work\" 9-11am tomorrow weekdays";
    app.send(Event::Typing(Typing::Text { text: line.to_owned(), cursor: u32::try_from(line.len()).unwrap() }));
    let view = app.view().line.unwrap();
    assert_eq!(view.readback, "New block Deep work, 9:00 AM to 11:00 AM, tomorrow, every weekday");
    assert!(!view.has_errors);
    let shell = app.send(Event::QuickAdd(line.to_owned()));
    assert!(said(&shell).iter().any(|s| s.starts_with("Added block Deep work")), "{shell:?}");
    app.send(Event::Open(Place::Blocks));
    let blocks: Vec<String> = app.view().list.unwrap().items.into_iter().map(|i| i.row.title).collect();
    assert_eq!(blocks, ["Deep work"]);

    app.send(Event::Typing(Typing::Text { text: "block Deep work".to_owned(), cursor: 15 }));
    let view = app.view().line.unwrap();
    assert!(view.has_errors);
    assert!(view.readback.contains("say when it runs"), "{}", view.readback);
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
    assert_eq!(form.heading, "New Block");
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
fn a_sitting_moves_up_and_down_its_block_and_one_not_started_is_skipped_and_planned_again() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.add("Write report");
    app.add("Buy milk");
    {
        let host = Lumenna::open(app.profile.to_str().unwrap()).unwrap();
        let block = host.list_blocks().unwrap().rows[0].id.clone();
        let tasks = host.list_tasks("").unwrap();
        for title in ["Write report", "Buy milk"] {
            let task = tasks.rows.iter().find(|r| r.title == title).unwrap().id.clone();
            host.assign(&task, &block, Some("tomorrow".to_owned()), None).unwrap();
        }
    }
    app.session.tick();
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Next));
    let sittings = |app: &App| -> Vec<lumenna_records::PlanAssignment> {
        app.view().day.unwrap().items.into_iter().filter_map(|i| i.sitting).collect()
    };
    let kinds = |s: &lumenna_records::PlanAssignment| s.actions.iter().map(|a| a.kind).collect::<Vec<_>>();
    let [first, second] = <[_; 2]>::try_from(sittings(&app)).unwrap();
    assert_eq!((first.title.as_str(), second.title.as_str()), ("Write report", "Buy milk"));
    assert!(kinds(&first).contains(&ActionKind::MoveDown) && !kinds(&first).contains(&ActionKind::MoveUp));
    assert!(kinds(&second).contains(&ActionKind::MoveUp) && !kinds(&second).contains(&ActionKind::MoveDown));

    let up = second.actions.iter().find(|a| a.kind == ActionKind::MoveUp).unwrap().clone();
    let shell = app.send(Event::Perform(up));
    assert!(said(&shell).contains(&"Moved Buy milk up"), "{shell:?}");
    let titles: Vec<_> = sittings(&app).into_iter().map(|s| s.title).collect();
    assert_eq!(titles, ["Buy milk", "Write report"]);

    let milk = sittings(&app).remove(0);
    let skip = milk.actions.iter().find(|a| a.kind == ActionKind::Skip).expect("Skip, not yet started").clone();
    let shell = app.send(Event::Perform(skip));
    assert!(said(&shell).contains(&"Skipped Buy milk"), "{shell:?}");
    let milk = sittings(&app).remove(0);
    assert_eq!(milk.status, "skipped");
    assert!(!kinds(&milk).contains(&ActionKind::Skip));
    let again = milk.actions.iter().find(|a| a.kind == ActionKind::Unskip).expect("Plan Again").clone();
    assert_eq!(again.title, "Plan Again");
    app.send(Event::Perform(again));
    assert_eq!(sittings(&app).remove(0).status, "planned");
}

#[test]
fn history_lists_what_was_done_newest_first_and_its_rows_are_tasks_with_details() {
    let mut app = App::new();
    app.add("Buy milk");
    app.add("Call the bank");
    app.send(Event::Open(Place::Tasks));
    let tasks = app.view().list.unwrap();
    let milk = tasks.items.iter().find(|i| i.row.title == "Buy milk").unwrap();
    let done = milk.actions.iter().map(|at| tasks.offered[*at as usize].clone()).find(|o| o.kind == ActionKind::MarkDone).unwrap();
    app.send(Event::Focus(milk.row.id.clone()));
    app.send(Event::Act(done));
    app.send(Event::Open(Place::History));
    let view = app.view();
    let list = view.list.unwrap();
    assert_eq!(list.announcement, "1 task done in the last two weeks");
    let row = &list.items[0].row;
    assert_eq!(row.title, "Buy milk");
    assert_eq!(row.due.as_deref(), Some("completed today"));
    assert!(row.due_time.is_some());
    assert_eq!(row.checked, Some(true));
    let offered = |at: &u32| list.offered[*at as usize].kind;
    assert!(list.items[0].actions.iter().any(|at| offered(at) == ActionKind::MarkNotDone), "a task's own actions");
    assert_eq!(view.details.map(|d| d.task.title), Some("Buy milk".to_owned()), "the details show it");
    let sidebar = app.view().sidebar;
    let at = |title: &str| sidebar.iter().position(|e| e.title == title).unwrap();
    assert_eq!(at("History"), at("Blocks") + 1, "beside Trash, after Blocks");
    assert_eq!(at("Trash"), at("History") + 1);
}

#[test]
fn a_saved_filters_query_is_asked_in_the_filter_language_and_its_name_is_not() {
    let mut app = App::new();
    app.send(Event::Perform(sidebar_action(&app, "Saved Filters", ActionKind::New)));
    assert_eq!(app.view().asking.unwrap().syntax, None, "its name first");
    app.send(Event::Answer(Answer::Text { text: "Urgent".to_owned() }));
    assert_eq!(app.view().asking.unwrap().syntax, Some(lumenna_records::Syntax::Filter), "then its query");
    app.send(Event::Answer(Answer::Text { text: "p1".to_owned() }));
    let change = sidebar_action(&app, "Urgent", ActionKind::ChangeQuery);
    app.send(Event::Perform(change));
    assert_eq!(app.view().asking.unwrap().syntax, Some(lumenna_records::Syntax::Filter));
    let task_filter = lumenna_records::block_form().into_iter().find(|f| f.key == "task_filter").unwrap();
    assert_eq!(task_filter.syntax, Some(lumenna_records::Syntax::Filter), "a block's Tasks from");
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
fn a_change_made_from_the_sidebar_lands_there_on_the_row_now_in_its_place_not_in_the_main_list() {
    let mut app = App::new();
    let store = Lumenna::open(app.profile.to_str().unwrap()).unwrap();
    store.add_project("Garden", None).unwrap();
    store.add_project("Work", None).unwrap();
    app.session.tick();
    let keys: Vec<String> = app.view().sidebar.iter().map(|e| e.key.clone()).collect();
    let garden = app.view().sidebar.into_iter().find(|e| e.title == "Garden").unwrap();
    let at = keys.iter().position(|k| *k == garden.key).unwrap();
    app.send(Event::Hold(lumenna_app::Side::Sidebar, garden.key.clone()));
    let (main, side) = (app.view().landing, app.view().sidebar_held.landing);
    app.send(Event::Perform(sidebar_action(&app, "Garden", ActionKind::Delete)));
    app.send(Event::Answer(Answer::Picked { id: "keep".to_owned(), length: None }));
    let view = app.view();
    assert!(view.sidebar.iter().all(|e| e.title != "Garden"));
    // The row now where Garden was is in hand there, and focus is the sidebar's to move.
    assert_eq!(view.sidebar_held.focus.as_deref(), Some(view.sidebar[at].key.as_str()));
    assert!(view.sidebar_held.landing > side);
    assert_eq!(view.landing, main, "the main list's row moved on quietly");
    // Doing anything else hands landing back to the main list: a task added lands there.
    app.send(Event::QuickAdd("Buy milk".to_owned()));
    assert!(app.view().landing > main);
    assert_eq!(app.view().sidebar_held.landing, view.sidebar_held.landing);
}

#[test]
fn a_shell_that_writes_sentence_case_gets_its_headings_buttons_and_answers_in_it() {
    let mut app = App::new();
    Lumenna::open(app.profile.to_str().unwrap()).unwrap().add_project("Work", None).unwrap();
    app.session.tick();
    let heading = |app: &App| app.view().sidebar.into_iter().find(|e| e.key() == "group:Filters").unwrap().title;
    assert_eq!(heading(&app), "Saved Filters");
    app.send(Event::Capitals(lumenna_app::Capitals::Sentence));
    assert_eq!(heading(&app), "Saved filters");

    // The question a delete asks, in the core's capitals and then the shell's: the same answers,
    // each in sentence case, and nothing the core marks as a name changed.
    app.send(Event::Capitals(lumenna_app::Capitals::Title));
    app.send(Event::Perform(sidebar_action(&app, "Work", ActionKind::Delete)));
    let Some(Question::Choose { answers: titled, .. }) = app.view().asking.map(|a| a.question) else { panic!("a choice of answers") };
    app.send(Event::Capitals(lumenna_app::Capitals::Sentence));
    let Some(Question::Choose { answers, .. }) = app.view().asking.map(|a| a.question) else { panic!("a choice of answers") };
    let expected: Vec<String> = titled.iter().map(|a| lumenna_records::sentence_case(a.title.clone())).collect();
    assert_eq!(answers.iter().map(|a| a.title.clone()).collect::<Vec<_>>(), expected);
    assert!(titled.iter().zip(&answers).any(|(t, s)| t.title != s.title), "some answer had a capital to lose");
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

fn pair(app: &mut App, event: lumenna_app::PairEvent) -> Vec<Shell> {
    app.send(Event::Devices(lumenna_app::DevicesEvent::Pair(event)))
}

#[test]
fn pairing_shows_its_code_then_asks_about_the_words_and_ends_closed() {
    use lumenna_app::PairEvent;
    let mut app = App::new();
    app.send(Event::Devices(lumenna_app::DevicesEvent::Show));
    pair(&mut app, PairEvent::Open("Test browser".to_owned()));
    app.network();
    pair(&mut app, PairEvent::Wait);
    let asked = app.network();
    let [(id, NetworkRequest::Pair { code: None, name })] = &asked[..] else { panic!("{asked:?}") };
    let id = *id;
    assert_eq!(name, "Test browser");
    app.answer_network(id, NetworkAnswer::Code("ab12".to_owned()));
    assert_eq!(app.view().devices.unwrap().pairing.unwrap().mine.as_deref(), Some("ab12"));
    app.answer_network(id, NetworkAnswer::Words(vec!["adroitness".to_owned(), "tycoon".to_owned(), "village".to_owned()]));
    assert_eq!(app.view().devices.unwrap().pairing.unwrap().words.map(|w| w.len()), Some(3));
    pair(&mut app, PairEvent::Confirm(true));
    assert!(matches!(app.network()[..], [(_, NetworkRequest::Confirm(true))]));
    let shell = app.answer_network(id, NetworkAnswer::Paired(Ok("Paired with Kitchen Mac".to_owned())));
    assert!(said(&shell).contains(&"Paired with Kitchen Mac"));
    assert!(app.view().devices.unwrap().pairing.is_none());
}

#[test]
fn a_code_typed_while_waiting_gives_up_the_wait_and_then_dials_it() {
    use lumenna_app::PairEvent;
    let mut app = App::new();
    app.send(Event::Devices(lumenna_app::DevicesEvent::Show));
    pair(&mut app, PairEvent::Open("Test browser".to_owned()));
    app.network();
    pair(&mut app, PairEvent::Wait);
    let wait = app.network()[0].0;
    pair(&mut app, PairEvent::Code(" cd34 ".to_owned()));
    pair(&mut app, PairEvent::Join);
    assert!(matches!(app.network()[..], [(_, NetworkRequest::CancelPairing)]));
    app.answer_network(wait, NetworkAnswer::Paired(Err("The pairing was given up.".to_owned())));
    let dialled = app.network();
    assert!(matches!(&dialled[..], [(_, NetworkRequest::Pair { code: Some(code), .. })] if code == "cd34"), "{dialled:?}");
    assert!(app.view().devices.unwrap().pairing.unwrap().problem.is_none());
}

#[test]
fn a_reminder_due_is_listed_once_said_shown_and_dismissed() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    let since = jiff::Timestamp::now().to_string();
    let tz = jiff::tz::TimeZone::system();
    let moment = |minute| jiff::Zoned::now().date().tomorrow().unwrap().at(8, minute, 0, 0).to_zoned(tz.clone()).unwrap().timestamp().to_string();
    let look = |app: &mut App, now: String| app.send(Event::Due(lumenna_app::DueEvent::Look { since: since.clone(), now: Some(now) }));
    let shell = look(&mut app, moment(56));
    let due = app.view().due;
    assert_eq!(due.len(), 1, "{due:?}");
    assert!(due[0].text.starts_with("Deep work starts in 5 minutes"), "{:?}", due[0]);
    assert!(due[0].snooze.is_some());
    assert!(said(&shell).iter().any(|s| s.starts_with("Deep work starts")));
    assert!(shell.iter().any(|s| matches!(s, Shell::Notify(n) if n.id == due[0].id)));
    // Looking again lists nothing twice.
    look(&mut app, moment(57));
    assert_eq!(app.view().due.len(), 1);
    app.send(Event::Due(lumenna_app::DueEvent::Dismiss(due[0].id.clone())));
    assert!(app.view().due.is_empty());
}

#[test]
fn a_sidebar_heading_folds_through_its_own_collapse_and_says_so() {
    let mut app = App::new();
    let collapse = sidebar_action(&app, "Projects", ActionKind::Collapse);
    let shell = app.send(Event::Perform(collapse));
    assert_eq!(said(&shell), ["Collapsed"]);
    assert_eq!(app.view().sidebar_folded, ["group:Projects"]);
    let expand = sidebar_action(&app, "Projects", ActionKind::Expand);
    app.send(Event::Perform(expand));
    assert!(app.view().sidebar_folded.is_empty());
}

#[test]
fn focus_lands_when_the_app_moves_it_and_never_for_the_persons_own_move() {
    let mut app = App::new();
    app.add("Buy milk");
    app.add("Call Sam");
    let before = app.view().landing;
    app.send(Event::Focus(app.id_of("Call Sam")));
    assert_eq!(app.view().landing, before, "the person moved there: nothing to land");
    app.send(Event::Run(vec![ActionKind::MarkDone]));
    assert!(app.view().landing > before, "the app put the next row in hand");
}

#[test]
fn a_write_answered_after_the_person_moved_leaves_the_row_they_moved_to_in_hand() {
    use crux_core::Core;
    // The core alone, answered by hand, so the write's answer can come after the move.
    let dir = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(dir.path().join("profile").to_str().unwrap()).unwrap();
    let core: Core<lumenna_app::Lumenna> = Core::new();
    let mut held = Vec::new();
    let run = |effects: Vec<Effect>, held: &mut Vec<Effect>| {
        let mut effects = effects;
        while !effects.is_empty() {
            let mut next = Vec::new();
            for effect in effects {
                match effect {
                    Effect::Watch(mut request) => {
                        let (arrival, _) = lumenna_bridge::arrival(&lumenna).unwrap();
                        next.extend(core.resolve(&mut request, arrival).unwrap());
                    }
                    Effect::Host(request) => held.push(Effect::Host(request)),
                    _ => {}
                }
            }
            effects = next;
        }
    };
    let answer = |held: &mut Vec<Effect>| {
        let mut effects = Vec::new();
        for effect in held.drain(..) {
            if let Effect::Host(request) = effect {
                let (operation, mut handle) = request.split();
                effects.extend(core.resolve(&mut handle, lumenna_bridge::answer(&lumenna, operation)).unwrap());
            }
        }
        effects
    };
    run(core.process_event(Event::Start), &mut held);
    run(core.process_event(Event::QuickAdd("Buy milk".to_owned())), &mut held);
    let effects = answer(&mut held);
    run(effects, &mut held);
    run(core.process_event(Event::QuickAdd("Call Sam".to_owned())), &mut held);
    let milk = core.view().screen(0).list.unwrap().items.into_iter().find(|i| i.row.title == "Buy milk").unwrap().row.id;
    run(core.process_event(Event::Focus(milk.clone())), &mut held);
    let effects = answer(&mut held);
    run(effects, &mut held);
    let view = core.view().screen(0);
    assert_eq!(view.list.unwrap().items.len(), 2, "Call Sam was added");
    assert_eq!(view.focus.as_deref(), Some(milk.as_str()), "the person moved to Buy milk after adding Call Sam");
}

#[test]
fn looks_for_reminders_asked_for_while_one_is_with_the_host_go_as_one_after_it() {
    use crux_core::Core;
    use lumenna_app::{DueEvent, HostRequest};
    let core: Core<lumenna_app::Lumenna> = Core::new();
    let looks = |effects: &[Effect]| -> Vec<(String, Option<String>)> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Host(request) => match &request.operation {
                    HostRequest::RemindersDue { since, now } => Some((since.clone(), now.clone())),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    };
    let look = |since: &str, now: &str| Event::Due(DueEvent::Look { since: since.to_owned(), now: Some(now.to_owned()) });
    let mut first = core.process_event(look("2030-01-15T08:00:00Z", "2030-01-15T08:01:00Z"));
    assert_eq!(looks(&first), [("2030-01-15T08:00:00Z".to_owned(), Some("2030-01-15T08:01:00Z".to_owned()))]);
    assert!(looks(&core.process_event(look("2030-01-15T08:01:00Z", "2030-01-15T08:02:00Z"))).is_empty(), "one at a time");
    assert!(looks(&core.process_event(look("2030-01-15T08:02:00Z", "2030-01-15T08:03:00Z"))).is_empty());
    let Some(Effect::Host(request)) = first.pop() else { panic!("a look with the host") };
    let (_, mut handle) = request.split();
    let found = lumenna_app::HostAnswer::Found(Ok(lumenna_records::reminders::DueReminders {
        announcement: String::new(),
        notices: Vec::new(),
        due: Vec::new(),
        missed: Vec::new(),
        missed_summary: String::new(),
        elsewhere: false,
    }));
    let next = core.resolve(&mut handle, found).unwrap();
    assert_eq!(
        looks(&next),
        [("2030-01-15T08:01:00Z".to_owned(), Some("2030-01-15T08:03:00Z".to_owned()))],
        "the two asked for meanwhile, as one",
    );
}

// MARK: - Screens

#[test]
fn two_screens_each_keep_their_own_place_row_in_hand_and_question() {
    let mut app = App::new();
    app.add("Buy milk");
    app.add("Call Sam");
    app.send(Event::Open(Place::Tasks));
    let milk = app.id_of("Buy milk");
    app.send(Event::Focus(milk.clone()));
    app.send_on(1, Event::Open(Place::Today));
    app.send_on(1, Event::Day(DayMove::Ask));
    let (first, second) = (app.view(), app.view_on(1));
    assert_eq!(first.place, Some(Place::Tasks));
    assert_eq!(first.focus, Some(milk));
    assert_eq!(first.asking, None, "the question is open on the screen that asked it");
    assert_eq!(second.place, Some(Place::Today));
    assert!(second.asking.is_some());
}

#[test]
fn a_write_on_one_screen_lands_and_is_said_there_and_the_other_shows_it_quietly() {
    let mut app = App::new();
    app.add("Buy milk");
    app.send(Event::Open(Place::Tasks));
    app.send_on(1, Event::Open(Place::Tasks));
    let (first, second) = (app.view().landing, app.view_on(1).landing);
    let there = app.send_on(1, Event::QuickAdd("Call Sam".to_owned()));
    assert!(said(&there).iter().any(|s| s.contains("Call Sam")), "{there:?}");
    assert!(app.view_on(1).landing > second, "it lands where it was made");
    assert_eq!(app.view().landing, first, "and nowhere else");
    assert!(app.titles().contains(&"Call Sam".to_owned()), "both screens show the store as it is");
    let elsewhere = app.session.send_heard(1, Event::QuickAdd("Pay rent".to_owned()), 0);
    assert!(said(&elsewhere).is_empty(), "{elsewhere:?}");
}

#[test]
fn a_new_screen_opens_over_the_store_and_a_closed_one_is_not_opened_again_by_it() {
    let mut app = App::new();
    app.add("Buy milk");
    app.send_on(1, Event::Open(Place::Tasks));
    assert!(app.view_on(1).list.unwrap().items.iter().any(|i| i.row.title == "Buy milk"), "it has the store at once");
    app.send(Event::CloseScreen(1));
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    assert_eq!(app.view_on(1), ViewModel::default(), "the store moving leaves it closed");
    assert!(app.titles().contains(&"Buy milk".to_owned()), "the first screen goes on");
}

#[test]
fn a_reminder_found_on_one_screen_is_due_on_every_screen_and_dismissed_from_any() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.send_on(1, Event::Open(Place::Tasks));
    let since = jiff::Timestamp::now().to_string();
    let tz = jiff::tz::TimeZone::system();
    let now = jiff::Zoned::now().date().tomorrow().unwrap().at(8, 56, 0, 0).to_zoned(tz).unwrap().timestamp().to_string();
    app.send_on(1, Event::Due(lumenna_app::DueEvent::Look { since, now: Some(now) }));
    let due = app.view().due;
    assert_eq!(due.len(), 1, "the device's reminders, on the first screen too: {due:?}");
    app.send(Event::Due(lumenna_app::DueEvent::Dismiss(due[0].id.clone())));
    assert!(app.view_on(1).due.is_empty(), "dealt with on one screen, gone from the other");
}

// MARK: - What the app asks itself

#[test]
fn put_in_a_block_ends_in_another_day_which_asks_the_day_and_offers_its_blocks() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    let far = jiff::Zoned::now().date().checked_add(jiff::Span::new().days(20)).unwrap().to_string();
    block(&mut app, "Far off", "10:00", &far, None);
    app.add("Write the report");
    app.send(Event::Run(vec![ActionKind::PutInBlock]));
    let choices = app.view().asking.unwrap().choices.unwrap().choices;
    let last = choices.last().unwrap();
    assert_eq!((last.id.as_str(), last.title.as_str()), (lumenna_app::ANOTHER_DAY, "Another Day"));
    app.send(Event::Answer(Answer::Picked { id: lumenna_app::ANOTHER_DAY.to_owned(), length: None }));
    let asking = app.view().asking.unwrap();
    assert!(matches!(asking.question, Question::Text { .. }), "which day: {:?}", asking.question);
    app.send(Event::Answer(Answer::Text { text: "the twelfth of never".to_owned() }));
    assert!(app.view().asking.unwrap().refusal.is_some(), "a day it cannot read stays asked, with why");
    app.send(Event::Answer(Answer::Text { text: far }));
    let asking = app.view().asking.unwrap();
    assert!(matches!(asking.question, Question::Pick { .. }), "back to the pick");
    let titles: Vec<String> = asking.choices.unwrap().choices.into_iter().map(|c| c.title).collect();
    assert!(titles.iter().any(|t| t == "Far off"), "{titles:?}");
}

#[test]
fn a_key_a_side_lists_row_does_not_offer_is_refused_there_in_the_cores_words() {
    let mut app = App::new();
    app.send(Event::Hold(lumenna_app::Side::Sidebar, "place:Project(\"Inbox\")".to_owned()));
    let shell = app.send(Event::Run(vec![ActionKind::Rename]));
    assert!(said(&shell).iter().any(|s| s.starts_with("The Inbox keeps its name")), "{shell:?}");
}

#[test]
fn unpair_on_this_devices_own_row_says_it_is_the_device_in_use() {
    use lumenna_records::{DeviceList, DeviceView};
    let mut app = App::new();
    app.send(Event::Devices(lumenna_app::DevicesEvent::Show));
    // Paired, as the host would list it: a device is listed only once paired.
    let this = DeviceView {
        name: "This PC".to_owned(),
        platform: "windows".to_owned(),
        node_id: "ab12".to_owned(),
        this_device: true,
        paired_at: "2026-10-20T00:00:00Z".to_owned(),
        last_attempt: None,
        last_success: None,
        last_error: None,
        schema_version: 1,
        status: Vec::new(),
        actions: lumenna_records::actions::device("This PC", "ab12", true),
    };
    let list = DeviceList { announcement: String::new(), notices: Vec::new(), devices: vec![this], empty: String::new() };
    app.send(Event::Answered(lumenna_app::HostAnswer::Devices(Ok(list))));
    app.send(Event::Hold(lumenna_app::Side::Devices, "ab12".to_owned()));
    let shell = app.send(Event::Run(vec![ActionKind::Unpair]));
    assert!(said(&shell).iter().any(|s| s.starts_with("This is the device you are using")), "{shell:?}");
}

#[test]
fn pairings_words_come_with_the_devices_as_the_shell_is_called_in_them() {
    let mut app = App::new();
    app.send(Event::PairingAs { called: "this watch".to_owned(), nearby: true });
    app.send(Event::Devices(lumenna_app::DevicesEvent::Show));
    let words = app.view().devices.unwrap().words;
    assert!(words.waiting.contains("this watch"), "{}", words.waiting);
    app.send(Event::PairingAs { called: "this browser".to_owned(), nearby: false });
    assert!(!app.view().devices.unwrap().words.waiting.contains("On this network"), "by code alone");
}

#[test]
fn a_reminder_without_carry_forward_refuses_it_in_the_cores_words() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    let since = jiff::Timestamp::now().to_string();
    let tz = jiff::tz::TimeZone::system();
    let now = jiff::Zoned::now().date().tomorrow().unwrap().at(8, 56, 0, 0).to_zoned(tz).unwrap().timestamp().to_string();
    app.send(Event::Due(lumenna_app::DueEvent::Look { since, now: Some(now) }));
    let id = app.view().due[0].id.clone();
    let shell = app.send(Event::Due(lumenna_app::DueEvent::Carry(id.clone())));
    assert!(said(&shell).iter().any(|s| s.contains("does not apply")), "{shell:?}");
    assert_eq!(app.view().due.len(), 1, "and it stays due");
}

#[test]
fn a_days_heading_refuses_an_action_the_day_lacks_as_the_days() {
    let mut app = App::new();
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Previous));
    app.send(Event::Focus("summary".to_owned()));
    let shell = app.send(Event::Run(vec![ActionKind::Replan]));
    assert!(said(&shell).iter().any(|s| s.contains("does not apply")), "{shell:?}");
}

#[test]
fn the_details_run_the_task_they_show_and_refuse_what_it_lacks() {
    let mut app = App::new();
    app.add("Write the report");
    let task = app.id_of("Write the report");
    app.send(Event::Open(Place::Today));
    app.send(Event::Details(DetailsEvent::Show(task.clone())));
    let refused = app.send(Event::Details(DetailsEvent::Run(vec![ActionKind::Restore])));
    assert!(said(&refused).iter().any(|s| s.contains("does not apply") || s.contains("not in the trash")), "{refused:?}");
    app.send(Event::Details(DetailsEvent::Run(vec![ActionKind::MarkDone])));
    assert!(app.view().details.unwrap().task.state.iter().any(|s| s == "done" || s == "completed"), "the task shown, done");
}

#[test]
fn a_days_own_action_runs_whatever_row_is_in_hand_and_moves_none() {
    let mut app = App::new();
    block(&mut app, "Deep work", "09:00", "tomorrow", None);
    app.send(Event::Open(Place::Today));
    app.send(Event::Day(DayMove::Next));
    let block = day_keys(&app).into_iter().find(|k| k.starts_with("block:")).unwrap();
    app.send(Event::Focus(block.clone()));
    app.send(Event::RunDay(vec![ActionKind::PlanDay]));
    assert!(app.view().proposal.is_some(), "Plan the Day, from a block's row");
    assert_eq!(app.view().focus, Some(block), "and the block stays in hand");
    app.send(Event::Proposal(lumenna_app::ProposalEvent::Close));
    let shell = app.send(Event::RunDay(vec![ActionKind::EndDay]));
    assert!(said(&shell).iter().any(|s| s.contains("does not apply")), "a day not today has no End the Day: {shell:?}");
}
