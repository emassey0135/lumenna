//! The filter language's meaning (§6.2) and its readback (§6.3).

use jiff::civil::{date, time};
use jiff::Zoned;
use lumenna_core::filter::{Context, DueFilter, Expr, NameKind, Predicate};
use lumenna_core::id::{ProjectId, TaskId};
use lumenna_core::model::{
    BlockAssignment, BlockKind, BlockRef, BlockSeries, Due, Label, Priority, Project, Task,
    TaskCompletion,
};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_core::state::State;

/// Wednesday 2026-05-06, mid-afternoon.
fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

struct World {
    snapshot: Snapshot,
    inbox: ProjectId,
    work: ProjectId,
    backend: ProjectId,
}

impl World {
    fn new() -> Self {
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let work = Project::new("Work", OrderKey::middle());
        let mut backend = Project::new("Backend", OrderKey::middle());
        backend.parent_id = Some(work.id);
        let (inbox_id, work_id, backend_id) = (inbox.id, work.id, backend.id);
        for p in [inbox, work, backend] {
            snapshot.projects.insert(p.id, p);
        }
        for name in ["laptop", "waiting"] {
            let label = Label::new(name, OrderKey::middle());
            snapshot.labels.insert(label.id, label);
        }
        Self { snapshot, inbox: inbox_id, work: work_id, backend: backend_id }
    }

    fn add(&mut self, project: ProjectId, title: &str) -> TaskId {
        let task = Task::new(project, title, OrderKey::middle());
        let id = task.id;
        self.snapshot.tasks.insert(id, task);
        id
    }

    fn label(&mut self, task: TaskId, name: &str) {
        let id = self.snapshot.label_by_name(name).unwrap().id;
        self.snapshot.tasks.get_mut(&task).unwrap().labels.insert(id);
    }

    fn titles(&self, expr: &Expr) -> Vec<String> {
        let now = now();
        let cx = Context::new(&self.snapshot, &now);
        expr.select(&cx).into_iter().map(|t| t.title.clone()).collect()
    }
}

/// Expected titles, so assertions read as the list they are.
fn expect(titles: &[&str]) -> Vec<String> {
    titles.iter().map(|s| (*s).to_owned()).collect()
}

fn project(name: &str) -> Expr {
    Expr::Predicate(Predicate::Project { name: name.to_owned(), include_descendants: false })
}

fn under(name: &str) -> Expr {
    Expr::Predicate(Predicate::Project { name: name.to_owned(), include_descendants: true })
}

fn label(name: &str) -> Expr {
    Expr::Predicate(Predicate::Label(name.to_owned()))
}

fn state(s: State) -> Expr {
    Expr::Predicate(Predicate::State(s))
}

// ---------------------------------------------------------------------------------------
// Predicates
// ---------------------------------------------------------------------------------------

#[test]
fn an_empty_query_matches_everything() {
    let mut world = World::new();
    world.add(world.inbox, "a");
    world.add(world.work, "b");
    assert_eq!(world.titles(&Expr::All).len(), 2);
}

#[test]
fn a_single_hash_matches_exactly_that_project() {
    let mut world = World::new();
    world.add(world.work, "in work");
    world.add(world.backend, "in backend");
    assert_eq!(world.titles(&project("Work")), expect(&["in work"]));
}

#[test]
fn a_double_hash_matches_the_whole_subtree_to_any_depth() {
    let mut world = World::new();
    let deep = Project::new("Deep", OrderKey::middle());
    let deep_id = deep.id;
    world.snapshot.projects.insert(deep.id, deep);
    world.snapshot.projects.get_mut(&deep_id).unwrap().parent_id = Some(world.backend);

    world.add(world.work, "in work");
    world.add(world.backend, "in backend");
    world.add(deep_id, "three levels down");
    world.add(world.inbox, "elsewhere");

    let mut found = world.titles(&under("Work"));
    found.sort_unstable();
    assert_eq!(found, expect(&["in backend", "in work", "three levels down"]));
}

#[test]
fn project_names_match_without_regard_to_case() {
    let mut world = World::new();
    world.add(world.work, "t");
    assert_eq!(world.titles(&project("work")).len(), 1);
    assert_eq!(world.titles(&project("WORK")).len(), 1);
}

#[test]
fn an_unknown_project_matches_nothing_rather_than_everything() {
    let mut world = World::new();
    world.add(world.work, "t");
    assert!(world.titles(&project("Nonexistent")).is_empty());
}

#[test]
fn a_project_cycle_does_not_hang_the_closure_walk() {
    // §3.13's repair should have run, but a filter that hangs takes the UI with it.
    let mut world = World::new();
    world.snapshot.projects.get_mut(&world.work).unwrap().parent_id = Some(world.backend);
    world.add(world.backend, "t");
    assert!(world.titles(&under("Work")).len() <= 1);
}

#[test]
fn labels_and_priorities_select() {
    let mut world = World::new();
    let a = world.add(world.work, "labelled");
    world.add(world.work, "bare");
    world.label(a, "laptop");
    world.snapshot.tasks.get_mut(&a).unwrap().priority = Priority::P1;

    assert_eq!(world.titles(&label("laptop")), expect(&["labelled"]));
    assert_eq!(world.titles(&Expr::Predicate(Predicate::Priority(Priority::P1))), expect(&["labelled"]));
    assert_eq!(world.titles(&Expr::Predicate(Predicate::Priority(Priority::P4))), expect(&["bare"]));
}

#[test]
fn free_text_searches_titles_and_notes() {
    let mut world = World::new();
    let a = world.add(world.work, "Pay the INVOICE");
    let b = world.add(world.work, "Something else");
    world.snapshot.tasks.get_mut(&b).unwrap().notes = "mentions invoice in the body".to_owned();
    world.add(world.work, "unrelated");

    let mut found = world.titles(&Expr::Predicate(Predicate::Search("invoice".to_owned())));
    found.sort_unstable();
    assert_eq!(found, expect(&["Pay the INVOICE", "Something else"]));
    assert!(world.snapshot.tasks.contains_key(&a));
}

#[test]
fn due_filters_cover_the_shapes_people_ask_for() {
    let mut world = World::new();
    let cases = [
        ("yesterday", date(2026, 5, 5)),
        ("today", date(2026, 5, 6)),
        ("in three days", date(2026, 5, 9)),
        ("next month", date(2026, 6, 20)),
    ];
    for (title, on) in cases {
        let id = world.add(world.work, title);
        world.snapshot.tasks.get_mut(&id).unwrap().due = Some(Due::on(on));
    }

    let due = |f| Expr::Predicate(Predicate::Due(f));
    assert_eq!(world.titles(&due(DueFilter::Today)), expect(&["today"]));

    let mut within = world.titles(&due(DueFilter::Within { days: 7 }));
    within.sort_unstable();
    assert_eq!(within, expect(&["in three days", "today"]));

    let mut before = world.titles(&due(DueFilter::Before(lumenna_core::time::DateSpec::Today)));
    before.sort_unstable();
    assert_eq!(before, expect(&["yesterday"]));

    let after = world.titles(&due(DueFilter::After(lumenna_core::time::DateSpec::Today)));
    let mut after = after;
    after.sort_unstable();
    assert_eq!(after, expect(&["in three days", "next month"]));
}

#[test]
fn a_task_with_no_due_date_satisfies_no_due_condition() {
    let mut world = World::new();
    world.add(world.work, "undated");
    let due = |f| Expr::Predicate(Predicate::Due(f));
    assert!(world.titles(&due(DueFilter::Today)).is_empty());
    assert!(world.titles(&due(DueFilter::Within { days: 30 })).is_empty());
    // Selecting for the absence is a state, not a date condition.
    assert_eq!(world.titles(&state(State::NoDate)), expect(&["undated"]));
}

#[test]
fn assignment_matches_a_recurring_occurrence_and_a_one_off_alike() {
    let mut world = World::new();
    let recurring_task = world.add(world.work, "in a recurring block");
    let one_off_task = world.add(world.work, "in a one-off block");
    world.add(world.work, "unassigned");

    let series = BlockSeries {
        rrule: Some("FREQ=DAILY".to_owned()),
        end_date: None,
        ..BlockSeries::one_off("Focus", BlockKind::Work, date(2026, 5, 1), time(9, 0, 0, 0), 60)
            .unwrap()
    };
    let one_off = BlockSeries::one_off(
        "Dentist",
        BlockKind::Event,
        date(2026, 5, 6),
        time(11, 0, 0, 0),
        60,
    )
    .unwrap();

    let a = BlockAssignment::new(
        BlockRef::Occurrence(series.id, date(2026, 5, 6)),
        recurring_task,
        OrderKey::middle(),
    );
    let b = BlockAssignment::new(BlockRef::OneOff(one_off.id), one_off_task, OrderKey::middle());
    world.snapshot.series.insert(series.id, series);
    world.snapshot.series.insert(one_off.id, one_off);
    world.snapshot.assignments.insert(a.id, a);
    world.snapshot.assignments.insert(b.id, b);

    let assigned_today =
        Expr::Predicate(Predicate::Assigned(lumenna_core::time::DateSpec::Today));
    let mut found = world.titles(&assigned_today);
    found.sort_unstable();
    assert_eq!(found, expect(&["in a one-off block", "in a recurring block"]));
    assert_eq!(world.titles(&state(State::Unassigned)), expect(&["unassigned"]));
}

// ---------------------------------------------------------------------------------------
// Booleans and list defaults
// ---------------------------------------------------------------------------------------

#[test]
fn the_canonical_query_composes_correctly() {
    // #work & (p1 | overdue) & !@waiting
    let mut world = World::new();
    let keep = world.add(world.work, "p1 in work");
    world.snapshot.tasks.get_mut(&keep).unwrap().priority = Priority::P1;

    let overdue = world.add(world.work, "overdue in work");
    world.snapshot.tasks.get_mut(&overdue).unwrap().due = Some(Due::on(date(2026, 1, 1)));

    let waiting = world.add(world.work, "p1 but waiting");
    world.snapshot.tasks.get_mut(&waiting).unwrap().priority = Priority::P1;
    world.label(waiting, "waiting");

    world.add(world.work, "ordinary work task");
    let elsewhere = world.add(world.inbox, "p1 elsewhere");
    world.snapshot.tasks.get_mut(&elsewhere).unwrap().priority = Priority::P1;

    let query = Expr::And(vec![
        project("Work"),
        Expr::Or(vec![
            Expr::Predicate(Predicate::Priority(Priority::P1)),
            state(State::Overdue),
        ]),
        Expr::Not(Box::new(label("waiting"))),
    ]);

    let mut found = world.titles(&query);
    found.sort_unstable();
    assert_eq!(found, expect(&["overdue in work", "p1 in work"]));
}

#[test]
fn completed_and_trashed_tasks_stay_out_unless_asked_for() {
    let mut world = World::new();
    let done = world.add(world.work, "finished");
    let gone = world.add(world.work, "trashed");
    world.add(world.work, "live");

    let completion = TaskCompletion::new(done, None);
    world.snapshot.completions.insert(completion.id, completion);
    world.snapshot.tasks.get_mut(&gone).unwrap().deleted_at = Some(lumenna_core::time::now());

    assert_eq!(world.titles(&Expr::All), expect(&["live"]));
    assert_eq!(world.titles(&state(State::Completed)), expect(&["finished"]));
    assert_eq!(world.titles(&state(State::Deleted)), expect(&["trashed"]));
}

#[test]
fn mentioning_a_state_under_a_negation_still_opts_in() {
    // `!completed` is a review query, and it has to reach the completed tasks to exclude
    // them — otherwise the default filter would silently do the work and the negation would
    // look like it had no effect.
    let mut world = World::new();
    let done = world.add(world.work, "finished");
    world.add(world.work, "live");
    let completion = TaskCompletion::new(done, None);
    world.snapshot.completions.insert(completion.id, completion);

    let query = Expr::Not(Box::new(state(State::Completed)));
    assert!(query.mentions(State::Completed));
    assert_eq!(world.titles(&query), expect(&["live"]));
}

#[test]
fn results_come_out_in_a_stable_order() {
    let mut world = World::new();
    let mut order = OrderKey::middle();
    for title in ["first", "second", "third"] {
        let mut task = Task::new(world.work, title, order.clone());
        order = OrderKey::after(&order);
        task.priority = Priority::P1;
        world.snapshot.tasks.insert(task.id, task);
    }
    assert_eq!(world.titles(&Expr::All), expect(&["first", "second", "third"]));
}

// ---------------------------------------------------------------------------------------
// Resolution and readback
// ---------------------------------------------------------------------------------------

#[test]
fn unknown_names_are_reported_with_the_nearest_match() {
    let world = World::new();
    let query = Expr::And(vec![project("Wrok"), label("lapto"), label("laptop")]);
    let found = query.unresolved(&world.snapshot);

    assert_eq!(found.len(), 2, "the real label is not reported");
    assert_eq!(found[0].kind, NameKind::Project);
    assert_eq!(found[0].name, "Wrok");
    assert_eq!(found[0].suggestion.as_deref(), Some("Work"));
    assert_eq!(found[1].kind, NameKind::Label);
    assert_eq!(found[1].suggestion.as_deref(), Some("laptop"));
    assert_eq!(found[0].kind.noun(), "project");
    assert_eq!(found[1].kind.noun(), "label");
}

#[test]
fn a_query_reads_back_as_a_sentence() {
    // §6.3: a mis-parsed filter shows wrong results silently, and wrong results are
    // invisible. The readback is the only way to notice.
    let query = Expr::And(vec![
        project("Work"),
        Expr::Or(vec![
            Expr::Predicate(Predicate::Priority(Priority::P1)),
            state(State::Overdue),
        ]),
        Expr::Not(Box::new(label("waiting"))),
    ]);
    assert_eq!(
        query.describe(),
        "tasks in Work, and either priority 1 or overdue, and not labelled waiting"
    );
}

#[test]
fn every_predicate_shape_reads_back() {
    assert_eq!(Expr::All.describe(), "all tasks");
    assert_eq!(under("Work").describe(), "tasks in Work or anything under it");
    assert_eq!(
        Expr::Predicate(Predicate::Due(DueFilter::Within { days: 7 })).describe(),
        "tasks due within 7 days"
    );
    assert_eq!(
        Expr::Predicate(Predicate::Due(DueFilter::Within { days: 1 })).describe(),
        "tasks due within 1 day"
    );
    assert_eq!(
        Expr::Predicate(Predicate::Due(DueFilter::Before(lumenna_core::time::DateSpec::Weekday {
            day: jiff::civil::Weekday::Friday,
            which: lumenna_core::time::Which::Next,
        })))
        .describe(),
        "tasks due before next Friday"
    );
    assert_eq!(
        Expr::Predicate(Predicate::Search("invoice".to_owned())).describe(),
        "tasks matching \"invoice\""
    );
    assert_eq!(state(State::NoEstimate).describe(), "tasks no estimate");
}

#[test]
fn every_state_is_selectable_and_describable() {
    // §6.2's rule: adding a computed state means adding a State variant, and anything a
    // filter can select on is something a screen reader can announce. This fails loudly if
    // the two lists ever drift.
    let world = World::new();
    let now = now();
    let cx = Context::new(&world.snapshot, &now);
    for s in State::ALL {
        let expr = state(*s);
        assert_eq!(State::from_keyword(s.keyword()), Some(*s));
        assert!(!expr.describe().is_empty());
        let _ = expr.select(&cx);
    }
}

#[test]
fn a_task_whose_parent_was_filtered_out_nests_under_its_grandparent() {
    let mut world = World::new();
    let release = world.add(world.work, "Ship release");
    let notes = world.add(world.work, "Write notes");
    let proofread = world.add(world.work, "Proofread");
    world.snapshot.tasks.get_mut(&notes).unwrap().parent_id = Some(release);
    world.snapshot.tasks.get_mut(&proofread).unwrap().parent_id = Some(notes);
    for id in [release, proofread] {
        world.snapshot.tasks.get_mut(&id).unwrap().priority = Priority::P1;
    }

    let now = now();
    let cx = Context::new(&world.snapshot, &now);
    let rows = world.snapshot.task_rows(&Expr::Predicate(Predicate::Priority(Priority::P1)), &cx);
    let shape: Vec<(&str, u32)> = rows.iter().map(|r| (r.title.as_str(), r.depth)).collect();
    assert_eq!(shape, vec![("Ship release", 0), ("Proofread", 1)]);
}
