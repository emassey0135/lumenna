//! `lum mcp`, driven as an assistant's client drives it: the real server over stdio, speaking
//! the Model Context Protocol's JSON-RPC, a request at a time.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// A server and the profile it serves.
struct Mcp {
    _profile: tempfile::TempDir,
    child: Child,
    /// Every message the server writes, read on a thread of its own so a test can wait for a
    /// notification with a deadline.
    out: Receiver<Value>,
    /// Notifications read while waiting for an answer, in the order they came.
    notifications: Vec<Value>,
    next: u64,
}

impl Mcp {
    fn start(writes: bool) -> Self {
        Self::start_on("2025-06-18", writes)
    }

    /// A server whose client speaks `version`; from 2026-07-28 there is no `initialize`, each
    /// request carrying who asks in its `_meta`.
    fn start_on(version: &str, writes: bool) -> Self {
        let profile = tempfile::tempdir().unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_lum"));
        command.arg("mcp");
        if writes {
            command.arg("--writes");
        }
        let mut child = command
            .env("LUMENNA_PROFILE", profile.path())
            .env("LUMENNA_BACKUP_DIR", profile.path().join("backups"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the server should start");
        let lines = BufReader::new(child.stdout.take().unwrap());
        let (sent, out) = channel();
        std::thread::spawn(move || {
            for line in lines.lines().map_while(Result::ok) {
                if sent.send(serde_json::from_str::<Value>(&line).unwrap()).is_err() {
                    return;
                }
            }
        });
        let mut mcp = Self { _profile: profile, child, out, notifications: Vec::new(), next: 1 };
        if version >= "2026-07-28" {
            return mcp;
        }
        let init = mcp.call(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
        );
        assert_eq!(init["serverInfo"]["name"], "lumenna", "{init}");
        mcp.notify("notifications/initialized");
        mcp
    }

    fn notify(&mut self, method: &str) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "method": method })).unwrap();
    }

    /// Sends a request and reads until its answer, past any notifications: its `result`, or
    /// its `error` as the result for a test that expects one.
    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap();
        loop {
            let message = self.out.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|_| panic!("no answer to {method}"));
            if message["id"] == id {
                return message.get("result").or_else(|| message.get("error")).cloned().unwrap();
            }
            if message.get("id").is_none() {
                self.notifications.push(message);
            }
        }
    }

    /// Waits for a notification `method`, taking it and those before it; none by the deadline.
    fn notified(&mut self, method: &str, within: Duration) -> Option<Value> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(at) = self.notifications.iter().position(|n| n["method"] == method) {
                return Some(self.notifications.drain(..=at).next_back().unwrap());
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            let message = self.out.recv_timeout(left).ok()?;
            if message.get("id").is_none() {
                self.notifications.push(message);
            }
        }
    }

    /// Runs `lum` on the same store, as another process writing to it would.
    fn lum(&self, args: &[&str]) {
        let status = Command::new(env!("CARGO_BIN_EXE_lum"))
            .args(args)
            .env("LUMENNA_PROFILE", self._profile.path())
            .env("LUMENNA_BACKUP_DIR", self._profile.path().join("backups"))
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "lum {args:?}");
    }

    /// Calls a tool; its record, parsed out of the text it answers with, and whether it was
    /// an error.
    fn tool(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let result = self.call("tools/call", json!({ "name": name, "arguments": arguments }));
        let text = result["content"][0]["text"].as_str().unwrap_or_default().to_owned();
        let error = result["isError"].as_bool().unwrap_or(false);
        (serde_json::from_str(&text).unwrap_or(Value::String(text)), error)
    }

    fn tools(&mut self) -> Vec<String> {
        self.call("tools/list", json!({}))["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// A resource's record.
    fn read(&mut self, uri: &str) -> Value {
        let result = self.call("resources/read", json!({ "uri": uri }));
        serde_json::from_str(result["contents"][0]["text"].as_str().unwrap_or("null")).unwrap_or(result)
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        self.child.stdin.take();
        let _ = self.child.wait();
    }
}

#[test]
fn without_writes_it_offers_only_what_reads() {
    let mut mcp = Mcp::start(false);
    let tools = mcp.tools();
    assert!(tools.contains(&"list_tasks".to_owned()) && tools.contains(&"day".to_owned()), "{tools:?}");
    assert!(!tools.iter().any(|t| ["add_task", "complete_task", "undo"].contains(&t.as_str())), "{tools:?}");
    let refused = mcp.call("tools/call", json!({ "name": "add_task", "arguments": { "text": "sneak in" } }));
    assert!(refused.get("code").is_some() || refused["isError"] == true, "a tool not offered is not run: {refused}");
    assert_eq!(mcp.read("lumenna://tasks")["count"], 0, "and nothing was added");
}

#[test]
fn with_writes_a_task_is_added_from_a_quick_add_line_and_its_reading_said() {
    let mut mcp = Mcp::start(true);
    let (added, error) = mcp.tool("add_task", json!({ "text": "call the bank tomorrow p1 @calls" }));
    assert!(!error, "{added}");
    let said = added["announcement"].as_str().unwrap();
    assert!(said.starts_with("call the bank, due tomorrow"), "{said}");
    assert!(said.contains("priority 1"), "{said}");
    let (listed, _) = mcp.tool("list_tasks", json!({ "query": "@calls" }));
    assert_eq!(listed["rows"][0]["title"], "call the bank", "{listed}");
}

#[test]
fn a_task_is_completed_by_its_identifier_and_undo_brings_it_back() {
    let mut mcp = Mcp::start(true);
    let (added, _) = mcp.tool("add_task", json!({ "text": "water the plants" }));
    let id = added["affected"]["tasks"][0].as_str().unwrap().to_owned();
    // A prefix that names one record is enough, as everywhere.
    let (done, error) = mcp.tool("complete_task", json!({ "id": &id[..13] }));
    assert!(!error, "{done}");
    assert_eq!(done["announcement"], "Completed water the plants");
    assert_eq!(mcp.read("lumenna://tasks")["count"], 0);
    let (undone, _) = mcp.tool("undo", json!({}));
    assert!(undone["announcement"].as_str().unwrap().starts_with("Undid"), "{undone}");
    assert_eq!(mcp.read(&format!("lumenna://task/{id}"))["title"], "water the plants");
}

#[test]
fn a_refusal_is_the_cores_sentence_as_a_tool_error() {
    let mut mcp = Mcp::start(true);
    let (refused, error) = mcp.tool("show_task", json!({ "id": "no-such-task" }));
    assert!(error, "{refused}");
    assert!(refused.as_str().is_some_and(|reason| !reason.is_empty()), "the core's reason, said: {refused}");
}

#[test]
fn the_day_and_a_filter_are_resources_to_read() {
    let mut mcp = Mcp::start(true);
    mcp.tool("add_task", json!({ "text": "ring Sam @calls" }));
    mcp.tool("add_task", json!({ "text": "block Deep work 9-11am" }));
    let listed = mcp.call("resources/list", json!({}));
    let uris: Vec<&str> = listed["resources"].as_array().unwrap().iter().map(|r| r["uri"].as_str().unwrap()).collect();
    assert_eq!(uris, ["lumenna://today", "lumenna://tasks", "lumenna://blocks"]);
    assert_eq!(mcp.read("lumenna://tasks/%40calls")["rows"][0]["title"], "ring Sam", "a filter, escaped in the URI");
    let blocks = mcp.read("lumenna://blocks");
    assert_eq!(blocks["rows"][0]["title"], "Deep work", "{blocks}");
    assert!(mcp.read("lumenna://today")["announcement"].is_string());
    let nothing = mcp.call("resources/read", json!({ "uri": "lumenna://elsewhere" }));
    assert!(nothing.get("code").is_some(), "{nothing}");
}

#[test]
fn a_replan_is_proposed_and_nothing_changes_until_it_is_applied() {
    let mut mcp = Mcp::start(true);
    let (proposal, error) = mcp.tool("propose_replan", json!({}));
    assert!(!error, "{proposal}");
    // Applied with nothing accepted: nothing changes, and the day is proposed again.
    let (applied, error) = mcp.tool("apply_replan", json!({ "proposal": proposal, "accepted": [] }));
    assert!(!error, "{applied}");
}

const UPDATED: &str = "notifications/resources/updated";

#[test]
fn a_resource_subscribed_to_is_told_of_when_another_process_changes_it_and_not_otherwise() {
    let mut mcp = Mcp::start(false);
    for uri in ["lumenna://tasks", "lumenna://blocks"] {
        let answer = mcp.call("resources/subscribe", json!({ "uri": uri }));
        assert!(answer.get("code").is_none(), "{answer}");
    }
    mcp.lum(&["task", "add", "call the bank"]);
    let told = mcp.notified(UPDATED, Duration::from_secs(10)).unwrap_or_else(|| panic!("the tasks were not told of: {:?}", mcp.notifications));
    assert_eq!(told["params"]["uri"], "lumenna://tasks", "{told}");
    // A block changes the blocks and leaves the tasks alone, so only the blocks are told of.
    mcp.lum(&["task", "add", "block Deep work 9-11am"]);
    let told = mcp.notified(UPDATED, Duration::from_secs(10)).unwrap_or_else(|| panic!("the blocks were not told of: {:?}", mcp.notifications));
    assert_eq!(told["params"]["uri"], "lumenna://blocks", "{told}");
    // Unsubscribed, nothing more is said of it.
    mcp.call("resources/unsubscribe", json!({ "uri": "lumenna://blocks" }));
    mcp.lum(&["task", "add", "block Admin 2-3pm"]);
    assert_eq!(mcp.notified(UPDATED, Duration::from_secs(3)), None);
}

#[test]
fn subscribing_to_what_is_not_a_resource_is_refused() {
    let mut mcp = Mcp::start(false);
    let refused = mcp.call("resources/subscribe", json!({ "uri": "lumenna://elsewhere" }));
    assert!(refused.get("code").is_some(), "{refused}");
}

#[test]
fn a_client_on_the_newest_protocol_listens_and_is_told_of_its_resources_changing() {
    let mut mcp = Mcp::start_on("2026-07-28", false);
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "0" },
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    let stdin = mcp.child.stdin.as_mut().unwrap();
    let listen = json!({
        "jsonrpc": "2.0", "id": 99, "method": "subscriptions/listen",
        "params": { "_meta": meta, "notifications": { "resourceSubscriptions": ["lumenna://tasks", "lumenna://elsewhere"] } },
    });
    writeln!(stdin, "{listen}").unwrap();
    let acknowledged = mcp.notified("notifications/subscriptions/acknowledged", Duration::from_secs(10)).expect("the listen was acknowledged");
    assert_eq!(
        acknowledged["params"]["notifications"]["resourceSubscriptions"],
        json!(["lumenna://tasks"]),
        "only what is a resource is accepted: {acknowledged}"
    );
    mcp.lum(&["task", "add", "call the bank"]);
    let told = mcp.notified(UPDATED, Duration::from_secs(10)).unwrap_or_else(|| panic!("the tasks were not told of: {:?}", mcp.notifications));
    assert_eq!(told["params"]["uri"], "lumenna://tasks", "{told}");
    assert_eq!(told["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"], 99, "{told}");
}

#[test]
fn a_change_made_through_the_servers_own_tools_is_told_of_too() {
    let mut mcp = Mcp::start(true);
    mcp.call("resources/subscribe", json!({ "uri": "lumenna://tasks/%40calls" }));
    mcp.tool("add_task", json!({ "text": "ring Sam @calls" }));
    let told = mcp.notified(UPDATED, Duration::from_secs(10)).expect("the filter's tasks were told of");
    assert_eq!(told["params"]["uri"], "lumenna://tasks/%40calls", "{told}");
}
