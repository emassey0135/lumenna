//! `lum rpc` — the typed command surface over JSON-RPC on stdio (§8).
//!
//! One server per client, for clients that cannot link Rust: Emacs (§16.10) and the BTSpeak
//! app (§16.11). It opens the store exactly as a linked client does, watches for changes
//! another process wrote, and pushes them — which is the whole reason to speak a protocol
//! rather than shell out to `lum --json` per command. No Iroh: syncing is the daemon's job,
//! and this is not the daemon.
//!
//! **The surface is `lumenna_surface`, serialised.** A method builds the same [`Command`] the
//! command line builds and hands it to the same `dispatch`, which calls the same typed
//! operations the phone apps link — so RPC cannot drift from the CLI or from them. There is
//! one implementation of every operation, and this is a way in for clients that cannot link
//! it. §8's *one protocol, two transports* then costs the daemon nothing beyond a socket: it
//! serves exactly this.
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

use lumenna_surface::Syntax;
use serde::Serialize;
use serde_json::{Value, json};

use crate::api::{self, Response};
use crate::error::{CliError, Result};
use crate::profile::Profile;
use crate::{
    BlockCommand, Command, ConfigCommand, DependCommand, FilterCommand, LabelCommand,
    ProjectCommand, TaskCommand, dispatch,
};

/// How often to look for changes another process wrote.
///
/// SQLite has no cross-process notification, so §8 sanctions a one-second timer where file
/// watching is unreliable. What makes it cheap is that `refresh` settles in one pragma read
/// whether there is anything at all to do.
const POLL: Duration = Duration::from_secs(1);

/// How often a resident server asks whether a backup is due. Hourly is plenty against a
/// daily default, and the question is a directory listing.
const BACKUP_CHECK: Duration = Duration::from_secs(60 * 60);

/// The notification a client listens for. Everything else is a reply.
const CHANGED: &str = "lumenna/changed";

/// What a pairing tells its client while it runs: the code to give the other device, then the
/// words to compare.
const PAIRING: &str = "lumenna/pairing";

/// How long a pairing waits for the person to say whether the words match.
const CONFIRM_WAIT: Duration = Duration::from_secs(10 * 60);

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
    "project.move",
    "project.order",
    "project.weight",
    "label.add",
    "label.list",
    "label.rename",
    "label.merge",
    "label.rm",
    "label.order",
    "label.colour",
    "filter.add",
    "filter.list",
    "filter.edit",
    "filter.order",
    "filter.rm",
    "plan",
    "block.add",
    "block.list",
    "block.show",
    "block.edit",
    "block.cancel",
    "block.restore",
    "block.rm",
    "assign",
    "unassign",
    "start",
    "stop",
    "config.get",
    "config.set",
    "undo",
    "redo",
    "pair",
    "pair.confirm",
    "pair.cancel",
    "sync",
    "sync.status",
    "device.list",
    "device.rename",
    "device.unpair",
    "backup",
    "restore",
    "export",
    "import",
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
    sync_hook: Option<SyncHook>,
    /// The pairing under way, if there is one. One at a time: two would each show words,
    /// and a person could not tell which answer went where.
    pairing: Mutex<Option<Arc<Pairing>>>,
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
pub fn serve(profile: Profile) -> Result<()> {
    // A resident server is exactly the "something that stays running" §9 means, so it takes
    // a backup when one is due at start, and checks again on the hour while it runs.
    crate::durability::back_up_if_due(&profile);
    serve_streams(profile, BufReader::new(std::io::stdin()), Box::new(std::io::stdout()), None)
}

/// What answers `sync` when this server runs inside `lum sync-daemon`: the daemon already
/// holds the endpoint, so a sync there is a request to it rather than a second endpoint.
pub type SyncHook = Arc<dyn Fn() -> Result<Response> + Send + Sync>;

/// Serves the surface over any pair of streams — stdio for `lum rpc`, a socket connection
/// for the daemon (§8: one protocol, two transports).
pub fn serve_streams(
    mut profile: Profile,
    mut reader: impl BufRead,
    out: Box<dyn Write + Send>,
    sync_hook: Option<SyncHook>,
) -> Result<()> {
    profile.detach_rows();
    let server = Arc::new(Server {
        profile: Mutex::new(profile),
        writer: Mutex::new(Writer { out, framing: Framing::Lines }),
        running: AtomicBool::new(true),
        sync_hook,
        pairing: Mutex::new(None),
    });

    let watcher = Arc::clone(&server);
    std::thread::spawn(move || watch(&watcher));

    let result = (|| {
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
    })();
    // The client has gone; the watcher stops with it, and a pairing nobody can answer
    // gives up rather than waiting out its ten minutes.
    server.running.store(false, Ordering::Relaxed);
    if let Some(pairing) = server.pairing.lock().ok().and_then(|p| p.clone()) {
        pairing.cancel();
    }
    result
}

/// Looks for changes another process wrote, and says so.
///
/// A cursor that lags re-reads a change already held, which is a no-op; a cursor that skips
/// loses an edit — so `refresh` is the only thing that moves it, here as everywhere.
fn watch(server: &Server) {
    let mut since_backup_check = Duration::ZERO;
    while server.running.load(Ordering::Relaxed) {
        std::thread::sleep(POLL);
        since_backup_check += POLL;
        if since_backup_check >= BACKUP_CHECK {
            since_backup_check = Duration::ZERO;
            if let Ok(profile) = server.profile.lock() {
                crate::durability::back_up_if_due(&profile);
            }
        }
        let changed = server
            .profile
            .lock()
            .map_or(Ok(false), |profile| profile.refresh());
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
    fn profile(&self) -> std::result::Result<std::sync::MutexGuard<'_, Profile>, RpcError> {
        self.profile
            .lock()
            .map_err(|_| RpcError { code: COMMAND_FAILED, message: "store is wedged".into() })
    }

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
fn handle(server: &Arc<Server>, text: &str) -> Option<Value> {
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

    // A pairing answers when it is over, minutes from now, from a thread of its own; the
    // server goes on answering in the meantime — `pair.confirm` among the rest.
    if method == "pair" {
        return start_pairing(server, id, &params).err().map(|RpcError { code, message }| {
            failure(&request.get("id").cloned().unwrap_or(Value::Null), code, &message)
        });
    }

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

impl From<lumenna_surface::LumennaError> for RpcError {
    fn from(error: lumenna_surface::LumennaError) -> Self {
        Self { code: COMMAND_FAILED, message: error.message().to_owned() }
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
    // Inside the daemon, a sync is a request to the endpoint it already holds.
    if method == "sync"
        && let Some(hook) = &server.sync_hook
    {
        return Ok(hook()?);
    }
    match method {
        "initialize" => Ok(Response::new(api::ServerInfo {
            announcement: "Lumenna".to_owned(),
            notices: Vec::new(),
            name: "lumenna",
            version: env!("CARGO_PKG_VERSION"),
            contract: api::VERSION,
            methods: METHODS.to_vec(),
        })),
        "shutdown" => {
            server.running.store(false, Ordering::Relaxed);
            Ok(Response::unchanged("Stopping"))
        }
        "complete" => completions(server, params),
        "preview" => preview(server, params),
        "pair.confirm" => {
            let Some(answer) = params.get("match").and_then(Value::as_bool) else {
                return Err(invalid("'match' is required: true if the words are the same"));
            };
            current_pairing(server)?.answer(answer);
            Ok(Response::unchanged(if answer { "Confirming" } else { "Refusing" }))
        }
        "pair.cancel" => {
            current_pairing(server)?.cancel();
            Ok(Response::unchanged("Cancelling the pairing"))
        }
        _ => {
            let command = command_for(method, params)?;
            // Every operation reads what another process wrote since the last poll first,
            // so a reply is never staler than the notification that preceded it.
            Ok(dispatch(&*server.profile()?, &command)?)
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

/// `"up"` or `"down"`.
fn way(params: &Value) -> std::result::Result<crate::Way, RpcError> {
    match maybe_text(params, "direction").as_deref() {
        Some("up") => Ok(crate::Way::Up),
        Some("down") => Ok(crate::Way::Down),
        _ => Err(invalid("'direction' is required: \"up\" or \"down\"")),
    }
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
            repeat: maybe_text(params, "repeat"),
            priority: maybe_number(params, "priority")?
                .map(|value| u8::try_from(value).unwrap_or(u8::MAX)),
            estimate: maybe_text(params, "estimate"),
            notes: maybe_text(params, "notes"),
            project: maybe_text(params, "project"),
            // An array of names; the command line's form is one comma-separated string.
            labels: match params.get("labels") {
                None | Some(Value::Null) => None,
                Some(Value::Array(names)) => Some(
                    names
                        .iter()
                        .map(|n| n.as_str().map(ToOwned::to_owned))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| invalid("'labels' has to be an array of names"))?
                        .join(","),
                ),
                Some(_) => return Err(invalid("'labels' has to be an array of names")),
            },
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
        "project.move" => Command::Project(ProjectCommand::Move {
            name: text_of(params, "name")?,
            parent: maybe_text(params, "parent"),
            top: maybe_text(params, "parent").is_none(),
        }),
        "project.order" => Command::Project(ProjectCommand::Order {
            name: text_of(params, "name")?,
            direction: way(params)?,
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
        "label.order" => Command::Label(LabelCommand::Order {
            name: text_of(params, "name")?,
            direction: way(params)?,
        }),
        "label.colour" => Command::Label(LabelCommand::Colour {
            name: text_of(params, "name")?,
            colour: maybe_text(params, "colour").unwrap_or_else(|| "none".to_owned()),
        }),
        "filter.add" => Command::Filter(FilterCommand::Add {
            name: text_of(params, "name")?,
            query: words("query")?,
        }),
        "filter.list" => Command::Filter(FilterCommand::List),
        "filter.rm" => Command::Filter(FilterCommand::Rm { name: text_of(params, "name")? }),
        "filter.edit" => Command::Filter(FilterCommand::Edit {
            name: text_of(params, "name")?,
            rename: maybe_text(params, "rename"),
            query: maybe_text(params, "query"),
        }),
        "filter.order" => Command::Filter(FilterCommand::Order {
            name: text_of(params, "name")?,
            direction: way(params)?,
        }),
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
        "block.show" => Command::Block(BlockCommand::Show { id: text_of(params, "id")? }),
        "block.rm" => Command::Block(BlockCommand::Rm { id: text_of(params, "id")? }),
        "block.edit" => Command::Block(BlockCommand::Edit {
            id: text_of(params, "id")?,
            title: maybe_text(params, "title"),
            at: maybe_text(params, "at"),
            minutes: maybe_number(params, "minutes")?,
            kind: maybe_text(params, "kind"),
            repeat: maybe_text(params, "repeat"),
            date: maybe_text(params, "date"),
            all: flag(params, "all"),
        }),
        "block.cancel" => Command::Block(BlockCommand::Cancel {
            id: text_of(params, "id")?,
            date: text_of(params, "date")?,
        }),
        "block.restore" => Command::Block(BlockCommand::Restore {
            id: text_of(params, "id")?,
            date: text_of(params, "date")?,
        }),
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
        "sync" => Command::Sync { what: None, local_only: flag(params, "local_only") },
        "sync.status" => {
            Command::Sync { what: Some(crate::SyncCommand::Status), local_only: false }
        }
        "device.list" => Command::Device(crate::DeviceCommand::List),
        "device.rename" => Command::Device(crate::DeviceCommand::Rename {
            device: text_of(params, "device")?,
            name: text_of(params, "name")?,
        }),
        "device.unpair" => Command::Device(crate::DeviceCommand::Unpair {
            device: text_of(params, "device")?,
        }),
        "undo" => Command::Undo,
        "redo" => Command::Redo,
        "backup" => Command::Backup { to: maybe_text(params, "to").map(Into::into) },
        "restore" => Command::Restore { file: text_of(params, "file")?.into() },
        // Without `output` the export comes back in the reply, as `content`.
        "export" => Command::Export {
            format: match maybe_text(params, "format") {
                None => crate::durability::ExportFormat::Json,
                Some(word) => lumenna_surface::ExportFormat::from_word(&word).map(Into::into).ok_or_else(|| {
                    invalid(format!("'{word}' is not a format; use json, markdown, org or ics"))
                })?,
            },
            output: maybe_text(params, "output").map(Into::into),
            force: flag(params, "force"),
        },
        "import" => Command::Import { file: text_of(params, "file")?.into() },
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
        .unwrap_or_else(|| u32::try_from(text.len()).unwrap_or(u32::MAX));
    let syntax = match maybe_text(params, "syntax").as_deref() {
        None | Some("quick-add") => Syntax::QuickAdd,
        Some("filter") => Syntax::Filter,
        Some(other) => {
            return Err(invalid(format!("'{other}' is not a syntax; use quick-add or filter")));
        }
    };
    // §6.3 wants the count announced before the list, and the announcement is core's
    // sentence, so it travels as one — the candidates travel as components.
    Ok(Response::new(server.profile()?.complete_text(&text, cursor, syntax)?))
}

fn preview(server: &Server, params: &Value) -> std::result::Result<Response, RpcError> {
    let text = text_of(params, "text")?;
    Ok(Response::new(server.profile()?.preview_task(&text)?))
}

// ---------------------------------------------------------------------------------------
// Pairing (§7)
// ---------------------------------------------------------------------------------------

/// One pairing's conversation with its client: whether the words matched, and whether the
/// person gave up.
#[derive(Default)]
struct Pairing {
    answer: Mutex<Option<bool>>,
    answered: std::sync::Condvar,
    cancelled: AtomicBool,
}

impl Pairing {
    fn answer(&self, matched: bool) {
        if let Ok(mut answer) = self.answer.lock() {
            *answer = Some(matched);
        }
        self.answered.notify_all();
    }

    /// Ends the wait for the other device, and says no to the words if they were asked.
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.answer(false);
    }
}

/// The pairing's questions, as notifications to the client that asked for it.
struct RpcPrompt {
    server: Arc<Server>,
    pairing: Arc<Pairing>,
    name: String,
}

impl lumenna_surface::PairingPrompt for RpcPrompt {
    fn show_code(&self, code: String) {
        self.server.send(&json!({
            "jsonrpc": "2.0",
            "method": PAIRING,
            "params": { "code": code, "name": self.name },
        }));
    }

    fn confirm(&self, words: Vec<String>) -> bool {
        self.server.send(&json!({
            "jsonrpc": "2.0",
            "method": PAIRING,
            "params": { "words": words },
        }));
        let Ok(answer) = self.pairing.answer.lock() else { return false };
        let waited = self.pairing.answered.wait_timeout_while(answer, CONFIRM_WAIT, |answer| {
            answer.is_none() && !self.pairing.cancelled.load(Ordering::Relaxed)
        });
        waited.ok().and_then(|(answer, _)| *answer).unwrap_or(false)
    }

    fn is_cancelled(&self) -> bool {
        self.pairing.cancelled.load(Ordering::Relaxed)
            || !self.server.running.load(Ordering::Relaxed)
    }
}

fn current_pairing(server: &Server) -> std::result::Result<Arc<Pairing>, RpcError> {
    server
        .pairing
        .lock()
        .ok()
        .and_then(|pairing| pairing.clone())
        .ok_or_else(|| RpcError { code: COMMAND_FAILED, message: "no pairing is under way".into() })
}

/// Starts `pair` on a thread of its own, which sends the reply when the pairing ends.
fn start_pairing(server: &Arc<Server>, id: Value, params: &Value) -> std::result::Result<(), RpcError> {
    let pairing = Arc::new(Pairing::default());
    {
        let mut current = server
            .pairing
            .lock()
            .map_err(|_| RpcError { code: COMMAND_FAILED, message: "store is wedged".into() })?;
        if current.is_some() {
            return Err(RpcError {
                code: COMMAND_FAILED,
                message: "a pairing is already under way; cancel it first".into(),
            });
        }
        *current = Some(Arc::clone(&pairing));
    }
    let lumenna = server.profile()?.shared();
    let code = maybe_text(params, "code");
    let reach = crate::network::network(flag(params, "local_only"));
    let name = maybe_text(params, "name").unwrap_or_else(crate::network::default_name);
    let server = Arc::clone(server);
    std::thread::spawn(move || {
        let prompt =
            Arc::new(RpcPrompt { server: Arc::clone(&server), pairing, name: name.clone() });
        let result =
            lumenna.pair(code, reach, name, crate::network::platform().to_owned(), prompt);
        if let Ok(mut current) = server.pairing.lock() {
            *current = None;
        }
        server.send(&match result {
            Ok(paired) => json!({ "jsonrpc": "2.0", "id": id, "result": Response::new(paired) }),
            Err(error) => failure(&id, COMMAND_FAILED, error.message()),
        });
    });
    Ok(())
}
