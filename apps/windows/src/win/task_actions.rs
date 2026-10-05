//! What can be done to one task, in one place: the list's context menu, the Task menu in the
//! menu bar and the details pane's buttons all come here, so none can drift from the others —
//! as the Mac's `TaskActions` does.
//!
//! Each acts on a task by identifier, wherever it was chosen: the task list, a sitting on the
//! day, or the details pane. After a change, a task list puts its selection on the same task
//! if it is still listed, and otherwise on whatever now holds its place.

use lumenna_surface::{Change, Lumenna, MoveTarget, Result, RowView, TaskDetail};

use super::app::App;
use super::clock::Locale;
use super::core::sentence;
use super::menu;
use super::prompts;
use crate::{choices, speech};

/// The commands for a task, read fresh so they match what it is now: for its context menu.
pub fn menu_items(task: &TaskDetail) -> Vec<(u16, String)> {
    let done = task.state.iter().any(|s| s == "completed");
    let mut items = vec![
        (menu::MARK_DONE, if done { "&Mark Not Done" } else { "&Mark Done" }.to_owned()),
        (menu::OPEN_TASK, "&Edit Details".to_owned()),
        (0, String::new()),
        (menu::PUT_IN_BLOCK, "Put in a &Block...".to_owned()),
        (menu::MOVE_TO_PROJECT, "Move to &Project...".to_owned()),
        (menu::MAKE_SUBTASK, "Make S&ubtask Of...".to_owned()),
    ];
    if task.parent.is_some() {
        items.push((menu::MOVE_TO_TOP, "Move to &Top Level".to_owned()));
    }
    items.push((menu::WAIT_FOR, "&Wait For...".to_owned()));
    for (index, other) in task.depends.iter().enumerate().take(99) {
        let title = other.title.replace('&', "&&");
        items.push((menu::STOP_WAITING + index as u16, format!("Stop Waiting for {title}")));
    }
    items.extend([(0, String::new()), (menu::TRASH_TASK, "Move to T&rash".to_owned())]);
    items
}

/// Whether a menu command is one of these.
pub fn handles(command: u16) -> bool {
    matches!(
        command,
        menu::MARK_DONE
            | menu::PUT_IN_BLOCK
            | menu::MOVE_TO_PROJECT
            | menu::MAKE_SUBTASK
            | menu::MOVE_TO_TOP
            | menu::WAIT_FOR
            | menu::TRASH_TASK
    ) || (menu::STOP_WAITING..menu::STOP_WAITING + 99).contains(&command)
}

/// Does `command` to the task `id`.
pub fn run(app: &App, command: u16, id: &str) {
    let task = match app.core.lumenna.show_task(id) {
        Ok(shown) => shown.task,
        Err(error) => return prompts::fail(app.main, &sentence(&error)),
    };
    match command {
        menu::MARK_DONE => {
            let done = task.state.iter().any(|s| s == "completed");
            perform(app, Some(&task.id), |l| if done { l.uncomplete_task(&task.id) } else { l.complete_task(&task.id) });
        }
        menu::PUT_IN_BLOCK => put_in_block(app, &task),
        menu::MOVE_TO_PROJECT => move_to_project(app, &task),
        menu::MAKE_SUBTASK => {
            if let Some(parent) = choose_task(app, &format!("Make {} a Subtask", task.title), "Of &task:", &[&task.id]) {
                perform(app, Some(&task.id), |l| l.move_task(&task.id, MoveTarget::Parent { id: parent }));
            }
        }
        menu::MOVE_TO_TOP => perform(app, Some(&task.id), |l| l.move_task(&task.id, MoveTarget::Top)),
        menu::WAIT_FOR => {
            // Not itself, and not what it already waits for.
            let mut excluded: Vec<&str> = task.depends.iter().map(|d| d.id.as_str()).collect();
            excluded.push(&task.id);
            if let Some(other) = choose_task(app, &format!("{} Waits For", task.title), "&Task:", &excluded) {
                perform(app, Some(&task.id), |l| l.add_dependency(&task.id, &other));
            }
        }
        menu::TRASH_TASK => perform(app, None, |l| l.trash_task(&task.id)),
        waiting if waiting >= menu::STOP_WAITING => {
            if let Some(other) = task.depends.get(usize::from(waiting - menu::STOP_WAITING)) {
                perform(app, Some(&task.id), |l| l.remove_dependency(&task.id, &other.id));
            }
        }
        _ => {}
    }
}

/// Runs a change, puts a task list's selection on `keep` or near where it was, and says
/// what happened — in that order, so the announcement follows the row focus lands on.
pub fn perform(app: &App, keep: Option<&str>, operation: impl FnOnce(&Lumenna) -> Result<Change>) {
    let list = app.task_list();
    let near = list.as_ref().and_then(|list| list.tree.selected());
    if let Some(change) = app.perform(operation) {
        if let Some(list) = list {
            list.tree.select_key_or_near(keep, near);
        }
        app.say_change(&change);
    }
}

/// Open tasks other than `excluded`, to choose one from.
fn choose_task(app: &App, title: &str, label: &str, excluded: &[&str]) -> Option<String> {
    let tasks: Vec<RowView> = app
        .core
        .lumenna
        .list_tasks("")
        .map(|r| r.rows.into_iter().filter(|t| !excluded.contains(&t.id.as_str())).collect())
        .unwrap_or_default();
    let texts: Vec<String> = tasks.iter().map(|t| speech::row(t, false)).collect();
    let index = prompts::pick(app.main, title, label, &texts)?;
    Some(tasks[index].id.clone())
}

fn move_to_project(app: &App, task: &TaskDetail) {
    let projects: Vec<String> = app
        .core
        .lumenna
        .list_projects()
        .map(|r| r.rows.into_iter().map(|p| p.title).filter(|p| Some(p) != task.project.as_ref()).collect())
        .unwrap_or_default();
    if let Some(index) = prompts::pick(app.main, &format!("Move {}", task.title), "To &project:", &projects) {
        let name = projects[index].clone();
        perform(app, Some(&task.id), |l| l.move_task(&task.id, MoveTarget::Project { name }));
    }
}

/// Puts a task into a work block of today or the next six days, asking how long the
/// sitting is meant to take. The planner reaches any other day, from the block's side.
fn put_in_block(app: &App, task: &TaskDetail) {
    let today = jiff::Zoned::now().date();
    let blocks = choices::work_blocks(&app.core.lumenna, today, 7, &Locale);
    if blocks.is_empty() {
        return prompts::fail(app.main, "There are no work blocks this week. Add one from Today.");
    }
    let texts: Vec<String> = blocks.iter().map(|b| b.text.clone()).collect();
    let Some(index) = prompts::pick(app.main, &format!("Put {} in a Block", task.title), "&Block:", &texts) else {
        return;
    };
    let Some(minutes) = ask_minutes(app, &format!("How Long Is {} Meant to Take?", task.title), "", true) else {
        return;
    };
    let block = &blocks[index];
    perform(app, Some(&task.id), |l| l.assign(&task.id, &block.id, Some(block.date.clone()), minutes));
}

/// Asks for a number of minutes. `Some(None)` is a deliberate "no planned length", where
/// `optional` allows one; `None` is cancelled.
pub fn ask_minutes(app: &App, title: &str, initial: &str, optional: bool) -> Option<Option<u32>> {
    let message = if optional {
        "Minutes, or empty for no planned length."
    } else {
        "The whole of this sitting, replacing what is logged."
    };
    loop {
        match prompts::ask(app.main, title, "&Minutes:", message, initial)? {
            text if text.is_empty() && optional => return Some(None),
            text => match text.parse::<u32>() {
                Ok(minutes) if minutes > 0 => return Some(Some(minutes)),
                _ => prompts::fail(app.main, "That is not a number of minutes."),
            },
        }
    }
}
