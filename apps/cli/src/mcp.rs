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

use std::collections::BTreeMap;
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
use lumenna_host::Lumenna;
use lumenna_records::assistant::{Argument, ArgumentValue, AssistantOperation};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation, JsonObject, ListToolsResult, Tool, ToolAnnotations, ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource, ResourceContents,
    ResourceTemplate, ResourceUpdatedNotificationParam, ServerCapabilities, ServerConfig, SubscribeRequestParams,
    SubscriptionFilter, UnsubscribeRequestParams,
};
use rmcp::service::{RequestContext, SubscriptionContext};
use rmcp::{ErrorData, Peer, RoleServer, ServerHandler, ServiceExt};
use tokio::sync::watch;
use serde_json::json;

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
    changes: watch::Receiver<u64>,
    /// What this client asked `resources/subscribe` to follow, in protocol versions before
    /// 2026-07-28 (a client on that one listens instead, `subscriptions/listen`), each with what
    /// it last read as.
    subscribed: Arc<Mutex<BTreeMap<String, Option<String>>>>,
    /// Whether this client's old-style subscriptions are being followed yet.
    following: Arc<Mutex<bool>>,
}

impl Server {
    fn new(lumenna: Arc<Lumenna>, writes: bool, changes: watch::Receiver<u64>) -> Self {
        Self { lumenna, writes, changes, subscribed: Arc::default(), following: Arc::default() }
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
    /// each resource that changed since it last read as it did. `changes` was taken before the
    /// first subscription was read, so no write after it goes untold.
    async fn follow(self, peer: Peer<RoleServer>, mut changes: watch::Receiver<u64>) {
        loop {
            if changes.changed().await.is_err() {
                return;
            }
            let uris: Vec<String> = self.subscribed.lock().unwrap().keys().cloned().collect();
            let now = self.reading_of(uris).await;
            let moved: Vec<String> = {
                let mut subscribed = self.subscribed.lock().unwrap();
                now.into_iter()
                    .filter_map(|(uri, text)| {
                        // Gone when unsubscribed meanwhile.
                        let kept = subscribed.get_mut(&uri)?;
                        (*kept != text).then(|| {
                            *kept = text;
                            uri
                        })
                    })
                    .collect()
            };
            for uri in moved {
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
// The tools: every assistant operation, as Siri, Shortcuts and Gemini have them too
// ---------------------------------------------------------------------------------------

/// The tools offered: every operation of `lumenna_records::assistant`, those that change the
/// store only with `--writes`, each named, described and marked as the core has it.
fn tools(writes: bool) -> Vec<Tool> {
    lumenna_records::assistant::operations()
        .into_iter()
        .filter(|op| writes || !op.writes)
        .map(|op| {
            let annotations = ToolAnnotations::with_title(op.sentence.clone())
                .read_only(!op.writes)
                .destructive(op.destructive)
                .idempotent(op.idempotent);
            Tool::new(op.id.clone(), op.description.clone(), Arc::new(input_schema(&op))).annotate(annotations)
        })
        .collect()
}

/// The JSON schema of what an operation takes.
fn input_schema(op: &AssistantOperation) -> JsonObject {
    use lumenna_records::assistant::ParameterKind as K;
    let mut properties = serde_json::Map::new();
    for parameter in &op.parameters {
        let mut schema = match parameter.kind {
            K::Minutes => json!({ "type": "integer", "minimum": 0 }),
            K::Priority => json!({ "type": "integer", "minimum": 1, "maximum": 4 }),
            K::Flag => json!({ "type": "boolean" }),
            K::Labels => json!({ "type": "array", "items": { "type": "string" } }),
            K::Replan | K::DayPlan => json!({ "type": "object" }),
            K::ReplanChanges | K::DayPlanSittings => json!({ "type": "array", "items": { "type": "object" } }),
            _ => json!({ "type": "string" }),
        };
        schema["description"] = json!(parameter.description);
        properties.insert(parameter.name.clone(), schema);
    }
    let required: Vec<&str> = op.parameters.iter().filter(|p| !p.optional).map(|p| p.name.as_str()).collect();
    let mut schema = serde_json::Map::new();
    schema.insert("type".to_owned(), json!("object"));
    schema.insert("properties".to_owned(), serde_json::Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_owned(), json!(required));
    }
    schema
}

/// What a tool call gave, as the operation's arguments, each read as its parameter's kind.
fn arguments_of(op: &AssistantOperation, mut given: JsonObject) -> Result<Vec<Argument>, String> {
    use lumenna_records::assistant::ParameterKind as K;
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
            (_, serde_json::Value::String(text)) => ArgumentValue::Text(text),
            (_, serde_json::Value::Bool(flag)) => ArgumentValue::Flag(flag),
            (_, serde_json::Value::Number(n)) => ArgumentValue::Number(n.as_i64().ok_or_else(|| wrong("a whole number"))?),
            (_, serde_json::Value::Array(items)) => ArgumentValue::Texts(
                items.into_iter().map(|item| item.as_str().map(str::to_owned).ok_or_else(|| wrong("names"))).collect::<Result<_, _>>()?,
            ),
            _ => return Err(wrong("something else")),
        };
        arguments.push(Argument { name: parameter.name.clone(), value });
    }
    Ok(arguments)
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

impl ServerHandler for Server {
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools(self.writes)))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // One that changes the store is no tool at all without `--writes`.
        let Some(op) = lumenna_records::assistant::operation(&request.name).filter(|op| self.writes || !op.writes) else {
            return Err(ErrorData::invalid_params(format!("there is no tool called {}", request.name), None));
        };
        let arguments = match arguments_of(&op, request.arguments.unwrap_or_default()) {
            Ok(arguments) => arguments,
            Err(why) => return Ok(CallToolResult::error(vec![ContentBlock::text(why)]).into()),
        };
        Ok(self.run(move |l| l.assist(&op.id, arguments)).await.into())
    }

    fn get_info(&self) -> ServerConfig {
        let writes = if self.writes {
            "Its tools may change the store; each change is one the person can undo, except erasing permanently, \
             which nothing undoes: ask the person first, in the words the erasing tools answer with."
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
        // The changes to follow are taken first, then the resource read as it stands, both before
        // the subscription is answered: a write after that is always told, and one before it
        // never is. Taken in the task instead, a write just after was its starting point.
        let start = !std::mem::replace(&mut *self.following.lock().unwrap(), true);
        let changes = self.changes.clone();
        let reading = self.reading_of([request.uri.clone()]).await;
        self.subscribed.lock().unwrap().extend(reading);
        if start {
            tokio::spawn(self.clone().follow(context.peer, changes));
        }
        Ok(())
    }

    #[allow(deprecated)]
    async fn unsubscribe(&self, request: UnsubscribeRequestParams, _context: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        self.subscribed.lock().unwrap().remove(&request.uri);
        Ok(())
    }
}
