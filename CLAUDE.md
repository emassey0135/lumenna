# Lumenna

A cross-platform task manager and day planner: Todoist's model (projects, labels,
priorities, subtasks, recurrence, quick add) joined to Structured's timeline of time blocks.
The defining feature is that **tasks are assigned to blocks repeatedly across multiple
sittings** until they are done.

`ROADMAP.md` lists what is still to build, with the decisions already made about it.

## Standing decisions

- **Rust core, native UI per platform.** Native controls are what give real screen-reader
  accessibility; a cross-platform toolkit does not.
- **The core owns every rule.** An app words what the core returns and decides nothing
  else. A rule kept in eleven apps drifts eleven ways.
- **Automerge is the source of truth**, stored as change chunks in SQLite. Any SQLite read
  model is a rebuildable index, never truth.
- **Peer-to-peer sync over Iroh.** No server sees the data. Pairing is one mechanism, word
  comparison on both devices, for everything a person does.
- **Accessibility is architectural, not a pass at the end.**
- **No platform is a cut-down version of another** because it shipped later.

## Workspace

```
crates/core/    domain model, queries, mutations, row projection. No I/O, no Automerge.
crates/parse/   quick-add and filter parsers, and the completion they share.
crates/store/   Automerge documents + the SQLite file they live in.
crates/surface/ the command surface: the `Lumenna` object, one method per operation.
crates/ffi/     the library the Swift and Kotlin apps link; re-exports surface over UniFFI.
crates/sync/    Iroh endpoints, the document sync session, and pairing.
crates/desktop/ what the Windows, GTK and web apps decide alike: row wording, flat rows as a
                tree, work-block choices, device wording, the profile.
apps/cli/       `lum`, and `lum rpc` for clients that cannot link Rust.
apps/btspeak/   the BTSpeak app, in Python, over `lum rpc`.
apps/emacs/     the Emacs client, in Elisp, over `lum rpc` or the profile's socket.
apps/android/   Jetpack Compose over the generated Kotlin bindings.
apps/apple/     the iOS and macOS apps over the generated Swift bindings, Shared/ between them.
apps/windows/   the Win32 app, linking the surface directly.
apps/gtk/       the Linux app, GTK 4, linking the surface directly.
apps/web/       the web client: the core in WASM (core/), TypeScript and React Aria over it.
```

## The command surface

`crates/surface` is every operation, once, typed. `Lumenna` holds the open store; its
methods take text identifiers and plain records and return records (`types.rs`). The CLI
calls it, `lum rpc` serialises what it returns, the desktop apps link it, and UniFFI
exports it unchanged to Swift and Kotlin (the `uniffi` feature, built by `crates/ffi`). No
JSON crosses the FFI.

- **Every returned record carries `announcement` and `notices`** (the `Announced` trait).
  The announcement is one composed sentence, and only because the core wrote it: an
  `Edit`'s description or a quick-add readback. Rows stay components (`role`, `state`,
  `title`, `value`) because speech and braille assemble them differently.
- **Composed phrases are the core's, as parts**: `PlanBlock::details`,
  `PlanAssignment::details` and `DeviceView::status`, built in `words.rs`. An app adds its
  own time format and title and joins the parts as its screen reader wants; it never
  composes them itself, which is how seven copies drifted.
- **A time of day goes out as `HH:MM`, for the app's clock.** A task row's `due` is words
  without the time ("due tomorrow") and `due_time` the time, which each app says as its
  platform's setting has it: the locale or desktop setting, BTSpeak's Time Format, Emacs's
  `display-time-24hr-format`, and for `lum` the `clock` device setting.
- **What every client's forms share is here too**, as free functions in `form.rs`:
  `task_fields`/`task_edit`, the block form's `block_fields`/`block_edit`/`new_block`,
  `parse_weight`, the `#"Home Office"` references. `task_edit` and `block_edit` hold only
  what changed: sending an unchanged field reverts a concurrent edit elsewhere. A client
  never keeps its own copy of any of them.
- **Wording stays client-neutral.** Nothing in the surface names a `lum` command or flag;
  the CLI adds those.
- **Identifiers are text**: a whole UUID or a prefix that names one record. A bare number is
  refused. Row numbers are the CLI's: `Profile::row` turns them into identifiers first.
- **Every method refreshes first** (`Lumenna::with`), so an answer is never staler than the
  last change on disk.
- **Reshaping a record breaks `--json`, `lum rpc`, and the Swift and Kotlin types at
  once.** That is the point: they cannot drift apart.
- **A surface error's field must not be called `message`**: Kotlin exceptions already have
  one, and UniFFI's class declared it twice. `LumennaError`'s field is `reason`.
- `uniffi.toml` names the Swift module `LumennaCore`. Methods named after Swift keywords
  (`import`, `open`) come out backticked, and work.

## The CLI

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
- **`--json` is a versioned compatibility contract** for scripts. Reshaping `api.rs` or a
  surface record is a breaking change.

## The RPC server

The surface over JSON-RPC (`crates/surface/src/rpc`, the `rpc` feature), for Emacs and the
BTSpeak app. `lum rpc` serves it on stdio to one client. Whichever process holds the
device's sync endpoint — `lum`'s daemon, or the Mac, GTK or Windows app — serves it at the
profile's address (`surface::endpoint`: `<profile>/lumenna.sock`, or on Windows a named
pipe named from the profile's path) to any number of clients, for as long as it holds the
endpoint (`Endpoint::serve`, `Lumenna::serve_commands`).

- **A method calls the surface's operation and returns its record in the `rpc::api`
  envelope**, the one `lum --json` writes, so no transport drifts from another.
- **Each client gets a store connection of its own**, so what it writes is another
  connection's write to the process serving it, which already redraws for those by
  `outside_version`.
- **`sync` runs on the holder's own service**; `Lumenna::sync_now` anywhere else on the
  device asks the holder over this address. `initialize` says what is answering
  (`process`: `daemon`, an app's name, `lum rpc`); a second daemon is refused by it, while a
  daemon finding an app waits its turn.
- **`lum rpc` relays to a running holder** before serving on its own. Emacs on Windows
  cannot open a local socket or a named pipe (the Emacs manual: MS-Windows does not
  support local sockets), so it starts `lum rpc`, which reaches the running app this way.
- **Three ways to ask "did something change"**, for three questions. `refresh` says whether
  *that call* took anything in, so any other call can use the answer up. `version` moves for
  every write, this connection's included: for a sync loop. `outside_version` (`PRAGMA
  data_version`) moves only when another connection commits: for redrawing after `lum` or
  the daemon wrote. `lumenna/changed` is pushed from `outside_version`, polled once a
  second; built on `refresh`, a request just after another process wrote took the change in
  and the push never went.
- **No row numbers**: the server addresses records by identifier only. The last listing is
  one file per profile, so a resident server and a shell would overwrite each other's
  numbering.
- **`task.erase` requires `"confirm": true`**: nothing here can prompt.
- **Two framings, chosen per message by what arrived**: newline-delimited JSON (MCP's stdio
  transport) and `Content-Length` headers (`jsonrpc.el`). A reply is framed as its request.
- **Spans are UTF-8 byte offsets.** A client that indexes by code point converts both ways.
- **`pair` replies when the pairing ends**, talking in between through `lumenna/pairing`
  notifications (`{code, name}`, then `{words}`); the client answers `pair.confirm {match}`
  or `pair.cancel`, and the server keeps answering everything else. One pairing at a time.
- **`complete` and `preview` have no command line**: completion is a keystroke-rate question
  and a process per keystroke is not an answer.
- **`lum rpc` takes backups at start and hourly** (`Host::backups`), as the CLI takes them
  before a command; a daemon or app serving the address takes its own.

## Sync

`crates/sync`: `session` reconciles every document with Automerge's sync protocol over any
byte stream; `pairing` is the word comparison; `node` is the device's Iroh endpoint;
`invite` is the short-lived endpoint a pairing runs on. Driving sync (pairing through a
`PairingPrompt`, `sync_now`, status, devices, the `SyncService` loop) is the surface's
`sync.rs`; the CLI adds only the terminal prompt, the daemon's socket and signals.

- **Membership in `devices` is the trust boundary.** `Node` refuses, in both directions, any
  key not listed there. The only way in is a pairing whose words were confirmed on both
  sides (`edit::enroll_devices`).
- **Pairing never uses the device key.** It runs on a key minted per pairing, so it needs no
  daemon and no IPC; device keys are exchanged only after the words are confirmed.
- **The words** come from the connection's TLS exporter secret plus a commit-then-reveal
  nonce exchange: three PGP words. Without the commitment, 24 bits could be ground offline.
- **One endpoint per device**, decided by an advisory lock on `<profile>/sync.lock`. A
  `SyncService` holds it, or, started while another process does, waits its turn (trying
  every five seconds) and takes over when that process stops. The holder serves
  the profile's address with the whole RPC surface (see the RPC server), so `sync_now`
  anywhere else on the device runs its round there; only when nothing answers is it
  `LumennaError::SyncElsewhere`.
- **A loop merging makes is written down once and said once** (`edit::repair`, through
  `Lumenna::told`): looked for whenever the documents' heads moved from outside, or right
  after an operation that called `merged()` (restore, import).
- **The service shares the operations' connection**, so it notices local edits by
  `Store::version()`; `refresh` cannot see a connection's own writes. The device key lives
  in the store's `local_state` table and never syncs.
- **Async underneath, blocking on top.** `pair_async` and `keep_in_sync` are the
  implementation; `pair`, `sync_now` and `SyncService` run them on a runtime of their own and
  are native only. The browser runs them on its event loop, so the sync crate uses
  n0-future for time and spawning, never tokio's runtime.
- **A round that misses a device is retried**, after 5 seconds and doubling to the full
  round's five minutes; otherwise a change sent while a device was briefly unreachable sat
  until the next full round.
- **The session is lockstep per document**: both sides send one frame (a message, or empty)
  then read one; both empty ends the document, so both stop together. Sync state is per
  session, not saved.
- **Local discovery is standard DNS-SD** (`discovery/`): `_lumenna._udp` (devices) and
  `_lumenna-pair._udp` (pairing), TXT `id` plus `a0`… addresses. Apple uses the system
  responder (on iOS, Bonjour, needing only `NSBonjourServices`, no multicast entitlement).
  Linux uses Avahi over D-Bus when it runs: beside Avahi, `mdns-sd` heard nothing, not even
  itself. `mdns-sd` covers Windows and Linux without Avahi. Iroh's own mDNS is not used: no
  standard browser can see it (no PTR record, every TTL zero).
- **Bonjour reports an instance once per interface**, and a gone device's record can
  linger. So an instance is resolved once, on its own thread, and lost only when gone from
  every interface; dials to heard pairing sessions give up after three seconds.
- **A session joining by code does not advertise.** Otherwise the waiting session dials it
  back and two connections each wait for the other to speak.
- **Tests that pair take turns** (`ONE_PAIRING_AT_A_TIME`): waiting sessions on one network
  find each other. Tests use `Network::LocalOnly` with explicit addresses.
- Device records are inline (`Record::INLINE`): both sides of a pairing write them.
- `lum daemon install` (`service.rs`) writes a systemd user unit plus linger on Linux, a
  system unit through `sudo` on BTSpeak, a LaunchAgent on macOS, a logon task on Windows,
  always with `--profile`. macOS and Linux have been run for real; BTSpeak's and Windows'
  are unit-tested only.

## The core and the store

`core` holds **materialized** records and knows nothing about Automerge; `store` hydrates
documents into `Snapshot` and translates edits back into Automerge operations. Every query
runs over plain structs, so the rules are testable without a CRDT, and a read model could
produce the same structs later. Two consequences: `notes` is a `String` here though it is
Automerge `Text` in the document, and nothing validates on construction that merge could
violate: dangling references and cycles must load, not be refused.

### Store rules that are easy to break

**Writes take the version the user edited.** `put(record, before)` writes only the fields
that differ. Passing a freshly read `before`, or `None` on an update, writes every field,
and each write wins a last-write-wins race against a concurrent edit from another device,
silently reverting it.

**Containers must not be created concurrently.** Automerge object identity comes from the
creating operation, so two devices creating a map at the same key create two maps, and on
merge one loses everything written into it. Root collections come from a deterministic
genesis change (`Doc::new`); per-record sets are created with the record. Any *new*
container needs the same care.

**A record keyed by something two devices can produce on their own is inline.** Exceptions
(series and date) and reminder acks (reminder and date) store each field as its own key,
`<record>/<field>`, so nothing is created for merge to choose between. Set `Record::INLINE`
for any new record keyed that way.

**Things that must exist once per store come from deterministic changes.** The genesis
creates the root collections; `Doc::ensure_schema` creates the Inbox under the fixed
`ProjectId::INBOX` and the `series_years` map. Both are frozen bytes: never edit what they
write; add another change on top. Clients never create an Inbox; `edit::adopt_inbox` folds
a pre-existing one into the shared one.

**A series lives in its start year's document, and recurs into later ones.**
`series_years` records which years hold recurring series, and `Store::load_year` loads
those too. Without it a routine vanishes on 1 January.

**Change rowids are `AUTOINCREMENT` and compaction holds the write lock.** Reused rowids
make running processes skip changes. `Db::compact` folds everything on disk into the
document under `BEGIN IMMEDIATE` before deleting anything, and `refresh` reads in one
transaction and loads a snapshot whose heads it lacks.

**The change cursor only advances in `refresh`.** `PRAGMA data_version` does not move for
your own connection's writes, and other processes' rows can interleave with yours. A cursor
that lags re-reads a change already held (a no-op); one that skips loses an edit.

### Versions and fields this build does not know

Builds of different versions sync with each other for good: nothing can make every device
update. So:

- **`SCHEMA_VERSION`** (core) is the stored format this build writes. Every document records
  the highest that has written it (`Doc::stamp_version`, on the way to disk), and every
  device its own in the device list (`edit::note_own_version`, on open; pairing says it
  too). A device on another version says so in its status. A change to the format waits for
  `Snapshot::all_devices_at_least`.
- **Never drop what you cannot read.** An edit writes only the fields it changed, so a field
  a newer build added survives. The store's own whole-record rewrites — a series moving to
  another year's document, a record becoming inline — copy the old record as it stands
  (`Raw`) and write only the changes over it, so an unknown field, or a value read as a
  default (an unknown block kind reads as work), survives those too. A new rewrite must do
  the same. Not yet kept: a record restored by undoing a delete from the trash, and JSON
  export, which carry only what the model holds.

### Mutations and undo

`core::edit` computes an `Edit` (before/after record pairs) and `store` applies it. Core
writes nothing, so every rule is testable against plain structs.

- **Everything a person does goes through `Store::apply_recorded`**; what the app does for
  them (folding a legacy Inbox, import, restore) uses `Store::apply` and is not recorded. A
  new command that writes must use an `Edit`, or it cannot be undone.
- **Undo applies an edit's other side**, never rewinds: Automerge gives history, not undo,
  and rewinding would discard concurrent remote changes. It is **rebased**: only fields
  still holding what the edit set go back; the rest are kept and reported, so the stack
  never jams on a conflict.
- **The undo history is per device and never syncs**: an `undo` table in the profile's
  SQLite file, so `lum undo` undoes what the BTSpeak app did too. The change an undo makes
  syncs like any other. Entries are the model serialised with serde; one a later version
  cannot read is dropped with a notice, not guessed at.
- **Operations own the multi-record rules.** Completing a task writes a completion,
  cascades to subtasks per the setting, and advances a recurring due date.
- A cascaded completion of a **recurring** subtask must name that subtask's own occurrence,
  or it reads as unfinished the moment it is written. Cascades never *advance* a subtask:
  that would resurrect what was just closed out.
- **A subtask of a recurring task recurs with it.** `Snapshot::occurrence_of` gives the
  occurrence a completion is scoped to: the task's own due date if it recurs, else its
  nearest recurring ancestor's.
- **Ask many tasks through `Facts`.** `Snapshot::has_state` and friends scan every
  completion and assignment per call; loops use `Context::new` or `Snapshot::facts()`.
- Every operation returns `Edit::nothing()` when nothing differs, so the undo stack does not
  fill with entries that appear to do nothing.

### Recurrence

Recurring **tasks** are one task whose due date advances: no generated series. Recurring
**blocks** are a series plus sparse exceptions, expanded on read. They share a syntax and
nothing else.

- **Expansion runs in UTC**, always: recurrence is a civil-calendar question, and UTC has no
  transitions to perturb it. What a block in a skipped DST hour does belongs where an
  occurrence meets a real zone.
- **An anchor that does not match the rule is not an occurrence** (RFC 5545): "every
  Monday, starting Tuesday" never occurs on its start date.
- **`advance` re-anchors the rule** on each completion, because the model keeps only the
  current due date. Exact for `UNTIL` and every `BY*` part, *not* for `COUNT`, which is why
  the caller passes `occurrences_completed`.
- Sub-daily frequencies are refused: a block has one start time.
- All contact with `rrule` (which brings `chrono`, `chrono-tz`, `regex`) is in
  `recur/convert.rs`, so the engine can be replaced without touching callers.

### Parsing

`parse` reads text; what it produces (`DueSpec`, `filter::Expr`, `State`) lives in `core`,
so evaluation, the readback and the accessibility layer never depend on the parser.

- **Both parsers work over words**, not characters, because what matters is knowing which
  *span* a phrase consumed. They are recursive descent: the date grammar is word-level and
  multi-word (`next friday`, `every mon, wed and fri`), so a character-level combinator
  grammar would need two tokenisations bridged at every `due before:`.
  `ParseError::expected` carries the valid token kinds, and completion is built on it.
- **Quick add cuts the title out of the original input**, so punctuation and spacing
  survive.
- **Nothing is ever silently swallowed.** An unrecognised or repeated token stays in the
  title with a notice. A property test asserts every non-space character is in the title or
  inside a recognised span.

### Backup and export

Kept apart in code, commands and wording, because confusing them is how someone sends a
"task list" holding everything they ever deleted.

- **A backup** (`store::backup`) is every document's Automerge `save()` in one file: the
  whole history, trash included. Restoring **merges**, and can never take back a later
  change.
- **An export** (`store::export`) is current state only. JSON is complete and versioned
  (`export::VERSION`), keeps identifiers, and is what import reads; Markdown, org and ics
  are for reading only. **A field added to the model must be added to `export/json.rs`**,
  or `crates/store/tests/export.rs`, which round-trips every field, fails.
- Automatic backups go to `<profile>-backups` beside the profile, never inside it, `0600`,
  pruned beyond `backup-keep`. `backup-dir`, `backup-keep`, `backup-every` (and `clock`) are **device
  settings** (`<profile>/device-settings`) and never sync. `LUMENNA_BACKUP_DIR` overrides
  the directory; every test harness sets it.

## The BTSpeak app

`apps/btspeak/`: Python, on the device's own `dialogs` library, talking to `lum rpc`. The
library is only on a BTSpeak (`/BTSpeak/Python/BTSpeak/`); elsewhere the tests run on
`tests/btspeak_stub.py`, with `target/debug` on `PATH` for `lum`.

- **A client and almost nothing else.** No date parsing, no computed states. `rows.py` is
  the only file that decides anything, and only about folding.
- **Speech and braille come from one string** with the stock library
  (`DynamicMenuDialog.draw()`), so components are flattened at the last step. A subclass is
  the fallback if they must diverge.
- **Depth is never indentation**: it says nothing in speech. The level is announced only
  where it *changes*, against the previous *visible* row, so folding keeps it right.
- **Our own writes do not come back as `lumenna/changed`**, so `Session` tracks its own
  edits and ORs them with the push.
- **Completion is offered after the line is entered.** `InputDialog` has no hook and Tab is
  form navigation; inline needs a subclass.
- **`main()` pushes an app context** (`host.push_app_context("lumenna", self_voice=True)`,
  popped in a `finally`). The device takes its braille table from the app it believes is in
  front; without the push, BT Code's `.py` table made every menu computer braille. The push
  and pop are device-wide. `# blazie-flags: self-voice` on line 2 of `__main__.py` is for
  the menu launcher. With self-voice on, printing is silence, so startup errors are dialogs
  and the server's stderr goes to `rpc.log`.
- **It is a line in `~/BTSpeak/user.menu`**, added with `user_menu.add_item`, not a `.menu`
  file. `connect.find_lum` finds `lum` in the checkout's `target/`, since a menu launch's
  `PATH` lacks it.

## The Emacs client

`apps/emacs/`: Elisp over `jsonrpc.el`, built-in libraries only. Tests are ERT against a
real `lum rpc`, answering the minibuffer by rebinding the reading functions:
`emacs --batch -Q -L apps/emacs -l apps/emacs/test/lumenna-test.el -f ert-run-tests-batch-and-exit`.

- **The socket, else a child.** `lumenna-profile-directory` repeats `lum`'s rule for the
  default profile, because the socket is found by path before there is anything to ask.
- **Depth is an outline level**: `outline-level` reads the `lumenna-level` text property,
  so folding and Emacspeak's level announcements come from outline mode.
- **A list's keys are defined once** (`lumenna-define-keys`), which binds them, builds the
  menu, and lists them for `?` in an ordinary buffer. Not transient: Emacsvox reads a
  transient by asking whether its command was called interactively, and Emacs 31's
  transient wraps each command, so moving through one was silent.
- **Booleans come back as `:json-false`**, which is non-nil: test them with `lumenna--true`.
- **Writes go through `lumenna-write`**, which refreshes every Lumenna buffer, runs
  `lumenna-changed-functions` and hands the announcement to `lumenna-announce-function`.
  No advice anywhere.
- **Three speech layers, each skipped where it would double up**: echo-area text; faces
  through `voice-setup-add-map` plus Emacspeak auditory icons; and under Emacsvox, facts on
  each line and events on the notification lane, whose rules are one per role and event (a
  rule per method played the sound twice). The Emacsvox layer is tested against Emacsvox
  itself (`test/lumenna-emacsvox-test.el`, skipped without it).

## The iOS app

`apps/apple/`: a UIKit shell with SwiftUI forms over the generated `LumennaCore.swift`.
`cd apps/apple && xcodegen` makes the project from `project.yml`; the `.xcodeproj` and
`Generated/` are not committed.

- **Xcode builds the core itself**: `build-core.sh` builds `lumenna-ffi` in a clean
  environment (Xcode's SDK variables break Cargo's host build scripts), then regenerates the
  bindings from a host build. It must pass `IPHONEOS_DEPLOYMENT_TARGET` through, or C
  objects target the SDK's own version; heed that linker warning.
- **The core is linked statically, by the `.a`'s full path.** `-llumenna_ffi` picks the
  dylib Cargo builds beside it, which works in the simulator and crashes on a phone at
  launch. A post-build phase fails the build on any `liblumenna_ffi` reference. A launch is
  not proof it ran: check the process is alive, or the crash logs.
- **Iroh needs `SystemConfiguration` and `Network`** linked.
- **The cell owns what VoiceOver says**: every list cell sets its own label and value from
  row components (`RowSpeech`); left alone, a cell says its secondary text twice. Depth is
  said only where it changes.
- **Rows are `UIListContentConfiguration`, not SwiftUI**: hosted `Text` failed the audit for
  Dynamic Type and clipping. SwiftUI stays for forms.
- **Text entry wraps** (`LineEntry`): a one-line field scrolls sideways at large sizes and
  the audit flags it as clipped.
- **A SwiftUI `TextField`'s title is only a placeholder**: with text in it, VoiceOver never
  says the name. See `NamedRow` below.
- **Focus after a change is chosen**: the same row if still listed, else whatever holds its
  position; the core's announcement is queued behind the focus change so neither cuts the
  other off. This holds on every platform.
- **Swipe actions are the only actions.** UIKit offers them to VoiceOver, Switch Control and
  Full Keyboard Access; custom actions are *added* to those, so both lists each twice. A
  swipe action's title is its spoken name ("Mark Done", not "Done").
- **No bottom toolbars**: inside a tab bar the floating bar sits over a toolbar's buttons,
  so a tap on Undo landed on the Today tab. Rows scrolled under the glass tab bar fail
  contrast, so pushed forms set `hidesBottomBarWhenPushed`.
- **`TZ` is set from `TimeZone.current`** at launch and on returning to the foreground: jiff
  finds the zone through `TZ` or `/etc/localtime`, and the sandbox is no place to rely on
  the second.
- **Operations run on the main thread**: milliseconds against local SQLite, and a view never
  shows a state the store has moved past. The automatic backup is the exception.
- **Background sync** (`BackgroundSync`): a round in the time `beginBackgroundTask` grants
  on leaving, and a `BGAppRefreshTask` whenever iOS chooses. A refresh launches the app with
  no scene, so the `Core` is the app delegate's, one per process. Apple's simulated launch
  hung in the iOS 27 simulator; the handler has only run on a device.
- **Pairing is by code on iOS**: local discovery of others needs Apple's multicast
  entitlement. `PairingPrompt` blocks the pairing thread on a semaphore while the words are
  asked on the main thread.
- **Stock controls unless there is a reason**, and each in `FormParts.swift` has one:
  `NamedRow` (the field carries the name and the visible name is hidden, as UIKit forms do;
  combining the row lost the text-field role), `example(_:)` placeholders and
  `FormParts.caption` (the system greys fail contrast). Give a control an empty title when
  it has an accessibility label, or VoiceOver says the name twice.
- **The tint is `UIColor.lumennaTint`** (system blue is about 4:1 on white), on the window
  **and with `.tint` on every SwiftUI form**, whose accent ignores the window's. Red text is
  `.warningLabel`. The colours have Increase Contrast variants.
- **The audits collect every issue and fail once** (`audit()`). Exemptions, all narrow:
  "potentially inaccessible text" on `NamedRow` screens, the keyboard's
  `TUIPredictionViewCell`, and "partially unsupported" Dynamic Type on SwiftUI nodes (the
  audit reports it even for a stock button). To see an unnamed finding, run with
  `AUDIT_ATTACH=1` and `xcrun xcresulttool export attachments`.
- **Keyboard commands are the Mac's keys and names, once** (`KeyboardCommands.swift`): on
  iPad the menu bar (`AppDelegate.buildMenu`, with Find and Format removed, which would take
  ⌘F and ⌘B), on iPhone, which has none, `RootTabs.keyCommands`; never both, or a key has
  two commands. Each is a selector up the responder chain, so the screen in front answers
  before `RootTabs`. **Something must be first responder** or the chain is empty and the
  Task and Day menus reach nothing: each screen with commands calls `takeKeyboardCommands`
  as it appears, unless text is being edited. ⌘Z in a field with typing to undo undoes
  the typing. Row commands act on the row with keyboard focus, else VoiceOver's, else the
  one open beside the list; the task open beside a list answers them itself.
- **The iPad is the Mac's layout, not the iPhone's tabs** (`IPadRoot`, three columns): the
  core's places in a sidebar (`SidebarViewController`, from `Lumenna.places`, plus
  Settings), the place chosen, and what was opened from it (`showBeside`, which pushes on
  iPhone and in a narrow window). The sidebar is an `ItemListViewController`, so headings
  and project trees fold with Expand and Collapse actions and say which they are; a
  project's, label's or filter's actions are the ones Browse offers (`PlaceActions.swift`,
  once for both). A screen chosen by a command acts a turn later: it is not in the window
  until then.
- **⌘F never reaches the app in the iPad simulator**: no responder is asked about it, where
  ⌘3 is. Filter Tasks stays on ⌘F in the Edit menu, and its test skips on iPad.
- **Three layers of iOS tests.** `LumennaTests` (unit tests hosted in the app) asks each
  list for its swipe actions (`leadingSwipeActions(at:)`, `trailingSwipeActions(at:)`)
  and runs them: the list VoiceOver offers as a row's actions, with no gesture to land, so
  they block in CI. `VoiceOverUITests` turns VoiceOver on (`XCUIVoiceOverService`, iOS 27)
  and checks what it says, acting through the keyboard commands, which act on VoiceOver's
  row; its output carries a hint's first word only ("Actions"). The rest of the UI tests
  drive the app by gestures, and report rather than block.
- **iPad UI tests run in portrait**: in landscape XCUITest's coordinates come out rotated, so
  part-swipes and taps land on the wrong row, and screenshots come out half black. A hidden
  sidebar's rows still exist off the screen, so a place is used only once it is hittable.
  Swipe actions are revealed by a slow, held part-swipe (`reveal(actionsOf:)`): across a
  narrow column a whole swipe runs the first action itself. Every failure attaches a
  screenshot (`record(_:)`).
- **`apps/apple/test-all.sh`** runs every iOS UI test on the iPhone, an SE and the iPad at
  once, built once, each device's tests shared over clones, each test stopped after three
  minutes: one waiting forever held a whole run.
- **The iPad's menu bar is invisible to XCUITest while hidden**, so the keyboard tests
  (`KeyboardUITests`) prove it by its keys: on iPad they exist only there.
- **A UI test names a fresh store** through `LUMENNA_TEST_PROFILE`.
- **Run UI tests with `-collect-test-diagnostics never`**, or any failure hangs xcodebuild
  for ten minutes collecting simulator diagnostics:

  ```
  cd apps/apple && xcodebuild -project Lumenna.xcodeproj -scheme Lumenna \
    -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
    -collect-test-diagnostics never test
  ```

## The macOS app

`apps/apple/Mac/`: AppKit, with SwiftUI only for leaf forms. `Shared/` holds what both
Apple apps compile, including **`Shared/Forms/`, the task, block and settings forms**; each
app supplies only presentation. Where the platforms need different answers it is an
`#if os` in `FormParts.swift` (`Named`, `Labelled`, `ChoiceSection`) or at the one row
that differs, so a fix to a form lands on both. Scheme `LumennaMac`.

- **Not sandboxed, and shares `lum`'s profile**: the app and the command line on one Mac
  are one device with one store. `Core` compares `outsideVersion` once a second.
- **Resident in the menu bar, and the device's sync process.** No daemon is needed or used.
- **Global shortcuts through Carbon's `RegisterEventHotKey`**, which needs no accessibility
  permission. Never Control-Option: it is VoiceOver's. The recorder warns of clashes with
  VoiceOver, VOCR (Control-Command S, P, L, I; Shift-Control-Command more) and macOS
  (Control-Command Q, F, D, Space).
- **The core has a record called `Timer`**, so Foundation's is named in full.
- **A stepper that stands for something says it** (`LengthStepper`): AppKit's says its
  number, which VoiceOver reads as a percentage of its range, and SwiftUI's
  `accessibilityValue` on it is ignored. An `NSStepper` subclass overrides
  `accessibilityValue` and posts the change.
- **SwiftUI form controls go inside `Named`/`Labelled`**: on macOS a `LabeledContent`. A
  bare `TextField("Title", …)` showed its name as separate text and left the field unnamed;
  a bare `Toggle` was an unnamed switch. On iOS the opposite (see `NamedRow`). A growing
  text field draws its name in grey, under contrast, so Notes is a `TextEditor`.
- **SwiftUI is hosted through `HostedForm`**: the hosting view steps out of VoiceOver's tree
  and gives its name to the form's scroll area. It has to be the hosting view's own
  override; set from outside, it goes on reporting itself as a group.
- **A table cell recolours itself on selection** (`TwoLineCell.backgroundStyle`): the quiet
  grey fails contrast on the selection highlight.
- **The window's minimum size is set after its content**, or the split view shrinks it to
  almost nothing.
- **UI tests enter text by pasting** (`enter(_:into:)`): the test Mac runs VoiceOver, and
  synthesized keystrokes reached it. Windows are found by identifier (`main`, `settings`):
  the first window can be Siri's.
- **The Mac audit judges only the window in front** (or the open sheet): dimmed windows
  fail contrast without being what anyone reads. It also skips contrast under VoiceOver's
  own caption and braille panels. Also excused: the Touch Bar, SwiftUI pop-ups' "Action is
  missing", and a "Parent/Child mismatch" with no element.
- UI tests need macOS to have authorized UI automation for Xcode once.

## The Windows app

`apps/windows/`: Rust over `windows-rs`, linking `lumenna-surface` directly, binary
`lumenna.exe`. Everything Win32 is in `src/win/` behind `cfg(windows)`.

- **Building on ARM64 Windows needs clang on `PATH`** for `ring`: Visual Studio's
  `VC\Tools\Llvm\ARM64\bin`. The manifest is embedded by the MSVC linker from `build.rs`.
- **Stock controls only.** Every list is a `SysTreeView32`, so level, position, set size,
  expansion and checkboxes are the control's to report; an item's text leaves exactly those
  out. Dialogs are in-memory templates run by the dialog manager.
- **Names and the live region go through Dynamic Annotation** (`IAccPropServices`), never a
  provider of our own. The status line is a polite live region; a dialog in front hides it,
  so each settings page and the pairing dialog has its own.
- **An empty static is named after the label made just before it**, so a page's status line
  is made first, or it reads the page's footer twice.
- **A reload updates a tree in place when its rows are the same ones**, so refreshes do not
  make the screen reader re-read the focused row.
- **Nothing that rebuilds a tree runs inside one of its notifications.** Space, Delete and
  double-click go through `App::defer`; `Tree::busy` marks notifications a refill sends.
- **What can be done to a task is in one place** (`win/task_actions.rs`).
- **IsDialogMessage runs over the whole main window**, with the panes
  `WS_EX_CONTROLPARENT`, so menu command ids are never 1 or 2 (`IDOK`, `IDCANCEL`). Field
  mnemonics avoid the menu bar's letters (F E V T D H): the dialog manager gives
  Alt+letter to a field first.
- **A tree view passes a keyboard context menu up altered** (naming the pane, at −2, −2),
  so a request from a pane is taken as from the keyboard; right-clicks come from
  `NM_RCLICK`.
- **Never disable the control that has focus** without moving focus first: focus goes
  nowhere.
- **The Notes field gives Tab and Escape back** (`WM_GETDLGCODE`), and panes ignore
  `WM_CLOSE`, which a multi-line edit sends its parent on Escape.
- **Global shortcuts and opening at sign-in are this PC's** (`HKCU\Software\Lumenna`, the
  Run key with `--background`). Shortcuts are unregistered while new keys are chosen.
  Windows does not say who holds a refused shortcut, so another open Lumenna window (class
  `LumennaMain-`) is named rather than "another program". `--no-shortcuts` leaves them to
  another copy.
- **One instance per profile** (a mutex and window class named from the profile path). The
  first launch triggers the firewall prompt, since the sync endpoint listens.
- **Completion is a popup menu** on Down or Ctrl+Space. Items lead with the name so a letter
  finds them; the first is highlighted by a Down queued before the menu opens, marked with
  a reserved bit of the key data so the field swallows it if the menu does not take it.
- **UI tests** (`tests/ui.rs`) drive the real app and assert on UI Automation. They take the
  foreground, so they are ignored by default and take turns:
  `cargo test -p lumenna-windows --test ui -- --ignored`. A posted key needs the app active
  and not minimized, and a window holding the foreground (a firewall prompt) fails the
  focus checks. They start the app with `--no-shortcuts`, or a test's copy takes the
  person's shortcuts and theirs says they are in use. While the person's own copy runs from
  `target/debug`, `lumenna.exe` cannot be replaced: build them with
  `CARGO_TARGET_DIR=target/ui-tests`.
- **`examples/inspect.rs`** is the same automation by hand (`cargo run -p lumenna-windows
  --example inspect -- "- Lumenna"`, with `--post` steps). Match `- Lumenna`: a terminal
  named after the checkout matches `Lumenna`. UI Automation's SetFocus does not move focus
  in a modal dialog, so `focus:` uses `WM_NEXTDLGCTL`, on the first *focusable* element of
  the name: a dialog's label is named like its field, and comes first. PowerShell's managed
  `System.Windows.Automation` is no substitute: under x64 emulation it saw unnamed panes.

## The GTK app

`apps/gtk/`: Rust over `gtk4-rs`, linking `lumenna-surface` directly, binary
`lumenna-gtk`.

- **Every list is a `GtkListView` that says it is a tree** (`tree.rs`). `GtkTreeView`
  exposes no rows to AT-SPI in GTK 4, and a list view with `TreeExpander` reports a flat
  list. So the view has `AccessibleRole::Tree`, the list item is not focusable, and a
  `TreeExpander` with `AccessibleRole::TreeItem` is, given its level and position in
  `bind`. Left and Right are ours, and `ListTabBehavior::Item` makes Tab leave the tree:
  by default it went through every row. Every list is one, the Devices list and the
  chooser dialog's included; a `GtkListBox` has no such setting.
- **GTK gives AT-SPI a tree item's level from 4.16, and its position and set size from
  4.22.** Below 4.16 (Ubuntu 24.04, Debian 13) a row says "level 2" in its text where the
  level changes against the row shown before, as the other apps word it; position is left
  to the platform. Decided at run time; `LUMENNA_LEVEL_IN_TEXT=1` forces it for tests. CI
  runs the UI tests on Arch, so the native path is what it covers by default.
- **Orca does not say a tree item's checked state**, so a done task says "completed" in its
  text.
- **Focus into a row waits for the row's widget**, which does not exist until GTK lays the
  list out. `Tree::move_to` retries each frame until it lands. `scroll_to(FOCUS)` also
  does not move focus into a list from outside it, so `land` grabs it too; and a rebuild
  takes focus out of the list for a moment, so a tree that had focus before one still
  counts as having it. `App::say` waits behind it
  (`tree::when_settled`): Orca reads an announcement as a message, and a later focus change
  cuts it off.
- **Announcements are `gtk_accessible_announce` from the window**, not from a label: one
  from a label in Settings never arrived.
- **GTK's accessible names need help**: a mnemonic check box keeps the underscore in its
  name (`prompts::check`); a button is named by its text whatever label it is given;
  read-only text is a non-editable entry, since a label named by the label above reads as
  that name.
- **Settings' tabs are a `GtkStackSwitcher`, not a notebook.** A notebook keeps focus
  itself, so Orca reached its tab bar as an unnamed "grouping". Only the current tab is
  focusable, so the row is one Tab stop; the arrows and Ctrl+Tab/Page Up/Down are ours.
- **GTK adds "Alt+" and the mnemonic to a menu item's key shortcuts**, after whatever the
  property holds, so Orca reads "Control+N Alt+N". Alt+N does nothing; the letter alone works
  in an open menu. Only dropping the mnemonic removes it.
- **A text view keeps Tab**; `prompts::leaves_on_tab` steps it out of the focus chain.
- **The sidebar's places are the surface's** (`surface::places`, `Lumenna.places`), shared
  by Windows, GTK, the web, the Mac, the iPad and Android.
- **A short list is a tree sized to its rows** (`Tree::fit`), hidden when empty. A list box
  in a scroller made the scroller a nameless Tab stop, and making the scroller
  non-focusable broke Tab for the whole page.
- **Dialogs are futures** (GTK 4 has no blocking `run`). While a popover menu is open,
  `window::spawn` waits for it to close, so a dialog is not mapped under a closing menu.
- **A keyboard context menu opens when its keys are let go**: a popover opened on
  Shift+F10's press is closed by the releases.
- **It serves the command surface while it holds the sync endpoint** (`Core::start_syncing`,
  the surface's `rpc` feature), and stops serving before it stops syncing.
- **One instance per profile** through GApplication's bus name, tagged per profile.
  `--no-shortcuts` leaves the shortcuts from anywhere to another copy; the UI tests pass it.
- **First focus on an expandable row says "expanded" twice**: GTK reports the new
  accessible object's initial state as a change.
- **The global shortcuts go through the GlobalShortcuts portal**, which knows an
  unsandboxed app only by an installed desktop entry (`install.sh`), so it is always asked
  under the app's own identifier. The tray (`ksni`) waits quietly for a host; GNOME has none
  without an extension.
- **UI tests** (`tests/ui/`, Python): `apps/gtk/tests/ui/run` starts a private runtime
  directory, D-Bus session, headless mutter and accessibility bus, presses keys through
  mutter's RemoteDesktop API and reads AT-SPI. **The runtime directory must be private, and
  made before the bus**: a run in the desktop's own `/run/user/<uid>` replaced the desktop's
  accessibility socket, leaving every app started afterwards without a screen reader.
  `ORCA=1` runs Orca too. A new window takes well over half a second to show in the
  headless compositor: wait with `Session.wait_for_window`.

## The Android app

`apps/android/`: Kotlin and Jetpack Compose over UniFFI's Kotlin bindings (JNA). Its README
gives the toolchain.

- **Gradle builds the core itself**: `buildCore<Variant>` runs `build-core.sh`. Only
  `liblumenna_ffi.so` is copied; `cargo ndk -o` would also copy Iroh's shared libraries,
  which nothing loads. Arm64 and x86_64 (Googlebooks have both, and the emulator is x86_64), and
  `abiFilters` keeps JNA's six others out of the APK; minSdk 28. Both are 16 KB aligned.
- **Espresso is pinned to 3.7**: Compose's test library brings 3.5, which calls an
  `InputManager` method Android 17 removed.
- **The tests run Google's accessibility checks on every interaction**, on a store of their
  own (`Core(directory)`). Buttons scroll into view before a click: a click on an
  off-screen node lands on nothing.
- **Material's text and outlined buttons are 40dp**, and a radio row is as tall as its
  padding: both failed the touch-target check. `Target` gives 48dp.
- **A row is one node** (`ListRow`: `clearAndSetSemantics`, custom actions) with one
  description, the title first. As a separate state description, the rest was said before
  the title. `RowTitle` carries the title alone, for tests to find a row by.
- **Nested lists fold** (`Folding.kt`): everything starts expanded, and a row with
  something under it has Collapse or Expand among its actions and says which it is. The
  level is said against the row shown before, after folding. The iPhone does the same
  (`Folding.swift`, as a swipe action).
- **TalkBack does not follow input focus under touch.** `RowFocus` gives the row input focus,
  then performs the accessibility-focus action through the view's
  `AccessibilityNodeProvider`, found by the `RowKey` property; the announcement is held
  (`Core.hold`/`release`) until then. The test of it is skipped unless TalkBack is on.
  Compose sends accessibility events only while a screen reader runs.
- **Keyboard commands are the Windows and GTK apps' keys** (`Shortcuts.kt`), on Ctrl:
  TalkBack's are on Alt or Search. A screen offers a command while shown (`Offer`), the
  newest winning, and the system's shortcuts helper (Meta+/) lists what is offered now. A
  row's own commands (Ctrl+K, Delete, Shift+F10) run its action of that name, so a key
  never does what the row's action list does not. Compose's root takes the keys while
  anything has focus; `MainActivity.dispatchKeyEvent` takes them when nothing does.
- **Buttons take focus only out of touch mode**, which a key press leaves; the keyboard
  tests leave it first (`setInTouchMode(false)`), or no tab could be focused.
- **A wide window has the desktop apps' sidebar** (`Sidebar.kt`, from `Lumenna.places`,
  plus Settings) from 840dp, with a stack of its own for the place chosen; headings and
  project trees fold with Expand and Collapse actions, and a project's, label's or filter's
  actions are Browse's (`PlaceActions.kt`, once for both). Between 600 and 840dp the tabs
  are a rail. `LumennaTest` and `NewTaskTest` show the app at a phone's width (`PhoneWidth`),
  so they run on the desktop-sized emulator too; `SidebarTest` and `KeyboardTest` are the
  wide window's.
- **A wide window** shows a screen beside the one it was opened from (`Navigator`'s depth: a row chosen in the parent replaces the
  child rather than stacking). F6 moves between the tabs and panes, back onto the row a
  pane was left on (`Pane`): Compose's `saveFocusedChild` keeps only the pane's immediate
  child. Tests force only the width: a forced size bigger than the window is drawn at a
  smaller density, and every touch target measures small. So touch targets are excused
  only when the forced width exceeds the screen; CI's desktop-screen run checks them in
  the wide layout for real. The desktop system images ended at API 34: `desktop_medium`
  on 36 is a large screen without freeform windows. A real Googlebook (Android 17) has
  those, a title bar per window, and no menu bar.
- **`run-instrumented-tests.sh`** runs the instrumented tests from APKs already built, with
  `am instrument`, which exits 0 whatever happened, so its summary decides. CI builds once
  and runs it on each emulator. It never uninstalls, unlike `connectedAndroidTest`, but
  never point it at a device whose own data matters.
- **The store is in no-backup storage** and `allowBackup` is off: Google's backup would copy
  a store that reaches other devices by pairing.
- **Background sync is WorkManager** (`SyncWorker`): a round on leaving (expedited on
  Android 12 and later; earlier, expedited work needs a foreground notification) and every
  15 minutes. Discovery hears multicast only under a `MulticastLock`, held while syncing or
  pairing; it is not exclusive and `mdns-sd` binds 5353 with `SO_REUSEPORT`, so other apps'
  mDNS is unaffected. The emulator's NAT passes no multicast.

## The web client

`apps/web/`: TypeScript, React and React Aria Components over the core compiled to
`wasm32-unknown-unknown` (`apps/web/core`, crate `lumenna-web`). `npm run core:dev` builds
it and generates `src/core/` (not committed) with wasm-bindgen, whose CLI must match the
crate's version in `Cargo.lock`. Tests are Playwright over the accessibility tree with axe:
`npm test`; `LUMENNA_NETWORK=1` adds pairing over n0's relays, `LUMENNA_LUM=<lum>` pairing
with `lum`.

- **The same store, in OPFS.** rusqlite builds for the browser on `sqlite-wasm-rs`; the
  `sahpool` VFS needs sync access handles, which only a dedicated worker may use, so the
  core runs in `src/worker.ts` behind Comlink. No WAL there.
- **One tab owns a store** (a Web Lock named for the profile). Opening is idempotent per
  worker, because React's development mode runs effects twice.
- **Records are typed from the surface's structs** (`tsify`). Every export returns through
  `js()` in `lib.rs`, which writes maps as plain objects: a `#[serde(flatten)]` record came
  out a JavaScript `Map` otherwise.
- **Spans cross as UTF-8 bytes**; the worker converts to the UTF-16 offsets an input counts.
- **A React Aria tree builds its collection in a pass of its own**, so rows can lag the
  data by a render: focus after a change waits for current rows (`TaskList`).
- **Quick add is a combobox completing mid-line**, its filter off (the core decides what
  fits), and Down waits for the offer for where the cursor is *now*.
- **What cannot be undone is an `alertdialog` with Cancel focused first**, never `confirm()`.
  A refused answer stays in its dialog.
- **A row's action runs after its menu has closed** and focus is back on the row, so a
  dialog it opens returns focus there.
- **The sidebar's row in hand is the one focus was last on**, not the selection: its
  headings (Projects, Labels, Saved Filters) are never selected.
- **A React Aria check box's input is visually hidden**, so Playwright's `check()` waits for
  it forever: tests focus it and press Space.
- **A browser is an ordinary Iroh peer, always through a relay** (no UDP from a sandbox),
  and pairs by code. `.cargo/config.toml` sets getrandom's `wasm_js` backend.
- **Callbacks into the core are wrapped in the worker**: a Comlink proxy answers any
  property, so wasm-bindgen's `f.call(…)` on one became a remote call named "call".

## Conventions

- `jiff`, not `chrono`. The civil/absolute distinction is load-bearing throughout.
- Timestamps are **millisecond precision**; use `lumenna_core::time::now()`. Automerge's
  timestamp scalar is milliseconds, so anything finer stops equalling itself once saved.
- All identifiers are UUIDv7, so `Ord` is creation order. Several repairs rely on this to
  pick "the newest" deterministically on every replica.
- Sibling ordering is fractional indexing on a single LWW string, never a list CRDT.
- Tests are named as the sentence they assert. Comments say *why*.
- `missing_docs` and `clippy::all` are warnings at the workspace level. Keep them at zero.

## Commands

```
cargo test --workspace
cargo clippy --workspace --all-targets
PROPTEST_CASES=20000 cargo test -p lumenna-core --test properties
```

CI (`.github/workflows/ci.yml`) builds each mobile app once and tests it on several devices
in parallel jobs that share the build as an artifact: `ios-build` with `build-for-testing`,
then an iPhone, an SE and an iPad with `test-without-building`; `android-build`, which
also checks 16 KB alignment and builds the core optimized (`LUMENNA_CORE_PROFILE`), then
a phone and a desktop-sized emulator with `run-instrumented-tests.sh`, on Android 36:
every Android 37 image crashed its display server on the runners' software renderer. The test jobs need
no Rust. **Two tiers**: the Rust tests, the builds, the clients, the GTK and Windows UI
tests and a smoke set of iOS and Android UI tests block; the full iOS and Android suites
report without blocking, since a slow runner fails them where nothing is wrong. A test
that has only failed for a reason can join the smoke set.

`crates/store/tests/convergence.rs` checks replicas reaching the same state from the same
changes in different orders. It is the slowest suite and the one worth running before
believing anything about merge.

`[profile.dev]` sets `debug = "line-tables-only"` and `incremental = false` for the small
disk of the BTSpeak machine: full debug info across `automerge`, `rrule` and a
bundled SQLite runs past a gigabyte, and the incremental cache did not pay for itself.
There, `cargo clippy --all-targets` and `cargo test` do not share artifacts, so running both
back to back can run out of space; `cargo clean` between them.

The toolchain is not pinned; only a watchOS target would need nightly.
