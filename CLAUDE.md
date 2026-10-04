# Lumenna

A cross-platform task manager and day planner: Todoist's model (projects, labels,
priorities, subtasks, recurrence, quick add) joined to Structured's timeline of time blocks.
The defining feature is that **tasks are assigned to blocks repeatedly across multiple
sittings** until they are done.

`PLAN.md` is the design document and the reference. Section numbers in code comments
(`§3.13`, `§10.1`) point into it. It is the source of truth for decisions; this file is the
short version for orientation.

## Standing decisions

- **Rust core, native UI per platform.** Native controls are what give real screen-reader
  accessibility; a cross-platform toolkit does not. Eleven targets (§16).
- **Automerge is the source of truth**, stored as change chunks in SQLite. SQLite's read
  model is a rebuildable index, never truth, and is optional for now (§8).
- **Peer-to-peer sync over Iroh.** No server sees the data. Pairing is one mechanism — word
  comparison on both devices — for everything a person does (§7).
- **Accessibility is architectural, not a pass at the end** (§13).
- **Effort determines order, not scope.** No platform is a cut-down version of another
  because it shipped later (§16.1).

## Workspace

```
crates/core/    domain model, queries, mutations, row projection. No I/O, no Automerge.
crates/parse/   quick-add and filter parsers, and the completion they share.
crates/store/   Automerge documents + the SQLite file they live in.
crates/sync/    Iroh endpoints, the document sync session, and pairing (§7).
apps/cli/       `lum` — the first target, and a permanent one.
```

The rest of §2's layout — `ffi`, the GUI apps — does not exist yet. §18's remaining
order: sync, first GUI, the rest.

### The CLI

Run it with `LUMENNA_PROFILE=/some/dir ./target/debug/lum ...` to keep test data out of the
real profile.

- **Every noun is a subcommand group.** `lum task ...`, `lum block ...`, `lum project ...`,
  `lum label ...`, `lum filter ...`, `lum config ...`. One shape covers every noun, and
  `--help` is a two-level tree rather than a flat list of twenty. The verbs that act *across*
  nouns stay at the top: `plan`, `assign`, `unassign`, `start`, `stop`. §15 originally spelt
  the task verbs bare (`lum add`, `lum done`) and has been updated to match.
- **Row numbers are counted within a kind.** The last listing stores `kind<TAB>id` per line,
  so `lum block list` then `lum assign 2 --block 1` resolves each half against the right
  thing. A number from the wrong kind says so and names the command that would fix it.
- **`lum plan` remembers blocks *and* assignments**, which is what lets `lum start 1` work
  straight afterwards.
- **`load_all_years` is deliberate.** Lazy year loading keeps the watch viable, but anything
  looking a block or assignment up *by identifier* cannot know which year to open. Those
  commands open everything rather than silently failing to find a record that is on disk.
- **`api.rs` is the typed command surface (§12), not a JSON formatter.** Every command
  returns a `Response`; `render` writes it as prose or as JSON, and neither computes anything
  the other does not have. `lum rpc` and the daemon socket will serialise the same structs
  over JSON-RPC, and `lum mcp` maps onto them — §12's *"this is not new work"* only holds if
  the surface exists once. A command reachable only by reading prose off stdout is not on it.
- **`--json` is versioned** and shaped for scripts, not derived from the model — §15 calls it
  a compatibility contract. Reshaping anything in `api.rs` is a breaking change.
- **Announcements are composed in core, components everywhere else.** `Response.announcement`
  is the one composed sentence, and only because core wrote it — an `Edit`'s description or a
  quick-add readback. Rows stay components (`role`, `state`, `title`, `value`) because speech
  and braille assemble them differently (§13).
- **The last listing is recorded from the response**, in `run`, not by each command that
  produces one. Only `Rows` and `Plan` write it: a mutation must not renumber what the user
  is working against, or `lum task done 1` twice would mean two different tasks.

### Sync (§7, §8)

`crates/sync`: `session` reconciles every document with Automerge's sync protocol over any
byte stream; `pairing` is the word comparison; `node` is the device's Iroh endpoint; `invite`
is the short-lived endpoint a pairing runs on.

- **Membership in `devices` is the trust boundary.** `Node` refuses, in both directions, any
  key not listed there. The only way in is a pairing whose words were confirmed on both sides
  (`edit::enroll_devices`, through the undo history).
- **Pairing never uses the device key.** It runs on a key minted per pairing, so it needs no
  daemon and no IPC; the device keys are exchanged only after the words are confirmed.
- **The words** come from the connection's TLS exporter secret plus a commit-then-reveal
  nonce exchange, three PGP words. Without the commitment, 24 bits could be ground offline.
- **One endpoint per device**, decided by an advisory lock on `<profile>/sync.lock`. The
  daemon holds it; `lum sync` takes it for a round or asks the daemon over the socket.
  The device key lives in the store's `local_state` table and never syncs.
- **The sync session is lockstep per document**: both sides send one frame (a message, or an
  empty frame for nothing) then read one; both empty ends the document. Both sides see the
  same two frames, so both stop together. Sync state is per session, not saved.
- **A session joining by code does not advertise on mDNS.** Otherwise the waiting session
  dials it back and two connections each wait for the other to speak.
- The daemon serves the RPC surface on `<profile>/lumenna.sock` (Unix); a `sync` request
  there is passed to the daemon's own endpoint through a hook, not dispatched.
- Device records are inline (`Record::INLINE`): both sides of a pairing write them.
- Tests use `Network::LocalOnly` with explicit addresses: offline, no outside server.

### `lum rpc`

The same surface over JSON-RPC on stdio, for clients that cannot link Rust — Emacs (§16.10)
and the BTSpeak app (§16.11). One server per client, no Iroh; syncing is the daemon's job,
and §8's *one protocol, two transports* means the daemon will serve exactly this over a
socket.

- **A method builds the same `Command` and calls the same `dispatch`.** There is one
  implementation of every operation and two ways in, so RPC cannot drift from the CLI.
- **Changes another process wrote are pushed** as a `lumenna/changed` notification, from a
  thread polling `Store::refresh` once a second — §8 sanctions the timer, and `refresh`
  settles in one pragma read whether there is anything to do. Every request also refreshes
  first, so a reply is never staler than the notification that preceded it.
- **Row numbers are turned off** (`Profile::detach_rows`). The last listing is one file per
  profile, so a resident server and a shell would overwrite each other's numbering and `1`
  would silently name the wrong task. Clients hold identifiers.
- **`task.erase` requires `"confirm": true`.** Nothing here can prompt, so the decision the
  prompt makes moves to the caller.
- **Two framings, chosen per message by what arrived**: newline-delimited JSON (MCP's stdio
  transport, §12) and `Content-Length` headers (`jsonrpc.el`, §16.10). A reply is framed the
  way its request was.
- **Spans are UTF-8 byte offsets**, as Rust strings are. A client in a language that indexes
  by code point converts both ways (`menus.char_offset` in the BTSpeak app).
- **`complete` and `preview` are RPC-only** — completion is a keystroke-rate question and a
  process per keystroke is not an answer. They are why the BTSpeak app speaks a protocol
  rather than shelling out.

**Not built yet:** the encrypted store and its account key (§8), `lum daemon install`,
wake-up push, Windows named pipes, reminders, hooks, auto-scheduling.

### The BTSpeak app

`apps/btspeak/` — Python, on the device's own `dialogs` library, talking to `lum rpc`. The
Raspberry Pi this repository is developed on **is** a BTSpeak, so `/BTSpeak/Python/BTSpeak/`
is readable here and the app can be run under a pty without a second machine.

- **It is a client and almost nothing else.** No date parsing, no computed states, no rule
  about what completing a task does to its subtasks — all of that is core's, reached over the
  typed surface. `rows.py` is the only file that decides anything, and only about folding.
- **Speech and braille cannot diverge with the stock library.** `DynamicMenuDialog.draw()`
  builds `content_text` and `content_braille` from one string, so §13's components are
  re-flattened at the last step. §16.11's compact level marker is what is in use; the
  ~20-line subclass is the fallback.
- **Depth is never indentation** (§16.11 — it does not work in speech). The level is
  announced only where it *changes*, computed against the previous *visible* row so folding
  keeps it right.
- **Our own writes do not come back as `lumenna/changed`** — `data_version` does not move for
  the connection that wrote — so `Session` tracks its own edits and ORs them with the push.
- **Completion is not inline.** `InputDialog` has no hook and Tab is form navigation, so
  candidates are offered after the line is entered. Inline needs the same kind of subclass.
- **Self-voice is set twice on purpose.** `# blazie-flags: self-voice` on line 2 of
  `__main__.py` is read by the menu launcher — which follows an interpreter to the script it
  runs — and `host.set_self_voice(True)` in `main()` covers being run from a shell or BT
  Code, where nothing read the header. Restoring it in a `finally` is the part that matters:
  the flag is device-wide in `/run/BTSpeak/`. With it on, printing to the terminal is
  silence, so startup errors are dialogs and the spawned server's stderr goes to `rpc.log`.
- **No `.menu` file.** BT Code adds the user-menu entry; §16.11 has been updated to match.

### The core/store boundary

`core` holds **materialized** records and knows nothing about Automerge. `store` hydrates
documents into `Snapshot` and translates edits back into Automerge operations.

This is what lets §8's optional SQLite read model arrive later without changing an
interface: both paths produce the same structs, and every query runs over those. It also
keeps the interesting logic testable without a CRDT in the room, which matters because the
core is the one component all eleven targets share.

Two consequences: `notes` is a `String` here even though it is Automerge `Text` in the
document, and nothing validates on construction that merge could violate — §3.1 requires
tolerating dangling references and cycles rather than refusing to load.

### Store rules that are easy to break

**Writes take the version the user edited.** `put(record, before)` writes only the fields
that differ. Passing a freshly read `before`, or `None` on an update, writes every field —
and each of those writes wins a last-write-wins race against a concurrent edit from another
device, silently reverting it.

**Containers must not be created concurrently.** Automerge object identity comes from the
operation that created the object, so two devices creating a map at the same key create two
maps, and on merge one loses everything written into it. Root collections are handled by a
deterministic genesis change (`Doc::new`); per-record sets are handled by creating them when
the record is created. Any *new* container needs the same care.

**A record keyed by something two devices can produce on their own is inline.** Exceptions
(series and date) and reminder acks (reminder and date) store each field as its own key,
`<record>/<field>`, in the collection, so nothing is ever created for merge to choose
between. Set `Record::INLINE` for any new record keyed that way.

**Things that must exist once per store come from deterministic changes.** The genesis
creates the root collections; `core`'s second change (`Doc::ensure_schema`) creates the Inbox
under the fixed `ProjectId::INBOX` and the `series_years` map. Both are frozen bytes: never
edit what they write; add another change on top instead. Clients never create an Inbox;
`edit::adopt_inbox` folds a pre-existing one into the shared one.

**A series lives in its start year's document, and recurs into later ones.**
`series_years` in `core` records which years hold recurring series, and `Store::load_year`
loads those years too. Without it a routine vanishes on 1 January.

**Change rowids are `AUTOINCREMENT` and compaction holds the write lock.** Rowids that get
reused after compaction make running processes skip changes. `Db::compact` folds everything
on disk (snapshot included) into the document under `BEGIN IMMEDIATE` before deleting
anything, and `refresh` reads in one transaction and loads a snapshot whose heads it lacks.

**The change cursor only advances in `refresh`.** `PRAGMA data_version` does not move for
your own connection's writes, and rows from another process can interleave with yours. A
cursor that lags re-reads a change already held, which is a no-op; a cursor that skips loses
an edit.

### Backup and export (§9)

Two different things, kept apart in code, commands and wording, because confusing them is
how someone sends a "task list" holding everything they ever deleted.

- **A backup** (`store::backup`, `lum backup`/`restore`) is every document's Automerge
  `save()`, framed in one file: the whole history, trash included. Restoring **merges** —
  it adds what the store lacks and can never take back a later change.
- **An export** (`store::export`, `lum export`) is current state only: no history, nothing
  from the trash. JSON is complete and versioned (`export::VERSION`), keeps identifiers,
  and is what `lum import` reads back; Markdown, org and ics are for reading only.
- `crates/store/tests/export.rs` round-trips a store with every field set. **A field added
  to the model must be added to `export/json.rs`**, or that test fails — which is the point.
- Backups are taken automatically when the newest is older than `backup-every` (a day by
  default): by the CLI before a command runs, and by `lum rpc` at start and hourly. They go
  to `<profile>-backups` beside the profile, never inside it, are `0600`, and the oldest
  beyond `backup-keep` are pruned. `backup-dir`, `backup-keep` and `backup-every` are
  **device settings**, in `<profile>/device-settings`, and never sync (§3.12).
- `LUMENNA_BACKUP_DIR` overrides the default directory; every test harness sets it, or
  test runs would leave backup folders beside their temporary profiles.

### Mutations and undo

`core::edit` computes an `Edit` — a list of before/after record pairs — and `store` applies
it. Core writes nothing, so every rule is testable against plain structs.

- **Undo applies an edit's other side**, because §9 says Automerge gives history, not
  undo, and rewinding would discard concurrent remote changes. The history is **saved per
  device**, in an `undo` table in the profile's SQLite file (`store::undo`), so `lum undo`
  works across commands and undoes what the BTSpeak app did too. It is local-only (§3.12):
  sync and backups carry Automerge documents, never that table. The change an undo produces
  syncs like any other.
- **Undo is rebased, never a plain swap.** An entry may be undone after other edits, here or
  merged from elsewhere, so only fields still holding what the edit set go back; the rest
  are kept and reported. That is also why the stack never jams on a conflict.
- **Everything a person does goes through `Store::apply_recorded`**; what the app does for
  them (folding a legacy Inbox, import, restore) uses `Store::apply` and is not recorded. A
  new command that writes must use an `Edit`, or it cannot be undone.
- Saved entries are the model serialised with serde (core's `serde` feature, which `store`
  enables). An entry a later version cannot read is dropped with a notice, not guessed at.
- **Operations own the multi-record rules.** Completing a task writes a completion, cascades
  to subtasks per §3.10's setting, and advances a recurring due date per §5. Leaving that to
  eleven UI targets is the business-logic leak principle 2 forbids.
- A cascaded completion of a **recurring** subtask must name that subtask's own occurrence,
  or it reads as unfinished the moment it is written. Cascades never *advance* a subtask —
  that would resurrect what was just closed out.
- **A subtask of a recurring task recurs with it.** `Snapshot::occurrence_of` gives the
  occurrence a completion is scoped to: the task's own due date if it recurs, else its nearest
  recurring ancestor's. Completions name it, and `is_completed` checks it.
- **Ask many tasks through `Facts`.** `Snapshot::has_state` and friends scan every completion
  and assignment per call. Anything looping over tasks uses `Context::new` (filters, rows) or
  `Snapshot::facts()`, which index them once.
- Every operation returns `Edit::nothing()` rather than a no-op change when nothing differs,
  so the undo stack does not fill with entries that appear to do nothing when reversed.

### Recurrence

Two systems sharing a syntax and nothing else (§5). Recurring **tasks** are one task whose
due date advances — no generated series. Recurring **blocks** are a series plus sparse
exceptions, expanded on read.

- **Expansion runs in UTC**, always. Recurrence is a civil-calendar question and UTC is the
  zone with no transitions to perturb it. §4's DST question — what a block scheduled in a
  skipped hour does — belongs where an occurrence is resolved against a real zone, not here.
- **An anchor that does not match the rule is not an occurrence.** RFC 5545 starts at the
  first date matching the rule *on or after* DTSTART. "Every Monday, starting Tuesday" never
  occurs on its start date. The block editor should normalise this; `recur` will not.
- **`advance` re-anchors the rule** on each completion, because the model keeps only the
  current due date. Exact for `UNTIL` and every `BY*` part; *not* for `COUNT`, which is why
  the caller passes `occurrences_completed` (from `Snapshot::completion_count`).
- Sub-daily frequencies are refused: a block has one start time, so `FREQ=HOURLY` would
  expand to one date twenty-four times over and silently collapse.
- `rrule` brings `chrono`, `chrono-tz` and `regex` into the tree, which is awkward next to
  §4's choice of jiff and matters for §16.9's memory budget. All contact with it is confined
  to `recur/convert.rs` so the engine can be replaced without touching callers.

### Parsing

`parse` reads text; what it produces lives in `core` — `DueSpec` for dates, `filter::Expr`
for queries, `State` for computed states. §6.2 calls the AST the stable interface, so
evaluation, the readback, and the accessibility layer never depend on the parser.

- **Both parsers work over words**, not characters, because §6.1's real requirement is
  knowing which *span* a phrase consumed.
- **Quick add cuts the title out of the original input** rather than rejoining words, so
  punctuation and spacing survive.
- **Nothing is ever silently swallowed.** An unrecognised token stays in the title with a
  notice; a repeated one (two dates, two priorities) also stays, with a notice. A property
  test asserts every non-space character is either in the title or inside a recognised span.
- **§6.3 specifies chumsky; this is recursive descent.** The date grammar is word-level and
  multi-word throughout (`next friday`, `every mon, wed and fri`, `no estimate`), so a
  character-level filter grammar would mean two tokenisations and a bridge between them at
  every `due before:` — the same mistake §6.2 warns about with two date parsers. The stated
  benefit is kept: `ParseError::expected` carries the valid token kinds and `complete` is
  built on it. Worth reflecting back into `PLAN.md`.

## Conventions

- `jiff`, not `chrono`. The civil/absolute distinction is load-bearing throughout (§4).
- Timestamps are **millisecond precision**; use `lumenna_core::time::now()`. Automerge's
  timestamp scalar is milliseconds, so anything finer stops equalling itself once saved.
- All identifiers are UUIDv7, so `Ord` is creation order. Several repairs rely on this to
  pick "the newest" deterministically on every replica without coordinating.
- Sibling ordering is fractional indexing on a single LWW string, never a list CRDT (§3.13).
- Tests are named as the sentence they assert. Comments say *why*, and point at the section
  that argued it.
- `missing_docs` and `clippy::all` are warnings at the workspace level. Keep them at zero.

## Commands

```
cargo test --workspace
cargo clippy --workspace --all-targets
PROPTEST_CASES=20000 cargo test -p lumenna-core --test properties
```

`crates/store/tests/convergence.rs` is §19's third risk — replicas reaching the same state
from the same changes in different orders. It is the slowest suite and the one worth running
before believing anything about merge.

### Disk

`/home` is 7.8 GB and this workspace is the largest thing on it, so two settings exist for
that reason alone: `[profile.dev] debug = "line-tables-only"` (full debug info across
`automerge`, `rrule` and a bundled SQLite runs past a gigabyte) and `incremental = false`
(the cache reached 360 MB and full rebuilds take under a minute). Raise either when the
tradeoff changes.

A clean build is about **740 MB** and grows past 1.6 GB over an editing session, because
cargo never collects artifacts from earlier builds. That growth is stale output, not the
project getting bigger.

**`cargo clippy --all-targets` and `cargo test --workspace` do not share artifacts** — the
clippy wrapper produces its own set — so running both back to back roughly doubles the
footprint. On this machine that is the difference between fine and a link failure with
`No space left on device`. Run one, then the other after a `cargo clean`, or accept the
clean between them.

The toolchain is not pinned yet. §17 wants `rust-toolchain.toml` on nightly, but only the
watchOS target needs it; pin it when that spike happens rather than putting every other
target on nightly until then.
