# The CLI

Read when working in `apps/cli/`: `lum`, `lum rpc`, `lum mcp` and the daemon. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

Run it with `LUMENNA_PROFILE=/some/dir ./target/debug/lum ...` to keep test data out of the
real profile.

- **Every noun is a subcommand group** (`lum task ...`, `lum block ...`). Verbs that act
  *across* nouns stay at the top: `plan`, `assign`, `unassign`, `start`, `pause`, `stop`.
- **Row numbers are counted within a kind.** The last listing stores `kind<TAB>id` per line,
  so `lum block list` then `lum assign 2 --block 1` resolves each half against the right
  thing. `lum plan` records blocks *and* assignments, so `lum start 1` works after it.
- **The last listing is recorded from the response**, in `run`, and only `Rows` and `Plan`
  write it: a mutation must not renumber what the user is working against, or
  `lum task done 1` twice would mean two different tasks.
- **`load_all_years` is deliberate.** Lazy year loading keeps a watch viable, but anything
  looking a block or assignment up *by identifier* cannot know which year to open.
- **`api.rs` is an envelope, not the surface**: the contract version and an `Outcome`
  wrapping a surface record, rendered as prose or JSON, neither computing anything the other
  lacks. A command reachable only by reading prose off stdout is not on the surface.
- **The shell completes names from the store** (`complete.rs`, clap_complete's dynamic mode):
  `lum completions <shell>` prints a registration that asks `lum` itself (`COMPLETE=bash lum`)
  as one types, so a project, label or saved filter argument offers the store's names.
- **`lum mcp` is the surface for an assistant** (`mcp.rs`, the Model Context Protocol over stdio, on
  the official SDK, `rmcp`): each tool calls one `Lumenna` method and answers with its record in the
  envelope `--json` writes, its `announcement` the core's sentence; a refusal is a tool error in the
  core's words. Read-only unless `--writes`, which adds the tools that change the store (their router
  joined to the reading one), set where the person configures the client. Resources: today, every
  open task, the blocks, and templates for a day, a task and a filter (`lumenna://tasks/{query}`,
  escaped). Identifiers, never row numbers; erasing is not offered (trash and undo are). A resource
  subscribed to is told of when it reads otherwise, whoever wrote: `subscriptions/listen` for a
  2026-07-28 client (`accepted_subscription_filter` keeps only what is a resource), and the older
  `resources/subscribe` for the rest. One poller per process looks each second at the heads (this
  process's writes) and `outside_version` (another's, even into a year's document not loaded here,
  which the heads alone missed), and only then are the subscribed resources read again and compared.
  Tests drive the real server over stdio (`apps/cli/tests/mcp.rs`), another `lum` writing beside it.
  `--listen [ADDR]` serves Streamable HTTP at `/mcp` instead (127.0.0.1:7341 by default), each
  session a `Server::fresh`: on loopback with no token, rmcp's check answering only a loopback
  `Host` (no rebound name, no tunnel); off loopback, or with `--require-token` behind a proxy, only
  `Authorization: Bearer` with the device's token (`lum mcp token`, `<profile>/mcp-token`, `0600`,
  never synced), read again per request so `--new` turns the old one away at once. Any request with
  an `Origin` is refused. No TLS. Its tests use rmcp's own client (`tests/mcp_http.rs`), which needs
  a rustls provider installed first: reqwest is built without one in this tree.
- **`--json` is a versioned compatibility contract** for scripts. Reshaping `api.rs` or a
  surface record is a breaking change.

