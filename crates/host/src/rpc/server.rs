//! One client's conversation: framing, the method table, pushed changes, and pairing.

use std::io::{BufRead, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    BlockEdit, Lumenna, LumennaError, MoveTarget, NewBlock, Reach, SyncReport,
    TaskEdit,
};
use serde::Serialize;
use serde_json::{Value, json};

use super::api::{self, Response};

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
    // The app's, where whoever serves gives one (`Apps`).
    "app.send",
    "app.view",
    "app.close",
    "app.still",
    "app.tick",
    "app.in_front",
    "app.clock",
    "app.complete",
    "shutdown",
    "task.add",
    "task.list",
    "task.show",
    "task.history",
    "task.edit",
    "task.done",
    "task.undone",
    "task.rm",
    "task.restore",
    "task.zone",
    "task.erase",
    "task.erase_permanently",
    "erase_everything_deleted",
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
    "project.colour",
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
    "filter.colour",
    "filter.rm",
    "plan",
    "block.add",
    "block.list",
    "block.show",
    "block.edit",
    "block.cancel",
    "block.restore",
    "block.rm",
    "block.zone",
    "end",
    "extend",
    "end_day",
    "clear_day",
    "assign",
    "unassign",
    "skip",
    "reorder",
    "carry",
    "carry_rest",
    "length",
    "start",
    "pause",
    "stop",
    "timer.control",
    "timer.toggle",
    "what_now",
    "start_on",
    "replan",
    "replan.apply",
    "day_plan",
    "day_plan.apply",
    "history",
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
    "reminder.give",
    "reminder.deliver",
    "reminder.due",
    "reminder.upcoming",
    "reminder.list",
    "reminder.add",
    "reminder.rm",
    "reminder.dismiss",
    "reminder.snooze",
    // The assistant table's operations, as `lum assist` runs them: for a client that offers
    // every one, such as the PowerShell module.
    "assist",
    "assist.list",
    "complete",
];

/// What a block's add and edit take beyond their own.
const EXTRAS: [&str; 8] =
    ["notes", "accepts_tasks", "counts_capacity", "anchored", "min_minutes", "task_filter", "until", "colour"];

/// The parameters each method takes, every one of [`METHODS`] but the app's (whose are the
/// app's to read). A name a method does not take is refused, as `assist` refuses one: a
/// client's typo should not pass for a parameter left out.
const TAKES: &[(&str, &[&str])] = &[
    ("initialize", &[]),
    ("shutdown", &[]),
    ("task.add", &["text"]),
    ("task.list", &["query"]),
    ("task.show", &["id"]),
    ("task.history", &["id"]),
    ("task.edit", &["id", "title", "due", "repeat", "priority", "estimate", "notes", "project", "labels"]),
    ("task.done", &["id"]),
    ("task.undone", &["id"]),
    ("task.rm", &["id"]),
    ("task.restore", &["id"]),
    ("task.zone", &["id", "zone"]),
    ("task.erase", &["id", "confirm"]),
    ("task.erase_permanently", &["id", "confirm"]),
    ("erase_everything_deleted", &["confirm"]),
    ("task.move", &["id", "parent", "project", "top"]),
    ("task.search", &["text"]),
    ("task.depend.add", &["id", "on"]),
    ("task.depend.rm", &["id", "on"]),
    ("project.add", &["name", "parent"]),
    ("project.list", &[]),
    ("project.rename", &["name", "to"]),
    ("project.archive", &["name"]),
    ("project.rm", &["name", "keep_tasks"]),
    ("project.move", &["name", "parent"]),
    ("project.order", &["name", "direction"]),
    ("project.weight", &["name", "value"]),
    ("project.colour", &["name", "colour"]),
    ("label.add", &["name"]),
    ("label.list", &[]),
    ("label.rename", &["name", "to"]),
    ("label.merge", &["from", "into"]),
    ("label.rm", &["name"]),
    ("label.order", &["name", "direction"]),
    ("label.colour", &["name", "colour"]),
    ("filter.add", &["name", "query"]),
    ("filter.list", &[]),
    ("filter.edit", &["name", "rename", "query"]),
    ("filter.order", &["name", "direction"]),
    ("filter.colour", &["name", "colour"]),
    ("filter.rm", &["name"]),
    ("plan", &["date"]),
    ("block.add", &["title", "at", "minutes", "date", "kind", "repeat", "+extras"]),
    ("block.list", &[]),
    ("block.show", &["id"]),
    ("block.edit", &["id", "title", "at", "minutes", "kind", "repeat", "date", "all", "+extras"]),
    ("block.cancel", &["id", "date"]),
    ("block.restore", &["id", "date"]),
    ("block.rm", &["id"]),
    ("block.zone", &["id", "zone"]),
    ("end", &["block"]),
    ("extend", &["block", "minutes"]),
    ("end_day", &[]),
    ("clear_day", &[]),
    ("assign", &["task", "block", "date", "minutes"]),
    ("unassign", &["assignment"]),
    ("skip", &["assignment", "again"]),
    ("reorder", &["assignment", "direction"]),
    ("carry", &["assignment", "to"]),
    ("carry_rest", &["block"]),
    ("length", &["assignment", "minutes"]),
    ("start", &["assignment"]),
    ("pause", &["assignment"]),
    ("stop", &["assignment", "minutes"]),
    ("timer.control", &["now"]),
    ("timer.toggle", &["now"]),
    ("what_now", &["now"]),
    ("start_on", &["choice", "now"]),
    ("replan", &["clock", "pins", "now"]),
    ("replan.apply", &["proposal", "accepted", "now"]),
    ("day_plan", &["date", "clock", "pins", "now"]),
    ("day_plan.apply", &["plan", "accepted", "now"]),
    ("history", &[]),
    ("config.get", &["key"]),
    ("config.set", &["key", "value"]),
    ("undo", &[]),
    ("redo", &[]),
    ("pair", &["code", "local_only", "name"]),
    ("pair.confirm", &["match"]),
    ("pair.cancel", &[]),
    ("sync", &["local_only"]),
    ("sync.status", &[]),
    ("device.list", &[]),
    ("device.rename", &["device", "name"]),
    ("device.unpair", &["device"]),
    ("backup", &["to"]),
    ("restore", &["file"]),
    ("export", &["format", "output", "force"]),
    ("import", &["file"]),
    ("reminder.give", &["now"]),
    ("reminder.deliver", &["id", "on", "now"]),
    ("reminder.due", &["since", "now"]),
    ("reminder.upcoming", &["until"]),
    ("reminder.list", &["target"]),
    ("reminder.add", &["target", "anchor", "offset_mins", "saying"]),
    ("reminder.rm", &["id"]),
    ("reminder.dismiss", &["key", "occurrence"]),
    ("reminder.snooze", &["key", "occurrence", "minutes"]),
    ("assist", &["operation", "args"]),
    ("assist.list", &[]),
    ("complete", &["kind", "text"]),
];

/// The refusal of a parameter `method` does not take, if `params` has one. A method this
/// does not know is left to the dispatcher, which says so in its own words.
fn untaken(method: &str, params: &Value) -> Option<RpcError> {
    let (_, takes) = TAKES.iter().find(|(name, _)| *name == method)?;
    let names: Vec<&str> =
        takes.iter().flat_map(|name| if *name == "+extras" { EXTRAS.to_vec() } else { vec![*name] }).collect();
    let given = params.as_object()?;
    let name = given.keys().find(|key| !names.contains(&key.as_str()))?;
    Some(invalid(match names.as_slice() {
        [] => format!("{method} takes nothing, so not '{name}'"),
        _ => format!("{method} takes no '{name}'; it takes {}", names.join(", ")),
    }))
}

/// What runs a round on the endpoint this process holds, when it holds one: a `sync` here is
/// a request to it rather than a second endpoint.
pub type SyncHook = Arc<dyn Fn() -> crate::Result<SyncReport> + Send + Sync>;

/// The notification a client of the app listens for: a screen's turn.
pub const TURN: &str = "lumenna/turn";

/// Where an app sends what it leaves a client: each turn's params, pushed as [`TURN`].
pub type Notify = Arc<dyn Fn(Value) + Send + Sync>;

/// What runs the Crux app for a client that cannot link it (Emacs): the app's screens over the
/// client's own store connection, opened at its first `app.*` call. The host knows nothing of
/// the app; whoever serves gives it this (`lumenna-ffi`'s).
pub trait Apps: Send + Sync {
    /// The app for one client, over `lumenna`, sending its turns through `notify`.
    ///
    /// # Errors
    ///
    /// If the app cannot be started.
    fn open(&self, lumenna: Arc<Lumenna>, notify: Notify) -> crate::Result<Box<dyn AppSession>>;
}

/// One client's app: answers its `app.*` methods.
pub trait AppSession: Send + Sync {
    /// Answers `method` with `params`, or why not.
    ///
    /// # Errors
    ///
    /// If the params do not read, or the app refuses.
    fn call(&self, method: &str, params: &Value) -> std::result::Result<Value, String>;
}

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
    /// The app, for a client that asks for it (`app.*`); none where nobody gave one.
    pub apps: Option<Arc<dyn Apps>>,
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
    /// The right to give this device's reminders, once this client asked to and no other
    /// process had it; let go with the connection.
    reminders: Mutex<Option<Arc<crate::ReminderLock>>>,
    /// This client's app, once it asked for it; gone with the connection.
    app: Mutex<Option<Box<dyn AppSession>>>,
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
        reminders: Mutex::new(None),
        app: Mutex::new(None),
    });

    let watcher = Arc::clone(&server);
    std::thread::spawn(move || watch(&watcher));

    let result = (|| {
        while server.running.load(Ordering::Relaxed) {
            let Some(message) = read_message(&mut reader)? else { break };
            let (framing, reply) = match message {
                Message::Text(framing, text) => (framing, handle(&server, &text)),
                Message::Refused(framing, why) => (framing, Some(failure(&Value::Null, PARSE_ERROR, &why))),
            };
            if let Ok(mut writer) = server.writer.lock() {
                writer.framing = framing;
            }
            if let Some(reply) = reply {
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
    // The app's threads stop with it.
    server.app.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
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

/// The longest message read, in either framing: far beyond any request, which is a few
/// hundred bytes or a proposal handed back, and short enough that one bad header cannot have
/// the server allocate what the machine does not have.
const LONGEST: usize = 64 * 1024 * 1024;

/// What a message read turned out to be.
enum Message {
    /// A request's text, in the framing it came in.
    Text(Framing, String),
    /// One the server will not read, and why, answered as a parse error. Its bytes are passed
    /// over, so the next message is read from where it starts.
    Refused(Framing, String),
}

/// Reads one message, in whichever framing it arrives in.
fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<Message>> {
    loop {
        let Some(line) = read_line(reader)? else { return Ok(None) };
        let Ok(line) = line else {
            return Ok(Some(Message::Refused(
                Framing::Lines,
                format!("a message is longer than the {} MiB this reads", LONGEST >> 20),
            )));
        };
        // A byte order mark is passed over: .NET Framework writes one at the start of a child's
        // input whenever the console's code page is UTF-8, before the client has sent anything,
        // and Windows PowerShell 5.1 starts `lum rpc` so.
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed.is_empty() {
            continue;
        }
        let Some(length) = trimmed.strip_prefix("Content-Length:") else {
            return Ok(Some(Message::Text(Framing::Lines, trimmed.to_owned())));
        };
        let length: usize = length.trim().parse().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bad Content-Length: {trimmed}"))
        })?;
        // Whatever other headers follow, the body starts after the blank line.
        loop {
            match read_line(reader)? {
                None => break,
                Some(Ok(header)) if header.trim().is_empty() => break,
                Some(_) => {}
            }
        }
        if length > LONGEST {
            // Passed over rather than read, so the conversation goes on after it.
            let skipped = std::io::copy(&mut reader.by_ref().take(length as u64), &mut std::io::sink())?;
            if skipped < length as u64 {
                return Ok(None);
            }
            return Ok(Some(Message::Refused(
                Framing::Headers,
                format!("a Content-Length of {length} is more than the {} MiB this reads", LONGEST >> 20),
            )));
        }
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body)?;
        return Ok(Some(Message::Text(Framing::Headers, String::from_utf8_lossy(&body).into_owned())));
    }
}

/// Reads one line, no longer than [`LONGEST`]: `Err` for one that was longer, whose rest is
/// passed over up to its end; `None` at the end of the input.
fn read_line(reader: &mut impl BufRead) -> std::io::Result<Option<std::result::Result<String, ()>>> {
    let mut line = Vec::new();
    let read = reader.by_ref().take(LONGEST as u64 + 1).read_until(b'\n', &mut line)?;
    if read == 0 {
        return Ok(None);
    }
    if line.len() > LONGEST {
        let mut rest = Vec::new();
        loop {
            rest.clear();
            if reader.by_ref().take(64 * 1024).read_until(b'\n', &mut rest)? == 0 || rest.ends_with(b"\n") {
                break;
            }
        }
        return Ok(Some(Err(())));
    }
    Ok(Some(Ok(String::from_utf8_lossy(&line).into_owned())))
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
    if let Some(RpcError { code, message }) = untaken(method, &params) {
        request.get("id")?;
        return Some(failure(&id, code, &message));
    }

    // A pairing answers when it is over, minutes from now, from a thread of its own; the
    // server goes on answering in the meantime — `pair.confirm` among the rest.
    if method == "pair" {
        return start_pairing(server, id, &params).err().map(|RpcError { code, message }| {
            failure(&request.get("id").cloned().unwrap_or(Value::Null), code, &message)
        });
    }

    if let Some(app) = method.strip_prefix("app.") {
        let outcome = call_app(server, app, &params);
        request.get("id")?;
        return Some(match outcome {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(RpcError { code, message }) => failure(&id, code, &message),
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

/// An `app.*` method, on this client's app, opened at the first.
fn call_app(server: &Arc<Server>, method: &str, params: &Value) -> std::result::Result<Value, RpcError> {
    let Some(apps) = server.host.apps.clone() else {
        return Err(RpcError { code: METHOD_NOT_FOUND, message: "no app is served here".to_owned() });
    };
    let mut app = server.app.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if app.is_none() {
        let weak = Arc::downgrade(server);
        let notify: Notify = Arc::new(move |params| {
            if let Some(server) = weak.upgrade() {
                server.send(&json!({ "jsonrpc": "2.0", "method": TURN, "params": params }));
            }
        });
        *app = Some(apps.open(Arc::clone(&server.lumenna), notify)?);
    }
    let session = app.as_ref().expect("just opened");
    session.call(method, params).map_err(invalid)
}

fn failure(id: &Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A code and a sentence, which is all JSON-RPC carries.
#[derive(Debug)]
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
        "task.history" => Response::new(l.task_history(&id()?)?),
        "task.edit" => Response::new(l.edit_task(&id()?, TaskEdit {
            title: maybe_text(params, "title"),
            due: maybe_text(params, "due"),
            repeat: maybe_text(params, "repeat"),
            // Anything else is the surface's to refuse, in the words every way in uses.
            priority: maybe_number(params, "priority")?
                .map(|value| {
                    u8::try_from(value).map_err(|_| failed(format!("Priority takes 1 to 4, where 1 is highest, not {value}.")))
                })
                .transpose()?,
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
        // A place's zone, or null to let it float with the device's.
        "task.zone" => Response::new(l.set_task_zone(&id()?, maybe_text(params, "zone"))?),
        "task.erase" => {
            // Nothing here can prompt, so the caller confirms instead.
            if !flag(params, "confirm") {
                return Err(invalid("deleting from the trash asks first; pass \"confirm\": true"));
            }
            Response::new(l.erase_task(&id()?)?)
        }
        "task.erase_permanently" => {
            if !flag(params, "confirm") {
                return Err(invalid("erasing permanently asks first; pass \"confirm\": true"));
            }
            Response::new(l.erase_task_permanently(&id()?)?)
        }
        "erase_everything_deleted" => {
            if !flag(params, "confirm") {
                return Err(invalid("erasing permanently asks first; pass \"confirm\": true"));
            }
            Response::new(l.erase_everything_deleted()?)
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
        "project.colour" => Response::new(l.recolour_project(&name()?, colour(params))?),
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
        "label.colour" => Response::new(l.recolour_label(&name()?, colour(params))?),

        "filter.add" => Response::new(l.add_filter(&name()?, &text_of(params, "query")?)?),
        "filter.list" => Response::new(l.list_filters()?),
        "filter.edit" => {
            Response::new(l.edit_filter(&name()?, maybe_text(params, "rename"), maybe_text(params, "query"))?)
        }
        "filter.order" => Response::new(l.reorder_filter(&name()?, way(params)?)?),
        "filter.colour" => Response::new(l.recolour_filter(&name()?, colour(params))?),
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
        "block.edit" => {
            let block = id()?;
            // Asked of a repeating block every time, never guessed.
            let scope = l.block_scope(&block, maybe_text(params, "date"), flag(params, "all"), "'date', or 'all': true")?;
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
        "block.zone" => Response::new(l.set_block_zone(&id()?, maybe_text(params, "zone"))?),
        // The day's own: the block under way ended now or given longer, the rest of today
        // cleared, with or without the block under way.
        "end" => Response::new(l.end_block_now(&text_of(params, "block")?)?),
        "extend" => Response::new(l.extend_block(
            &text_of(params, "block")?,
            maybe_number(params, "minutes")?.unwrap_or(crate::day::EXTEND_MINS),
        )?),
        "end_day" => Response::new(l.end_day(true)?),
        "clear_day" => Response::new(l.end_day(false)?),

        "assign" => Response::new(l.assign(
            &text_of(params, "task")?,
            &text_of(params, "block")?,
            maybe_text(params, "date"),
            maybe_number(params, "minutes")?,
        )?),
        "unassign" => Response::new(l.unassign(&text_of(params, "assignment")?)?),
        // Skipped, it stays on the record; `again` plans a skipped one again.
        "skip" => Response::new(l.skip_sitting(&text_of(params, "assignment")?, !flag(params, "again"))?),
        "reorder" => Response::new(l.reorder_sitting(&text_of(params, "assignment")?, way(params)?)?),
        "carry" => Response::new(l.carry_forward(&text_of(params, "assignment")?, &text_of(params, "to")?)?),
        // A number, or null to clear it.
        "length" => Response::new(l.plan_minutes(&text_of(params, "assignment")?, maybe_number(params, "minutes")?)?),
        "start" => Response::new(l.start_timer(&text_of(params, "assignment")?)?),
        "pause" => Response::new(l.pause_timer(&text_of(params, "assignment")?)?),
        "stop" => Response::new(l.stop_timer(&text_of(params, "assignment")?, maybe_number(params, "minutes")?)?),
        // The timer's one control, as a tile or a watch shows it, and pressing it.
        "timer.control" => Response::new(api::TimerShown::from(l.timer_control(maybe_text(params, "now"))?)),
        "timer.toggle" => Response::new(l.toggle_timer(maybe_text(params, "now"))?),
        "history" => Response::new(l.history()?),

        "config.get" => Response::new(l.settings(maybe_text(params, "key"))?),
        "config.set" => {
            let (key, value) = (text_of(params, "key")?, text_of(params, "value")?);
            if key == "backup-dir" && !value.trim().is_empty() {
                whole_path(&key, &value)?;
            }
            Response::new(l.set_setting(&key, &value)?)
        }
        "assist" => {
            let args = match params.get("args") {
                None | Some(Value::Null) => serde_json::Map::new(),
                Some(Value::Object(args)) => args.clone(),
                Some(_) => return Err(invalid("'args' has to be an object, the operation's parameters by name")),
            };
            // One that confirms needs "confirm": true among them, as task.erase_permanently does.
            Response::new(super::assist::assist(l, &text_of(params, "operation")?, args)?)
        }
        "assist.list" => Response::new(api::Operations::all()),
        "complete" => {
            let kind = serde_json::from_value(params.get("kind").cloned().unwrap_or(Value::Null))
                .map_err(|_| invalid("'kind' is required: line, filter, project, label or saved_filter"))?;
            Response::new(l.complete_typed(kind, &maybe_text(params, "text").unwrap_or_default())?)
        }
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

        "backup" => Response::new(l.backup(maybe_path(params, "to")?)?),
        "restore" => Response::new(l.restore(&path(params, "file")?)?),
        // Without `output` the export comes back in the reply, as `content`.
        "export" => {
            let format = match maybe_text(params, "format") {
                None => crate::ExportFormat::Json,
                Some(word) => crate::ExportFormat::from_word(&word)
                    .ok_or_else(|| invalid(format!("'{word}' is not a format; use json, markdown, org or ics")))?,
            };
            match l.export(format, maybe_path(params, "output")?, flag(params, "force")) {
                Ok(exported) => Response::new(exported),
                Err(refused) if refused.message().ends_with(crate::NOT_ASKED_TO_REPLACE) => {
                    return Err(failed(format!("{}; pass \"force\": true to replace it", refused.message())));
                }
                Err(refused) => return Err(refused.into()),
            }
        }
        "import" => Response::new(l.import(&path(params, "file")?)?),

        // What a block's end offers: its short work carried to the next blocks that take it.
        "carry_rest" => Response::new(l.carry_rest_forward(&text_of(params, "block")?)?),
        // What to work on now, and starting on it.
        "what_now" => Response::new(l.what_should_i_work_on(maybe_text(params, "now"))?),
        "start_on" => Response::new(l.start_on(&text_of(params, "choice")?, maybe_text(params, "now"))?),
        // Replanning the rest of today: a proposal, then the changes the person accepted.
        "replan" => Response::new(l.replan(
            time_style(params)?,
            maybe_record(params, "pins")?.unwrap_or_default(),
            maybe_text(params, "now"),
        )?),
        "replan.apply" => Response::new(l.apply_replan(
            record(params, "proposal")?,
            record(params, "accepted")?,
            maybe_text(params, "now"),
        )?),
        // Planning a whole day: a proposal, then the sittings the person accepted.
        "day_plan" => Response::new(l.propose_day_plan(
            maybe_text(params, "date"),
            time_style(params)?,
            maybe_record(params, "pins")?.unwrap_or_default(),
            maybe_text(params, "now"),
        )?),
        "day_plan.apply" => Response::new(l.apply_day_plan(
            record(params, "plan")?,
            record(params, "accepted")?,
            maybe_text(params, "now"),
        )?),

        // Reminders: what to give, a target's own, and dealing with one firing.
        // Which devices one of a target's own reminders fires on, as `reminder deliver` says it.
        "reminder.deliver" if params.get("id").is_some() => {
            Response::new(l.deliver_reminder(id()?, text_of(params, "on")?)?)
        }
        // What to give now, for a client that gives this device's reminders (BTSpeak's service,
        // Emacs): only while no other process does, so a running app and a client never both
        // say one. Asked as `reminder.deliver` without an `id` too, its name until this one,
        // for the clients built before it.
        "reminder.give" | "reminder.deliver" => {
            let mut held = server.reminders.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if held.is_none() {
                *held = l.take_reminders()?;
            }
            match held.clone() {
                Some(lock) => Response::new(l.deliver_reminders(lock, maybe_text(params, "now"))?),
                None => Response::new(crate::DueReminders::elsewhere()),
            }
        }
        "reminder.due" => Response::new(l.reminders_due(text_of(params, "since")?, maybe_text(params, "now"))?),
        "reminder.upcoming" => Response::new(l.upcoming_reminders(text_of(params, "until")?)?),
        "reminder.list" => Response::new(l.reminders_of(text_of(params, "target")?)?),
        // A phrase of the person's own, as the apps take one ("tomorrow at 9am", "5m before
        // start"), or what it counts from and how far.
        "reminder.add" if params.get("saying").is_some() => {
            Response::new(l.add_reminder_saying(text_of(params, "target")?, text_of(params, "saying")?)?)
        }
        "reminder.add" => {
            let offset = match params.get("offset_mins") {
                None | Some(Value::Null) => 0,
                Some(value) => value
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(|| invalid("'offset_mins' has to be a whole number, negative before"))?,
            };
            Response::new(l.add_reminder(text_of(params, "target")?, text_of(params, "anchor")?, offset)?)
        }
        "reminder.rm" => Response::new(l.remove_reminder(id()?)?),
        "reminder.dismiss" => Response::new(l.dismiss_reminder(text_of(params, "key")?, text_of(params, "occurrence")?)?),
        "reminder.snooze" => Response::new(l.snooze_reminder(
            text_of(params, "key")?,
            text_of(params, "occurrence")?,
            maybe_number(params, "minutes")?,
        )?),

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

/// The clock a proposal's sentences use: `clock`, the device setting's own words.
fn time_style(params: &Value) -> std::result::Result<crate::TimeStyle, RpcError> {
    match maybe_text(params, "clock").as_deref() {
        None | Some("24-hour") => Ok(crate::TimeStyle::TwentyFourHour),
        Some("12-hour") => Ok(crate::TimeStyle::TwelveHour),
        Some(other) => Err(invalid(format!("'{other}' is not a clock; use 12-hour or 24-hour"))),
    }
}

/// A colour by name, or none for `none` or nothing.
fn colour(params: &Value) -> Option<String> {
    maybe_text(params, "colour").filter(|c| !c.eq_ignore_ascii_case("none"))
}

/// A file or directory, refused unless whole: a relative one would be taken against this
/// server's working directory, which is not the client's, and land somewhere it did not mean.
fn path(params: &Value, key: &str) -> std::result::Result<String, RpcError> {
    let path = text_of(params, key)?;
    whole_path(key, &path)?;
    Ok(path)
}

fn maybe_path(params: &Value, key: &str) -> std::result::Result<Option<String>, RpcError> {
    match maybe_text(params, key) {
        Some(path) => whole_path(key, &path).map(|()| Some(path)),
        None => Ok(None),
    }
}

fn whole_path(key: &str, path: &str) -> std::result::Result<(), RpcError> {
    if std::path::Path::new(path).is_absolute() {
        return Ok(());
    }
    Err(invalid(format!(
        "'{key}' has to be a whole path, from the root: {path} would be taken from where this server runs, not where the client is"
    )))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The methods `answer` and `handle` have an arm for, read from this file: the table
    /// cannot leave one out without this seeing it.
    fn answered() -> Vec<String> {
        let source = include_str!("server.rs");
        let start = source.find("fn answer(").expect("answer");
        let end = start + source[start..].find("        other =>").expect("the last arm");
        let mut methods: Vec<String> = source[start..end]
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with('"'))
            .flat_map(|line| {
                let arm = line.split("=>").next().unwrap_or_default();
                let arm = arm.split(" if ").next().unwrap_or_default();
                arm.split('|').map(|name| name.trim().trim_matches('"').to_owned()).collect::<Vec<_>>()
            })
            .collect();
        // Answered in `handle`, before the table.
        methods.push("pair".to_owned());
        methods.sort();
        methods.dedup();
        methods
    }

    #[test]
    fn every_method_answered_is_listed_and_every_one_listed_is_answered() {
        let mut listed: Vec<String> =
            METHODS.iter().filter(|m| !m.starts_with("app.")).map(|m| (*m).to_owned()).collect();
        listed.sort();
        assert_eq!(listed, answered());
    }

    #[test]
    fn every_method_listed_says_what_it_takes() {
        let mut listed: Vec<&str> = METHODS.iter().copied().filter(|m| !m.starts_with("app.")).collect();
        let mut taken: Vec<&str> = TAKES.iter().map(|(name, _)| *name).collect();
        listed.sort_unstable();
        taken.sort_unstable();
        assert_eq!(listed, taken);
    }

    #[test]
    fn a_parameter_a_method_does_not_take_is_refused_by_name() {
        let refused = untaken("task.edit", &json!({ "id": "0193", "titel": "Essay" })).expect("refused");
        assert_eq!(refused.code, INVALID_PARAMS);
        assert!(refused.message.contains("takes no 'titel'"), "{}", refused.message);
        assert!(untaken("block.add", &json!({ "title": "Deep work", "colour": "red" })).is_none());
        assert!(untaken("undo", &json!({ "all": true })).expect("refused").message.contains("takes nothing"));
    }

    #[test]
    fn a_content_length_beyond_what_is_read_is_refused_and_the_next_message_still_read() {
        let huge = LONGEST + 10;
        let mut input = format!("Content-Length: {huge}\r\n\r\n").into_bytes();
        input.extend(std::iter::repeat_n(b'x', huge));
        input.extend(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"undo\"}\n");
        let mut reader = std::io::BufReader::new(input.as_slice());
        assert!(matches!(read_message(&mut reader).unwrap(), Some(Message::Refused(Framing::Headers, _))));
        assert!(matches!(read_message(&mut reader).unwrap(), Some(Message::Text(Framing::Lines, text)) if text.contains("undo")));
    }

    #[test]
    fn a_content_length_far_beyond_the_input_ends_it_without_reading_that_much() {
        let mut reader = std::io::BufReader::new(&b"Content-Length: 99999999999999\r\n\r\n{}"[..]);
        assert!(read_message(&mut reader).unwrap().is_none());
    }

    #[test]
    fn a_line_longer_than_what_is_read_is_refused() {
        let mut input = vec![b'x'; LONGEST + 2];
        input.extend(b"\n{\"method\":\"undo\"}\n");
        let mut reader = std::io::BufReader::new(input.as_slice());
        assert!(matches!(read_message(&mut reader).unwrap(), Some(Message::Refused(Framing::Lines, _))));
        assert!(matches!(read_message(&mut reader).unwrap(), Some(Message::Text(_, text)) if text.contains("undo")));
    }

    #[test]
    fn a_relative_path_is_refused_and_a_whole_one_taken() {
        assert!(path(&json!({ "file": "backup.lumbak" }), "file").is_err());
        let whole = if cfg!(windows) { r"C:\backups\b.lumbak" } else { "/backups/b.lumbak" };
        assert_eq!(path(&json!({ "file": whole }), "file").unwrap(), whole);
    }
}
