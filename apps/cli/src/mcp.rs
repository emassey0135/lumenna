//! `lum mcp`: the command surface as a Model Context Protocol server on stdin and stdout, for
//! an assistant — whichever model the person chooses, in the cloud or on the machine.
//!
//! Each tool calls one `Lumenna` method and answers with its record in the envelope `lum
//! --json` and `lum rpc` write (`rpc::api::Response`), its `announcement` the core's own
//! sentence, so a model reads what a person would hear. Nothing here decides anything. The
//! tools that change the store are offered only with `--writes`, which the person sets in the
//! client's own configuration, where they choose what an assistant may do. Records are
//! addressed by identifier (a whole UUID or a prefix of one), never by row number.
//!
//! With `--listen` it is Streamable HTTP instead, for a client that cannot start `lum` itself:
//! on loopback by default, where only a loopback `Host` is answered (so neither a page that
//! rebinds a name nor a tunnel reaches it unasked); anywhere else only with this device's token
//! as a bearer. No TLS: a person serving it beyond the machine puts Tailscale or a reverse
//! proxy in front, as they would for anything else they host.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Request, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

use lumenna_host::rpc::api::{Outcome, Response};
use lumenna_host::{Lumenna, MoveTarget, TaskEdit};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource, ResourceContents,
    ResourceTemplate, ResourceUpdatedNotificationParam, ServerCapabilities, ServerConfig, SubscribeRequestParams,
    SubscriptionFilter, UnsubscribeRequestParams,
};
use rmcp::service::{RequestContext, SubscriptionContext};
use rmcp::{ErrorData, Peer, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use tokio::sync::watch;
use serde::Deserialize;

use crate::error::CliError;
use crate::profile::Profile;

/// Where to serve over HTTP.
pub(crate) struct Listen {
    /// The address to listen on, `host:port`.
    pub address: String,
    /// Whether to ask for the token on loopback too.
    pub require_token: bool,
}

/// Serves `profile`'s store until the client goes: over stdio, or over HTTP at `listen` until
/// interrupted.
pub(crate) fn serve(profile: &Profile, writes: bool, listen: Option<Listen>) -> crate::error::Result<()> {
    let lumenna = profile.shared();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Message(e.to_string()))?;
    runtime.block_on(async move {
        let server = Server::new(Arc::clone(&lumenna), writes, changes(lumenna));
        if let Some(listen) = listen {
            return http(server, &listen, token_path(profile)).await;
        }
        let running = server.serve(rmcp::transport::stdio()).await.map_err(|e| CliError::Message(e.to_string()))?;
        running.waiting().await.map_err(|e| CliError::Message(e.to_string()))?;
        Ok(())
    })
}

/// Where this device's token is kept: in the profile, beside the store, never synced.
fn token_path(profile: &Profile) -> PathBuf {
    profile.directory().join("mcp-token")
}

/// This device's token for HTTP clients, made the first time it is asked for, or afresh with
/// `new`.
pub(crate) fn token(profile: &Profile, new: bool) -> crate::error::Result<String> {
    token_at(token_path(profile), new)
}

fn token_at(path: PathBuf, new: bool) -> crate::error::Result<String> {
    let failed = |e: std::io::Error| CliError::Message(format!("the token at {}: {e}", path.display()));
    if !new && let Ok(kept) = std::fs::read_to_string(&path) && !kept.trim().is_empty() {
        return Ok(kept.trim().to_owned());
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| CliError::Message(e.to_string()))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // Written whole beside it, readable by this user alone, then put in place.
    let beside = path.with_extension("new");
    std::fs::write(&beside, &token).map_err(failed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&beside, std::fs::Permissions::from_mode(0o600)).map_err(failed)?;
    }
    std::fs::rename(&beside, &path).map_err(failed)?;
    Ok(token)
}

/// Serves Streamable HTTP at `/mcp` on `listen` until interrupted.
async fn http(server: Server, listen: &Listen, token: PathBuf) -> crate::error::Result<()> {
    let failed = |e: std::io::Error| CliError::Message(format!("cannot listen on {}: {e}", listen.address));
    let listener = tokio::net::TcpListener::bind(&listen.address).await.map_err(failed)?;
    let local = listener.local_addr().map_err(failed)?;
    // The token is read again for each request, so a new one (`lum mcp token --new`) turns the
    // old away at once.
    let token = if local.ip().is_loopback() && !listen.require_token {
        None
    } else {
        token_at(token.clone(), false)?;
        Some(Arc::new(token))
    };
    // A browser page never gets in, whatever it names (a request with an `Origin` is refused).
    // Without a token only a loopback `Host` is answered, rmcp's default; with one, any, since
    // the token is what is checked and a proxy or a tailnet names the machine its own way.
    let mut config = StreamableHttpServerConfig::default().enforce_origin_validation();
    let stopping = config.cancellation_token.clone();
    if token.is_some() {
        config = config.with_allowed_hosts(Vec::<String>::new());
    }
    let service = StreamableHttpService::new(move || Ok(server.fresh()), Arc::new(LocalSessionManager::default()), config);
    let asks = if token.is_some() { ", which asks for this device's token (lum mcp token)" } else { "" };
    eprintln!("Lumenna's MCP server is at http://{local}/mcp{asks}. Interrupt to stop.");
    loop {
        let (stream, _) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(accepted) => accepted,
                Err(_) => continue,
            },
            _ = tokio::signal::ctrl_c() => break,
        };
        let service = service.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let answering = hyper::service::service_fn(move |request: Request<Incoming>| {
                let service = service.clone();
                let token = token.clone();
                async move { Ok::<_, Infallible>(answer(&service, token.as_deref().map(PathBuf::as_path), request).await) }
            });
            let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(TokioIo::new(stream), answering)
                .await;
        });
    }
    stopping.cancel();
    Ok(())
}

type Answer = hyper::Response<http_body_util::combinators::BoxBody<Bytes, Infallible>>;

/// Answers one request: refused without the token where one is asked for, else rmcp's.
async fn answer(
    service: &StreamableHttpService<Server, LocalSessionManager>,
    token: Option<&std::path::Path>,
    request: Request<Incoming>,
) -> Answer {
    if request.uri().path() != "/mcp" {
        return plain(StatusCode::NOT_FOUND, "Lumenna's MCP server is at /mcp.");
    }
    if let Some(path) = token {
        // Unreadable, it is no token at all, and nothing is let in.
        let token = std::fs::read_to_string(path).map(|kept| kept.trim().to_owned()).unwrap_or_default();
        let sent = request.headers().get(hyper::header::AUTHORIZATION).and_then(|v| v.to_str().ok());
        let given = sent.and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
        if token.is_empty() || !same(given.trim().as_bytes(), token.as_bytes()) {
            let mut refused = plain(StatusCode::UNAUTHORIZED, "This needs the device's token, as `Authorization: Bearer <token>`.");
            refused.headers_mut().insert(hyper::header::WWW_AUTHENTICATE, hyper::header::HeaderValue::from_static("Bearer"));
            return refused;
        }
    }
    service.handle(request).await
}

/// Whether two tokens are the same, taking as long whichever byte differs.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |differ, (x, y)| differ | (x ^ y)) == 0
}

fn plain(status: StatusCode, text: &'static str) -> Answer {
    let mut response = hyper::Response::new(Full::new(Bytes::from_static(text.as_bytes())).boxed());
    *response.status_mut() = status;
    response
}

/// How often the store is looked at for a change another process or device made.
const LOOK: Duration = Duration::from_secs(1);

/// A count that moves whenever the store does: this process's own writes, which move the heads,
/// and another process's commits (an app's, `lum`'s, a sync's), which move `outside_version`
/// even when they write a document this process has not loaded yet, as a year's first block
/// does. One looks for every client of this process.
fn changes(lumenna: Arc<Lumenna>) -> watch::Receiver<u64> {
    let (moved, changes) = watch::channel(0);
    tokio::spawn(async move {
        let mut last = None;
        let mut count = 0;
        loop {
            let looking = Arc::clone(&lumenna);
            let heads = tokio::task::spawn_blocking(move || Some((looking.outside_version().ok()?, looking.heads().ok()?)))
                .await
                .ok()
                .flatten();
            if heads.is_some() && last.is_some() && heads != last {
                count += 1;
                if moved.send(count).is_err() {
                    return;
                }
            }
            last = heads.or(last);
            tokio::time::sleep(LOOK).await;
        }
    });
    changes
}

#[derive(Clone)]
struct Server {
    lumenna: Arc<Lumenna>,
    writes: bool,
    tool_router: ToolRouter<Self>,
    changes: watch::Receiver<u64>,
    /// What this client asked `resources/subscribe` to follow, in protocol versions before
    /// 2026-07-28; a client on that one listens instead (`subscriptions/listen`).
    subscribed: Arc<Mutex<BTreeSet<String>>>,
    /// Whether this client's old-style subscriptions are being followed yet.
    following: Arc<Mutex<bool>>,
}

impl Server {
    fn new(lumenna: Arc<Lumenna>, writes: bool, changes: watch::Receiver<u64>) -> Self {
        let mut tool_router = Self::reading();
        if writes {
            tool_router += Self::writing();
        }
        Self { lumenna, writes, tool_router, changes, subscribed: Arc::default(), following: Arc::default() }
    }

    /// The same server for another HTTP session, following nothing yet.
    fn fresh(&self) -> Self {
        Self { subscribed: Arc::default(), following: Arc::default(), ..self.clone() }
    }

    /// What each of `uris` reads as now, so a change can be told from a write that left it alone.
    async fn reading_of(&self, uris: impl IntoIterator<Item = String>) -> BTreeMap<String, Option<String>> {
        let lumenna = Arc::clone(&self.lumenna);
        let uris: Vec<String> = uris.into_iter().collect();
        tokio::task::spawn_blocking(move || {
            uris.into_iter().map(|uri| {
                let text = render(&lumenna, &uri).ok().flatten();
                (uri, text)
            }).collect()
        })
        .await
        .unwrap_or_default()
    }

    /// Waits for the store to change, then says which of `read`'s resources now read otherwise,
    /// keeping their new readings; none when the server is going.
    async fn changed(&self, changes: &mut watch::Receiver<u64>, read: &mut BTreeMap<String, Option<String>>) -> Option<Vec<String>> {
        changes.changed().await.ok()?;
        let now = self.reading_of(read.keys().cloned()).await;
        let moved = now.iter().filter(|(uri, text)| read.get(*uri) != Some(text)).map(|(uri, _)| uri.clone()).collect();
        *read = now;
        Some(moved)
    }

    /// Follows the old-style subscriptions for as long as the client is there, telling it of
    /// each resource that changed.
    async fn follow(self, peer: Peer<RoleServer>) {
        let mut changes = self.changes.clone();
        let subscribed = || self.subscribed.lock().unwrap().clone();
        let mut read = self.reading_of(subscribed()).await;
        loop {
            // A resource subscribed since the last change is read as it was then, so its first
            // change is told; one unsubscribed is no longer looked at.
            let now = subscribed();
            read.retain(|uri, _| now.contains(uri));
            let new: Vec<String> = now.into_iter().filter(|uri| !read.contains_key(uri)).collect();
            read.extend(self.reading_of(new).await);
            let Some(moved) = self.changed(&mut changes, &mut read).await else { return };
            for uri in moved.into_iter().filter(|uri| self.subscribed.lock().unwrap().contains(uri)) {
                if peer.notify_resource_updated(ResourceUpdatedNotificationParam::new(uri)).await.is_err() {
                    return;
                }
            }
        }
    }

    /// Runs one operation off the protocol's threads, as every operation blocks on the store,
    /// and answers with its record, or its refusal as a tool error the model can read.
    async fn run<T>(&self, operation: impl FnOnce(&Lumenna) -> lumenna_host::Result<T> + Send + 'static) -> CallToolResult
    where
        T: Into<Outcome> + Send + 'static,
    {
        let lumenna = Arc::clone(&self.lumenna);
        match tokio::task::spawn_blocking(move || operation(&lumenna)).await {
            Ok(Ok(record)) => CallToolResult::success(vec![ContentBlock::text(enveloped(record))]),
            Ok(Err(error)) => CallToolResult::error(vec![ContentBlock::text(error.message().to_owned())]),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        }
    }
}

/// A record in the envelope `lum --json` writes.
fn enveloped(record: impl Into<Outcome>) -> String {
    serde_json::to_string_pretty(&Response::new(record)).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------
// What the tools take
// ---------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Query {
    /// A filter in Lumenna's filter language, such as `overdue`, `today & #Work`, `@calls`, `p1`,
    /// or `search: words`; empty for every open task.
    #[serde(default)]
    query: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Id {
    /// The record's identifier: a whole UUID, or a prefix that names one record.
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Day {
    /// The day, as a person says it ("tomorrow", "friday", "2026-11-03"); today if left out.
    #[serde(default)]
    date: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Text {
    /// Words to search titles and notes for.
    text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Line {
    /// A quick-add line, read as a person's would be: the title, then any of a due date or time
    /// ("tomorrow at 3pm", "every weekday"), a priority (p1 to p4), labels (@calls), a project
    /// (#Work, #"Home Office"), an estimate (~30m). A line starting with `block` makes a time
    /// block instead ("block Deep work 9-11am weekdays"). The answer says how it was read.
    text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Edit {
    /// The task's identifier.
    id: String,
    /// A new title.
    #[serde(default)]
    title: Option<String>,
    /// A date phrase ("next monday", "in 3 days at 9am"), or `none` to clear it.
    #[serde(default)]
    due: Option<String>,
    /// A repetition ("every monday", "every! 2 weeks"), or `none` to stop it repeating.
    #[serde(default)]
    repeat: Option<String>,
    /// 1 to 4, where 1 is highest.
    #[serde(default)]
    priority: Option<u8>,
    /// How long it should take ("45m", "1h30m"), or `none`.
    #[serde(default)]
    estimate: Option<String>,
    /// Its notes, whole.
    #[serde(default)]
    notes: Option<String>,
    /// The project to move it to, by name.
    #[serde(default)]
    project: Option<String>,
    /// Its labels, all of them, by name.
    #[serde(default)]
    labels: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Move {
    /// The task's identifier.
    id: String,
    /// A task to make it a subtask of, by identifier.
    #[serde(default)]
    parent: Option<String>,
    /// A project to move it to, by name.
    #[serde(default)]
    project: Option<String>,
    /// True to make it a task of its own again, under no other.
    #[serde(default)]
    top: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Assign {
    /// The task's identifier.
    task: String,
    /// The block's identifier, as the day lists it.
    block: String,
    /// The day, for a block that repeats; the block's own day otherwise.
    #[serde(default)]
    date: Option<String>,
    /// How long the sitting is planned to take, in minutes.
    #[serde(default)]
    minutes: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Sitting {
    /// The sitting's identifier: a task's place in a block, as the day lists it.
    assignment: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Stop {
    /// The sitting's identifier.
    assignment: String,
    /// The minutes to log instead of the timer's, when it ran on by mistake.
    #[serde(default)]
    minutes: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Choice {
    /// The `id` of one of the choices What Should I Work On? offered.
    choice: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Apply {
    /// The proposal as `propose_replan` or `propose_day_plan` returned it, whole.
    proposal: serde_json::Value,
    /// Those of its `changes` (a replan) or `sittings` (a day plan) the person accepted, each
    /// as it was proposed.
    accepted: serde_json::Value,
}

fn record<T: serde::de::DeserializeOwned>(value: serde_json::Value, what: &str) -> lumenna_host::Result<T> {
    serde_json::from_value(value).map_err(|e| lumenna_host::LumennaError::new(format!("that is not a {what}: {e}")))
}

// ---------------------------------------------------------------------------------------
// Tools that only read
// ---------------------------------------------------------------------------------------

#[tool_router(router = reading)]
impl Server {
    /// Lists tasks: by default every open task, most urgent first, subtasks under their task;
    /// or those a filter admits.
    #[tool(annotations(title = "List tasks", read_only_hint = true))]
    async fn list_tasks(&self, Parameters(Query { query }): Parameters<Query>) -> CallToolResult {
        self.run(move |l| l.list_tasks(&query)).await
    }

    /// Searches task titles and notes for words.
    #[tool(annotations(title = "Search tasks", read_only_hint = true))]
    async fn search_tasks(&self, Parameters(Text { text }): Parameters<Text>) -> CallToolResult {
        self.run(move |l| l.search_tasks(&text)).await
    }

    /// One task whole: its fields, states, subtasks, what it waits for, its sittings and its
    /// history.
    #[tool(annotations(title = "Show a task", read_only_hint = true))]
    async fn show_task(&self, Parameters(Id { id }): Parameters<Id>) -> CallToolResult {
        self.run(move |l| l.show_task(&id)).await
    }

    /// A day as it is lived: its blocks in time order, each with the tasks in it (its
    /// sittings), the free time between them, and where now falls.
    #[tool(annotations(title = "The day", read_only_hint = true))]
    async fn day(&self, Parameters(Day { date }): Parameters<Day>) -> CallToolResult {
        self.run(move |l| l.plan(date.filter(|d| !d.trim().is_empty()))).await
    }

    /// Every time block, each with when it happens and how often.
    #[tool(annotations(title = "List blocks", read_only_hint = true))]
    async fn list_blocks(&self) -> CallToolResult {
        self.run(Lumenna::list_blocks).await
    }

    /// Every project, its subprojects under it.
    #[tool(annotations(title = "List projects", read_only_hint = true))]
    async fn list_projects(&self) -> CallToolResult {
        self.run(Lumenna::list_projects).await
    }

    /// Every label.
    #[tool(annotations(title = "List labels", read_only_hint = true))]
    async fn list_labels(&self) -> CallToolResult {
        self.run(Lumenna::list_labels).await
    }

    /// Every saved filter, with its query.
    #[tool(annotations(title = "List saved filters", read_only_hint = true))]
    async fn list_filters(&self) -> CallToolResult {
        self.run(Lumenna::list_filters).await
    }

    /// What to work on now, ranked, each choice with the reason it is offered: in a block under
    /// way its own unfinished sittings, then its suggestions; in free time, what fits before
    /// the next block. Starting on one is `start_on`.
    #[tool(annotations(title = "What should I work on?", read_only_hint = true))]
    async fn what_should_i_work_on(&self) -> CallToolResult {
        self.run(|l| l.what_should_i_work_on(None)).await
    }

    /// Proposes how to replan the rest of today, when the day has slipped: every change as a
    /// whole sentence. Nothing changes until `apply_replan` is given the ones the person
    /// accepted; say them to the person first.
    #[tool(annotations(title = "Propose a replan of today", read_only_hint = true))]
    async fn propose_replan(&self) -> CallToolResult {
        self.run(|l| l.replan(lumenna_host::TimeStyle::TwentyFourHour, Default::default(), None)).await
    }

    /// Proposes which tasks to put in each of a day's work blocks still to come, each with
    /// why. Nothing changes until `apply_day_plan` is given the sittings the person accepted.
    #[tool(annotations(title = "Propose a plan for a day", read_only_hint = true))]
    async fn propose_day_plan(&self, Parameters(Day { date }): Parameters<Day>) -> CallToolResult {
        self.run(move |l| l.propose_day_plan(date, lumenna_host::TimeStyle::TwentyFourHour, Default::default(), None)).await
    }
}

// ---------------------------------------------------------------------------------------
// Tools that change the store, offered only with `--writes`
// ---------------------------------------------------------------------------------------

#[tool_router(router = writing)]
impl Server {
    /// Adds a task (or, from a line starting with `block`, a time block) from one quick-add
    /// line, and says how it read it.
    #[tool(annotations(title = "Add a task", read_only_hint = false, destructive_hint = false))]
    async fn add_task(&self, Parameters(Line { text }): Parameters<Line>) -> CallToolResult {
        self.run(move |l| l.add_task(&text)).await
    }

    /// Changes a task's fields: only those given.
    #[tool(annotations(title = "Edit a task", read_only_hint = false, destructive_hint = false))]
    async fn edit_task(&self, Parameters(edit): Parameters<Edit>) -> CallToolResult {
        let Edit { id, title, due, repeat, priority, estimate, notes, project, labels } = edit;
        self.run(move |l| l.edit_task(&id, TaskEdit { title, due, repeat, priority, estimate, notes, project, labels })).await
    }

    /// Marks a task done. A repeating one moves on to its next date.
    #[tool(annotations(title = "Complete a task", read_only_hint = false, destructive_hint = false, idempotent_hint = true))]
    async fn complete_task(&self, Parameters(Id { id }): Parameters<Id>) -> CallToolResult {
        self.run(move |l| l.complete_task(&id)).await
    }

    /// Marks a done task not done.
    #[tool(annotations(title = "Mark a task not done", read_only_hint = false, destructive_hint = false, idempotent_hint = true))]
    async fn uncomplete_task(&self, Parameters(Id { id }): Parameters<Id>) -> CallToolResult {
        self.run(move |l| l.uncomplete_task(&id)).await
    }

    /// Moves a task to the trash, from which `restore_task` brings it back.
    #[tool(annotations(title = "Move a task to the trash", read_only_hint = false, destructive_hint = true, idempotent_hint = true))]
    async fn trash_task(&self, Parameters(Id { id }): Parameters<Id>) -> CallToolResult {
        self.run(move |l| l.trash_task(&id)).await
    }

    /// Brings a task back from the trash.
    #[tool(annotations(title = "Restore a task", read_only_hint = false, destructive_hint = false, idempotent_hint = true))]
    async fn restore_task(&self, Parameters(Id { id }): Parameters<Id>) -> CallToolResult {
        self.run(move |l| l.restore_task(&id)).await
    }

    /// Makes a task a subtask of another, moves it to a project, or makes it a task of its own.
    #[tool(annotations(title = "Move a task", read_only_hint = false, destructive_hint = false))]
    async fn move_task(&self, Parameters(Move { id, parent, project, top }): Parameters<Move>) -> CallToolResult {
        let to = match (parent, project, top) {
            (Some(id), _, _) => MoveTarget::Parent { id },
            (_, Some(name), _) => MoveTarget::Project { name },
            (_, _, true) => MoveTarget::Top,
            _ => return CallToolResult::error(vec![ContentBlock::text("say where: a parent, a project, or top")]),
        };
        self.run(move |l| l.move_task(&id, to)).await
    }

    /// Puts a task in a block, as a sitting: one session of work on it there.
    #[tool(annotations(title = "Put a task in a block", read_only_hint = false, destructive_hint = false))]
    async fn assign(&self, Parameters(Assign { task, block, date, minutes }): Parameters<Assign>) -> CallToolResult {
        self.run(move |l| l.assign(&task, &block, date, minutes)).await
    }

    /// Takes a sitting out of its block; the task itself stays.
    #[tool(annotations(title = "Take a task out of a block", read_only_hint = false, destructive_hint = true))]
    async fn unassign(&self, Parameters(Sitting { assignment }): Parameters<Sitting>) -> CallToolResult {
        self.run(move |l| l.unassign(&assignment)).await
    }

    /// Starts, or resumes, a sitting's timer.
    #[tool(annotations(title = "Start a timer", read_only_hint = false, destructive_hint = false))]
    async fn start_timer(&self, Parameters(Sitting { assignment }): Parameters<Sitting>) -> CallToolResult {
        self.run(move |l| l.start_timer(&assignment)).await
    }

    /// Pauses a sitting's timer, keeping what it has logged.
    #[tool(annotations(title = "Pause a timer", read_only_hint = false, destructive_hint = false))]
    async fn pause_timer(&self, Parameters(Sitting { assignment }): Parameters<Sitting>) -> CallToolResult {
        self.run(move |l| l.pause_timer(&assignment)).await
    }

    /// Stops a sitting's timer and logs its time.
    #[tool(annotations(title = "Stop a timer", read_only_hint = false, destructive_hint = false))]
    async fn stop_timer(&self, Parameters(Stop { assignment, minutes }): Parameters<Stop>) -> CallToolResult {
        self.run(move |l| l.stop_timer(&assignment, minutes)).await
    }

    /// Starts on one of What Should I Work On?'s choices at once, its timer running: in the
    /// block under way, or in a block of its own in free time.
    #[tool(annotations(title = "Start on a choice", read_only_hint = false, destructive_hint = false))]
    async fn start_on(&self, Parameters(Choice { choice }): Parameters<Choice>) -> CallToolResult {
        self.run(move |l| l.start_on(&choice, None)).await
    }

    /// Makes the replan changes the person accepted, as one change they can undo, and proposes
    /// again around what they turned down.
    #[tool(annotations(title = "Apply a replan", read_only_hint = false, destructive_hint = false))]
    async fn apply_replan(&self, Parameters(Apply { proposal, accepted }): Parameters<Apply>) -> CallToolResult {
        self.run(move |l| l.apply_replan(record(proposal, "replan")?, record(accepted, "list of changes")?, None)).await
    }

    /// Puts the sittings the person accepted into their blocks, as one change they can undo,
    /// and plans again around what they turned down.
    #[tool(annotations(title = "Apply a day plan", read_only_hint = false, destructive_hint = false))]
    async fn apply_day_plan(&self, Parameters(Apply { proposal, accepted }): Parameters<Apply>) -> CallToolResult {
        self.run(move |l| l.apply_day_plan(record(proposal, "day plan")?, record(accepted, "list of sittings")?, None)).await
    }

    /// Undoes the last change made on this device, whoever made it: an app, `lum`, or this.
    #[tool(annotations(title = "Undo", read_only_hint = false, destructive_hint = false))]
    async fn undo(&self) -> CallToolResult {
        self.run(Lumenna::undo).await
    }

    /// Redoes what was last undone.
    #[tool(annotations(title = "Redo", read_only_hint = false, destructive_hint = false))]
    async fn redo(&self) -> CallToolResult {
        self.run(Lumenna::redo).await
    }
}

// ---------------------------------------------------------------------------------------
// The server, and what it can be asked to read
// ---------------------------------------------------------------------------------------

const TODAY: &str = "lumenna://today";
const TASKS: &str = "lumenna://tasks";
const BLOCKS: &str = "lumenna://blocks";

/// `%20` and the like in a URI's part, as a filter written in one arrives.
fn unescaped(part: &str) -> String {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match (bytes[at], bytes.get(at + 1..at + 3).and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                at += 1;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a resource reads as: its record in the envelope, or nothing at a URI that names none.
fn render(lumenna: &Lumenna, uri: &str) -> Result<Option<String>, String> {
    let rest = uri.strip_prefix("lumenna://").unwrap_or("");
    let message = |e: lumenna_host::LumennaError| e.message().to_owned();
    Ok(Some(match rest.split_once('/') {
        None if rest == "today" => enveloped(lumenna.plan(None).map_err(message)?),
        None if rest == "tasks" => enveloped(lumenna.list_tasks("").map_err(message)?),
        None if rest == "blocks" => enveloped(lumenna.list_blocks().map_err(message)?),
        Some(("day", date)) => enveloped(lumenna.plan(Some(unescaped(date))).map_err(message)?),
        Some(("task", id)) => enveloped(lumenna.show_task(&unescaped(id)).map_err(message)?),
        Some(("tasks", query)) => enveloped(lumenna.list_tasks(&unescaped(query)).map_err(message)?),
        _ => return Ok(None),
    }))
}

/// Whether `uri` names a resource this server reads.
fn readable(uri: &str) -> bool {
    match uri.strip_prefix("lumenna://").map(|rest| rest.split_once('/')) {
        Some(None) => [TODAY, TASKS, BLOCKS].contains(&uri),
        Some(Some((kind, rest))) => ["day", "task", "tasks"].contains(&kind) && !rest.is_empty(),
        None => false,
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        let writes = if self.writes {
            "Its tools may change the store; each change is one the person can undo."
        } else {
            "Its tools only read: the person has not allowed changes (lum mcp --writes)."
        };
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().enable_resources_subscribe().build())
            .with_server_info(Implementation::new("lumenna", env!("CARGO_PKG_VERSION")))
            .with_instructions(format!(
            "Lumenna: a person's tasks (projects, labels, priorities, subtasks, due dates, repetition) and the \
             day's time blocks, which tasks are put in, one sitting at a time, until they are done. Every answer \
             is a record whose `announcement` is the sentence the person would hear; say it, or what it says. \
             Records are named by identifier: a whole UUID, or a prefix of one. A resource subscribed to is \
             told of whenever it changes, whoever changed it: an app, `lum`, another device. {writes}"
        ))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resource = |uri: &str, name: &str, description: &str| {
            Resource::new(uri, name).with_description(description).with_mime_type("application/json")
        };
        Ok(ListResourcesResult::with_all_items(vec![
            resource(TODAY, "Today", "Today's blocks, the tasks in them, the free time between, and where now falls."),
            resource(TASKS, "Tasks", "Every open task, most urgent first, subtasks under their task."),
            resource(BLOCKS, "Blocks", "Every time block, with when it happens and how often."),
        ]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let template = |uri: &str, name: &str, description: &str| {
            ResourceTemplate::new(uri, name).with_description(description).with_mime_type("application/json")
        };
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            template("lumenna://day/{date}", "A day", "A day's blocks and sittings: an ISO date, or \"tomorrow\"."),
            template("lumenna://task/{id}", "A task", "One task whole, by identifier."),
            template("lumenna://tasks/{query}", "Tasks a filter admits", "The tasks a filter in Lumenna's language admits."),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let uri = request.uri.clone();
        let lumenna = Arc::clone(&self.lumenna);
        let read = tokio::task::spawn_blocking(move || render(&lumenna, &uri))
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        match read {
            Ok(Some(text)) => Ok(ReadResourceResult::new(vec![ResourceContents::text(text, request.uri)]).into()),
            Ok(None) => Err(ErrorData::resource_not_found(format!("there is nothing at {}", request.uri), None)),
            Err(reason) => Err(ErrorData::invalid_params(reason, None)),
        }
    }

    fn accepted_subscription_filter(&self, requested: &SubscriptionFilter) -> Option<SubscriptionFilter> {
        let mut accepted = requested.supported_by(&self.get_info().capabilities);
        accepted.resource_subscriptions = accepted
            .resource_subscriptions
            .map(|uris| uris.into_iter().filter(|uri| readable(uri)).collect::<Vec<_>>())
            .filter(|uris| !uris.is_empty());
        Some(accepted)
    }

    async fn listen(&self, context: SubscriptionContext) -> Result<(), ErrorData> {
        let uris = context.accepted().resource_subscriptions.clone().unwrap_or_default();
        let mut changes = self.changes.clone();
        let mut read = self.reading_of(uris).await;
        loop {
            let moved = tokio::select! {
                () = context.cancelled() => return Ok(()),
                moved = self.changed(&mut changes, &mut read) => moved,
            };
            let Some(moved) = moved else { return Ok(()) };
            for uri in moved {
                if context.sink().notify_resource_updated(uri).await.is_err() {
                    return Ok(());
                }
            }
        }
    }

    #[allow(deprecated)]
    async fn subscribe(&self, request: SubscribeRequestParams, context: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        if !readable(&request.uri) {
            return Err(ErrorData::resource_not_found(format!("there is nothing at {}", request.uri), None));
        }
        self.subscribed.lock().unwrap().insert(request.uri);
        let start = !std::mem::replace(&mut *self.following.lock().unwrap(), true);
        if start {
            tokio::spawn(self.clone().follow(context.peer));
        }
        Ok(())
    }

    #[allow(deprecated)]
    async fn unsubscribe(&self, request: UnsubscribeRequestParams, _context: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        self.subscribed.lock().unwrap().remove(&request.uri);
        Ok(())
    }
}
