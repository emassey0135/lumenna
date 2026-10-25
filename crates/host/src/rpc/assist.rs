//! An assistant's operation given as JSON: what the `assist` method runs, and what `lum mcp` and
//! `lum assist` read their arguments with, so no way in reads one differently.

use lumenna_records::assistant::{Argument, ArgumentValue, AssistantOperation, Assisted, ParameterKind as K, operation};
use serde_json::{Map, Value};

use crate::{Lumenna, LumennaError, Result};

/// What an operation was given, by name, as its arguments, each read as its parameter's kind.
/// A name it does not take is left out: `lum mcp` ignores what a model adds, and [`assist`]
/// refuses it before this.
///
/// # Errors
///
/// If a value is not of its parameter's kind, in words for the caller.
pub fn arguments_of(op: &AssistantOperation, mut given: Map<String, Value>) -> std::result::Result<Vec<Argument>, String> {
    let mut arguments = Vec::new();
    for parameter in &op.parameters {
        let Some(value) = given.remove(&parameter.name).filter(|v| !v.is_null()) else { continue };
        let wrong = |what: &str| format!("{} takes {what}", parameter.name);
        let typed = |e: serde_json::Error| format!("{} is not as it was proposed: {e}", parameter.name);
        let value = match (parameter.kind, value) {
            (K::Replan, value) => ArgumentValue::Replan(serde_json::from_value(value).map_err(typed)?),
            (K::DayPlan, value) => ArgumentValue::DayPlan(serde_json::from_value(value).map_err(typed)?),
            (K::ReplanChanges, value) => ArgumentValue::ReplanChanges(serde_json::from_value(value).map_err(typed)?),
            (K::DayPlanSittings, value) => ArgumentValue::DayPlanSittings(serde_json::from_value(value).map_err(typed)?),
            (_, Value::String(text)) => ArgumentValue::Text(text),
            (_, Value::Bool(flag)) => ArgumentValue::Flag(flag),
            (_, Value::Number(n)) => ArgumentValue::Number(n.as_i64().ok_or_else(|| wrong("a whole number"))?),
            (_, Value::Array(items)) => ArgumentValue::Texts(
                items
                    .into_iter()
                    .map(|item| item.as_str().map(str::to_owned).ok_or_else(|| wrong("names")))
                    .collect::<std::result::Result<_, _>>()?,
            ),
            _ => return Err(wrong("something else")),
        };
        arguments.push(Argument { name: parameter.name.clone(), value });
    }
    Ok(arguments)
}

/// The refusal of a name in `given` that `op` does not take, if there is one.
#[must_use]
pub fn untaken(op: &AssistantOperation, given: &Map<String, Value>) -> Option<LumennaError> {
    let name = given.keys().find(|name| op.parameters.iter().all(|p| &p.name != *name))?;
    let names: Vec<&str> = op.parameters.iter().map(|p| p.name.as_str()).collect();
    Some(LumennaError::new(match names.as_slice() {
        [] => format!("{} takes nothing, so not {name}", op.id),
        _ => format!("{} takes no {name}; it takes {}", op.id, names.join(", ")),
    }))
}

/// Runs the operation `id` with its arguments by name, through [`Lumenna::assist`], refusing a
/// name it does not take: a script's typo should not pass for a parameter left out. One that
/// `confirms` runs only with `confirm` true.
///
/// # Errors
///
/// If there is no such operation, it was given what it does not take or a value of the wrong
/// kind, or it refuses.
pub fn assist(lumenna: &Lumenna, id: &str, given: Map<String, Value>) -> Result<Assisted> {
    // An unknown operation is refused in the dispatcher's own words.
    let Some(op) = operation(id) else { return lumenna.assist(id, Vec::new()) };
    if let Some(refused) = untaken(&op, &given) {
        return Err(refused);
    }
    let arguments = arguments_of(&op, given).map_err(LumennaError::new)?;
    lumenna.assist(&op.id, arguments)
}
