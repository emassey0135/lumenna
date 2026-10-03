//! `lum rpc`, exercised as a client would drive it.
//!
//! §8 makes this the path for every client that cannot link Rust, and §12 says it shares the
//! CLI's command surface rather than restating it. Both claims are only worth anything if
//! the protocol is driven end to end, so these tests spawn the real server and talk to it.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

/// A server, and the requests to feed it.
struct Rpc {
    profile: tempfile::TempDir,
}

impl Rpc {
    fn new() -> Self {
        Self { profile: tempfile::tempdir().unwrap() }
    }

    fn spawn(&self) -> Child {
        Command::new(env!("CARGO_BIN_EXE_lum"))
            .arg("rpc")
            .env("LUMENNA_PROFILE", self.profile.path())
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the server should start")
    }

    /// Sends newline-delimited requests, then reads everything the server said.
    fn talk(&self, requests: &[&str]) -> String {
        let mut server = self.spawn();
        {
            let stdin = server.stdin.as_mut().expect("stdin");
            for request in requests {
                writeln!(stdin, "{request}").unwrap();
            }
        }
        server.stdin.take();
        let mut out = String::new();
        server.stdout.as_mut().unwrap().read_to_string(&mut out).unwrap();
        server.wait().unwrap();
        out
    }

    /// Runs a plain `lum` command against the same profile, as a second process would.
    fn cli(&self, args: &[&str]) {
        let status = Command::new(env!("CARGO_BIN_EXE_lum"))
            .args(args)
            .env("LUMENNA_PROFILE", self.profile.path())
            .env("NO_COLOR", "1")
            .stdout(Stdio::null())
            .status()
            .expect("the binary should run");
        assert!(status.success(), "`lum {}` failed", args.join(" "));
    }
}

#[test]
fn initialize_names_the_contract_and_every_method() {
    // §15 calls the JSON shapes a compatibility contract. A client that cannot ask which
    // version it is talking to has to guess, and guessing is how a contract stops being one.
    let rpc = Rpc::new();
    let out = rpc.talk(&[r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#]);
    assert!(out.contains(r#""result":"server""#), "{out}");
    assert!(out.contains(r#""contract":1"#), "{out}");
    assert!(out.contains(r#""name":"lumenna""#), "{out}");
    assert!(out.contains(r#""task.add""#), "{out}");
    assert!(out.contains(r#""complete""#), "{out}");
}

#[test]
fn a_method_reaches_the_same_surface_the_command_line_does() {
    // §12: one typed command surface, two ways in. A method builds the same `Command` and
    // gets the same `Response`, so the two cannot drift.
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"task.add","params":{"text":"review PR tomorrow p1"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"task.list"}"#,
    ]);
    assert!(out.contains(r#""result":"change""#), "{out}");
    assert!(out.contains(r#""title":"review PR""#), "{out}");
    assert!(out.contains(r#""priority":1"#), "{out}");
    assert!(out.contains(r#""result":"rows""#), "{out}");
    assert!(out.contains(r#""count":1"#), "{out}");
}

#[test]
fn a_write_from_another_process_is_pushed_without_being_asked_for() {
    // This is the whole reason to speak a protocol rather than shell out per command (§8).
    let rpc = Rpc::new();
    let mut server = rpc.spawn();
    {
        let stdin = server.stdin.as_mut().expect("stdin");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"task.list"}}"#).unwrap();
        stdin.flush().unwrap();
        // Give the server its first answer before anything else touches the store.
        std::thread::sleep(std::time::Duration::from_millis(500));

        rpc.cli(&["task", "add", "from another process", "--quiet"]);
        // Long enough for the one-second poll §8 sanctions to come round.
        std::thread::sleep(std::time::Duration::from_millis(2500));
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"shutdown"}}"#).unwrap();
    }
    server.stdin.take();
    let mut out = String::new();
    server.stdout.as_mut().unwrap().read_to_string(&mut out).unwrap();
    server.wait().unwrap();

    assert!(out.contains(r#""method":"lumenna/changed""#), "no push arrived: {out}");
}

#[test]
fn refreshing_before_answering_keeps_a_reply_from_being_stale() {
    // The push says something happened; the next answer has to already know what.
    let rpc = Rpc::new();
    let mut server = rpc.spawn();
    {
        let stdin = server.stdin.as_mut().expect("stdin");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"task.list"}}"#).unwrap();
        stdin.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        rpc.cli(&["task", "add", "written elsewhere", "--quiet"]);
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"task.list"}}"#).unwrap();
    }
    server.stdin.take();
    let mut out = String::new();
    server.stdout.as_mut().unwrap().read_to_string(&mut out).unwrap();
    server.wait().unwrap();

    assert!(out.contains(r#""title":"written elsewhere""#), "{out}");
}

#[test]
fn row_numbers_do_not_address_anything_here() {
    // The last listing is one file per profile, so a resident server and a shell would
    // overwrite each other's numbering — and `1` would silently name the wrong task.
    let rpc = Rpc::new();
    rpc.cli(&["task", "add", "review PR", "--quiet"]);
    rpc.cli(&["task", "list"]);
    let out = rpc.talk(&[r#"{"jsonrpc":"2.0","id":1,"method":"task.done","params":{"id":"1"}}"#]);
    assert!(out.contains("row number"), "{out}");
    assert!(out.contains("pass an identifier"), "{out}");
}

#[test]
fn erasing_needs_the_caller_to_confirm_because_nothing_here_can_ask() {
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"task.erase","params":{"id":"whatever"}}"#,
    ]);
    assert!(out.contains("-32602"), "{out}");
    assert!(out.contains("confirm"), "{out}");
}

#[test]
fn an_unknown_method_names_the_nearest_one() {
    let rpc = Rpc::new();
    let out = rpc.talk(&[r#"{"jsonrpc":"2.0","id":1,"method":"task.lsit"}"#]);
    assert!(out.contains("-32601"), "{out}");
    assert!(out.contains("did you mean 'task.list'?"), "{out}");
}

#[test]
fn a_notification_does_the_work_and_says_nothing() {
    // JSON-RPC: no id, no reply. The write still has to happen.
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","method":"task.add","params":{"text":"silent"}}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"task.list"}"#,
    ]);
    assert_eq!(out.matches(r#""jsonrpc":"2.0""#).count(), 1, "one reply, not two: {out}");
    assert!(out.contains(r#""title":"silent""#), "{out}");
}

#[test]
fn broken_json_is_a_parse_error_rather_than_a_dead_server() {
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        "{not json",
        r#"{"jsonrpc":"2.0","id":1,"method":"task.list"}"#,
    ]);
    assert!(out.contains("-32700"), "{out}");
    assert!(out.contains(r#""result":"rows""#), "it kept serving: {out}");
}

#[test]
fn content_length_framing_is_answered_in_kind() {
    // MCP's stdio transport delimits by newline; `jsonrpc.el` uses headers (§12, §16.10).
    // Both are clients this has to serve, so the reply is framed the way the request was.
    let rpc = Rpc::new();
    let mut server = rpc.spawn();
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"task.list"}"#;
    {
        let stdin = server.stdin.as_mut().expect("stdin");
        write!(stdin, "Content-Length: {}\r\n\r\n{request}", request.len()).unwrap();
    }
    server.stdin.take();
    let mut out = String::new();
    server.stdout.as_mut().unwrap().read_to_string(&mut out).unwrap();
    server.wait().unwrap();

    assert!(out.starts_with("Content-Length: "), "{out}");
    assert!(out.contains(r#""result":"rows""#), "{out}");
}

#[test]
fn completion_is_reachable_here_and_nowhere_else() {
    // A process per keystroke is not an answer, which is why §16.11 wants a protocol.
    let rpc = Rpc::new();
    rpc.cli(&["label", "add", "deep"]);
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"complete","params":{"text":"review @de","syntax":"quick-add"}}"#,
    ]);
    assert!(out.contains(r#""result":"completions""#), "{out}");
    assert!(out.contains(r#""text":"@deep""#), "{out}");
    assert!(out.contains(r#""kind":"label""#), "{out}");
    assert!(out.contains(r#""label":"label deep""#), "the announcement is a component: {out}");
}

#[test]
fn a_preview_says_what_would_happen_without_making_it_happen() {
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"preview","params":{"text":"review PR tomorrow p1 @new"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"task.list"}"#,
    ]);
    assert!(out.contains(r#""result":"preview""#), "{out}");
    assert!(out.contains(r#""title":"review PR""#), "{out}");
    assert!(out.contains(r#""new_labels":["new"]"#), "{out}");
    assert!(out.contains(r#""has_errors":false"#), "{out}");
    assert!(out.contains(r#""count":0"#), "nothing was written: {out}");
}
