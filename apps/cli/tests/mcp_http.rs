//! `lum mcp --listen`, driven by the MCP SDK's own client over Streamable HTTP, and by requests
//! no client should get answered: without the token, from a browser page, or naming another host.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

/// A server listening on a port of its own, and the profile it serves.
struct Listening {
    profile: tempfile::TempDir,
    child: Child,
    /// `http://127.0.0.1:<port>/mcp`.
    url: String,
}

impl Listening {
    fn start(flags: &[&str]) -> Self {
        // reqwest is built with rustls and no provider (iroh's choice), so one is installed
        // before any client is made, though nothing here speaks TLS.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let profile = tempfile::tempdir().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_lum"))
            .args(["mcp", "--listen", "127.0.0.1:0"])
            .args(flags)
            .env("LUMENNA_PROFILE", profile.path())
            .env("LUMENNA_BACKUP_DIR", profile.path().join("backups"))
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server should start");
        let mut said = String::new();
        BufReader::new(child.stderr.take().unwrap()).read_line(&mut said).unwrap();
        let url = said.split_whitespace().find(|word| word.starts_with("http://")).expect(&said);
        let url = url.trim_end_matches([',', '.']).to_owned();
        Self { profile, child, url }
    }

    fn lum(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_lum"))
            .args(args)
            .env("LUMENNA_PROFILE", self.profile.path())
            .env("LUMENNA_BACKUP_DIR", self.profile.path().join("backups"))
            .output()
            .unwrap();
        assert!(out.status.success(), "lum {args:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }

    fn client(&self, token: Option<&str>) -> StreamableHttpClientTransport<reqwest::Client> {
        let mut config = StreamableHttpClientTransportConfig::with_uri(self.url.as_str());
        if let Some(token) = token {
            config = config.auth_header(token);
        }
        StreamableHttpClientTransport::from_config(config)
    }

    /// The status of an initialize posted by hand, with `headers` beside the usual.
    async fn status_of(&self, headers: &[(&str, &str)]) -> u16 {
        let mut request = reqwest::Client::new()
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.send().await.unwrap().status().as_u16()
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn on_loopback_a_client_reads_and_writes_with_no_token() {
    let server = Listening::start(&["--writes"]);
    let client = ().serve(server.client(None)).await.expect("the client connects");
    let added = client
        .call_tool(CallToolRequestParams::new("add_task").with_arguments(serde_json::json!({ "text": "ring Sam" }).as_object().unwrap().clone()))
        .await
        .unwrap();
    assert_ne!(added.is_error, Some(true), "{added:?}");
    let listed = client.call_tool(CallToolRequestParams::new("list_tasks")).await.unwrap();
    let text = format!("{listed:?}");
    assert!(text.contains("ring Sam"), "{text}");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn a_token_asked_for_is_refused_when_missing_or_wrong_and_taken_when_right() {
    let server = Listening::start(&["--require-token"]);
    assert_eq!(server.status_of(&[]).await, 401);
    assert_eq!(server.status_of(&[("authorization", "Bearer not-the-token")]).await, 401);
    assert!(().serve(server.client(None)).await.is_err(), "no token, no session");
    let token = server.lum(&["mcp", "token"]);
    assert_eq!(token.len(), 64, "{token}");
    let client = ().serve(server.client(Some(&token))).await.expect("the token lets it in");
    assert!(client.list_all_tools().await.unwrap().iter().any(|tool| tool.name == "day"));
    client.cancel().await.unwrap();
    // A new token turns the old one away at once.
    let new = server.lum(&["mcp", "token", "--new"]);
    assert_ne!(new, token);
    assert_eq!(server.status_of(&[("authorization", &format!("Bearer {token}"))]).await, 401);
    assert_eq!(server.status_of(&[("authorization", &format!("Bearer {new}"))]).await, 200);
}

#[tokio::test]
async fn a_browser_page_and_a_name_rebound_to_loopback_are_turned_away() {
    let server = Listening::start(&[]);
    assert_eq!(server.status_of(&[]).await, 200, "the same request, as a client sends it");
    assert_eq!(server.status_of(&[("origin", "https://example.com")]).await, 403, "from a page");
    assert_eq!(server.status_of(&[("host", "attacker.example")]).await, 403, "a name rebound to 127.0.0.1");
}
