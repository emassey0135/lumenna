# The CLI

Read when working in `apps/cli/`: `lum`, `lum rpc`, `lum mcp` and the daemon. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

Run it with `LUMENNA_PROFILE=/some/dir ./target/debug/lum ...` to keep test data out of the
real profile.

- **Every noun is a subcommand group** (`lum task ...`, `lum block ...`). Verbs that act
  *across* nouns stay at the top: `plan`, `assign`, `unassign`, `start`, `pause`, `stop`.
- **Row numbers are counted within a kind.** The last listing stores `kind<TAB>id` per line,
  so `lum block list` then `lum assign 2 --block 1` resolves each half against the right
  thing. `lum plan` records blocks *and* assignments, so `lum start 1` works after it.
- **The last listing is recorded from the response**, in `run` (`listing_of`), and only `Rows`
  and `Plan` write it: a mutation must not renumber what the user is working against, or
  `lum task done 1` twice would mean two different tasks.
- **`load_all_years` is deliberate** (in the host's operations a row number resolves to). Lazy
  year loading keeps a watch viable, but anything looking a block or assignment up *by
  identifier* cannot know which year to open.
- **`api.rs` is an envelope, not the surface** (`lumenna_host::rpc::api`, used here as `api`):
  the contract version and an `Outcome` wrapping a surface record, rendered as prose or JSON,
  neither computing anything the other lacks. A command reachable only by reading prose off
  stdout is not on the surface. **`--json` is a versioned compatibility contract** for scripts:
  reshaping `api.rs` or a surface record is a breaking change.
- **The shell completes names from the store** (`complete.rs`, clap_complete's dynamic mode):
  `lum completions <shell>` prints a registration that asks `lum` itself (`COMPLETE=bash lum`)
  as one types, so a project, label or saved filter argument offers the store's names.
- **`lum daemon install` runs `lum sync-daemon` as a service** (`service.rs`), always with
  `--profile`: a systemd user unit plus `loginctl enable-linger` on Linux (on a BTSpeak, found
  by `/BTSpeak`, no linger, since it logs its person in at every start), a LaunchAgent on macOS,
  a logon task on Windows. A profile other than the default gets a service name of its own.
  `lum --json daemon status` carries `service` (`installed`, `running`) in the envelope, which
  Emacs's Sync in the background reads.
- **`lum mcp` is the surface for an assistant** (`mcp.rs`, the Model Context Protocol over
  stdio, on `rmcp`): each tool is an operation of the core's assistant table
  (`records::assistant`, its schema and annotations built from it), run through
  `Lumenna::assist` as Siri's and Gemini's are, and answers with its record in the envelope
  `--json` writes; a refusal is a tool error in the core's words. A tool is added there, not here. Read-only unless
  `--writes`, set where the person configures the client. Resources: today, every open task,
  the blocks, and templates for a day, a task and a filter (`lumenna://tasks/{query}`,
  escaped). Identifiers, never row numbers. Delete from Trash is not offered (trash and undo
  are); erasing permanently is (`erase_task_permanently`, `erase_everything_deleted`), and
  without `confirm: true` each refuses with the core's question for the model to ask. A
  resource subscribed to is told of when it reads otherwise, whoever wrote:
  `subscriptions/listen` for a 2026-07-28 client (`accepted_subscription_filter` keeps only
  what is a resource), `resources/subscribe` for the rest. One poller per process looks each
  second at the heads (this process's writes) and `outside_version` (another's, even into a
  year's document not loaded here, which the heads alone miss), and only then are the
  subscribed resources read again. Tests drive the real server over stdio
  (`tests/mcp.rs`), another `lum` writing beside it.
- **`lum mcp --listen [ADDR]` serves Streamable HTTP at `/mcp`** (127.0.0.1:7341 by default),
  each session a `Server::fresh`: on loopback with no token, rmcp's check answering only a
  loopback `Host` (no rebound name, no tunnel); off loopback, or with `--require-token` behind
  a proxy, only `Authorization: Bearer` with the device's token (`lum mcp token`,
  `<profile>/mcp-token`, `0600`, never synced), read again per request so `--new` turns the old
  one away at once. Any request with an `Origin` is refused. No TLS. Its tests use rmcp's own
  client (`tests/mcp_http.rs`), which needs a rustls provider installed first: reqwest is built
  without one in this tree.
