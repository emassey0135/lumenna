//! What can be done to one task, in one place: the list's context menu, the Task menu in the
//! menu bar and the details pane's buttons all come here, so none can drift from the others —
//! as the Mac's `TaskActions` and the Windows app's `task_actions` do.
//!
//! Each is a window action (`win.mark-done` and so on) acting on the task in hand: the one in
//! the details when focus is there, else the one selected in the list or on the day. After a
//! change, a task list puts its selection on the same task if it is still listed, and
//! otherwise on whatever now holds its place (§13).

use std::rc::Rc;

use gtk::gio;
use lumenna_desktop::{choices, speech};
use lumenna_surface::{Change, Lumenna, MoveTarget, Result, RowView, TaskDetail};

use crate::core::sentence;
use crate::prompts;
use crate::window::App;

/// What can be done to a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    MarkDone,
    PutInBlock,
    MoveToProject,
    MakeSubtask,
    MoveToTop,
    WaitFor,
    /// Stops it waiting for the task with this identifier.
    StopWaiting(String),
    Trash,
    Restore,
    Erase,
}

/// The commands for a task, read fresh so they match what it is now: its context menu.
pub fn menu(task: &TaskDetail) -> gio::Menu {
    let done = task.state.iter().any(|s| s == "completed");
    let menu = gio::Menu::new();
    let first = gio::Menu::new();
    first.append(Some(if done { "_Mark Not Done" } else { "_Mark Done" }), Some("win.mark-done"));
    first.append(Some("_Edit Details"), Some("win.open-task"));
    menu.append_section(None, &first);
    let moving = gio::Menu::new();
    moving.append(Some("Put in a _Block…"), Some("win.put-in-block"));
    moving.append(Some("Move to _Project…"), Some("win.move-to-project"));
    moving.append(Some("Make S_ubtask Of…"), Some("win.make-subtask"));
    if task.parent.is_some() {
        moving.append(Some("Move to _Top Level"), Some("win.move-to-top"));
    }
    moving.append(Some("_Wait For…"), Some("win.wait-for"));
    for other in &task.depends {
        let item = gio::MenuItem::new(Some(&format!("Stop Waiting for {}", other.title.replace('_', "__"))), None);
        item.set_action_and_target_value(Some("win.stop-waiting"), Some(&glib_string(&other.id)));
        moving.append_item(&item);
    }
    menu.append_section(None, &moving);
    let last = gio::Menu::new();
    last.append(Some("Move to T_rash"), Some("win.trash-task"));
    menu.append_section(None, &last);
    menu
}

/// A trashed task's two commands.
pub fn trash_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("_Restore"), Some("win.restore-task"));
    menu.append(Some("_Erase for Good…"), Some("win.erase-task"));
    menu
}

fn glib_string(text: &str) -> gtk::glib::Variant {
    gtk::glib::variant::ToVariant::to_variant(text)
}

/// Does `command` to the task `id`, asking first where it needs to know more.
pub fn run(app: &Rc<App>, command: Command, id: &str) {
    let task = match app.core.lumenna.show_task(id) {
        Ok(shown) => shown.task,
        Err(error) => return app.fail(&sentence(&error)),
    };
    let app = Rc::clone(app);
    crate::window::spawn(async move {
        match command {
            Command::MarkDone => {
                let done = task.state.iter().any(|s| s == "completed");
                perform(&app, Some(&task.id), |l| if done { l.uncomplete_task(&task.id) } else { l.complete_task(&task.id) });
            }
            Command::PutInBlock => put_in_block(&app, &task).await,
            Command::MoveToProject => move_to_project(&app, &task).await,
            Command::MakeSubtask => {
                let title = format!("Make {} a Subtask", task.title);
                if let Some(parent) = choose_task(&app, &title, "Of _task:", &[&task.id]).await {
                    perform(&app, Some(&task.id), |l| l.move_task(&task.id, MoveTarget::Parent { id: parent }));
                }
            }
            Command::MoveToTop => perform(&app, Some(&task.id), |l| l.move_task(&task.id, MoveTarget::Top)),
            Command::WaitFor => {
                // Not itself, and not what it already waits for.
                let mut excluded: Vec<&str> = task.depends.iter().map(|d| d.id.as_str()).collect();
                excluded.push(&task.id);
                let title = format!("{} Waits For", task.title);
                if let Some(other) = choose_task(&app, &title, "_Task:", &excluded).await {
                    perform(&app, Some(&task.id), |l| l.add_dependency(&task.id, &other));
                }
            }
            Command::StopWaiting(other) => perform(&app, Some(&task.id), |l| l.remove_dependency(&task.id, &other)),
            Command::Trash => perform(&app, None, |l| l.trash_task(&task.id)),
            Command::Restore => perform(&app, None, |l| l.restore_task(&task.id)),
            Command::Erase => {
                // Erasing rebuilds the document without the task and cannot be undone (§9).
                let heading = format!("Erase {}?", task.title);
                let detail = "It and its history are deleted for good. This cannot be undone.";
                if prompts::confirm(&app.window, &heading, detail, "Erase").await {
                    perform(&app, None, |l| l.erase_task(&task.id));
                }
            }
        }
    });
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
async fn choose_task(app: &App, title: &str, label: &str, excluded: &[&str]) -> Option<String> {
    let tasks: Vec<RowView> = app
        .core
        .lumenna
        .list_tasks("")
        .map(|r| r.rows.into_iter().filter(|t| !excluded.contains(&t.id.as_str())).collect())
        .unwrap_or_default();
    let texts: Vec<String> = tasks.iter().map(|t| speech::row(t, false)).collect();
    let index = prompts::pick(&app.window, title, label, &texts).await?;
    Some(tasks[index].id.clone())
}

async fn move_to_project(app: &App, task: &TaskDetail) {
    let projects: Vec<String> = app
        .core
        .lumenna
        .list_projects()
        .map(|r| r.rows.into_iter().map(|p| p.title).filter(|p| Some(p) != task.project.as_ref()).collect())
        .unwrap_or_default();
    let title = format!("Move {}", task.title);
    if let Some(index) = prompts::pick(&app.window, &title, "To _project:", &projects).await {
        let name = projects[index].clone();
        perform(app, Some(&task.id), |l| l.move_task(&task.id, MoveTarget::Project { name }));
    }
}

/// Puts a task into a work block of today or the next six days (§3.7), asking how long the
/// sitting is meant to take. The planner reaches any other day, from the block's side.
async fn put_in_block(app: &App, task: &TaskDetail) {
    let today = jiff::Zoned::now().date();
    let blocks = choices::work_blocks(&app.core.lumenna, today, 7, &app.clock);
    if blocks.is_empty() {
        return app.fail("There are no work blocks this week. Add one from Today.");
    }
    let texts: Vec<String> = blocks.iter().map(|b| b.text.clone()).collect();
    let title = format!("Put {} in a Block", task.title);
    let Some(index) = prompts::pick(&app.window, &title, "_Block:", &texts).await else { return };
    let title = format!("How Long Is {} Meant to Take?", task.title);
    let Some(minutes) = ask_minutes(app, &title, "", true).await else { return };
    let block = &blocks[index];
    perform(app, Some(&task.id), |l| l.assign(&task.id, &block.id, Some(block.date.clone()), minutes));
}

/// Asks for a number of minutes. `Some(None)` is a deliberate "no planned length", where
/// `optional` allows one; `None` is cancelled.
pub async fn ask_minutes(app: &App, title: &str, initial: &str, optional: bool) -> Option<Option<u32>> {
    let message = if optional {
        "Minutes, or empty for no planned length."
    } else {
        "The whole of this sitting, replacing what is logged."
    };
    let mut initial = initial.to_owned();
    loop {
        match prompts::ask(&app.window, title, "_Minutes:", message, &initial).await? {
            text if text.trim().is_empty() && optional => return Some(None),
            text => match text.trim().parse::<u32>() {
                Ok(minutes) if minutes > 0 => return Some(Some(minutes)),
                _ => {
                    prompts::tell(&app.window, "That is not a number of minutes.").await;
                    initial = text;
                }
            },
        }
    }
}
