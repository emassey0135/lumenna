//! One client's conversation: framing, the method table, pushed changes, and pairing.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    BlockEdit, BlockScope, Lumenna, LumennaError, MoveTarget, NewBlock, Reach, SyncReport, Syntax,
    TaskEdit,
};
use serde::Serialize;
use serde_json::{Value, json};

use super::api::{self, Derived, Response};

/// How often to look for changes another process wrote.
///
/// SQLite has no cross-process notification, and file watching is unreliable, so a timer it
/// is. What makes it cheap is that `outside_version` is one pragma read.
const POLL: Duration = Duration::from_secs(1);

/// How often a server that takes backups asks whether one is due. Hourly is plenty against a
/// daily default, and the question is a directory listing.
const BACKUP_CHECK: Duration = Duration::from_secs(60 * 60);

/// The notification a client listens for. Everything else is a reply.
const CHANGED: &str = "lumenna/changed";

/// What a pairing tells its client while it runs: the code to give the other device, then the
/// words to compare.
const PAIRING: &str = "lumenna/pairing";

/// How long a pairing waits for the person to say whether the words match.
const CONFIRM_WAIT: Duration = Duration::from_secs(10 * 60);

/// Every method this server answers.
pub const METHODS: &[&str] = &[
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
    "block.choices",
    "block.edit",
    "block.cancel",
    "block.restore",
    "block.rm",
    "assign",
    "unassign",
    "length",
    "start",
    "pause",
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
    "act",
    "choices",
    "places",
    "form.task_fields",
    "form.task_edit",
    "form.block_fields",
    "form.day_block_fields",
    "form.block_edit",
    "form.new_block",
    "form.block_defaults",
    "form.priorities",
    "form.not_offered",
];

/// What runs a round on the endpoint this process holds, when it holds one: a `sync` here is
/// a request to it rather than a second endpoint.
pub type SyncHook = Arc<dyn Fn() -> crate::Result<SyncReport> + Send + Sync>;

/// Who is serving, which the client is told and pairing says of this device.
#[derive(Clone)]
pub struct Host {
    /// What `initialize` says is answering: `daemon`, an app's name, or `lum rpc`.
    pub process: String,
    /// What a pairing calls this device, unless the client names it.
    pub device_name: String,
    /// What this device runs, as the device list says it: `macos`, `linux`, `btspeak`.
    pub platform: String,
    /// The holder's round, when this process holds the endpoint.
    pub sync: Option<SyncHook>,
    /// Whether this server takes the automatic backup, at start and hourly. A server of a
    /// client's own does; one inside a process that already takes them does not.
    pub backups: bool,
}

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
    lumenna: Arc<Lumenna>,
    host: Host,
    writer: Mutex<Writer>,
    running: AtomicBool,
    /// The pairing under way, if there is one. One at a time: two would each show words,
    /// and a person could not tell which answer went where.
    pairing: Mutex<Option<Arc<Pairing>>>,
}

/// Where replies go, and the framing to write with.
struct Writer {
    out: Box<dyn Write + Send>,
    framing: Framing,
}

/// Serves one client over any pair of streams — stdio, a socket connection, a named pipe —
/// until its input ends or it asks to stop.
///
/// Row numbers mean nothing here: the store is addressed by identifier, which never goes
/// stale, where a row number is one terminal's last listing.
///
/// # Errors
///
/// If the input cannot be read.
pub fn serve_streams(
    lumenna: Arc<Lumenna>,
    mut reader: impl BufRead,
    out: Box<dyn Write + Send>,
    host: Host,
) -> std::io::Result<()> {
    if host.backups {
        back_up_if_due(&lumenna);
    }
    let server = Arc::new(Server {
        lumenna,
        host,
        writer: Mutex::new(Writer { out, framing: Framing::Lines }),
        running: AtomicBool::new(true),
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

fn back_up_if_due(lumenna: &Lumenna) {
    if let Err(error) = lumenna.back_up_if_due() {
        eprintln!("lumenna: the automatic backup failed: {error}");
    }
}

/// Looks for changes another process wrote, and says so.
///
/// By `outside_version`, not by `refresh`'s answer: a request refreshes first too, and one
/// that arrived just after another process wrote would take the change in and leave this
/// poll told nothing — the client's other views would stay stale. The client's own writes do
/// not move it, as they never came back as a notification.
fn watch(server: &Server) {
    let mut seen = server.lumenna.outside_version().ok();
    let mut since_backup_check = Duration::ZERO;
    while server.running.load(Ordering::Relaxed) {
        std::thread::sleep(POLL);
        since_backup_check += POLL;
        if server.host.backups && since_backup_check >= BACKUP_CHECK {
            since_backup_check = Duration::ZERO;
            back_up_if_due(&server.lumenna);
        }
        let changed = server.lumenna.outside_version().map(|now| {
            let moved = Some(now) != seen;
            seen = Some(now);
            moved
        });
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
                "params": { "error": error.message() },
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
fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<(Framing, String)>> {
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
        let length: usize = length.trim().parse().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bad Content-Length: {trimmed}"))
        })?;
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

impl From<LumennaError> for RpcError {
    fn from(error: LumennaError) -> Self {
        Self { code: COMMAND_FAILED, message: error.message().to_owned() }
    }
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError { code: INVALID_PARAMS, message: message.into() }
}

fn failed(message: impl Into<String>) -> RpcError {
    RpcError { code: COMMAND_FAILED, message: message.into() }
}

type Answer = std::result::Result<Response, RpcError>;

#[allow(clippy::too_many_lines)] // One arm per method, which is the table this is.
fn answer(server: &Server, method: &str, params: &Value) -> Answer {
    let l = &*server.lumenna;
    let id = || text_of(params, "id");
    let name = || text_of(params, "name");
    Ok(match method {
        "initialize" => Response::new(api::ServerInfo {
            announcement: "Lumenna".to_owned(),
            notices: Vec::new(),
            name: "lumenna",
            version: env!("CARGO_PKG_VERSION"),
            process: server.host.process.clone(),
            contract: api::VERSION,
            methods: METHODS.to_vec(),
        }),
        "shutdown" => {
            server.running.store(false, Ordering::Relaxed);
            Response::unchanged("Stopping")
        }

        "task.add" => Response::new(l.add_task(&text_of(params, "text")?)?),
        "task.list" => Response::new(l.list_tasks(&maybe_text(params, "query").unwrap_or_default())?),
        "task.show" => Response::new(l.show_task(&id()?)?),
        "task.edit" => Response::new(l.edit_task(&id()?, TaskEdit {
            title: maybe_text(params, "title"),
            due: maybe_text(params, "due"),
            repeat: maybe_text(params, "repeat"),
            priority: maybe_number(params, "priority")?.map(|value| u8::try_from(value).unwrap_or(u8::MAX)),
            estimate: maybe_text(params, "estimate"),
            notes: maybe_text(params, "notes"),
            project: maybe_text(params, "project"),
            labels: match params.get("labels") {
                None | Some(Value::Null) => None,
                Some(Value::Array(names)) => Some(
                    names
                        .iter()
                        .map(|n| n.as_str().map(|n| n.trim().to_owned()))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| invalid("'labels' has to be an array of names"))?
                        .into_iter()
                        .filter(|n| !n.is_empty())
                        .collect(),
                ),
                Some(_) => return Err(invalid("'labels' has to be an array of names")),
            },
        })?),
        "task.done" => Response::new(l.complete_task(&id()?)?),
        "task.undone" => Response::new(l.uncomplete_task(&id()?)?),
        "task.rm" => Response::new(l.trash_task(&id()?)?),
        "task.restore" => Response::new(l.restore_task(&id()?)?),
        "task.erase" => {
            // Nothing here can prompt, so the caller confirms instead.
            if !flag(params, "confirm") {
                return Err(invalid("deleting from the trash asks first; pass \"confirm\": true"));
            }
            Response::new(l.erase_task(&id()?)?)
        }
        "task.move" => {
            let to = match (maybe_text(params, "parent"), maybe_text(params, "project"), flag(params, "top")) {
                (Some(parent), _, _) => MoveTarget::Parent { id: parent },
                (_, Some(project), _) => MoveTarget::Project { name: project },
                (_, _, true) => MoveTarget::Top,
                _ => return Err(invalid("say where: 'parent', 'project', or 'top': true")),
            };
            Response::new(l.move_task(&id()?, to)?)
        }
        "task.search" => Response::new(l.search_tasks(&text_of(params, "text")?)?),
        "task.depend.add" => Response::new(l.add_dependency(&id()?, &text_of(params, "on")?)?),
        "task.depend.rm" => Response::new(l.remove_dependency(&id()?, &text_of(params, "on")?)?),

        "project.add" => Response::new(l.add_project(&name()?, maybe_text(params, "parent"))?),
        "project.list" => Response::new(l.list_projects()?),
        "project.rename" => Response::new(l.rename_project(&name()?, &text_of(params, "to")?)?),
        "project.archive" => Response::new(l.archive_project(&name()?)?),
        "project.rm" => Response::new(l.delete_project(&name()?, flag(params, "keep_tasks"))?),
        "project.move" => Response::new(l.move_project(&name()?, maybe_text(params, "parent"))?),
        "project.order" => Response::new(l.reorder_project(&name()?, way(params)?)?),
        "project.weight" => {
            // A number, or the string "inherit" to go back to the parent's weight.
            let value = match params.get("value") {
                Some(Value::Number(number)) => number.to_string(),
                Some(Value::String(text)) if text.eq_ignore_ascii_case("inherit") => text.clone(),
                _ => return Err(invalid("'value' is required: a number, or \"inherit\"")),
            };
            Response::new(l.weigh_project(&name()?, crate::parse_weight(value)?)?)
        }

        "label.add" => Response::new(l.add_label(&name()?)?),
        "label.list" => Response::new(l.list_labels()?),
        "label.rename" => Response::new(l.rename_label(&name()?, &text_of(params, "to")?)?),
        "label.merge" => Response::new(l.merge_labels(&text_of(params, "from")?, &text_of(params, "into")?)?),
        "label.rm" => Response::new(l.delete_label(&name()?)?),
        "label.order" => Response::new(l.reorder_label(&name()?, way(params)?)?),
        "label.colour" => Response::new(l.recolour_label(
            &name()?,
            maybe_text(params, "colour").filter(|c| !c.eq_ignore_ascii_case("none")),
        )?),

        "filter.add" => Response::new(l.add_filter(&name()?, &text_of(params, "query")?)?),
        "filter.list" => Response::new(l.list_filters()?),
        "filter.edit" => {
            Response::new(l.edit_filter(&name()?, maybe_text(params, "rename"), maybe_text(params, "query"))?)
        }
        "filter.order" => Response::new(l.reorder_filter(&name()?, way(params)?)?),
        "filter.rm" => Response::new(l.delete_filter(&name()?)?),

        "plan" => Response::new(l.plan(maybe_text(params, "date").filter(|d| !d.trim().is_empty()))?),
        "block.add" => {
            let extras = extras(params)?;
            Response::new(l.add_block(NewBlock {
                title: text_of(params, "title")?,
                at: text_of(params, "at")?,
                minutes: maybe_number(params, "minutes")?.ok_or_else(|| invalid("'minutes' is required"))?,
                date: maybe_text(params, "date"),
                kind: maybe_text(params, "kind").unwrap_or_else(|| "work".to_owned()),
                repeat: maybe_text(params, "repeat"),
                notes: extras.notes,
                accepts_tasks: extras.accepts_tasks,
                counts_capacity: extras.counts_capacity,
                anchored: extras.anchored,
                min_minutes: extras.min_minutes,
                task_filter: extras.task_filter,
                until: extras.until,
                colour: extras.colour,
            })?)
        }
        "block.list" => Response::new(l.list_blocks()?),
        "block.show" => Response::new(l.show_block(&id()?)?),
        "block.choices" => {
            // The work blocks a task could go in, from the task: a chooser's question.
            Response::new(l.work_blocks(maybe_text(params, "from"), maybe_number(params, "days")?)?)
        }
        "block.edit" => {
            let block = id()?;
            let scope = match maybe_text(params, "date") {
                Some(date) => BlockScope::Occurrence { date },
                None => BlockScope::Series,
            };
            // Asked of a repeating block every time, never guessed.
            if scope == BlockScope::Series && !flag(params, "all") && l.show_block(&block)?.repeats {
                return Err(invalid(
                    "that block repeats; give a 'date' to change one day, or 'all': true to change every one",
                ));
            }
            let extras = extras(params)?;
            Response::new(l.edit_block(&block, BlockEdit {
                title: maybe_text(params, "title"),
                at: maybe_text(params, "at"),
                minutes: maybe_number(params, "minutes")?,
                kind: maybe_text(params, "kind"),
                repeat: maybe_text(params, "repeat"),
                notes: extras.notes,
                accepts_tasks: extras.accepts_tasks,
                counts_capacity: extras.counts_capacity,
                anchored: extras.anchored,
                min_minutes: extras.min_minutes,
                task_filter: extras.task_filter,
                until: extras.until,
                colour: extras.colour,
            }, scope)?)
        }
        "block.cancel" => Response::new(l.cancel_occurrence(&id()?, &text_of(params, "date")?)?),
        "block.restore" => Response::new(l.restore_occurrence(&id()?, &text_of(params, "date")?)?),
        "block.rm" => Response::new(l.delete_block(&id()?)?),

        "assign" => Response::new(l.assign(
            &text_of(params, "task")?,
            &text_of(params, "block")?,
            maybe_text(params, "date"),
            maybe_number(params, "minutes")?,
        )?),
        "unassign" => Response::new(l.unassign(&text_of(params, "assignment")?)?),
        // A number, or null to clear it.
        "length" => Response::new(l.plan_minutes(&text_of(params, "assignment")?, maybe_number(params, "minutes")?)?),
        "start" => Response::new(l.start_timer(&text_of(params, "assignment")?)?),
        "pause" => Response::new(l.pause_timer(&text_of(params, "assignment")?)?),
        "stop" => Response::new(l.stop_timer(&text_of(params, "assignment")?, maybe_number(params, "minutes")?)?),

        "config.get" => Response::new(l.settings(maybe_text(params, "key"))?),
        "config.set" => Response::new(l.set_setting(&text_of(params, "key")?, &text_of(params, "value")?)?),
        "undo" => Response::new(l.undo()?),
        "redo" => Response::new(l.redo()?),

        "pair.confirm" => {
            let Some(answer) = params.get("match").and_then(Value::as_bool) else {
                return Err(invalid("'match' is required: true if the words are the same"));
            };
            current_pairing(server)?.answer(answer);
            Response::unchanged(if answer { "Confirming" } else { "Refusing" })
        }
        "pair.cancel" => {
            current_pairing(server)?.cancel();
            Response::unchanged("Cancelling the pairing")
        }
        "sync" => Response::new(match &server.host.sync {
            // Inside the process holding the endpoint, a sync is a request to it.
            Some(hook) => hook()?,
            None => l.sync_now(reach(params))?,
        }),
        "sync.status" => Response::new(l.sync_status()?),
        "device.list" => Response::new(l.devices()?),
        "device.rename" => Response::new(l.rename_device(&text_of(params, "device")?, &name()?)?),
        "device.unpair" => Response::new(l.unpair_device(&text_of(params, "device")?)?),

        "backup" => Response::new(l.backup(maybe_text(params, "to"))?),
        "restore" => Response::new(l.restore(&text_of(params, "file")?)?),
        // Without `output` the export comes back in the reply, as `content`.
        "export" => {
            let format = match maybe_text(params, "format") {
                None => crate::ExportFormat::Json,
                Some(word) => crate::ExportFormat::from_word(&word)
                    .ok_or_else(|| invalid(format!("'{word}' is not a format; use json, markdown, org or ics")))?,
            };
            let output = maybe_text(params, "output");
            let force = flag(params, "force");
            if let Some(output) = &output
                && std::path::Path::new(output).exists()
                && !force
            {
                return Err(failed(format!("{output} already exists; pass \"force\": true to replace it")));
            }
            Response::new(l.export(format, output, force)?)
        }
        "import" => Response::new(l.import(&text_of(params, "file")?)?),

        "complete" => {
            let text = text_of(params, "text")?;
            let cursor = maybe_number(params, "cursor")?.unwrap_or_else(|| u32::try_from(text.len()).unwrap_or(u32::MAX));
            let syntax = match maybe_text(params, "syntax").as_deref() {
                None | Some("quick-add") => Syntax::QuickAdd,
                Some("filter") => Syntax::Filter,
                Some(other) => return Err(invalid(format!("'{other}' is not a syntax; use quick-add or filter"))),
            };
            // The count is said before the list, and the announcement is core's sentence, so
            // it travels as one — the candidates travel as components.
            Response::new(l.complete_text(&text, cursor, syntax)?)
        }
        "preview" => Response::new(l.preview_task(&text_of(params, "text")?)?),

        // What a row's actions ask and do, as every client offers them.
        "act" => Response::new(l.act(record(params, "action")?, maybe_record(params, "answer")?.unwrap_or(crate::Answer::Yes))?),
        "choices" => Response::new(l.choices(record(params, "action")?)?),
        "places" => Response::new(l.places()),

        // The forms' rules, for a client that cannot call them in-process.
        "form.task_fields" => Response::new(Derived::of(crate::task_fields(record(params, "task")?))),
        "form.task_edit" => Response::new(Derived::of(crate::task_edit(record(params, "task")?, record(params, "fields")?))),
        "form.block_fields" => Response::new(Derived::of(crate::block_fields(record(params, "block")?))),
        "form.day_block_fields" => Response::new(Derived::of(crate::day_block_fields(record(params, "block")?))),
        "form.block_edit" => {
            Response::new(Derived::of(crate::block_edit(record(params, "before")?, record(params, "after")?)?))
        }
        "form.new_block" => {
            Response::new(Derived::of(crate::new_block(record(params, "fields")?, maybe_text(params, "date"))?))
        }
        "form.block_defaults" => Response::new(Derived::of(crate::block_defaults(text_of(params, "kind")?))),
        "form.priorities" => Response::new(Derived::of(crate::priorities())),
        "form.not_offered" => Response::new(Derived::of(crate::not_offered(
            record(params, "kind")?,
            record(params, "subject")?,
            flag(params, "this_device"),
        ))),

        other => {
            let hint = lumenna_core::suggest::nearest(other, METHODS.iter().copied())
                .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"));
            return Err(RpcError { code: METHOD_NOT_FOUND, message: format!("no method called '{other}'{hint}") });
        }
    })
}

// ---------------------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------------------

/// A parameter holding one of the surface's records, as the surface serialises it.
fn record<T: serde::de::DeserializeOwned>(params: &Value, key: &str) -> std::result::Result<T, RpcError> {
    maybe_record(params, key)?.ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn maybe_record<T: serde::de::DeserializeOwned>(params: &Value, key: &str) -> std::result::Result<Option<T>, RpcError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|e| invalid(format!("'{key}' does not read: {e}"))),
    }
}

fn text_of(params: &Value, key: &str) -> std::result::Result<String, RpcError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid(format!("'{key}' is required and has to be a string")))
}

/// A yes-or-no parameter: absent, or a boolean.
fn maybe_bool(params: &Value, key: &str) -> std::result::Result<Option<bool>, RpcError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(invalid(format!("'{key}' must be true or false"))),
    }
}

fn maybe_text(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn flag(params: &Value, key: &str) -> bool {
    params.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn reach(params: &Value) -> Reach {
    if flag(params, "local_only") { Reach::LocalOnly } else { Reach::Internet }
}

/// `"up"` or `"down"`.
fn way(params: &Value) -> std::result::Result<crate::Direction, RpcError> {
    match maybe_text(params, "direction").as_deref() {
        Some("up") => Ok(crate::Direction::Up),
        Some("down") => Ok(crate::Direction::Down),
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

/// What a block can be given beyond its time, length, kind and repetition.
struct Extras {
    notes: Option<String>,
    accepts_tasks: Option<bool>,
    counts_capacity: Option<bool>,
    anchored: Option<bool>,
    min_minutes: Option<u32>,
    task_filter: Option<String>,
    until: Option<String>,
    colour: Option<String>,
}

fn extras(params: &Value) -> std::result::Result<Extras, RpcError> {
    Ok(Extras {
        notes: maybe_text(params, "notes"),
        accepts_tasks: maybe_bool(params, "accepts_tasks")?,
        counts_capacity: maybe_bool(params, "counts_capacity")?,
        anchored: maybe_bool(params, "anchored")?,
        min_minutes: maybe_number(params, "min_minutes")?,
        task_filter: maybe_text(params, "task_filter"),
        until: maybe_text(params, "until"),
        colour: maybe_text(params, "colour"),
    })
}

// ---------------------------------------------------------------------------------------
// Pairing
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

impl crate::PairingPrompt for RpcPrompt {
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
        self.pairing.cancelled.load(Ordering::Relaxed) || !self.server.running.load(Ordering::Relaxed)
    }
}

fn current_pairing(server: &Server) -> std::result::Result<Arc<Pairing>, RpcError> {
    server.pairing.lock().ok().and_then(|pairing| pairing.clone()).ok_or_else(|| failed("no pairing is under way"))
}

/// Starts `pair` on a thread of its own, which sends the reply when the pairing ends.
fn start_pairing(server: &Arc<Server>, id: Value, params: &Value) -> std::result::Result<(), RpcError> {
    let pairing = Arc::new(Pairing::default());
    {
        let mut current = server.pairing.lock().map_err(|_| failed("store is wedged"))?;
        if current.is_some() {
            return Err(failed("a pairing is already under way; cancel it first"));
        }
        *current = Some(Arc::clone(&pairing));
    }
    let lumenna = Arc::clone(&server.lumenna);
    let code = maybe_text(params, "code");
    let reach = reach(params);
    let name = maybe_text(params, "name").unwrap_or_else(|| server.host.device_name.clone());
    let platform = server.host.platform.clone();
    let server = Arc::clone(server);
    std::thread::spawn(move || {
        let prompt = Arc::new(RpcPrompt { server: Arc::clone(&server), pairing, name: name.clone() });
        let result = lumenna.pair(code, reach, name, platform, prompt);
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
