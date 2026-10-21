//! `lum mcp`, driven as an assistant's client drives it: the real server over stdio, speaking
//! the Model Context Protocol's JSON-RPC, a request at a time.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// A server and the profile it serves.
struct Mcp {
    _profile: tempfile::TempDir,
    child: Child,
    out: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    fn start(writes: bool) -> Self {
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
        let out = BufReader::new(child.stdout.take().unwrap());
        let mut mcp = Self { _profile: profile, child, out, next: 1 };
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
            let mut line = String::new();
            assert!(self.out.read_line(&mut line).unwrap() > 0, "the server stopped before answering {method}");
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                return message.get("result").or_else(|| message.get("error")).cloned().unwrap();
            }
        }
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
