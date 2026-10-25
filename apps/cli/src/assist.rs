//! `lum assist`: an operation of the assistant table (`lumenna_records::assistant`) run from the
//! command line, through the dispatcher `lum mcp`, Siri and Gemini run it through
//! (`Lumenna::assist`), for a script that wants what an assistant can do and nothing else.
//!
//! What it is given is read as RPC's `assist` reads it (`lumenna_host::rpc::assist`), the
//! `name=value` pairs first typed by each parameter's kind, so `lum assist`, `lum rpc` and
//! `lum mcp` cannot read one differently.

use std::io::Read;

use lumenna_host::rpc::assist::assist;
use lumenna_records::assistant::ParameterKind as K;
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
        // A name the operation does not take is refused with the rest, in `assist`.
        let value = match op.parameters.iter().find(|p| p.name == name) {
            Some(parameter) => typed(parameter.kind, name, value)?,
            None => Value::from(value),
        };
        given.insert(name.to_owned(), value);
    }
    Ok(Response::new(assist(profile, &op.id, given)?))
}

/// Every operation and what it takes.
pub(crate) fn list() -> Response {
    Response::new(Operations::all())
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
