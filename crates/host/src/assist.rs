//! The one way an assistant acts on the store: [`Lumenna::assist`] runs an operation of
//! `lumenna_records::assistant::operations` by its `id`, with what its parameters were given,
//! for `lum mcp`, Siri and Shortcuts, and Gemini alike.

use lumenna_records::assistant::{Argument, ArgumentValue, Assisted, operation};
use lumenna_records::{ActionKind, LumennaError, MoveTarget, Question, Result, TaskEdit};

use crate::Lumenna;

/// What an operation was given, by parameter.
struct Given {
    id: String,
    arguments: Vec<Argument>,
}

impl Given {
    fn take(&mut self, name: &str) -> Option<ArgumentValue> {
        let at = self.arguments.iter().position(|a| a.name == name)?;
        Some(self.arguments.remove(at).value)
    }

    fn text(&mut self, name: &str) -> Result<Option<String>> {
        match self.take(name) {
            None => Ok(None),
            Some(ArgumentValue::Text(text)) => Ok(Some(text)),
            Some(ArgumentValue::Number(n)) => Ok(Some(n.to_string())),
            Some(_) => Err(self.wrong(name, "words")),
        }
    }

    fn required(&mut self, name: &str) -> Result<String> {
        self.text(name)?.filter(|t| !t.trim().is_empty()).ok_or_else(|| {
            LumennaError::new(format!("{} needs {name}", self.id))
        })
    }

    fn number(&mut self, name: &str) -> Result<Option<i64>> {
        match self.take(name) {
            None => Ok(None),
            Some(ArgumentValue::Number(n)) => Ok(Some(n)),
            Some(ArgumentValue::Text(text)) if text.trim().is_empty() => Ok(None),
            Some(ArgumentValue::Text(text)) => text.trim().parse().map(Some).map_err(|_| self.wrong(name, "a number")),
            Some(_) => Err(self.wrong(name, "a number")),
        }
    }

    fn minutes(&mut self, name: &str) -> Result<Option<u32>> {
        self.number(name)?.map(|n| u32::try_from(n).map_err(|_| self.wrong(name, "a number of minutes"))).transpose()
    }

    fn flag(&mut self, name: &str) -> Result<bool> {
        match self.take(name) {
            None => Ok(false),
            Some(ArgumentValue::Flag(flag)) => Ok(flag),
            Some(_) => Err(self.wrong(name, "yes or no")),
        }
    }

    fn texts(&mut self, name: &str) -> Result<Option<Vec<String>>> {
        match self.take(name) {
            None => Ok(None),
            Some(ArgumentValue::Texts(texts)) => Ok(Some(texts)),
            Some(ArgumentValue::Text(text)) => Ok(Some(text.split(',').map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()).collect())),
            Some(_) => Err(self.wrong(name, "names")),
        }
    }

    fn wrong(&self, name: &str, what: &str) -> LumennaError {
        LumennaError::new(format!("{}'s {name} takes {what}", self.id))
    }
}

/// The refusal of an erasure not yet confirmed: the question the core puts before the action
/// of `kind` among `actions`, in the words every client asks, for the assistant to put to the
/// person.
fn unconfirmed(actions: Vec<lumenna_records::Action>, kind: ActionKind) -> LumennaError {
    let question = actions.into_iter().find(|a| a.kind == kind).map(|a| a.question);
    let asked = match question {
        Some(Question::Confirm { title, message, .. }) => format!("{title} {message} "),
        _ => String::new(),
    };
    LumennaError::new(format!("{asked}Ask the person first, and go ahead with confirm only once they say yes."))
}

impl Lumenna {
    /// Runs the assistant operation `id` with `arguments`, by its parameters' names: the same
    /// operation however an assistant reached it. An operation that `confirms` refuses without
    /// `confirm`, answering with the core's question.
    ///
    /// # Errors
    ///
    /// If there is no such operation, an argument is not of its parameter's kind, or the
    /// operation refuses.
    #[allow(clippy::too_many_lines)]
    pub fn assist(&self, id: &str, arguments: Vec<Argument>) -> Result<Assisted> {
        let Some(found) = operation(id) else {
            return Err(LumennaError::new(format!("there is no operation called {id}")));
        };
        let mut given = Given { id: found.id.clone(), arguments };
        Ok(match found.id.as_str() {
            "list_tasks" => Assisted::Rows(self.list_tasks(&given.text("query")?.unwrap_or_default())?),
            "search_tasks" => Assisted::Rows(self.search_tasks(&given.required("text")?)?),
            "show_task" => Assisted::Task(self.show_task(&given.required("id")?)?),
            "day" => Assisted::Plan(self.plan(given.text("date")?.filter(|d| !d.trim().is_empty()))?),
            "list_blocks" => Assisted::Rows(self.list_blocks()?),
            "list_projects" => Assisted::Rows(self.list_projects()?),
            "list_labels" => Assisted::Rows(self.list_labels()?),
            "list_filters" => Assisted::Filters(self.list_filters()?),
            "what_should_i_work_on" => Assisted::Choices(self.what_should_i_work_on(None)?),
            "propose_replan" => {
                Assisted::Replan(self.replan(lumenna_records::proposals::TimeStyle::TwentyFourHour, Default::default(), None)?)
            }
            "propose_day_plan" => Assisted::DayPlan(self.propose_day_plan(
                given.text("date")?.filter(|d| !d.trim().is_empty()),
                lumenna_records::proposals::TimeStyle::TwentyFourHour,
                Default::default(),
                None,
            )?),
            "add_task" => Assisted::Change(self.add_task(&given.required("text")?)?),
            "edit_task" => {
                let task = given.required("id")?;
                let priority = given.number("priority")?.map(|p| u8::try_from(p).map_err(|_| given.wrong("priority", "1 to 4"))).transpose()?;
                let edit = TaskEdit {
                    title: given.text("title")?,
                    due: given.text("due")?,
                    repeat: given.text("repeat")?,
                    priority,
                    estimate: given.text("estimate")?,
                    notes: given.text("notes")?,
                    project: given.text("project")?,
                    labels: given.texts("labels")?,
                };
                Assisted::Change(self.edit_task(&task, edit)?)
            }
            "complete_task" => Assisted::Change(self.complete_task(&given.required("id")?)?),
            "uncomplete_task" => Assisted::Change(self.uncomplete_task(&given.required("id")?)?),
            "trash_task" => Assisted::Change(self.trash_task(&given.required("id")?)?),
            "erase_task_permanently" => {
                let task = given.required("id")?;
                if !given.flag("confirm")? {
                    return Err(unconfirmed(self.show_task(&task)?.task.actions, ActionKind::ErasePermanently));
                }
                Assisted::Change(self.erase_task_permanently(&task)?)
            }
            "erase_everything_deleted" => {
                if !given.flag("confirm")? {
                    return Err(unconfirmed(lumenna_records::actions::store_actions(), ActionKind::EraseEverythingDeleted));
                }
                Assisted::Change(self.erase_everything_deleted()?)
            }
            "restore_task" => Assisted::Change(self.restore_task(&given.required("id")?)?),
            "move_task" => {
                let task = given.required("id")?;
                let to = match (given.text("parent")?, given.text("project")?, given.flag("top")?) {
                    (Some(id), _, _) if !id.trim().is_empty() => MoveTarget::Parent { id },
                    (_, Some(name), _) if !name.trim().is_empty() => MoveTarget::Project { name },
                    (_, _, true) => MoveTarget::Top,
                    _ => return Err(LumennaError::new("say where: a parent, a project, or top")),
                };
                Assisted::Change(self.move_task(&task, to)?)
            }
            "assign" => {
                let (task, block) = (given.required("task")?, given.required("block")?);
                let (date, minutes) = (given.text("date")?, given.minutes("minutes")?);
                Assisted::Change(self.assign(&task, &block, date, minutes)?)
            }
            "unassign" => Assisted::Change(self.unassign(&given.required("assignment")?)?),
            "start_timer" => Assisted::Change(self.start_timer(&given.required("assignment")?)?),
            "pause_timer" => Assisted::Timer(self.pause_timer(&given.required("assignment")?)?),
            "stop_timer" => {
                let sitting = given.required("assignment")?;
                Assisted::Timer(self.stop_timer(&sitting, given.minutes("minutes")?)?)
            }
            "start_on" => Assisted::Change(self.start_on(&given.required("choice")?, None)?),
            "apply_replan" => {
                let (Some(ArgumentValue::Replan(proposal)), Some(ArgumentValue::ReplanChanges(accepted))) =
                    (given.take("proposal"), given.take("accepted"))
                else {
                    return Err(LumennaError::new("apply_replan needs the proposal, and the changes accepted"));
                };
                Assisted::Replanned(self.apply_replan(proposal, accepted, None)?)
            }
            "apply_day_plan" => {
                let (Some(ArgumentValue::DayPlan(proposal)), Some(ArgumentValue::DayPlanSittings(accepted))) =
                    (given.take("proposal"), given.take("accepted"))
                else {
                    return Err(LumennaError::new("apply_day_plan needs the proposal, and the sittings accepted"));
                };
                Assisted::DayPlanned(self.apply_day_plan(proposal, accepted, None)?)
            }
            "undo" => Assisted::Change(self.undo()?),
            "redo" => Assisted::Change(self.redo()?),
            other => return Err(LumennaError::new(format!("{other} is listed but not done; this is a bug"))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_listed_is_one_the_dispatcher_does() {
        let directory = tempfile::tempdir().unwrap();
        let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
        for op in lumenna_records::assistant::operations() {
            if let Err(refused) = lumenna.assist(&op.id, Vec::new()) {
                assert!(!refused.message().contains("this is a bug"), "{}: {}", op.id, refused.message());
            }
        }
    }
}
