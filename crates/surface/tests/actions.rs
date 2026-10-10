//! What can be done to each row, decided once for every client, and doing it.

use lumenna_surface::{Action, ActionKind, Answer, Lumenna, NewBlock, Question, Subject};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn titles(actions: &[Action]) -> Vec<&str> {
    actions.iter().map(|a| a.title.as_str()).collect()
}

fn find(actions: &[Action], title: &str) -> Action {
    actions.iter().find(|a| a.title == title).unwrap_or_else(|| panic!("no {title} in {:?}", titles(actions))).clone()
}

fn task_actions(lumenna: &Lumenna, title: &str) -> Vec<Action> {
    let rows = lumenna.list_tasks("").unwrap().rows;
    rows.into_iter().find(|r| r.title == title).unwrap().actions
}

fn add(lumenna: &Lumenna, text: &str) -> String {
    lumenna.add_task(text).unwrap().affected.tasks[0].clone()
}

#[test]
fn a_tasks_actions_come_in_the_order_every_client_offers_them() {
    let (_directory, lumenna) = open();
    add(&lumenna, "write report");
    assert_eq!(
        titles(&task_actions(&lumenna, "write report")),
        ["Mark Done", "Edit Details", "Put in a Block", "Move to Project", "Make Subtask Of", "Wait For", "Move to Trash"]
    );
}

#[test]
fn a_subtask_can_move_to_the_top_and_a_waiting_task_can_stop_waiting() {
    let (_directory, lumenna) = open();
    let parent = add(&lumenna, "write report");
    let child = add(&lumenna, "outline");
    let other = add(&lumenna, "gather figures");
    lumenna.move_task(&child, lumenna_surface::MoveTarget::Parent { id: parent }).unwrap();
    lumenna.add_dependency(&child, &other).unwrap();
    let actions = task_actions(&lumenna, "outline");
    assert!(titles(&actions).contains(&"Move to Top Level"));
    let stop = find(&actions, "Stop Waiting for gather figures");
    assert!(lumenna.act(stop, Answer::Yes).unwrap().changed);
    assert!(lumenna.show_task(&child).unwrap().task.depends.is_empty());
}

#[test]
fn a_task_is_never_offered_as_a_parent_of_itself_or_of_its_ancestors() {
    let (_directory, lumenna) = open();
    let top = add(&lumenna, "top");
    let middle = add(&lumenna, "middle");
    add(&lumenna, "elsewhere");
    lumenna.move_task(&middle, lumenna_surface::MoveTarget::Parent { id: top }).unwrap();
    let offered = lumenna.choices(find(&task_actions(&lumenna, "top"), "Make Subtask Of")).unwrap();
    let names: Vec<&str> = offered.choices.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(names, ["elsewhere"], "not itself, and not what is already under it");
}

#[test]
fn waiting_is_never_offered_where_it_would_close_a_loop() {
    let (_directory, lumenna) = open();
    let a = add(&lumenna, "a");
    let b = add(&lumenna, "b");
    add(&lumenna, "c");
    lumenna.add_dependency(&b, &a).unwrap();
    let offered = lumenna.choices(find(&task_actions(&lumenna, "a"), "Wait For")).unwrap();
    let names: Vec<&str> = offered.choices.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(names, ["c"], "b already waits for a");
    let offered = lumenna.choices(find(&task_actions(&lumenna, "b"), "Wait For")).unwrap();
    let names: Vec<&str> = offered.choices.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(names, ["c"], "it already waits for a");
}

#[test]
fn marking_done_through_an_action_offers_mark_not_done_after() {
    let (_directory, lumenna) = open();
    let id = add(&lumenna, "water plants");
    let done = find(&task_actions(&lumenna, "water plants"), "Mark Done");
    assert_eq!(done.question, Question::Immediate);
    lumenna.act(done, Answer::Yes).unwrap();
    let shown = lumenna.show_task(&id).unwrap().task;
    assert_eq!(shown.actions[0].title, "Mark Not Done");
    assert!(!titles(&shown.actions).contains(&"Edit Details"), "the task's own screen is its form");
}

#[test]
fn a_trashed_task_is_restored_or_deleted_from_the_trash_after_saying_what_that_means() {
    let (_directory, lumenna) = open();
    let id = add(&lumenna, "old idea");
    lumenna.trash_task(&id).unwrap();
    let rows = lumenna.list_tasks("deleted").unwrap().rows;
    let actions = &rows[0].actions;
    assert_eq!(titles(actions), ["Restore", "Delete from Trash"]);
    let erase = find(actions, "Delete from Trash");
    let Question::Confirm { message, yes, .. } = &erase.question else { panic!("{:?}", erase.question) };
    assert!(message.starts_with("Undo can bring it back"));
    assert_eq!(yes, "Delete");
    assert!(erase.destructive);
    assert!(!lumenna.act(erase.clone(), Answer::Text { text: String::new() }).unwrap().changed, "only a yes erases");
    assert!(lumenna.act(erase, Answer::Yes).unwrap().changed);
}

#[test]
fn the_inbox_is_only_reordered_and_weighed() {
    let (_directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    let rows = lumenna.list_projects().unwrap().rows;
    let inbox = rows.iter().find(|r| r.title == "Inbox").unwrap();
    assert_eq!(titles(&inbox.actions), ["Move Down", "Weight"]);
    let work = rows.iter().find(|r| r.title == "Work").unwrap();
    assert_eq!(
        titles(&work.actions),
        ["Rename", "New Project Inside", "Move Under", "Move Up", "Weight", "Archive", "Delete"]
    );
}

#[test]
fn a_project_is_never_offered_a_place_inside_itself() {
    let (_directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    lumenna.add_project("Reports", Some("Work".to_owned())).unwrap();
    lumenna.add_project("Home", None).unwrap();
    let rows = lumenna.list_projects().unwrap().rows;
    let work = rows.iter().find(|r| r.title == "Work").unwrap();
    let offered = lumenna.choices(find(&work.actions, "Move Under")).unwrap();
    let names: Vec<&str> = offered.choices.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(names, ["Home"]);
}

#[test]
fn renaming_and_deleting_a_project_keeping_its_tasks_go_through_the_answers() {
    let (_directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    add(&lumenna, "call #Work");
    let work = |lumenna: &Lumenna| lumenna.list_projects().unwrap().rows.into_iter().find(|r| r.title.starts_with("Job") || r.title == "Work").unwrap();
    let rename = find(&work(&lumenna).actions, "Rename");
    let Question::Text { initial, .. } = &rename.question else { panic!() };
    assert_eq!(initial, "Work");
    lumenna.act(rename, Answer::Text { text: "Job ".to_owned() }).unwrap();
    let delete = find(&work(&lumenna).actions, "Delete");
    let Question::Choose { answers, .. } = &delete.question else { panic!() };
    assert_eq!(answers.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), ["Delete and Trash Its Tasks", "Delete and Keep Its Tasks"]);
    lumenna.act(delete, Answer::Picked { id: "keep".to_owned(), length: None }).unwrap();
    assert_eq!(lumenna.list_tasks("#Inbox").unwrap().count, 1, "its task moved to the Inbox");
}

#[test]
fn an_empty_name_is_refused_by_the_core_in_every_client() {
    let (_directory, lumenna) = open();
    lumenna.add_label("calls").unwrap();
    let rows = lumenna.list_labels().unwrap().rows;
    let rename = find(&rows[0].actions, "Rename");
    assert!(lumenna.act(rename, Answer::Text { text: "  ".to_owned() }).is_err());
}

#[test]
fn a_weight_starts_from_the_projects_own() {
    let (_directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    lumenna.weigh_project("Work", lumenna_surface::Weight::Value { value: 1.5 }).unwrap();
    let rows = lumenna.list_projects().unwrap().rows;
    let weight = find(&rows.iter().find(|r| r.title == "Work").unwrap().actions, "Weight");
    let Question::Text { initial, .. } = &weight.question else { panic!() };
    assert_eq!(initial, "1.5");
}

fn day_block(lumenna: &Lumenna, title: &str, repeat: Option<&str>) {
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: "00:00".to_owned(),
            minutes: 1439,
            date: Some("today".to_owned()),
            kind: "work".to_owned(),
            repeat: repeat.map(str::to_owned),
            ..NewBlock::default()
        })
        .unwrap();
}

#[test]
fn a_repeating_block_skips_a_day_and_the_skipped_day_comes_back() {
    let (_directory, lumenna) = open();
    day_block(&lumenna, "Deep work", Some("every day"));
    let plan = lumenna.plan(None).unwrap();
    let actions = &plan.blocks[0].actions;
    assert_eq!(titles(actions), ["Assign a Task", "Edit Block", "Cancel This Day", "Delete Block"]);
    let Question::Confirm { message, .. } = &find(actions, "Delete Block").question else { panic!() };
    assert!(message.starts_with("Every occurrence goes"));
    lumenna.act(find(actions, "Cancel This Day"), Answer::Yes).unwrap();
    let plan = lumenna.plan(None).unwrap();
    assert!(plan.blocks.is_empty());
    lumenna.act(find(&plan.cancelled[0].actions, "Restore This Day"), Answer::Yes).unwrap();
    assert_eq!(lumenna.plan(None).unwrap().blocks.len(), 1);
}

#[test]
fn a_block_offers_only_tasks_not_already_in_it_and_asks_how_long() {
    let (_directory, lumenna) = open();
    day_block(&lumenna, "Deep work", None);
    let report = add(&lumenna, "write report");
    add(&lumenna, "call bank");
    let block = lumenna.plan(None).unwrap().blocks.remove(0);
    lumenna.assign(&report, &block.id, None, None).unwrap();
    let assign = find(&block.actions, "Assign a Task");
    let Question::Pick { length, .. } = &assign.question else { panic!() };
    assert!(length.is_some());
    let offered = lumenna.choices(assign.clone()).unwrap();
    assert_eq!(offered.choices.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["call bank"]);
    let id = offered.choices[0].id.clone();
    lumenna.act(assign, Answer::Picked { id, length: Some("45m".to_owned()) }).unwrap();
    let sittings = &lumenna.plan(None).unwrap().blocks[0].assignments;
    assert_eq!(sittings.iter().find(|s| s.title == "call bank").unwrap().planned_mins, Some(45));
}

#[test]
fn a_sitting_offers_pause_and_stop_once_its_timer_runs() {
    let (_directory, lumenna) = open();
    day_block(&lumenna, "Deep work", None);
    let report = add(&lumenna, "write report");
    let block = lumenna.plan(None).unwrap().blocks.remove(0);
    lumenna.assign(&report, &block.id, None, None).unwrap();
    let sitting = |lumenna: &Lumenna| lumenna.plan(None).unwrap().blocks.remove(0).assignments.remove(0);
    let actions = sitting(&lumenna).actions;
    assert_eq!(titles(&actions), ["Start Timer", "Planned Length", "Log Minutes", "Edit Task Details", "Unassign"]);
    lumenna.act(find(&actions, "Start Timer"), Answer::Yes).unwrap();
    assert_eq!(&titles(&sitting(&lumenna).actions)[..2], ["Pause Timer", "Stop Timer"]);
    lumenna.act(find(&sitting(&lumenna).actions, "Log Minutes"), Answer::Text { text: "25".to_owned() }).unwrap();
    assert_eq!(sitting(&lumenna).minutes, 25);
}

#[test]
fn free_time_adds_a_block_through_the_clients_own_form() {
    let (_directory, lumenna) = open();
    let plan = lumenna.plan(None).unwrap();
    let free = plan.timeline.iter().find_map(|item| match item {
        lumenna_surface::PlanItem::Free { actions, .. } => Some(actions.clone()),
        _ => None,
    });
    let actions = free.expect("an empty day is free time");
    assert_eq!(actions[0].kind, ActionKind::AddBlock);
    assert_eq!(actions[0].question, Question::Form);
    assert!(lumenna.act(actions[0].clone(), Answer::Yes).is_err(), "a form is the client's to show");
}

#[test]
fn the_sidebars_headings_add_and_its_places_carry_their_own_actions() {
    let (_directory, lumenna) = open();
    lumenna.add_label("calls").unwrap();
    lumenna.add_filter("Urgent", "p1").unwrap();
    let entries = lumenna.places().entries;
    let heading = entries.iter().find(|e| e.text == "Projects").unwrap();
    assert_eq!(titles(&heading.actions), ["New Project"]);
    lumenna.act(heading.actions[0].clone(), Answer::Text { text: "Garden".to_owned() }).unwrap();
    assert!(lumenna.places().entries.iter().any(|e| e.text.starts_with("Garden")));
    let urgent = entries.iter().find(|e| e.text.starts_with("Urgent")).unwrap();
    assert_eq!(titles(&urgent.actions), ["Rename", "Change Query", "Delete"]);
    assert_eq!(urgent.actions[0].subject, Subject::Filter);
}
