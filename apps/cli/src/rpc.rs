//! `lum rpc` — the typed command surface over JSON-RPC on stdio (§8).
//!
//! One server per client, for clients that cannot link Rust: Emacs (§16.10) and the BTSpeak
//! app (§16.11). It opens the store exactly as a linked client does, watches for changes
//! another process wrote, and pushes them — which is the whole reason to speak a protocol
//! rather than shell out to `lum --json` per command. No Iroh: syncing is the daemon's job,
//! and this is not the daemon.
//!
//! **The surface is [`crate::api`], unchanged.** A method builds the same [`Command`] the
//! command line builds and hands it to the same `dispatch`, so RPC cannot drift from the CLI
//! — there is one implementation of every operation and two ways in. §8's *one protocol, two
//! transports* then costs the daemon nothing beyond a socket: it will serve exactly this.
//!
//! # Two things the terminal has that a client does not
//!
//! **Row numbers.** They address the last listing, which is one file per profile, so a
//! resident server and a person typing in a shell would overwrite each other's numbering.
//! [`Profile::detach_rows`] turns them off here: clients hold identifiers, which never go
//! stale.
//!
//! **A prompt.** `lum task erase` asks before destroying history. Nothing here can ask, so
//! `task.erase` requires `confirm: true` in its parameters and refuses without it. That is
//! the same decision the prompt makes, moved to where the caller can make it.
//!
//! # Framing
//!
//! Newline-delimited JSON, and LSP-style `Content-Length` headers, chosen per message by
//! what arrived: MCP's stdio transport uses the first (§12) and `jsonrpc.el` uses the second
//! (§16.10), and both are clients this has to serve. A reply is framed the way its request
//! was.

use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jiff::Zoned;
use lumenna_parse::complete::{Syntax, complete};
use lumenna_parse::quickadd::{Known, parse_quick_add};
use serde::Serialize;
use serde_json::{Value, json};

use crate::api::{self, Outcome, Response};
use crate::error::{CliError, Result};
use crate::profile::Profile;
use crate::{
    BlockCommand, Command, ConfigCommand, DependCommand, FilterCommand, LabelCommand,
    ProjectCommand, TaskCommand, dispatch, state,
};

/// How often to look for changes another process wrote.
///
/// SQLite has no cross-process notification, so §8 sanctions a one-second timer where file
/// watching is unreliable. What makes it cheap is that `refresh` settles in one pragma read
/// whether there is anything at all to do.
const POLL: Duration = Duration::from_secs(1);

/// The notification a client listens for. Everything else is a reply.
const CHANGED: &str = "lumenna/changed";

/// Every method this server answers, in the order `--help` presents the commands.
const METHODS: &[&str] = &[
    "initialize",
    "shutdown",
    "task.add",
    "task.list",
    "task.show",
    "task.edit",
    "task.done",
    "task.undone",
    "task.rm",
    "task.restore",
    "task.erase",
    "task.move",
    "task.search",
    "task.depend.add",
    "task.depend.rm",
    "project.add",
    "project.list",
    "project.rename",
    "project.archive",
    "project.rm",
    "project.weight",
    "label.add",
    "label.list",
    "label.rename",
    "label.merge",
    "label.rm",
    "filter.add",
    "filter.list",
    "filter.rm",
    "plan",
    "block.add",
    "block.list",
    "block.rm",
    "assign",
    "unassign",
    "start",
    "stop",
    "config.get",
    "config.set",
    "complete",
    "preview",
];

/// How a message is delimited on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Framing {
    /// One JSON object per line — MCP's stdio transport.
    Lines,
    /// `Content-Length` headers, a blank line, then the body — what `jsonrpc.el` speaks.
    Headers,
}

/// Everything both threads touch.
struct Server {
    profile: Mutex<Profile>,
    writer: Mutex<Writer>,
    running: AtomicBool,
}

/// Stdout, and the framing to write with.
struct Writer {
    out: Box<dyn Write + Send>,
    framing: Framing,
}

/// Serves one client until its input ends or it asks to stop.
///
/// # Errors
///
/// If stdin cannot be read.
pub fn serve(mut profile: Profile) -> Result<()> {
    profile.detach_rows();
    let server = Arc::new(Server {
        profile: Mutex::new(profile),
        writer: Mutex::new(Writer {
            out: Box::new(std::io::stdout()),
            framing: Framing::Lines,
        }),
        running: AtomicBool::new(true),
    });

    let watcher = Arc::clone(&server);
    std::thread::spawn(move || watch(&watcher));

    let mut reader = BufReader::new(std::io::stdin());
    while server.running.load(Ordering::Relaxed) {
        let Some((framing, text)) = read_message(&mut reader)? else { break };
        if let Ok(mut writer) = server.writer.lock() {
            writer.framing = framing;
        }
        if let Some(reply) = handle(&server, &text) {
            server.send(&reply);
        }
    }
    Ok(())
}

/// Looks for changes another process wrote, and says so.
///
/// A cursor that lags re-reads a change already held, which is a no-op; a cursor that skips
/// loses an edit — so `refresh` is the only thing that moves it, here as everywhere.
fn watch(server: &Server) {
    while server.running.load(Ordering::Relaxed) {
        std::thread::sleep(POLL);
        let changed = server
            .profile
            .lock()
            .map_or(Ok(false), |mut profile| profile.store.refresh());
        match changed {
            Ok(true) => server.send(&json!({
                "jsonrpc": "2.0",
                "method": CHANGED,
                "params": { "contract": api::VERSION },
            })),
            // A failed read is worth saying once per occurrence and not worth dying over:
            // the next poll may well succeed, and a client that lost its server entirely
            // has no way to tell why.
            Err(error) => server.send(&json!({
                "jsonrpc": "2.0",
                "method": CHANGED,
                "params": { "error": error.to_string() },
            })),
            Ok(false) => {}
        }
    }
}

impl Server {
    /// Writes one message, framed the way the last request was.
    fn send<T: Serialize>(&self, message: &T) {
        let Ok(text) = serde_json::to_string(message) else { return };
        let Ok(mut writer) = self.writer.lock() else { return };
        let framed = match writer.framing {
            Framing::Lines => format!("{text}\n"),
            Framing::Headers => format!("Content-Length: {}\r\n\r\n{text}", text.len()),
        };
        let _ = writer.out.write_all(framed.as_bytes());
        let _ = writer.out.flush();
    }
}

/// Reads one message, in whichever framing it arrives in.
fn read_message(reader: &mut impl BufRead) -> Result<Option<(Framing, String)>> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(length) = trimmed.strip_prefix("Content-Length:") else {
            return Ok(Some((Framing::Lines, trimmed.to_owned())));
        };
        let length: usize = length
            .trim()
            .parse()
            .map_err(|_| CliError::Message(format!("bad Content-Length: {trimmed}")))?;
        // Whatever other headers follow, the body starts after the blank line.
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body)?;
        return Ok(Some((Framing::Headers, String::from_utf8_lossy(&body).into_owned())));
    }
}

// JSON-RPC 2.0 error codes. The first four are the specification's; the last is ours, for a
// request that was well formed and could not be carried out.
const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const COMMAND_FAILED: i32 = -32000;

/// Answers one message, or returns nothing if it was a notification.
fn handle(server: &Server, text: &str) -> Option<Value> {
    let request: Value = match serde_json::from_str(text) {
        Ok(request) => request,
        Err(error) => return Some(failure(&Value::Null, PARSE_ERROR, &error.to_string())),
    };
    if !request.is_object() {
        // Batches are not supported. Saying so beats answering half of one.
        return Some(failure(&Value::Null, INVALID_REQUEST, "expected one request object"));
    }
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Some(failure(&id, INVALID_REQUEST, "no method"));
    };
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    let outcome = answer(server, method, &params);
    // A request without an id is a notification: it wants the work done and no reply.
    request.get("id")?;
    Some(match outcome {
        Ok(response) => json!({ "jsonrpc": "2.0", "id": id, "result": response }),
        Err(RpcError { code, message }) => failure(&id, code, &message),
    })
}

fn failure(id: &Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A code and a sentence, which is all JSON-RPC carries.
struct RpcError {
    code: i32,
    message: String,
}

impl From<CliError> for RpcError {
    fn from(error: CliError) -> Self {
        Self { code: COMMAND_FAILED, message: error.to_string() }
    }
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError { code: INVALID_PARAMS, message: message.into() }
}

fn answer(
    server: &Server,
    method: &str,
    params: &Value,
) -> std::result::Result<Response, RpcError> {
    match method {
        "initialize" => Ok(Response::new(
            "Lumenna",
            Outcome::Server(api::ServerInfo {
                name: "lumenna",
                version: env!("CARGO_PKG_VERSION"),
                contract: api::VERSION,
                methods: METHODS.to_vec(),
            }),
        )),
        "shutdown" => {
            server.running.store(false, Ordering::Relaxed);
            Ok(Response::unchanged("Stopping"))
        }
        "complete" => completions(server, params),
        "preview" => preview(server, params),
        _ => {
            let command = command_for(method, params)?;
            let now = Zoned::now();
            let mut profile = server
                .profile
                .lock()
                .map_err(|_| RpcError { code: COMMAND_FAILED, message: "store is wedged".into() })?;
            // Another process may have written since the last poll, and answering from a
            // stale document would be a wrong answer rather than a slow one.
            profile.store.refresh().map_err(CliError::Store)?;
            Ok(dispatch(&mut profile, &command, &now)?)
        }
    }
}

// ---------------------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------------------

fn text_of(params: &Value, key: &str) -> std::result::Result<String, RpcError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid(format!("'{key}' is required and has to be a string")))
}

fn maybe_text(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn flag(params: &Value, key: &str) -> bool {
    params.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn maybe_number(params: &Value, key: &str) -> std::result::Result<Option<u32>, RpcError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .map(Some)
            .ok_or_else(|| invalid(format!("'{key}' has to be a whole number"))),
    }
}

/// Builds the same command the command line builds.
fn command_for(method: &str, params: &Value) -> std::result::Result<Command, RpcError> {
    let words = |key: &str| -> std::result::Result<Vec<String>, RpcError> {
        Ok(vec![text_of(params, key)?])
    };
    let maybe_words =
        |key: &str| -> Vec<String> { maybe_text(params, key).into_iter().collect() };

    Ok(match method {
        "task.add" => Command::Task(TaskCommand::Add { text: words("text")?, quiet: false }),
        "task.list" => Command::Task(TaskCommand::List { query: maybe_words("query") }),
        "task.show" => Command::Task(TaskCommand::Show { id: text_of(params, "id")? }),
        "task.edit" => Command::Task(TaskCommand::Edit {
            id: text_of(params, "id")?,
            title: maybe_text(params, "title"),
            due: maybe_text(params, "due"),
            priority: maybe_number(params, "priority")?
                .map(|value| u8::try_from(value).unwrap_or(u8::MAX)),
            estimate: maybe_text(params, "estimate"),
            notes: maybe_text(params, "notes"),
            project: maybe_text(params, "project"),
        }),
        "task.done" => Command::Task(TaskCommand::Done { id: text_of(params, "id")? }),
        "task.undone" => Command::Task(TaskCommand::Undone { id: text_of(params, "id")? }),
        "task.rm" => Command::Task(TaskCommand::Rm { id: text_of(params, "id")? }),
        "task.restore" => Command::Task(TaskCommand::Restore { id: text_of(params, "id")? }),
        "task.erase" => {
            // Nothing here can prompt, so the caller confirms instead. §9: this rebuilds the
            // document and cannot be undone.
            if !flag(params, "confirm") {
                return Err(invalid(
                    "erasing is permanent and cannot be undone; pass \"confirm\": true",
                ));
            }
            Command::Task(TaskCommand::Erase { id: text_of(params, "id")?, yes: true })
        }
        "task.move" => Command::Task(TaskCommand::Move {
            id: text_of(params, "id")?,
            parent: maybe_text(params, "parent"),
            project: maybe_text(params, "project"),
            top: flag(params, "top"),
        }),
        "task.search" => Command::Task(TaskCommand::Search { text: words("text")? }),
        "task.depend.add" => Command::Task(TaskCommand::Depend(DependCommand::Add {
            id: text_of(params, "id")?,
            on: text_of(params, "on")?,
        })),
        "task.depend.rm" => Command::Task(TaskCommand::Depend(DependCommand::Rm {
            id: text_of(params, "id")?,
            on: text_of(params, "on")?,
        })),
        "project.add" => Command::Project(ProjectCommand::Add {
            name: text_of(params, "name")?,
            parent: maybe_text(params, "parent"),
        }),
        "project.list" => Command::Project(ProjectCommand::List),
        "project.rename" => Command::Project(ProjectCommand::Rename {
            name: text_of(params, "name")?,
            to: text_of(params, "to")?,
        }),
        "project.archive" => {
            Command::Project(ProjectCommand::Archive { name: text_of(params, "name")? })
        }
        "project.rm" => Command::Project(ProjectCommand::Rm {
            name: text_of(params, "name")?,
            keep_tasks: flag(params, "keep_tasks"),
        }),
        "project.weight" => Command::Project(ProjectCommand::Weight {
            name: text_of(params, "name")?,
            // A number, or the string "inherit" to go back to the parent's weight.
            value: match params.get("value") {
                Some(Value::Number(number)) => number.to_string(),
                Some(Value::String(text)) if text.eq_ignore_ascii_case("inherit") => text.clone(),
                _ => return Err(invalid("'value' is required: a number, or \"inherit\"")),
            },
        }),
        "label.add" => Command::Label(LabelCommand::Add { name: text_of(params, "name")? }),
        "label.list" => Command::Label(LabelCommand::List),
        "label.rename" => Command::Label(LabelCommand::Rename {
            name: text_of(params, "name")?,
            to: text_of(params, "to")?,
        }),
        "label.merge" => Command::Label(LabelCommand::Merge {
            from: text_of(params, "from")?,
            into: text_of(params, "into")?,
        }),
        "label.rm" => Command::Label(LabelCommand::Rm { name: text_of(params, "name")? }),
        "filter.add" => Command::Filter(FilterCommand::Add {
            name: text_of(params, "name")?,
            query: words("query")?,
        }),
        "filter.list" => Command::Filter(FilterCommand::List),
        "filter.rm" => Command::Filter(FilterCommand::Rm { name: text_of(params, "name")? }),
        "plan" => Command::Plan { date: maybe_words("date") },
        "block.add" => Command::Block(BlockCommand::Add {
            title: text_of(params, "title")?,
            at: text_of(params, "at")?,
            minutes: maybe_number(params, "minutes")?
                .ok_or_else(|| invalid("'minutes' is required"))?,
            date: maybe_text(params, "date"),
            kind: maybe_text(params, "kind").unwrap_or_else(|| "work".to_owned()),
            repeat: maybe_text(params, "repeat"),
        }),
        "block.list" => Command::Block(BlockCommand::List),
        "block.rm" => Command::Block(BlockCommand::Rm { id: text_of(params, "id")? }),
        "assign" => Command::Assign {
            task: text_of(params, "task")?,
            block: text_of(params, "block")?,
            date: maybe_text(params, "date"),
            minutes: maybe_number(params, "minutes")?,
        },
        "unassign" => Command::Unassign { assignment: text_of(params, "assignment")? },
        "start" => Command::Start { assignment: text_of(params, "assignment")? },
        "stop" => Command::Stop {
            assignment: text_of(params, "assignment")?,
            minutes: maybe_number(params, "minutes")?,
        },
        "config.get" => Command::Config(ConfigCommand::Get { key: maybe_text(params, "key") }),
        "config.set" => Command::Config(ConfigCommand::Set {
            key: text_of(params, "key")?,
            value: text_of(params, "value")?,
        }),
        other => {
            let hint = lumenna_core::suggest::nearest(other, METHODS.iter().copied())
                .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"));
            return Err(RpcError {
                code: METHOD_NOT_FOUND,
                message: format!("no method called '{other}'{hint}"),
            });
        }
    })
}

// ---------------------------------------------------------------------------------------
// Reachable over RPC only
// ---------------------------------------------------------------------------------------

fn completions(server: &Server, params: &Value) -> std::result::Result<Response, RpcError> {
    let text = text_of(params, "text")?;
    let cursor = maybe_number(params, "cursor")?
        .map_or(text.len(), |cursor| cursor as usize)
        .min(text.len());
    let syntax = match maybe_text(params, "syntax").as_deref() {
        None | Some("quick-add") => Syntax::QuickAdd,
        Some("filter") => Syntax::Filter,
        Some(other) => {
            return Err(invalid(format!("'{other}' is not a syntax; use quick-add or filter")));
        }
    };

    let profile = server
        .profile
        .lock()
        .map_err(|_| RpcError { code: COMMAND_FAILED, message: "store is wedged".into() })?;
    let snapshot = state(&profile);
    let found = complete(&text, cursor, syntax, &Known::from_snapshot(&snapshot));
    // §6.3 wants the count announced before the list, and the announcement is core's
    // sentence, so it travels as one — the candidates travel as components.
    Ok(Response::new(
        found.announcement.clone(),
        Outcome::Completions(api::Completions::of(&found)),
    ))
}

fn preview(server: &Server, params: &Value) -> std::result::Result<Response, RpcError> {
    let text = text_of(params, "text")?;
    let now = Zoned::now();
    let profile = server
        .profile
        .lock()
        .map_err(|_| RpcError { code: COMMAND_FAILED, message: "store is wedged".into() })?;
    let snapshot = state(&profile);
    let parsed = parse_quick_add(&text, &Known::from_snapshot(&snapshot));
    let resolved = parsed.resolve(&snapshot, &now);
    Ok(Response::new(
        resolved.announcement(),
        Outcome::Preview(api::Preview::of(&resolved, &snapshot)),
    ))
}
