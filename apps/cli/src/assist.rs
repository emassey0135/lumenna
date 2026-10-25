//! `lum assist`: an operation of the assistant table (`lumenna_records::assistant`) run from the
//! command line, through the dispatcher `lum mcp`, Siri and Gemini run it through
//! (`Lumenna::assist`), for a script that wants what an assistant can do and nothing else: the
//! PowerShell module's cmdlets are each one of these.
//!
//! Its arguments are the operation's parameters by name, read as each parameter's kind says, so
//! `lum assist` and `lum mcp` cannot read one differently.

use std::io::Read;

use lumenna_records::assistant::{Argument, ArgumentValue, AssistantOperation, ParameterKind as K};
use serde_json::{Map, Value};

use crate::api::{Operations, Response};
use crate::error::{CliError, Result};
use crate::profile::Profile;

/// Runs `operation` with what it was given: `name=value` pairs, and `json`, one object of
/// them (`-` reads it from standard input). A name given both ways takes the pair's value.
pub(crate) fn run(profile: &Profile, operation: &str, pairs: &[String], json: Option<&str>) -> Result<Response> {
    let Some(op) = lumenna_records::assistant::operation(operation) else {
        return Err(CliError::Message(format!("there is no operation called {operation}; lum assist --list names them all")));
    };
    let mut given = match json {
        None => Map::new(),
        Some(text) => {
            let text = if text == "-" {
                let mut read = String::new();
                std::io::stdin().read_to_string(&mut read)?;
                read
            } else {
                text.to_owned()
            };
            match serde_json::from_str(&text) {
                Ok(Value::Object(object)) => object,
                Ok(_) => return Err(CliError::Message("--args takes one JSON object, its parameters by name".to_owned())),
                Err(e) => return Err(CliError::Message(format!("--args is not JSON: {e}"))),
            }
        }
    };
    for pair in pairs {
        let Some((name, value)) = pair.split_once('=') else {
            return Err(CliError::Message(format!("{pair} needs a value, as name=value")));
        };
        let kind = parameter(&op, name)?;
        given.insert(name.to_owned(), typed(kind, name, value)?);
    }
    if let Some(name) = given.keys().find(|name| op.parameters.iter().all(|p| &p.name != *name)) {
        parameter(&op, name)?;
    }
    let arguments = arguments_of(&op, given).map_err(CliError::Message)?;
    Ok(Response::new(profile.assist(&op.id, arguments)?))
}

/// Every operation and what it takes.
pub(crate) fn list() -> Response {
    Response::new(Operations::all())
}

/// The kind of `op`'s parameter called `name`, or why there is none.
fn parameter(op: &AssistantOperation, name: &str) -> Result<K> {
    op.parameters.iter().find(|p| p.name == name).map(|p| p.kind).ok_or_else(|| {
        let names: Vec<&str> = op.parameters.iter().map(|p| p.name.as_str()).collect();
        CliError::Message(match names.as_slice() {
            [] => format!("{} takes nothing, so not {name}", op.id),
            _ => format!("{} takes no {name}; it takes {}", op.id, names.join(", ")),
        })
    })
}

/// A `name=value` pair's value, as its parameter's kind reads it: a flag `true` or `false`,
/// minutes and a priority a number, labels separated by commas, and a proposal JSON.
fn typed(kind: K, name: &str, value: &str) -> Result<Value> {
    let wrong = |what: &str| CliError::Message(format!("{name} takes {what}, not {value}"));
    Ok(match kind {
        K::Flag => match value.trim().to_lowercase().as_str() {
            "true" | "yes" => Value::Bool(true),
            "false" | "no" => Value::Bool(false),
            _ => return Err(wrong("true or false")),
        },
        K::Minutes | K::Priority => Value::from(value.trim().parse::<i64>().map_err(|_| wrong("a whole number"))?),
        K::Labels => Value::from(value.split(',').map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>()),
        K::Replan | K::ReplanChanges | K::DayPlan | K::DayPlanSittings => {
            serde_json::from_str(value).map_err(|e| CliError::Message(format!("{name} is not as it was proposed: {e}")))?
        }
        K::Text | K::Line | K::Query | K::Date | K::Task | K::Block | K::Sitting | K::Project | K::Choice => {
            Value::from(value)
        }
    })
}

/// What an operation was given, by name, as its arguments, each read as its parameter's kind.
/// A name it does not take is left out: `lum mcp` ignores what a model adds, and `lum assist`
/// refuses it before this.
pub(crate) fn arguments_of(op: &AssistantOperation, mut given: Map<String, Value>) -> std::result::Result<Vec<Argument>, String> {
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
                items.into_iter().map(|item| item.as_str().map(str::to_owned).ok_or_else(|| wrong("names"))).collect::<std::result::Result<_, _>>()?,
            ),
            _ => return Err(wrong("something else")),
        };
        arguments.push(Argument { name: parameter.name.clone(), value });
    }
    Ok(arguments)
}
