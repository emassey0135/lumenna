//! `lum rpc`, exercised as a client would drive it.
//!
//! This is the path for every client that cannot link Rust, and it shares the CLI's command
//! surface rather than restating it. Both claims are only worth anything if
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
            .env("LUMENNA_BACKUP_DIR", self.profile.path().join("backups"))
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
            .env("LUMENNA_BACKUP_DIR", self.profile.path().join("backups"))
            .env("NO_COLOR", "1")
            .stdout(Stdio::null())
            .status()
            .expect("the binary should run");
        assert!(status.success(), "`lum {}` failed", args.join(" "));
    }
}

#[test]
fn initialize_names_the_contract_and_every_method() {
    // The JSON shapes are a compatibility contract. A client that cannot ask which
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
    // One typed command surface, two ways in. A method builds the same `Command` and
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
    // This is the whole reason to speak a protocol rather than shell out per command.
    let rpc = Rpc::new();
    let mut live = Live::start(&rpc);
    // The server's first answer comes before anything else touches the store, so what
    // follows is news to it rather than something it read on the way in.
    live.send(r#"{"jsonrpc":"2.0","id":1,"method":"task.list"}"#);
    live.wait_for(r#""id":1"#);
    rpc.cli(&["task", "add", "from another process", "--quiet"]);
    // Within the one-second poll.
    live.wait_for(r#""method":"lumenna/changed""#);
}

#[test]
fn a_write_from_another_process_is_pushed_even_when_a_request_took_it_in_first() {
    // A request refreshes before answering, so it can be the call that takes the write in.
    // The push must not depend on the poll being the first to look: the client's other views
    // only hear of it this way.
    let rpc = Rpc::new();
    let mut live = Live::start(&rpc);
    live.send(r#"{"jsonrpc":"2.0","id":1,"method":"task.list"}"#);
    live.wait_for(r#""id":1"#);
    rpc.cli(&["task", "add", "from another process", "--quiet"]);
    live.send(r#"{"jsonrpc":"2.0","id":2,"method":"task.list"}"#);
    let lines = live.until(r#""id":2"#);
    assert!(lines.last().unwrap().contains("from another process"), "{lines:?}");
    if !lines.iter().any(|line| line.contains("lumenna/changed")) {
        live.wait_for(r#""method":"lumenna/changed""#);
    }
}

#[test]
fn the_clients_own_writes_are_not_pushed_back_to_it() {
    let rpc = Rpc::new();
    let mut live = Live::start(&rpc);
    live.send(r#"{"jsonrpc":"2.0","id":1,"method":"task.add","params":{"text":"mine"}}"#);
    live.wait_for(r#""id":1"#);
    // Three polls' worth: anything pushed would be here by then.
    std::thread::sleep(std::time::Duration::from_secs(3));
    live.send(r#"{"jsonrpc":"2.0","id":2,"method":"task.list"}"#);
    let lines = live.until(r#""id":2"#);
    assert!(!lines.iter().any(|line| line.contains("lumenna/changed")), "{lines:?}");
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
    // MCP's stdio transport delimits by newline; `jsonrpc.el` uses headers.
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
    // A process per keystroke is not an answer, which is why the BTSpeak app speaks a protocol.
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

#[test]
fn the_block_choices_are_a_weeks_work_blocks_unless_asked_for_more() {
    let rpc = Rpc::new();
    rpc.cli(&["block", "add", "Focus", "--at", "9am", "--minutes", "90", "--date", "tomorrow"]);
    rpc.cli(&["block", "add", "Lunch", "--at", "noon", "--minutes", "30", "--kind", "break", "--date", "tomorrow"]);
    rpc.cli(&["block", "add", "Review", "--at", "4pm", "--minutes", "60", "--date", "in 10 days"]);
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"block.choices"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"block.choices","params":{"days":14}}"#,
    ]);
    let (week, fortnight) = out.split_once('\n').expect("two replies");
    assert!(week.contains(r#""result":"work_blocks""#), "{week}");
    assert!(week.contains(r#""announcement":"1 work block over 7 days""#), "{week}");
    assert!(week.contains(r#""title":"Focus""#) && week.contains(r#""start":"09:00""#), "{week}");
    assert!(!week.contains("Lunch"), "a break takes no tasks: {week}");
    assert!(!week.contains("Review"), "ten days off is past the week: {week}");
    assert!(fortnight.contains(r#""title":"Review""#), "{fortnight}");
}

#[test]
fn a_block_takes_its_settings_and_a_sitting_pauses_over_rpc() {
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"task.add","params":{"text":"read the paper"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"block.add","params":{"title":"Train","at":"00:00","minutes":1439,"kind":"break","accepts_tasks":true,"notes":"window seat","colour":"Blue"}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"plan"}"#,
    ]);
    assert!(out.contains(r#""accepts_tasks":true"#), "{out}");
    assert!(out.contains(r#""takes tasks""#), "the details say what differs from the kind: {out}");
    assert!(out.contains(r#""colour":"blue""#), "{out}");
    let block = out.split(r#""series":""#).nth(1).and_then(|rest| rest.split('"').next()).expect("a series").to_owned();
    let task = out.split(r#""task":{"#).nth(1).and_then(|rest| rest.split(r#""id":""#).nth(1)).and_then(|rest| rest.split('"').next()).expect("a task").to_owned();
    let assign = format!(r#"{{"jsonrpc":"2.0","id":4,"method":"assign","params":{{"task":"{task}","block":"{block}"}}}}"#);
    let out = rpc.talk(&[
        &assign,
        r#"{"jsonrpc":"2.0","id":5,"method":"plan"}"#,
    ]);
    let sitting = out.split(r#""assignments":[{"#).nth(1).and_then(|rest| rest.split(r#""id":""#).nth(1)).and_then(|rest| rest.split('"').next()).expect("a sitting").to_owned();
    let start = format!(r#"{{"jsonrpc":"2.0","id":6,"method":"start","params":{{"assignment":"{sitting}"}}}}"#);
    let pause = format!(r#"{{"jsonrpc":"2.0","id":7,"method":"pause","params":{{"assignment":"{sitting}"}}}}"#);
    let out = rpc.talk(&[&start, &pause, r#"{"jsonrpc":"2.0","id":8,"method":"plan"}"#]);
    assert!(out.contains("Paused timer"), "{out}");
    assert!(out.contains(r#""status":"paused""#) && out.contains(r#""running":false"#), "{out}");
}

#[test]
fn an_export_comes_back_in_the_reply_and_a_bad_format_is_named() {
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"task.add","params":{"text":"write the chapter"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"export","params":{"format":"markdown"}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"export","params":{"format":"pdf"}}"#,
    ]);
    assert!(out.contains(r#""result":"export""#), "{out}");
    assert!(out.contains(r"- [ ] write the chapter"), "{out}");
    assert!(out.contains("'pdf' is not a format"), "{out}");
}

/// A running server read line by line, for a conversation that has to answer what it hears.
struct Live {
    child: Child,
    lines: std::sync::mpsc::Receiver<String>,
}

impl Live {
    fn start(rpc: &Rpc) -> Self {
        let mut child = rpc.spawn();
        let out = child.stdout.take().unwrap();
        let (send, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, lines }
    }

    fn send(&mut self, request: &str) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{request}").unwrap();
        stdin.flush().unwrap();
    }

    /// Every line up to and including the next one containing `needle`.
    fn until(&self, needle: &str) -> Vec<String> {
        let mut seen = Vec::new();
        loop {
            let line = self
                .lines
                .recv_timeout(std::time::Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("nothing containing {needle} arrived: {seen:?}"));
            let found = line.contains(needle);
            seen.push(line);
            if found {
                return seen;
            }
        }
    }

    /// The next line containing `needle`, skipping the rest.
    fn wait_for(&self, needle: &str) -> String {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) if line.contains(needle) => return line,
                Ok(_) => {}
                Err(_) => panic!("nothing containing {needle} arrived"),
            }
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.child.stdin.take();
        let _ = self.child.wait();
    }
}

/// One string field out of a line of JSON.
fn field(line: &str, key: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(line).unwrap();
    value["params"][key].as_str().unwrap_or_default().to_owned()
}

#[test]
fn a_pairing_can_be_cancelled_and_only_one_runs_at_a_time() {
    let rpc = Rpc::new();
    let mut live = Live::start(&rpc);
    live.send(r#"{"jsonrpc":"2.0","id":1,"method":"pair.confirm","params":{"match":true}}"#);
    assert!(live.wait_for(r#""id":1"#).contains("no pairing is under way"));

    live.send(r#"{"jsonrpc":"2.0","id":2,"method":"pair","params":{"local_only":true}}"#);
    let waiting = live.wait_for("lumenna/pairing");
    assert!(!field(&waiting, "code").is_empty(), "the code to give the other device: {waiting}");

    // The server goes on answering while the pairing waits.
    live.send(r#"{"jsonrpc":"2.0","id":3,"method":"task.list"}"#);
    assert!(live.wait_for(r#""id":3"#).contains(r#""result":"rows""#));
    live.send(r#"{"jsonrpc":"2.0","id":4,"method":"pair","params":{"local_only":true}}"#);
    assert!(live.wait_for(r#""id":4"#).contains("already under way"));

    live.send(r#"{"jsonrpc":"2.0","id":5,"method":"pair.cancel"}"#);
    let ended = live.wait_for(r#""id":2"#);
    assert!(ended.contains("cancelled"), "{ended}");
}

#[test]
fn two_servers_pair_by_code_comparing_words_through_notifications() {
    let (laptop, phone) = (Rpc::new(), Rpc::new());
    let mut waiting = Live::start(&laptop);
    waiting.send(r#"{"jsonrpc":"2.0","id":1,"method":"task.add","params":{"text":"made on the laptop"}}"#);
    waiting.wait_for(r#""id":1"#);
    waiting.send(r#"{"jsonrpc":"2.0","id":2,"method":"pair","params":{"local_only":true,"name":"laptop"}}"#);
    let code = field(&waiting.wait_for("lumenna/pairing"), "code");

    let mut joining = Live::start(&phone);
    joining.send(&format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"pair","params":{{"local_only":true,"name":"phone","code":"{code}"}}}}"#
    ));
    let words_here: serde_json::Value =
        serde_json::from_str(&joining.wait_for(r#""words""#)).unwrap();
    let words_there: serde_json::Value =
        serde_json::from_str(&waiting.wait_for(r#""words""#)).unwrap();
    assert_eq!(words_here["params"]["words"], words_there["params"]["words"]);

    joining.send(r#"{"jsonrpc":"2.0","id":2,"method":"pair.confirm","params":{"match":true}}"#);
    waiting.send(r#"{"jsonrpc":"2.0","id":3,"method":"pair.confirm","params":{"match":true}}"#);
    let joined = joining.wait_for(r#""id":1"#);
    assert!(joined.contains(r#""result":"paired""#) && joined.contains("laptop"), "{joined}");
    assert!(waiting.wait_for(r#""id":2"#).contains(r#""result":"paired""#));

    joining.send(r#"{"jsonrpc":"2.0","id":3,"method":"task.list"}"#);
    assert!(joining.wait_for(r#""id":3"#).contains("made on the laptop"), "the first sync came across");
}

#[test]
fn hints_that_name_a_lum_command_stay_at_the_terminal() {
    // A client says these its own way, and a screen reader would read the backticks out.
    let rpc = Rpc::new();
    let out = rpc.talk(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"task.add","params":{"text":"tidy"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"sync.status"}"#,
    ]);
    let id = out.split(r#""id":""#).nth(1).and_then(|rest| rest.split('"').next()).unwrap().to_owned();
    let out = rpc.talk(&[&format!(r#"{{"jsonrpc":"2.0","id":3,"method":"task.rm","params":{{"id":"{id}"}}}}"#)]);
    assert!(!out.contains("`lum"), "{out}");
    let status = rpc.talk(&[r#"{"jsonrpc":"2.0","id":4,"method":"sync.status"}"#]);
    assert!(!status.contains("`lum"), "{status}");
}

#[test]
fn a_rows_action_is_sent_back_as_it_came_and_does_what_it_names() {
    // A client offers what the core listed and returns it with the answer; it decides
    // nothing about which actions a row has.
    let rpc = Rpc::new();
    rpc.cli(&["task", "add", "water plants"]);
    let out = rpc.talk(&[r#"{"jsonrpc":"2.0","id":1,"method":"task.list","params":{}}"#]);
    let reply: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
    let action = reply["result"]["rows"][0]["actions"][0].clone();
    assert_eq!(action["title"], "Mark Done", "{out}");
    let request = serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "act", "params": {"action": action, "answer": {"answer": "yes"}}});
    let out = rpc.talk(&[&request.to_string()]);
    assert!(out.contains(r#""changed":true"#), "{out}");
    let out = rpc.talk(&[r#"{"jsonrpc":"2.0","id":3,"method":"task.list","params":{}}"#]);
    assert!(out.contains(r#""count":0"#), "{out}");
}

#[test]
fn what_saving_a_form_sends_is_the_cores_to_work_out_for_a_client_over_the_pipe() {
    let rpc = Rpc::new();
    rpc.cli(&["task", "add", "water plants"]);
    let out = rpc.talk(&[r#"{"jsonrpc":"2.0","id":1,"method":"task.list","params":{}}"#]);
    let reply: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
    let id = reply["result"]["rows"][0]["id"].as_str().unwrap().to_owned();
    let out = rpc.talk(&[&format!(r#"{{"jsonrpc":"2.0","id":2,"method":"task.show","params":{{"id":"{id}"}}}}"#)]);
    let reply: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
    let task = reply["result"].clone();
    let request = serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "form.task_fields", "params": {"task": task}});
    let out = rpc.talk(&[&request.to_string()]);
    let reply: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
    let mut fields = reply["result"]["value"].clone();
    assert_eq!(fields["title"], "water plants", "{out}");
    fields["priority"] = serde_json::json!(1);
    let request = serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "form.task_edit", "params": {"task": task, "fields": fields}});
    let out = rpc.talk(&[&request.to_string()]);
    let reply: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
    assert_eq!(reply["result"]["value"]["priority"], 1, "{out}");
    assert!(reply["result"]["value"]["title"].is_null(), "only what changed: {out}");
}
