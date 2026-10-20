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
crates/records/ what a person is told, as records built from a snapshot alone: rows and their
                actions, questions, forms, the listings, the day, what a pick offers, the
                proposals, the words. No store.
crates/host/    the command surface: the `Lumenna` object, one method per operation, over the
                store; re-exports records under the paths it always had.
crates/app/     the Crux app: the screens' state (place, row in hand, folding, the question
                open) as events in and view models out; every write an effect. No host.
crates/bridge/  the app beside its store: Crux middleware answering the app's store effects.
crates/ffi/     the app for the Apple apps, Android and BTSpeak through BoltFFI: `App`, its
                turns, and `Device`, the store for what runs without the screens. The web's is
                apps/web/core, through BoltFFI too; the shell's events (`ShellEvent`) are the
                app crate's.
crates/sync/    Iroh endpoints, the document sync session, and pairing.
crates/desktop/ what the Windows, GTK and web apps decide alike: row wording, flat rows as a
                tree, work-block choices, device wording, the profile; with `app`, the app's
                view worded line by line (`lines`) for the web and Android.
apps/cli/       `lum`, and `lum rpc` for clients that cannot link Rust.
apps/btspeak/   the BTSpeak app, in Python, over the app in-process (crates/ffi).
apps/emacs/     the Emacs client, in Elisp, over `lum rpc` or the profile's socket.
apps/android/   Jetpack Compose over the Crux app's Kotlin bindings: the phone (app/), Wear OS
                (wear/), and what they share (shared/).
apps/apple/     the iOS, macOS and watchOS apps over the Crux app's Swift bindings, Shared/
                between them.
apps/windows/   the Win32 app, linking the surface directly.
apps/gtk/       the Linux app, GTK 4, linking the surface directly.
apps/web/       the web client: the core in WASM (core/), TypeScript and React Aria over it.
```

## The command surface

`crates/host` is every operation, once, typed. `Lumenna` holds the open store; its
methods take text identifiers and plain records and return records (`types.rs`). The CLI
calls it, `lum rpc` serialises what it returns, the Windows and GTK apps link it, and the
bridge answers the Crux app's writes with it; `Device` (`crates/ffi`) exports what runs
without the screens over BoltFFI. No JSON crosses the FFI.

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
- **Every row's actions are the core's** (`actions.rs`): each listed record carries
  `actions` (which apply now, in order, spoken names, `destructive`, and the `Question` each
  asks: immediate, a confirmation, a line of text, a pick, a choice of answers, or the
  client's own form). A client offers them, asks the question its own way, and hands the
  action back with the `Answer` to `Lumenna::act`; what a pick offers is
  `Lumenna::choices`, so no client offers a task as its own parent or a wait that loops.
  `ActionKind` is generic across subjects so a key means one thing on every row (Delete is
  `Delete`), with `Subject` saying what it is done to. Only `Question::Form` is the
  client's: the task and block forms. Never decide in a client which actions a row has.
  A key pressed on a row without that action says `not_offered(kind, subject, …)`.
  What a picker's row says is `desktop::speech::choice` for Windows, GTK and the web.
- **What every client's forms share is here too**, as free functions in `form.rs`:
  `task_fields`/`task_edit`, the block form's `block_fields`/`block_edit`/`new_block`,
  `parse_weight`, the `#"Home Office"` references. `task_edit` and `block_edit` hold only
  what changed: sending an unchanged field reverts a concurrent edit elsewhere. A client
  never keeps its own copy of any of them.
- **The forms' and the pairing screen's words are the core's** (`fields.rs`): `task_form()`
  and `block_form()` give each field's label, hint, example, control and options;
  `pairing_words(this_device, local)` every sentence and button of pairing;
  `proposal_words(kind)` (RPC `form.proposal`) the Replan from Now and Plan the Day screens';
`go_to_day_question`, `new_filter_questions` and `length_question` (RPC `form.go_to_day`, `form.new_filter`, `form.length`) what the apps' own
  commands ask. A field marked `one_day` is what one day of a repeating block can change. A new block starts at `new_block_start(date)`: the next whole hour today, when
  the day starts on any other day.
  A pick and a text question name their button (`yes`); "OK" is never the answer. A mnemonic or
  access key is the platform's to add; the words are not.
- **Each action has a `sentence` and a `primary`.** Capitals follow the platform: Title
  Case (`title`) on Apple, Windows, GTK and Emacs; sentence case (`sentence`, and
  `sentence_case()` for a fixed button or question) on Android, Wear OS, the web and BTSpeak,
  where every capital also costs a braille cell. A row shows its `primary` actions by itself
  (the iPhone's swipe actions, a watch row's) and offers the rest a menu away; every client
  still offers every action.
- **Lines are joined in one order everywhere**, the clock and day words being the app's: a
  block "<start> to <end>, <title>, <details>"; a sitting "<title>, <details>"; free time
  "<title>, <details>, <start> to <end>"; now "<title>, <time>"; a cancelled day "<start>,
  <title>, <details>"; a block to pick "<day>, <start> to <end>, <title>"; anything else to
  pick "<title>, <detail>"; the day's heading "<day>. <summary>". A Blocks-list row is a task row: `due` ("every weekday") said
  with `due_time` ("at 9:00 AM"), then `value`. An empty listing says `Rows::empty`.
- **A setting describes itself** (`Setting::title`, `kind`, `options`, `syncs`, `hint`), and
  the priorities are `priorities()`: a settings screen or a priority picker builds its
  controls from them, never from a table of its own.
- **Wording stays client-neutral.** Nothing in the surface names a `lum` command or flag;
  the CLI adds those.
- **Identifiers are text**: a whole UUID or a prefix that names one record. A bare number is
  refused. Row numbers are the CLI's: `Profile::row` turns them into identifiers first.
- **Every method refreshes first** (`Lumenna::with`), so an answer is never staler than the
  last change on disk.
- **Reshaping a record breaks `--json`, `lum rpc`, and the Swift, Kotlin and Python types at
  once.** That is the point: they cannot drift apart.
- **A surface error's field must not be called `message`**: Kotlin exceptions already have
  one, and a generated class would declare it twice. `LumennaError`'s field is `reason`.

## The app

`crates/app` is the screens as a Crux 0.20 app; `crates/bridge` runs it beside a store. The
web, the Android phone, Wear OS, BTSpeak and the iPhone, iPad, Mac and Apple Watch are drawn
from it entirely; the Windows and GTK apps, the CLI and Emacs still call the host directly
until their turn (the `legacy` tag is the last commit before the move).

- **The app never depends on the host.** It holds the snapshot the host last sent (`Watch`, a
  stream answered at once and on every change) and works out from it, with `lumenna-records`,
  everything that is a function of the store: the listings, the sidebar, the day's plan, the
  Blocks list, a task's details, a reminder list, completion and the quick-add preview, which
  fields a block form shows, what a pick offers (`records::choices`), What Should I Work On?,
  and Replan's and Plan the Day's proposals (`records::replan`, `records::dayplan`). Every
  write is a `HostRequest`, whose answer comes back as an event carrying the store as the
  write left it; applying a proposal is one, since it checks the day has not moved. What only
  the host can answer is asked of it the same way: settings (some are a device's own),
  devices, reminders due (a device's own setting and key), backups and exports.
- **The store's effects are Crux middleware** (`crates/bridge`, Crux pinned at 0.20.0, whose
  middleware is marked experimental): `Watching` (the `Watch` stream) and `Writing`
  (`HostRequest`) hand both to one `StoreWorker`, which answers them in order off the caller's
  thread, a thread of its own natively and a task on the event loop in a browser, and answers
  every watch again when a `Store::tick` finds the heads or the minute moved. What reaches other
  devices (`NetworkRequest`: syncing, pairing) is a third middleware, the platform's, since each
  syncs on its own runtime (`bridge::stack` takes it); a pairing answers its request more than
  once. A resolver must never be called inside `try_process_effect` (Crux asserts it): every
  middleware hands its work on through a channel. What is left for a shell is drawing, saying
  (`Shell::Announce`, and `Heading` for a day newly shown, in the shell's own words for the
  day), saving a file and showing a notification: each batch the app leaves later arrives
  through the layer's callback, and a shell draws it as it comes, never waiting on the thread
  it draws on. `Busy` counts the store's work under way, so a test can wait until the app is
  still: the flow tests' own `Session` waits on every call.
- **The person can move on before a write is answered**: a write's answer that comes after an
  `Event::Focus` the person made since they last acted leaves the row they moved to in hand
  (`Model::moves`, `acted_at`), rather than pulling them to the row the write would have put
  there.
- **What every app decided for itself is the app's now**: the row in hand after a change (the
  same row, else whatever holds its place among the rows shown); a place gone back to, which
  opens on the row in hand and the filter it was left with (`Model::left`), and the place shown
  opened again, which keeps both; folding, the sidebar's too
  (Expand and Collapse are row actions; a folded row comes marked `hidden`, for a tree control
  that folds by itself); the question open, a refusal staying in it, a pick's length asked
  once one is picked; where a place's action leads (renamed, made, deleted); a new saved
  filter's two questions; the day shown, Go to Day, and today opening on now; the details'
  draft, kept through a change elsewhere and saved as only what changed; the quick-add line
  and a completion spliced into it; the block form, asking which occurrences of a repeating
  block first; the proposal screens, checks, partial applies and other options; settings, a
  refused line kept beside why; pairing's code, words and a code typed while waiting; when to
  start syncing; reminders due and their buttons, an overrun's Replan dismissing it once
  something is replanned; and what to say.
- **A list's first row is in hand as soon as it opens**, as in the desktop apps' trees, so the
  details show it.
- **The view says when focus is the app's to move** (`ViewModel::landing`, a count): it moves
  when a turn wrote or moved the row in hand, and never for `Event::Focus`, the person's own
  move. A shell lands focus on the row in hand when the count goes up and leaves it alone
  otherwise, so focus never jumps because a sync or a minute redrew the list.
- **The lists beside the main one have their rows in hand too** (`held.rs`: the sidebar, a
  reminder list, the devices; `ViewModel::sidebar_held` and the like). A shell reports a move
  there with `Event::Hold(side, key)` (a sidebar row by its entry's `key`, a reminder by `id`, a
  device by `node_id`), and its row's actions are held there before they run. A change made
  from one lands there, on the same row or the one now in its place, and the main list's row
  moves on quietly; a place renamed, made or merged into from the sidebar takes the sidebar's
  row with it. The side list stays where changes land only while the person goes on acting from
  it (the action, its question, its answer); anything else they do hands landing back to the
  main list.
- **A shell follows the app's own moves between places**: when the app goes on from the place
  shown (a rename, a merge, a delete), the screen showing it goes too.
- **A shell's clock is the shell's**: it tells the app its clock (`Event::Clock`) for the
  sentences the core composes whole, and when to look for reminders and the moment it looks at.
  A shell whose clock is pinned for tests says the moment proposals are made against
  (`Event::PlanFrom`), which applying them carries to the host.
- **So are its capitals** (`Event::Capitals`): a shell that writes sentence case says so and
  the view comes cased (`capitals.rs`): the sidebar's headings, a question's buttons and
  answers, a question's title where it is its action's own name (else the core's, which keeps
  names), a proposal's words, a reminder's buttons, the export labels, the block form's button.
  An action carries both its `title` and `sentence`; a shell picks one by its platform.
- **A view's lines are worded once** (`lumenna_desktop::lines`, over the `speech` functions):
  each list row, the list's readback, the day's title and rows, a pick's choices, the devices,
  the quick-add offer, the details' state. A `Line` is its first part, the rest, and the two
  joined (`text`), for a list that shows two lines and one that reads one. The platform gives
  only its locale's formatting of a time and a date (`speech::Formats`); "Today", "Tomorrow",
  and when to give the year are `speech::Spoken`'s. The web's and Android's turns carry them,
  and a day newly shown is said as "<day>. <summary>" before it reaches either.
- Flow tests are `crates/bridge/tests/flows.rs`: events in, view models and sentences out,
  against a real store; a stand-in middleware takes the network's requests, and the test
  answers them. One test drives the core by hand, to hold a write's answer back.

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

The surface over JSON-RPC (`crates/host/src/rpc`, the `rpc` feature), for Emacs, and for the
BTSpeak tests to seed and check a store. `lum rpc` serves it on stdio to one client. Whichever process holds the
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
- **`act`, `choices`, `places` and `form.*`** (the form functions: `task_fields`,
  `task_edit`, `block_fields`, `day_block_fields`, `block_edit`, `new_block`,
  `block_defaults`, `priorities`, `not_offered`, `project_reference`, `label_reference`; `choices` with `from` is Put in a Block on another day) are here so a client over the pipe keeps no copy of any rule. A form
  function's answer is a `value` result: computed from what the client sent, nothing read.
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
  key not listed there. The ways in are a pairing whose words were confirmed on both sides
  (`edit::enroll_devices`), and the first sync over a Wear OS watch's link to its phone, which
  only Google's pairing of the two can open (`introduce`).
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
- **A local lookup ends** (`LISTEN`, three seconds): iroh reports a device's lookup only
  once every service has finished, and starts no other while one runs. Ours waiting for
  good, as it did in a browser, which hears no network, kept a failed relay lookup from
  ever being said, and no dial after it looked again: two browsers paired and never synced.
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

### Reminders

`core::remind::firings` works out every firing between two instants from the synced records
alone; each device asks of its own store and delivers its own, with no coordinator. The
surface's `reminders.rs` words them and answers the apps.

- **A civil time becomes an instant only in `remind::instant`**, by RFC 5545's rule: a time a
  daylight-saving change skips moves forward by the gap (2:30 becomes 3:30 on the spring
  day), a time that happens twice is the first. A block keeps its real length across a
  change: its end is its start plus its minutes. Expansion stays in UTC.
- **A target's own reminders replace the defaults** (`default_task_reminders`,
  `default_block_reminders`); a default fires under `remind::default_key`, derived from the
  target and trigger and fixed forever, so a dismissal of it syncs and holds everywhere.
- **What fires**: an open task at its due time or, date-only, the all-day hour; each block
  occurrence at its start or end, a cancelled one never; snoozed at the snooze's end;
  dismissed never. Windows are `(from, until]`, so checks in any steps fire each once (a
  test proves it across the spring change).
- **The text is the whole message**: "Deep work starts in 5 minutes — 3 tasks assigned, first
  is Review PR", the detail left out under terse announcements. `missed` is the same said of
  the past, for the one summary of what came while nothing ran (`STALE_MINS`).
- **One process per device delivers** (`take_reminders`, a lock on `reminders.lock`): a
  running app or `lum notify` holds it and calls `deliver_reminders`, which remembers when it
  last delivered (`reminders-delivered`, a device setting) and goes back at most a day; over
  RPC, `reminder.deliver` takes it for its connection. A
  system that schedules notifications itself asks `upcoming_reminders(until)` instead.
- **A reminder is a phrase the core reads** (`parse_trigger`, `trigger_phrase`): "5m before
  start", "at end", "30m before" (a task's due time needs no saying). Adding one is the
  reminder list's Add Reminder action, a Text question; the defaults are settings in the
  same phrases, separated by commas (`SettingKind::Line`). A task with a due date and every
  block offer Reminders (`Question::Form`: the app's list of `reminders_of`), whose own
  reminders each offer Remove Reminder. A notification's buttons are `DueReminder::dismiss`
  and `snooze`.
- **A system that schedules ahead also asks what it missed** (`reminders_missed`), on launch,
  on returning to the front and after a restart: iOS and watchOS keep what they were handed,
  so only what came past the window last handed over (`upcoming_reminders` records it) was
  missed (`kept`); Android and Wear OS lose their alarms on a restart, so they pass what they
  showed (`<key>@<occurrence>`) and the rest since the last ask was missed.
- **Who delivers, per client**: the resident desktop apps through `deliver_reminders`; iOS,
  watchOS, Android and Wear OS by scheduling `upcoming_reminders`; `lum notify` on a console;
  Emacs's `lumenna-reminders-mode` through `reminder.deliver` and BTSpeak's resident service
  (`reminders.py`, its unit and toggle in `system/`, installed only from its Settings, after
  asking) through `Device::deliver_reminders`, whose `elsewhere` says another process has it,
  so ask again soon. The
  web's one tab delivers through `reminders_due` with a "since" of its own, and keeps
  "Reminders on this device" in local storage: a browser has no device-settings file.
- **Dismissing and snoozing write a `ReminderAck`**, which syncs: best effort, since a sleeping
  device may still show it. "Reminders on this device" is a device setting and never syncs.

### Urgency and history

`core::urgency` scores a task, computed and never stored, after Taskwarrior's: additive
terms (due, rising and still rising while overdue to a ceiling; priority; age; partly
worked; blocking; blocked, strongly negative; already in a block today, mildly negative),
the sum **multiplied** by the project's weight, so the order within a project holds and only
the scale between projects changes. `core::history` counts what the records already hold:
sittings and time, how often a task was put in a block and left, its age, its completions.
`core::suggestions` ranks what a block is offered: the open tasks its filter admits, by
urgency, a task whose estimate fits the time left raised, nothing excluded for not fitting.
Assign a Task offers them in that order, each `Choice::reason` the sentence a person hears
("Suggested because overdue by 1 day, priority 1, and its estimate of 30 minutes fits the 35
minutes left"); Put in a Block offers only the blocks whose filter admits the task.
The day's own commands are `day.rs`, each the person's decision and one undoable change:
End Now and Add 15 Minutes on the block under way (a modified day of a repeating block, the
block itself for a one-off), End the Day and Clear the Rest of the Day (`Plan::actions`, on
today: later repeating blocks cancelled for today, one-offs to the trash), and Carry Forward
on a sitting left short, its task open, to a block whose filter admits it, the old sitting
marked Deferred. **A block's end offers it** (`remind::falls_short`, `Firing::carry`): the
first reminder at or after the end says it, or, with none, a firing of its own under
`carry_key`, armed by scheduling devices from the sittings as they stand and re-armed as
they change. Its words are the surface's `carry.rs` ("Essay: 40 minutes of 1 hour 30
minutes. Carry the remaining 50 minutes to Admin tomorrow?", overfill said), its button
`DueReminder::carry`, which runs `carry_rest_forward(<series>@<date>)`: every sitting to its
next block in one change; a block with more than one short sitting offers Carry the Rest
Forward too. **What Should I Work On?** (`whatnow.rs`, a day action and `lum next`) is a
Pick: in a block under way its own unfinished sittings, then its suggestions; in free time
the suggestions for the time until the next block. The answer starts on it at once, in the
block under way or a block of its own in the free time, its timer running, one change. The
day's Plan the Day and Replan from Now are `Question::Form`, the app's screens over
`propose_day_plan` and `replan`.
Re-flow is a proposal, never a change (`core::reflow`, the surface's `replan.rs`, `lum
replan`, RPC `replan`/`replan.apply`): the work moves first (a block's last sittings beyond
its time left go to the next block of the week that takes them, else back to the list), then
the walk from now, anchored and kept blocks fixed, a pushed block shrinking to its minimum
keeping its end, then moving later. Every change is a whole sentence in the clock the app
passes (`TimeStyle`), since for someone who cannot glance at a timeline the description is
the check. `apply_replan(proposal, accepted)` makes the changes accepted, refuses any the
day no longer matches, is one undoable change, and **proposes again** (`Replanned::next`):
a change taken alone can leave the day crowded, so what was not accepted becomes a pin (a
block kept where it is, a sitting kept in its block) and the day as it now is is proposed
afresh. A proposal carries its pins and clock, so a client hands it back whole. **Other
options** (`Replan::alternatives`, at most three) each let one block not yet over shorten
below its minimum (`Pin::Floor`, `ReplanPins::floors`: a work block to 15 minutes, a break to
half) or skip it, offered only where that does better (less unfit, then fewer minutes moved
and shrunk), shortening first as the cheaper, each saying its cost and what follows ("Let
Lunch shorten from 30 minutes to 15 minutes: 15 minutes less break. Then …"); taking one is
`replan` again with its `pins`. The minimum binds the main proposal and never an option. A
block pushed against an anchored one shrinks to the gap before it when its floor allows. **The overrun warning**
(`remind::overrun_at`, `Firing::overrun`, under `overrun_key`) is said before the clock says
it: when a block's planned work first outruns its time by more than the free time after it,
or `FEW_MINS`. Idle, that moment is fixed by the records; with a timer running, work and time
shrink together, so it outruns from the timer's start or never. Its `DueReminder::replan`
button opens Replan from Now. No block starts past midnight; a day planned past its window
runs to midnight. The Replan from Now day action waits until every app has its screen.
The whole-day plan (`core::dayplan`, the surface's `dayplan.rs`, `lum plan-day`, RPC
`day_plan`/`day_plan.apply`) walks the day's work blocks still to come and gives each the
tasks its suggestions rank first, greedily, until the estimates fill it: a task once a day,
none already in a block that day, none blocked, one without an estimate counted as 30
minutes and said so. What is due by then and fits nowhere is named. Applying it is one undo,
refused where a task was done or put in that block since, and plans again
(`DayPlanned::next`), each sitting turned down kept out of its block alone.
The surface words both (`task_history`, `TaskDetail::history`): asked for about one task,
never said in a list or a live region. The weights are a synced setting
(`Settings::urgency`, a key per weight so concurrent edits of different weights both hold),
said and read as one line, `urgency-weights` ("due 12, priority 6 3.9 1.8 0, …"). **Task
lists are most urgent first by default** (`Settings::task_order`, the synced `task-order`:
`urgency` or `added`): siblings by urgency, a subtask kept under its parent, the order added
breaking ties. Tasks have no order of their own to move up or down, so nothing is lost. A
list's query may end in `sort: urgency` or `sort: added` (`parse_query`, `Sort`) to choose
for itself. A block's filter selects and never orders, so `parse_filter`
refuses `sort:` rather than search for it.

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

`apps/btspeak/`: Python, on the device's own `dialogs` library, drawing the Crux app, which runs
in the same process beside the store: `crates/ffi` built by BoltFFI's Python target
(`build-core.sh`, `boltffi pack python`, needing Python 3.10 or later with pip) into `native/`,
not committed. The library is only on a BTSpeak (`/BTSpeak/Python/BTSpeak/`); elsewhere the
tests run on `tests/btspeak_stub.py`, with the core built and `target/debug` on `PATH` for
`lum`. On the Mac the system's Python is 3.9: build and test with a newer one (`PYTHON=…`).

- **A shell and nothing else.** No date parsing, no computed states, no folding of its own:
  each screen sends what the person did (`Session.send`, which waits until the app is still and
  says what it said) and draws `view()` with the core's `lines()`. Turns arrive on the app's
  thread (`core.py`) and queue; a menu wakes on them (`restless`), and what the app said that no
  action asked for is said as it settles. Seeing another process's writes is `tick`, every five
  seconds.
- **A row's context menu is its `actions`** (`actions.py`): the rows' actions merged into
  one list of commands that keeps each row's order, each offered where its row has it, a
  letter per kind (`KEYS`), the delete keys for `Delete`/`Unassign`/`Unpair`; a letter the
  row does not offer says the core's `not_offered`. Running one puts the row in hand (`Focus`,
  or `Hold` for a side list's row) and performs it; `follow` then answers whatever the app
  opens, one after another, with the device's dialogs: a question (a refusal staying in it), the
  block form, a proposal, a reminder list. The task form registers in `actions.FORMS`.
- **Speech and braille come from one string** with the stock library
  (`DynamicMenuDialog.draw()`), so components are flattened at the last step. A subclass is
  the fallback if they must diverge.
- **Depth is never indentation**: it says nothing in speech. The app says the level only where
  it *changes*, against the previous *visible* row, so folding keeps it right.
- **A menu reopens on the app's row in hand** (`live_menu`'s `focus`), and is drawn after it is
  built, the arrival flag cleared then: building sends (a place opened), and a flag left set
  drew every screen twice.
- **Completion is offered after the line is entered.** `InputDialog` has no hook and Tab is
  form navigation; inline needs a subclass.
- **`main()` pushes an app context** (`host.push_app_context("lumenna", self_voice=True)`,
  popped in a `finally`). The device takes its braille table from the app it believes is in
  front; without the push, BT Code's `.py` table made every menu computer braille. The push
  and pop are device-wide. `# blazie-flags: self-voice` on line 2 of `__main__.py` is for
  the menu launcher. With self-voice on, printing is silence, so startup errors are dialogs.
- **It is a line in `~/BTSpeak/user.menu`**, added with `user_menu.add_item`, not a `.menu`
  file.
- **The app syncs while it runs** (`App::in_front`), waiting its turn for the endpoint while the
  daemon holds it, and takes the automatic backups, hourly, as `lum rpc` did.
- **The day's own actions lead the Day menu**: `live_menu`'s `app` may be a function, read
  after `build`, so the menu offers what the plan shown offers (`day.day_commands`).
- **A choice's reason is its row's hint.** `request_choice` has no description, so a pick
  whose choices carry a `reason` is a `dynamic_menu` instead (`session.choose(hints=…)`),
  the device saying each hint after the row; a pick without reasons stays a choice dialog.
  Put in a Block adds Another day, which asks the core's Go to Day question and sends
  `PickFrom`: the app offers that day's blocks in the same pick.
- **Replan from now and Plan the day are one menu** (`proposals.py`) over the app's proposal:
  rows toggled by Enter with callable titles, so checking rewords a row in place, the title the
  whole proposal (said as it opens, not again after an apply, whose announcement already says
  the next). It is drawn again only for a new proposal or a refusal (`shown`).
  `proposals.NOW` fixes the moment for tests (`PlanFrom`, sent before every action).
- **Carry Forward at a block's end and Replan on an overrun are the Reminders screen's**
  (`f`, `p`, the app's due list): the service has no keys, and only says the text, which
  already ends in the question or says the overrun.
- **The tests seed and check the store over `lum rpc`** (`tests/client.py`, `harness.py`), as
  another process would, and `tick` the app before each script. They run in UTC: the core
  reads the zone once per process, so a test cannot set its own.

## The Emacs client

`apps/emacs/`: Elisp over `jsonrpc.el`, built-in libraries only. Tests are ERT against a
real `lum rpc`, answering the minibuffer by rebinding the reading functions:
`emacs --batch -Q -L apps/emacs -l apps/emacs/test/lumenna-test.el -f ert-run-tests-batch-and-exit`.

- **The socket, else a child.** `lumenna-profile-directory` repeats `lum`'s rule for the
  default profile, because the socket is found by path before there is anything to ask.
- **Depth is an outline level**: `outline-level` reads the `lumenna-level` text property,
  so folding and Emacspeak's level announcements come from outline mode.
- **A list's keys are defined once** (`lumenna-define-keys`), which binds them, builds the
  menu, and lists them for `?` in an ordinary buffer. A row's key runs the row's action of
  that kind (`lumenna-define-action`, `lumenna-act-kind`), its question asked in the
  minibuffer and answered through `act`; `.` offers them all by title, and a key the row
  does not offer refuses with `form.not_offered`'s sentence. Sounds key on the
  action's `subject/kind`. Not transient: Emacsvox reads a
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
- **The day's own actions are its heading's**: on the first line, which has no row, `.`
  offers the plan's `actions`; `P`, `r`, `w`, `E` and `C` run them from anywhere in the day.
- **A proposal is a buffer of checkable lines** (`lumenna-proposal.el`). Its source draws
  the proposal it holds and never asks again, since `lumenna-refresh-all` redraws it after
  every write and asking would lose the checks; `g` asks again, keeping the pins.
  `lumenna-proposal--now` fixes the moment for tests. Other options take the place of the
  proposal as a partial apply's `next` does. A reminder's `replan` button, and `r` in the
  due list, open Replan from Now.
- **A choice's reason is its completion annotation**, never part of the line chosen.

## The iOS app

`apps/apple/`: SwiftUI screens in a UIKit shell (the tab bar, the iPad's columns, navigation,
the keyboard's commands), drawing the Crux app through BoltFFI's Swift. What the screens show
is followed once for every Apple app in `Shared/Screens` (`TaskListModel`, `DayModel`,
`PairingModel`, the row actions' split `AppRowActions`, the Mac's `Outline` of nested rows),
and drawn by each platform its own way. UIKit stays for quick add, whose completion needs the
cursor's place, which a SwiftUI field gives only from iOS 18. `cd apps/apple && xcodegen` makes
the project from `project.yml`; the `.xcodeproj` and `Generated/` are not committed.

- **Every screen draws the app's view and sends it what the person did**, as Android and the
  web do. `AppModel` (`Shared/`) holds the app over its `Device`, takes each turn on the main
  thread in order and posts `AppModel.shown`; a screen draws `view` with the turn's `lines`,
  opens its place as it comes to the front unless the app shows it already, and moves
  VoiceOver only when `landing` went up. The app's questions are `AppQuestions`, asked through
  each platform's `QuestionAsking` at each window's root; what the app opens by itself (the
  block form, a proposal, a reminder list) is `AppSheets` (the watch's `WatchScreens`). A form
  takes the app's values only once the app is still (`AppModel.whenStill`): an answer for
  earlier typing would undo later typing, and the last turn's notice can come while a tick is
  under way.
- **The generated Swift is a framework of its own** (`LumennaApp`, one per platform, embedded
  in its app): BoltFFI emits a crate's exports only when it is the crate being built, so
  `crates/ffi` is the library, force-loaded into the framework, and the app, its tests and the
  framework share one core in the process. As a module of its own, its names (`Locale`,
  `Action`) do not shadow Foundation's; code names them `LumennaApp.Action`.
- **Xcode builds the core itself**: `build-core.sh` builds `lumenna-ffi` in a clean
  environment (Xcode's SDK variables break Cargo's host build scripts), the Mac's and the
  phone's with the `rpc` feature, then has BoltFFI generate the Swift (`Generated/App`, the
  watch's `Generated/Watch/App`). What it makes is named as the build phase's output, or a
  fresh checkout looks for the Swift before the phase has run. It must pass
  `IPHONEOS_DEPLOYMENT_TARGET` through, or C objects target the SDK's own version; heed that
  linker warning.
- **The core is linked statically, by the `.a`'s full path.** `-llumenna_ffi` picks the
  dylib Cargo builds beside it, which works in the simulator and crashes on a phone at
  launch. A post-build phase fails the build on any `liblumenna_ffi` reference. A launch is
  not proof it ran: check the process is alive, or the crash logs.
- **Iroh needs `SystemConfiguration` and `Network`** linked.
- **A row owns what VoiceOver says**: every list row sets its own label (the title) and
  value (the rest of the app's line, whether it is folded, and the level where it changes:
  `AppModel.spoken`), its children ignored; left alone, a row says its secondary text twice.
- **The screens are moving to SwiftUI, one at a time**, each judged by the audits, every
  test and what VoiceOver says, as the one before it was. The task list, Blocks (the same
  screen: no filter, a row's tap its Edit Block) and the day are done
  (`TaskListViewController` and `DayViewController`, each a `UIHostingController` over its
  SwiftUI screen and the shared `TaskListModel` or `DayModel`): their rows keep the UIKit
  lists' speech and action split (`AppRowActions`:
  Mark Done swiped from the leading edge, the other primary actions and folding from the
  trailing, the rest accessibility actions, all in the context menu), and VoiceOver goes to
  the row in hand through `@AccessibilityFocusState` only when `landing` went up. Every other
  list (Browse, the iPad's sidebar, a reminder list, Devices, Settings, a pick) is an
  `AppListViewController`, a UIKit controller holding the SwiftUI `AppListScreen`, its
  subclasses saying only what they list and where it is held. Hosted `Text` in a UIKit cell
  failed the audit for Dynamic Type and clipping, where a SwiftUI `List` passes. A SwiftUI
  row is a button in its cell, so a UI test finds a row by its title with `app.row`.
  A SwiftUI field's first responder is SwiftUI's own text view, its name on SwiftUI's element,
  so focus is given through `@FocusState` (`TaskListModel.filterWanted`), never by finding a
  view. A model takes the person's own move into `focus` at once (`moved(to:)`), so a key
  pressed straight after acts on that row, not the one the app's next turn has yet to replace.
- **Text entry wraps** (`LineEntry`): a one-line field scrolls sideways at large sizes and
  the audit flags it as clipped.
- **A SwiftUI `TextField`'s title is only a placeholder**: with text in it, VoiceOver never
  says the name. See `NamedRow` below.
- **Focus after a change is chosen**: the same row if still listed, else whatever holds its
  position; the core's announcement is queued behind the focus change so neither cuts the
  other off. This holds on every platform.
- **A row's primary actions are its swipe actions; the rest are its custom actions**, and
  all of them are in its long-press menu. A trailing swipe never runs its first action by
  itself (`allowsFullSwipe: false`): on the SE a test's swipe did, and the first can be Delete. UIKit offers swipe actions to VoiceOver, Switch
  Control and Full Keyboard Access and *adds* custom actions to them, so an action in both
  would be listed twice: each is in exactly one. Eight swipe actions did not fit on a phone
  row. A swipe action's title is its spoken name ("Mark Done", not "Done").
- **No bottom toolbars**: inside a tab bar the floating bar sits over a toolbar's buttons,
  so a tap on Undo landed on the Today tab. Rows scrolled under the glass tab bar fail
  contrast, so pushed forms set `hidesBottomBarWhenPushed`.
- **`TZ` is set from `TimeZone.current`** at launch and on returning to the foreground: jiff
  finds the zone through `TZ` or `/etc/localtime`, and the sandbox is no place to rely on
  the second.
- **`Core.changed` is what follows a change outside the screens** (reminders re-armed, the
  watch told): `Core` looks at the store's heads once a second (`Device.heads`, which move
  for the app's own writes too) and posts it, marked `arrived` unless the person did
  something in the last three seconds. The app ticks once a second itself, taking in what
  another process wrote.
- **Background sync** (`BackgroundSync`): a round in the time `beginBackgroundTask` grants
  on leaving, and a `BGAppRefreshTask` whenever iOS chooses. A refresh launches the app with
  no scene, so the `Core` is the app delegate's, one per process. Apple's simulated launch
  hung in the iOS 27 simulator; the handler has only run on a device.
- **Reminders are scheduled, not delivered** (`Shared/Reminders.swift`): `ReminderSchedule`
  hands `UNUserNotificationCenter` what `upcoming_reminders` says fires in the next 48 hours,
  the earliest 60 (iOS keeps 64), identified `reminder <key> <occurrence>` so re-arming
  replaces and what is no longer listed is cancelled. It re-arms at launch, on every
  `Core.changed` (the foreground's included), and in `BackgroundSync`'s leaving round and
  refresh, before the task ends. On launch, the foreground and a refresh it first catches up
  (`reminders_missed`, `kept: true`): what fired past the last window is shown at once, the
  older as one summary. Dismiss and Snooze are a category whose buttons are the
  core's words, answered by `ReminderResponder`, set as delegate before launch finishes.
  Permission is asked when the person opens a Reminders list, turns reminders on, or makes an
  edit that leaves something to fire, never at launch, and never under `LUMENNA_TEST_PROFILE`
  (the system's alert would cover the app under test). A sync's change notice carries
  `arrived`, so it is not taken for the person's edit. The scheduling is unit-tested through
  the `NotificationCentre` seam (`ReminderScheduleTests`); UI tests cannot post real ones.
  A firing with `carry` set is in a second category, its third button the core's Carry
  Forward (`carry_rest_forward` of `<target>@<occurrence>`). Setting categories replaces them
  all, so the carry one is set from the last carry reminder seen whenever any is handed over:
  one already showing keeps its button. An overrun warning (`replan` set) is a third, its
  button the core's Replan, `.foreground`: `ReminderResponder` hands on a `RemindedOf` with
  `replan`, and each app opens today with Replan from Now over it (`openReplan`).
- **The day's own actions** (`Plan.actions`) are a Day Actions menu button among the day's
  buttons (in the navigation bar it crowded the title, which the iPad's narrow column clipped)
  and the heading's custom actions, which make the heading at least 44 points tall: an element with
  actions fails the audit's hit area. Replan from Now and Plan the Day are one SwiftUI
  screen for all three apps (`Shared/Forms/AppProposalForm.swift`, the app's proposal): each change
  a toggle named by its sentence, its reason and the core's `item` its hint, and after a
  partial apply the core's `next` in its place until nothing is proposed. On iPad it is full
  screen: as a form sheet, the day's rows half under it failed the audit as cut-off text. A
  refusal is shown on the screen and the proposal asked for again with the same pins. The clock it asks in is
  `Clock.style`, the locale's. Its other options (`Replan.alternatives`) are buttons under
  `others`, each proposing again with its pins in place. A pick's `Choice.reason` is its second line and its hint.
- **A Reminders list is the app's** (`ViewModel.reminders`), opened by `AppSheets` when the
  app opens it and held in its own hand: on iOS an `AppListViewController`
  (`RemindersViewController`), whose Add Reminder in the bar asks the app's Text question,
  again with the reason when refused, and whose own reminders swipe Remove Reminder.
- **Pairing follows the app's pairing once for every Apple app** (`Shared/Screens/PairingModel`):
  the status, this device's code, and the three words, asked in the platform's own alert,
  whose safe answer, No, is the one a stray Return gives. One SwiftUI screen draws it for all
  three (`PairingScreen`), the iPhone's with SwiftUI's `PasteButton` beside the code field. Its
  frame is drawn 34 points tall at every control size, but Apple's hit-region audit, which
  knows the area a finger can hit, passes it, so the test asks the audit rather than the
  frame. UIKit's `UIPasteControl` hosted in SwiftUI got the tap and never pasted.
- **Stock controls unless there is a reason**, and each in `FormParts.swift` has one:
  `NamedRow` (the field carries the name and the visible name is hidden, as UIKit forms do;
  combining the row lost the text-field role), `example(_:)` placeholders and
  `FormParts.caption` (the system greys fail contrast). Give a control an empty title when
  it has an accessibility label, or VoiceOver says the name twice.
- **The tint is `UIColor.lumennaTint`** (system blue is about 4:1 on white), on the window
  **and with `.tint` on every SwiftUI form**, whose accent ignores the window's. Red text is
  `.warningLabel`. The colours have Increase Contrast variants.
- **The audits collect every issue and fail once** (`audit()`), a second after the screen
  settles and never while a sheet or alert is closing: under its fading dimming, black text on
  white failed contrast (`add` waits for quick add to close, as the Mac tests' `addTask` does,
  where a click while it closed was lost). A contrast finding is looked at again, once, two
  seconds on: a task's title fading in failed on CI's slower iPhone; text that fails at rest
  fails both looks. Exemptions, all narrow:
  "potentially inaccessible text" on `NamedRow` screens, the keyboard's
  `TUIPredictionViewCell`, contrast on text under a navigation bar or the soft scroll edge
  just beneath it (the line at the top of a SwiftUI list failed there on the SE and the iPad
  only), contrast the audit judges of a SwiftUI text on the iPad by its whole row's frame
  (`judgesAPadRowNotItsText`: the task list's count failed so in pure black), and "partially
  unsupported" Dynamic Type on SwiftUI nodes (the
  audit reports it even for a stock button), or with no element at all on a `NamedRow` screen,
  where it comes and goes on the iPad between runs of one build (likely a row's hidden name). To see an unnamed finding, run with
  `AUDIT_ATTACH=1` and `xcrun xcresulttool export attachments`.
- **Keyboard commands are the Mac's keys and names, once** (`KeyboardCommands.swift`): on
  iPad the menu bar (`AppDelegate.buildMenu`, with Find and Format removed, which would take
  ⌘F and ⌘B), on iPhone, which has none, `RootTabs.keyCommands`; never both, or a key has
  two commands. Each is a selector up the responder chain, so the screen in front answers
  before `RootTabs`. **Something must be first responder** or the chain is empty and the
  Task and Day menus reach nothing: each screen with commands calls `takeKeyboardCommands`
  as it appears, unless text is being edited. ⌘Z in a field with typing to undo undoes
  the typing. Row commands act on the row with keyboard focus, else VoiceOver's (found
  through its containers when VoiceOver's element is not a view, as the iPad's is:
  `UIAccessibility.voiceOverView`), else the one open beside the list; the task open beside a list answers them itself.
- **The iPad is the Mac's layout, not the iPhone's tabs** (`IPadRoot`, three columns): the
  app's places in a sidebar (`SidebarViewController`, from `ViewModel.sidebar`, plus
  Settings), the place chosen, and what was opened from it (`showBeside`, which pushes on
  iPhone and in a narrow window). The sidebar is an `AppListViewController`, so headings
  and project trees fold with Expand and Collapse actions and say which they are; a
  project's, label's or filter's actions are its own, the core's, as in Browse. A screen
  chosen by a command acts a turn later: it is not in the window until then.
- **The Day menu ends in the day's own actions**, one item per kind (`dayActionItems`), since
  the menu bar is built once: each runs the shown day's action of that kind, read again first,
  or says `notOffered`. From another place they go to today first.
- **⌘F never reaches the app in the iPad simulator**: no responder is asked about it, where
  ⌘3 is. Filter Tasks stays on ⌘F in the Edit menu, and its test skips on iPad.
- **Three layers of iOS tests.** `LumennaTests` (unit tests hosted in the app) asks each
  list for its swipe actions, custom actions and menu (`AppRowActions`) and runs them: what
  VoiceOver offers as a row's actions, with no gesture to land, so they block in CI. They seed
  the store through `core.app.device` (`newBlock`, in `Seeds.swift`, since BoltFFI's records
  have no defaults), tick the app to take it in, and wait on `core.app.isStill`.
  `VoiceOverUITests` turns VoiceOver on (`XCUIVoiceOverService`, iOS 27) and checks what it
  says, acting through the keyboard commands, which act on VoiceOver's row; its output
  carries a hint's first word only ("Actions"). VO-Command-Down Arrow steps the actions
  rotor, so a test reads a row's actions as VoiceOver lists them; what it says last after a
  change is the announcement, so a test of focus listens while it waits. The rest of the UI tests
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
- **A UI test names a fresh store** through `LUMENNA_TEST_PROFILE`, and turns animations off
  (`LUMENNA_NO_ANIMATIONS`): under load, something left animating as the iPad's sidebar hid had
  XCUITest wait a minute a step for an idle app, and the audit judged text still fading in.
- **Run UI tests with `-collect-test-diagnostics never`**, or any failure hangs xcodebuild
  for ten minutes collecting simulator diagnostics:

  ```
  cd apps/apple && xcodebuild -project Lumenna.xcodeproj -scheme Lumenna \
    -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
    -collect-test-diagnostics never test
  ```

## The macOS app

`apps/apple/Mac/`: SwiftUI screens in an AppKit window (the split view, the menus, sheets,
alerts), AppKit beneath each list's outline, which is where its focus, clicks and keys are
taken. AppKit stays for quick add, as on the iPhone, and for the pick sheet, whose search field
keeps focus while the arrows move through the list and say each choice. `Shared/` holds what
the Apple apps compile, including **`Shared/Forms/`, the task, block and settings forms**, and
`Shared/Screens`, what the screens follow; each app supplies only presentation. Where the platforms need different answers it is an
`#if os` in `FormParts.swift` (`Named`, `Labelled`, `ChoiceSection`) or at the one row
that differs, so a fix to a form lands on both. Scheme `LumennaMac`.

- **Not sandboxed, and shares `lum`'s profile**: the app and the command line on one Mac
  are one device with one store. The app ticks once a second, taking in what `lum` wrote.
- **The window follows the app**: the middle pane shows the app's place, so a place renamed,
  merged or deleted takes the window with it, and the right pane the details of the task in
  hand, which follow the row in hand. Every list is an outline (or a table) over the app's
  rows: the selection is the row in hand, scrolled to only when the app moved it, and folding
  there is the row's own Expand or Collapse. The task list is SwiftUI's `List`, rows nested in
  disclosure groups, which is AppKit's outline underneath, reporting levels and expansion
  itself; so are Blocks (the same screen, its Return and double click Edit Block), the day
  and the sidebar (`SidebarScreen`, whose headings keep the heading trait and are never
  selected, and whose rows' context menus are their own, a heading's included). The rest are
  still AppKit tables, moving to SwiftUI one at a time.
  **A SwiftUI list takes focus through its own `NSOutlineView`** (`preferredFirstResponder`,
  for F6): the window gives it to the middle pane's list once SwiftUI has made it, as it first
  opens, since SwiftUI's `@FocusState` asked then went nowhere and AppKit's own first choice
  was the sidebar; a list coming into the pane takes it only when focus is nowhere
  (`takeFocusIfNowhere`), or arrowing through the sidebar, which opens each place, pulled
  focus out of it at the first place. A click anywhere on a list gives it focus as the mouse
  goes down (`FocusOnClick`, an event monitor): a click on the row already selected did not,
  and a recognizer's, at the click's end, came after a key pressed straight away. Space,
  Delete and Return are taken the same way (`ListKeys`) while the outline or a view in it is
  first responder, never by SwiftUI's `onKeyPress`, which waits on SwiftUI's focus, a moment
  behind the first responder.
- **Resident in the menu bar, and the device's sync process.** No daemon is needed or used:
  the app's network middleware syncs for as long as it runs and, while it holds the endpoint,
  serves the command surface at the profile's address (`App.serveCommands`, the bridge's `rpc`
  feature), as the daemon would. `LUMENNA_NO_SYNC` (the UI tests') keeps it from both.
- **The resident app delivers reminders** (`ReminderDelivery`): while it holds
  `take_reminders`, `deliver_reminders` every 20 seconds and on becoming active, each shown at
  once through `UNUserNotificationCenter` with the iPhone's category, the missed summary as one
  of its own. Without notification permission it lets the right go, for a process that can say
  them. The Reminders list is the shared SwiftUI form (`Shared/Forms/ReminderList.swift`) in a
  sheet, shared with the watch: its Add Reminder answers the app's question itself, a refusal
  kept in its field with the app's reason.
- **The Day menu ends in the day's own actions**, rebuilt as it opens (`MainMenu.DayMenu`):
  the day shown's, else today's, which goes to the day first; also a Day Actions menu beside
  the day's buttons. Plan the Day and Replan from Now have Command-Option-P and -R (`DayKeys`,
  the iPad's too), answered by `DayMenu.menuHasKeyEquivalent` with the menu closed; that
  stops AppKit looking through the menu's items, so it matches the fixed ones (Command-[)
  itself. The proposal screen is a `HostedForm` sheet with its own Cancel and default
  button. A picker sheet's reason is its row's second line and VoiceOver help.
- **Global shortcuts through Carbon's `RegisterEventHotKey`**, which needs no accessibility
  permission. Never Control-Option: it is VoiceOver's. The recorder warns of clashes with
  VoiceOver, VOCR (Control-Command S, P, L, I; Shift-Control-Command more) and macOS
  (Control-Command Q, F, D, Space).
- **The core has a record called `Timer`**, so Foundation's is named in full.
- **Every stepper says what it stands for** (`SpokenStepper`: the block's length, a number
  setting): AppKit's and SwiftUI's say their number as a percentage of the range ("9.1%"), and
  SwiftUI's `accessibilityValue` on one is ignored. An `NSStepper` subclass overrides
  `accessibilityValue`, posts the change and names itself. Never inside a SwiftUI
  accessibility group or `LabeledContent` with only hidden text beside it: SwiftUI does not
  count an AppKit control as its group's child, and the audit finds the mismatch.
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
  missing" (and a menu button's: the day's Day Actions, which AXPress opens), a "Parent/Child
  mismatch" with no element, and a contrast finding with no element whose text is in the
  dimmed window behind an open sheet and not in the sheet.
- UI tests need macOS to have authorized UI automation for Xcode once.
- **What VoiceOver says can be heard without listening**: with "Allow VoiceOver to be
  controlled with AppleScript" on, `tell application "VoiceOver" to return content of last
  phrase` gives the last thing it said. Interact with a list by VO-Shift-Down
  (`key code 125 using {control down, option down, shift down}` through System Events), then
  the arrows step its rows. The accessibility tree itself (roles, names, values, a row's
  `AXDisclosureLevel`) is read with `AXUIElementCopyAttributeValue`, which needs the terminal
  to have Accessibility permission. The tree alone does not say what VoiceOver will speak: a
  row whose element is static text was said by its value, without its level or expansion.
- **A SwiftUI list row is a group named by its whole line** (`MacTaskListScreen`): as static
  text VoiceOver said its value alone and no level or expansion. The check box is a pointer's
  alone, as the web's; a disclosure group's label, which SwiftUI makes a heading, has the
  heading taken off; the hosting view steps out of the tree (`FlatHostingView`), or it is a
  nameless group the audit finds. A row holding one text is read by that text, not the
  row's name, so the sidebar's text says the whole line itself (`accessibilityRepresentation`),
  with its folding and level in words, as the iPad's (`AppModel.sidebarItems`): hidden, the row
  was said with "group" after it. No tooltip on a row: VoiceOver read it as the row's help,
  saying the count twice. A selection turned off on a disclosure group is off for everything
  in it, so a heading's is turned off on its label. Tests find a sidebar row by its
  identifier, the title.

## The Apple Watch app

`apps/apple/Watch/`: SwiftUI, embedded in the iPhone app. Scheme `LumennaWatch`.

- **No Iroh on the watch.** watchOS allows low-level networking only inside an audio session
  (TN3135), and that includes sockets and even `URLSessionWebSocketTask`; the simulator does
  not enforce it, so a build that syncs there proves nothing. The core is built without
  `lumenna-ffi`'s `network` feature (the app's network a stand-in, `offline.rs`, saying the
  watch syncs through its phone), with bindings of its own (`Generated/Watch/App`, generated
  in `target/watch-host`).
- **A whole store, synced through the phone** (`Device.linkStart` and `linkTake` on the
  watch, `linkAnswer` on the phone: `lumenna-sync`'s `exchange`): every document in each
  message, over WatchConnectivity's message and reply. The watch starts every exchange, since
  its message wakes the iPhone app and the reverse needs the watch app open, and starts one
  whenever its store's heads move; the phone only nudges an open watch app when its store
  changes. Each answers on its app's own connection, so the phone's sync carries what the
  watch wrote to every other device. The watch is not in the device list: trust is the
  system's pairing of watch and phone.
- **The watch schedules its own reminders**, from its own store, as the iPhone does
  (`ReminderSchedule`, re-armed at launch, on becoming active, after each change and on a
  background refresh every six hours). A phone's notification goes to the watch when the phone
  is locked and the watch worn, so each firing would show twice. Apple's answer (DTS, forums
  thread 765669): watchOS waits for the phone's notification to judge whether the two are one,
  and they are one when their `UNNotificationRequest.identifier`s match and they fire at the
  same moment; without a phone's, the watch shows its own after about 13 seconds. So the
  identifier is the firing's (key and occurrence, the same on every device) and the trigger a
  calendar one at the core's moment on both. Unverified on hardware.
- **The phone's forms are the watch's** (`Shared/Forms`: task, block, planning settings),
  with its own answers beside the others' in `FormParts.swift`: the iPhone's wherever its
  SwiftUI behaves the same, else the name always above the field and Save in the form. So
  `WatchCore` is the forms' `Core` (a typealias), its `app` the same `AppModel`, and the watch
  has an `Announcer`. Backups and exports stay on the phone: a watch cannot save or pick a
  file. `Shared/Clock.swift` and `Words.swift` are shared too; the watch's core is its own.
- **The watch's lists and day follow the phone's models** (`TaskListModel`, `DayModel`): what
  is listed, the row in hand put back on returning, the filter, and VoiceOver taken to a row
  the app moved, its swipes split by the phone's `AppRowActions`.
- **A row's actions are the app's, run once for every Apple app** (`AppModel.run`: the row in
  hand, or held in a side list, then the action), their questions `AppQuestions`' (alerts and
  a `ChoicePicker` on iOS, sheets on the Mac and, one at a time, a `WatchAsker`'s on the
  watch, each sheet's closing without an answer its own cancel). Keys and menu commands run
  the row's action of that kind, and say `notOffered` when it has none; on the Mac a menu
  item that asks ends in "…". Settings screens and the priority picker are built from the
  core's descriptions (`Setting.kind`, `priorities()`). Folding is the app's, by each row's
  Expand and Collapse.
- **The device list builds without Iroh** (`crates/host/src/devices.rs`): the watch is in
  no list and syncs with no device directly, so nothing is "this device" and a device's
  status says only its version (`device_status(…, direct: false)`).
- **A row offers its actions when tapped** where the phone has more swipe actions than a
  watch row holds (a block, a sitting, free time); a project, label or filter's are buttons
  at the foot of its list. The day's heading, tapped, offers the day's own actions, Plan the
  Day and Replan from Now opening the shared proposal screen in the asker's sheet. Completion
  is offered after the line is entered, beside the
  field, as BTSpeak offers it: the system's input screen takes no suggestions.
- **A text field is named explicitly** (`accessibilityLabel`): with text in it, its title
  gives way to the text.
- **A row owns its speech**: the title as its name and the rest as its value, set inside the
  `NavigationLink`; set on the link, watchOS made the whole line the name.
- **UI tests launch with `LUMENNA_NO_SYNC`**: a simulated watch is paired with a simulated
  phone, whose store otherwise arrives. A watch list holds only the rows on screen, so tests
  scroll to what they look for: by the Digital Crown, a little at a time, in a list, where a
  swipe moved past it; by swipes in a form, where the Crown goes to the control in focus. A text field opens the system's input screen: type into its text view, then Done. A
  place's row is named with what is in it ("Inbox, no tasks").
- **watchOS 27 and later**, the first without 32-bit watches, so the core is built for
  `aarch64-apple-watchos` on stable and `arm64_32` (tier 3, nightly) never.

## The Windows app

`apps/windows/`: Rust over `windows-rs`, linking `lumenna-host` directly, binary
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
- **A row's actions are offered and asked in one place** (`win/actions.rs`): the context
  menus, the Task menu, the details buttons and Delete, Space and Enter all take the row's
  core actions by kind. Only the forms are the app's: the details pane, the block form, a
  new saved filter.
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
- **A list item's reason is its description, set twice** (`a11y::set_item_description`, on
  the item's child id: `TVM_MAPHTREEITEMTOACCID` in a tree, index plus one in a list box):
  as MSAA's `PROPID_ACC_DESCRIPTION`, which NVDA and JAWS read, and as UI Automation's
  `HelpText`, which Narrator reads. UIA's proxy does not carry the MSAA one over. A pick's
  `Choice::reason` and a proposal's sitting reasons go this way.
- **Reminders are toasts** (`toast.rs`), under an app identity of their own
  (`LUMENNA_APP_ID` for tests), their buttons the core's: Carry Forward and Replan where a
  firing has them, then Dismiss and Snooze. Tests can read a shown toast's XML from the
  notification history but cannot press its buttons, so what each button does is checked by
  hand.
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

`apps/gtk/`: Rust over `gtk4-rs`, linking `lumenna-host` directly, binary
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
- **A row's actions are the core's**, run by `actions.rs`: it asks each `Question` in GTK's
  dialogs and answers only `Question::Form` itself. Space, Delete and Enter run a row's
  action of a fixed set of kinds (`actions::SPACE`, `DELETE`, `ENTER`); the Task menu's
  commands run the in-hand task's action of their kind, so Mark Done toggles. A settings
  page runs them through `run_from`, over its own window and status line.
- **A dialog's buttons are GNOME's order**: Cancel first and the default, the answer last
  and a verb (`Question::Text.yes`, a pick's action title, a form's Save), never OK. A
  yes-or-no question puts the safe answer where Cancel would be (`prompts::yes_or_no`).
  Alerts are `AdwAlertDialog`, a destructive answer drawn as one. It lays its answers side by
  side or stacked as they fit, and Tab follows the layout, so tests `tab_to` a button by name.
  Its message is a nameless Tab stop ("scroll pane") that libadwaita makes focusable once
  shown; making it unfocusable lost focus altogether, so it stays.
- **libadwaita 1.5 is the floor** (Ubuntu 24.04; Debian 13 has 1.7), for `AdwAlertDialog` and
  `AdwAboutDialog`. Tried and not adopted, by what Orca said: `AdwShortcutsDialog` (1.8) read
  its items by title alone, never their keys, so the keyboard help (`help.rs`, from
  `lumenna_desktop::keys`) is a tree of ours; `AdwPreferencesDialog` dropped each entry row's
  hint, opened a page only on Space, and did not say which page opened; a toast is announced
  as "A toast appeared: …" beside the status line. `AdwApplicationWindow` does not show a menu
  bar, which is how commands are found here.
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
- **The sidebar's places are the surface's** (`surface::places`, `Lumenna.places`, the
  app's `ViewModel.sidebar`), shared by Windows, GTK, the web, the Mac, the iPad and Android.
- **A sidebar entry carries its `title` and `detail`** beside its one line (`text`), so a sidebar
  that shows two lines never cuts the line apart; `SidebarEntry::key_of` names a row by its kind
  alone.
- **A short list is a tree sized to its rows** (`Tree::fit`), hidden when empty. A list box
  in a scroller made the scroller a nameless Tab stop, and making the scroller
  non-focusable broke Tab for the whole page.
- **Dialogs are futures** (GTK 4 has no blocking `run`). While a popover menu is open,
  `window::spawn` waits for it to close, so a dialog is not mapped under a closing menu.
- **A keyboard context menu opens when its keys are let go**: a popover opened on
  Shift+F10's press is closed by the releases.
- **Reminders are `GNotification`s** (`reminders.rs`), so GNOME Shell shows them and Orca
  reads them as it reads any other; their buttons and opening are application actions
  (`app.reminder-dismiss`, `-snooze`, `-open`), which reach the app with its window closed.
  It asks for `take_reminders` every five seconds until it has them, then delivers every 20.
  The UI tests stand in for the shell with a notification server on the private bus
  (`harness.Notifications`), started before the app, which picks its backend at the first
  notification.
- **The day's own actions are its heading row's** (the summary, `Plan::actions`) and the Day
  menu's. Replan from Now and Plan the Day are one screen (`proposal.rs`): a check box per
  change, the core's sentence as its label and a sitting's reason as its description, what
  fits nowhere as read-only lines under a heading, the proposal said once the window is in
  front. Applying shows the core's `next` until it proposes nothing. `LUMENNA_NOW` (a local date and
  time, `clock::pinned_now`) pins the now they and reminder delivery pass the core, for
  tests of the rest of a day at any hour.
  Replan's other options are buttons named by their sentences, under their heading; one
  proposes again with its pins, in place. An overrun warning's Replan button
  (`app.reminder-replan`) brings the window up on Replan from Now. A tree row's
  `description` (`Item::description`) is how a choice's reason reaches Orca, after its name.
- **The core words a whole sentence in `Locale::style()`**: the desktop's clock setting,
  else whether the locale's own time says 13:00 as 13.
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

`apps/android/`: Kotlin and Jetpack Compose over the Crux app (`crates/ffi`), through
BoltFFI's Kotlin bindings. Its README gives the toolchain. Three modules: `app` (the phone),
`wear` (Wear OS) and `shared`.

- **Every screen draws the app's view model and sends it what the person did**
  (`ShellEvent`), as the web does; UniFFI and JNA are gone from Android. One library
  (`liblumenna-ffi.so`) and its Kotlin (`io.github.emassey0135.lumenna.app`) come from
  `build-core.sh` (`boltffi pack android`, an overlay pointing its outputs into the build
  directory). What runs without the screens — reminders' alarms and receivers, `SyncWorker`,
  the watch's link, backups — calls `Device` (`crates/ffi/src/device.rs`), the host's
  operations of the same names over the one connection the `App` runs on; so do the tests, to
  seed and check the store. BoltFFI's records have no defaults, so the tests seed blocks
  through `newBlock` (`Seeds.kt`). BoltFFI carries the records typed, by its own encoding, so
  serde's shapes are no concern; `#[boltffi::data]` marks every record the view reaches.
  Its scanner does not see a marker inside `cfg_attr`, so it is unconditional, and it resolves a
  type of another crate only by that crate's root path or an import, never a path into its
  modules. BoltFFI cannot leave a variant out, so the shell's events are `ShellEvent`, the
  app's `Event` less what its own effects answer; an exhaustive match each way sorts a new one.
  The network is the bridge's native middleware (`lumenna_bridge::network`, the `network`
  feature): each request on a thread over the host's blocking sync, syncing only while the app
  is in front (`App::in_front`), which is the platform's to say. Turns reach the Kotlin `Shell`
  from one thread of the app's, in order, what arrived together as one.
- **`shared` holds the core and what decides nothing about looks**: the core's build and
  bindings, `Core` (the `Device` and, over it, the app), `AppModel` (turns taken on the main
  thread in order, speech held while a list lands, the turn's `lines`), `Words.kt` (the
  locale's time and date, handed to the core as `Clock.locale`), `AppWords.kt` (only what a
  tree control would say by itself and a Compose list does not: folding and the level),
  `RowAction`, landing on a row (`Landing.kt`: `rememberLanding`, `RowFocus`, `RowKey`, given the
  list's own scroll), a screen on what the app has open (`Showing.kt`: a proposal, a reminder
  list, the block form, gone when the app lets it go and given up when left), the forms'
  fields by the core's keys (`FormFields.kt`), asking for notifications, reminders' alarms,
  the watch's link and `SyncWorker` (through
  `CoreHolder`, which each app's Application is). What is in hand, what is folded, which
  question is open, the forms, settings, pairing and proposals are the app's; each app draws
  them, the questions once at its root (`AppQuestions`, `WearQuestions`).
  A shared file must not be named like one of the phone's in the same package: both
  compile to one `…Kt` class, and the dex keeps only one (a shared `Settings.kt` hid the
  phone's `SettingsScreen`).

- **Gradle builds the core itself**: `buildCore<Variant>` runs `build-core.sh`, whose
  `boltffi pack android` builds one library per ABI with its JNI glue compiled in; only the
  Kotlin beside it is copied. The phone takes Arm64 and x86_64 (Googlebooks have both, and
  CI's emulator is x86_64), the watch 32-bit ARM too (`abiFilters`); minSdk 28. Every library
  is 16 KB aligned, which CI checks. `LUMENNA_ANDROID_ABIS` narrows the build.
- **Espresso is pinned to 3.7**: Compose's test library brings 3.5, which calls an
  `InputManager` method Android 17 removed.
- **The tests run Google's accessibility checks on every interaction**, on a store of their
  own (`Core(directory)`). Buttons scroll into view before a click: a click on an
  off-screen node lands on nothing.
- **The tests wait on the app as Compose waits on itself** (`stillApp`, an idling resource
  over `Core.still`: `App::is_still`, nothing with the store and every turn handed on, and no
  turn posted to the main thread undrawn). A turn comes from the app's own thread, so without
  it a test looked at the screen between pressing Add and the row arriving. A pairing or a sync
  under way does not count.
- **Material's text and outlined buttons are 40dp**, and a radio row is as tall as its
  padding: both failed the touch-target check. `Target` gives 48dp.
- **A row is one node** (`ListRow`: `clearAndSetSemantics`, custom actions) with one
  description, the title first. As a separate state description, the rest was said before
  the title. `RowTitle` carries the title alone, for tests to find a row by.
- **Nested lists fold, as the app folds them**: everything starts expanded, and a row with
  something under it has the app's Collapse or Expand among its actions and says which it
  is. The level is the app's, said against the row shown before, after folding. The iPhone
  does the same, as a swipe action.
- **TalkBack does not follow input focus under touch**, on the phone or the watch. `RowFocus`
  gives the row input focus,
  then performs the accessibility-focus action through the view's
  `AccessibilityNodeProvider`, found by the `RowKey` property; the announcement is held
  (`Core.hold`/`release`) until then. The test of it is skipped unless TalkBack is on.
  Compose sends accessibility events only while a screen reader runs.
- **Keyboard commands are the Windows and GTK apps' keys**, taken from the core's shared table
  (`keyboard_shortcuts()`, `lumenna_desktop::keys`) by each command's id, keys and name both;
  only the folding arrows are Android's own (`Shortcuts.kt`). On Ctrl:
  TalkBack's are on Alt or Search. A screen offers a command while shown (`Offer`), the
  newest winning, and the system's shortcuts helper (Meta+/) lists what is offered now. A
  row's own commands (Ctrl+K, Delete, Shift+F10) run its action of that `ActionKind`
  (`Command.rowAction`), so a key never does what the row's action list does not; on a row
  without one it says the core's reason (`notOffered`, the subject from the row's own
  actions). Compose's root takes the keys while
  anything has focus; `MainActivity.dispatchKeyEvent` takes them when nothing does.
- **Buttons take focus only out of touch mode**, which a key press leaves; the keyboard
  tests leave it first (`setInTouchMode(false)`), or no tab could be focused.
- **A wide window has the desktop apps' sidebar** (`Sidebar.kt`, from `Lumenna.places`,
  plus Settings) from 840dp, with a stack of its own for the place chosen; headings and
  project trees fold with Expand and Collapse actions, and a heading's or place's actions
  are the app's, run as Browse runs them. Between 600 and 840dp the tabs
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
  never point it at a device whose own data matters. It first sets the device up as a
  person's would be (provisioned, screen kept on) and waits for the same ordinary activity to
  stay in front: on CI's fresh watch, something took the foreground part way through the
  tests once a run, and that test found "No compose hierarchies".
- **The store is in no-backup storage** and `allowBackup` is off: Google's backup would copy
  a store that reaches other devices by pairing.
- **Background sync is WorkManager** (`SyncWorker`): a round on leaving (expedited on
  Android 12 and later; earlier, expedited work needs a foreground notification) and every
  15 minutes. Discovery hears multicast only under a `MulticastLock`, held while syncing or
  pairing; it is not exclusive and `mdns-sd` binds 5353 with `SO_REUSEPORT`, so other apps'
  mDNS is unaffected. The emulator's NAT passes no multicast.
- **Reminders are AlarmManager alarms** (`shared/Reminders.kt`, for the phone and the watch):
  the next 48 hours of `upcoming_reminders`, at most 100, each a broadcast whose data is its
  firing (`key@occurrence`), so re-arming replaces it; what was set and is no longer listed is
  cancelled. Armed when the store changes (`Reminders.follow`, which the app's return to the
  front moves too, since it says the store changed), after `SyncWorker`'s round, on boot, an
  update, a clock or zone change and exact alarms being granted (`ReminderRearmReceiver`), and
  by an alarm a day on. `ReminderSchedule` over an `Alarms` is the seam the tests check.
- **Exact where the person allows it**: `SCHEDULE_EXACT_ALARM`, not `USE_EXACT_ALARM`, which
  Google Play keeps for alarm clocks and calendars. Refused, alarms are inexact and the
  Reminders settings page says so, with the system's page that grants it. Revoking exact
  alarms kills the app, tests and all, so the page reads what is allowed through
  `LocalReminderAccess`, which tests set.
- **An alarm going off asks the core again** (`reminders_due` over that one moment): a firing
  dismissed or done elsewhere since it was armed is not shown, and one Android held back past
  the core's few minutes is shown in its `missed` words. Each arming also takes away shown
  notifications dealt with since on another device, but only its own store's (a
  notification names its store in its extras): the app's store arming took away a test
  store's. Posting is asynchronous, so tests judge what `fire` showed, not the manager's list. Dismiss and Snooze are `ReminderReceiver`,
  not exported. A notification is a reminder's category, so Do Not Disturb decides.
- **Notifications are asked for once** (Android 13 and later), when the person first adds a
  reminder or turns reminders on; after that the Reminders page says they are off and opens
  the system's page. Tests grant the permission first, or the system's question covers the app.
- **Carry Forward on a reminder** (`DueReminder.carry`, sentence case like Dismiss and Snooze)
  is `ReminderReceiver`'s too: it runs `carryRestForward("<target>@<occurrence>")` and says the
  change in a quiet notification of its own (tag `carried`, no `@`, so arming does not take it
  for a firing), since the app may not be open to say it.
- **The planner's screens are the app's proposal** (`Proposal.kt` in each app): Replan from
  Now and Plan the Day, which the day's own actions (`Plan.actions`, on the day's heading as
  custom actions and as buttons) open. The proposal is said on opening; after a partial apply
  the core's `next` is the screen's list until it is empty. `LocalPlannerNow` fixes the
  moment proposed against, so the tests' proposals do not depend on when they run. A pick's
  `Choice.reason` is `Option.reason`, a line of its own beneath the name. A replan's other
  options are buttons under the core's `others`, each proposing again in place (`take`).
- **An overrun's Replan button** (`DueReminder.replan`) opens the activity, not the receiver,
  with `ReminderOpened.kind` `replan`: the app goes to Replan from Now for today. A button that
  opens an activity does not cancel its notification, so `Reminders.opened(context, intent)`
  takes it away.

## The Wear OS app

`apps/android/wear/`: Compose for Wear OS over `shared`, the phone's application identifier,
standalone. A full peer: Wear OS allows sockets, so it runs Iroh and pairs and syncs as the
phone does.

- **No text field on a watch**: every line comes from the system's input screen
  (`RemoteInput`, through `TextEntry`), which replaces a whole line; quick add changes or adds
  to it, with completions for its last word beneath. Tests provide their own `TextEntry`.
- **`AppScaffold` shows the time** at the top of every screen, as Wear OS asks; each list's
  `ScreenScaffold` moves it aside as it scrolls. A row's primary actions come first in its
  custom actions and its long-press list.
- **Rows keep full size at the screen's edges** (`edgeScale = 1f`): scaled, a button fell
  under 48dp and failed the touch-target check.
- **A row's actions are TalkBack's custom actions and a long press**; the day's rows offer
  theirs when tapped too. Announcements are a 1dp polite live region, never shown: drawn
  over the list they hid the rows beneath, and the checks found a button a third visible.
- **The watch's own link to its phone is preferred to Iroh while they are near**
  (`shared/WatchLink.kt`): the Wearable Data Layer's `ChannelClient`, a stream over Bluetooth
  or the local network. Only nodes Google reports `isNearby`: further off the Data Layer can
  route through Google's servers, and what syncs is the store. Each app's `LinkService`
  answers a channel the other opens, so neither need be open; each syncs over it on opening
  and two seconds after its store last changed. Iroh stays for the two apart and for every
  other device, and its round counts a device the link has just synced as synced
  (`Store::note_linked`).
- **The first link pairs the phone and the watch without words** (`lumenna_sync::introduce`,
  `Lumenna::sync_over_link`): the Data Layer joins only a phone and its paired watch, between
  apps with the same package and signing key. Both are enrolled, and the watch joins every
  device's list. Each side remembers, in `local_state`, that it introduced itself, so a watch
  unpaired since is not enrolled again: the link refuses on both sides.
- **A firing is shown once on the wrist.** The watch gives its own store's reminders, as the
  phone does (`shared/Reminders.kt`), and turns off bridging of the phone's reminder
  notifications only (`BridgingManager`, excluding `Reminders.BRIDGE_TAG`, which each carries).
  Not the phone's local-only flag: with that, a watch without this app would get no
  reminders at all. A dismissal on either reaches the other at the next sync, which takes
  its copy away.
- **A row's custom action that opens a screen runs on the main thread in tests**
  (`actOnMain`): Wear's navigation refuses the test thread, where TalkBack never calls it.
- The watch says it runs `wearos` (`PLATFORM`), in pairing and over the link.
- **32-bit ARM too** (`armeabi-v7a`): many Wear OS watches run a 32-bit Android. The phone's
  APK leaves it out.
- Tests (`WearTest`) run on a Wear OS emulator with the accessibility checks, as the phone's;
  `APP=wear ./run-instrumented-tests.sh`. A fresh Wear emulator opens on its setup screen:
  `settings put global device_provisioned 1` and `secure user_setup_complete 1`.

## The web client

`apps/web/`: TypeScript, React and React Aria Components over the core compiled to
`wasm32-unknown-unknown` (`apps/web/core`, crate `lumenna-web`), exported through BoltFFI as
Android's is. `npm run core:dev` (`scripts/build-core.mjs`) has BoltFFI build it, with the
web's own Cargo profile through `cargo_args`, and pack it into `src/core/` (not committed): the
module, its loader, the wasm-bindgen half its dependencies need (BoltFFI runs a wasm-bindgen CLI
matching `Cargo.lock`'s, building one if none is on `PATH`), and the TypeScript types, which
need `@boltffi/runtime` at the crates' version. wasm-opt is off: the `web` profile already
builds for size. Tests are Playwright over the accessibility tree with axe:
`npm test`; `LUMENNA_NETWORK=1` adds pairing over n0's relays, `LUMENNA_LUM=<lum>` pairing
with `lum`.

- **The same store, in OPFS.** rusqlite builds for the browser on `sqlite-wasm-rs`; the
  `sahpool` VFS needs sync access handles, which only a dedicated worker may use, so the
  core runs in `src/worker.ts` behind Comlink. No WAL there.
- **One tab owns a store** (a Web Lock named for the profile). Opening is idempotent per
  worker, because React's development mode runs effects twice.
- **Records are typed as BoltFFI writes them**, from the crates' own: fields in camelCase,
  data enums as tagged unions (`{ tag: "Open", value0: place }`), absent values `null`, and
  enums with no data as numbers (`ActionKind.MarkDone`), compared by their constants, never
  by value; `core.ts` re-exports the constants. A form field keeps the core's name for itself
  (`accepts_tasks`), which its edit sends back, so a form reads its value by BoltFFI's spelling
  (`fieldOf`). An event the page sends is `ShellEvent` (`lumenna_app::shell`), Android's too.
- **BoltFFI asks every export to be `Send`**, and what only a browser holds (the store's `Rc`
  state, its promises) is not: `Local` (`core/src/app.rs`) says it is, which a browser's one
  thread, built without atomics, makes true. The page's bundle takes only the enumerations'
  constants from the bindings (`treeshake` in `vite.config.ts`); the worker runs them whole.
- **Spans cross as UTF-8 bytes**; the worker converts to the UTF-16 offsets an input counts.
- **A React Aria tree builds its collection in a pass of its own**, so rows can lag the
  data by a render: focus after a change waits for current rows (`TaskList`).
- **Quick add is a combobox completing mid-line**, its filter off (the core decides what
  fits), and Down waits for the offer for where the cursor is *now*.
- **What cannot be undone is an `alertdialog` with Cancel focused first**, never `confirm()`.
  A refused answer stays in its dialog.
- **A row's action runs after its menu has closed** and focus is back on the row, so a
  dialog it opens returns focus there.
- **Every menu, key and button sends a record's own `actions` to the app**, and nothing waits
  on the answer: the question the app opens is drawn from the screen (`Questions.tsx`: a
  confirmation or a choice of answers as an `alertdialog`, a line of text with the app's
  refusal at the field, a pick from the lines the worker words), and answering it is another
  event. Space and Delete run the row's action of the matching kind. The forms are drawn from
  the app's: the details, the block form, quick add, Settings, a proposal, pairing's words.
- **A dialog that is the app's is open while the screen says so**, and closes when the app lets
  it go. Quick add opens at once, before the app's turn, so what is typed straight away goes in.
- **The sidebar's row in hand is the one focus was last on**, not the selection: its
  headings (Projects, Labels, Saved Filters) are never selected.
- **A React Aria check box's input is visually hidden**, so Playwright's `check()` waits for
  it forever: tests focus it and press Space.
- **A browser is an ordinary Iroh peer, always through a relay** (no UDP from a sandbox),
  and pairs by code. `.cargo/config.toml` sets getrandom's `wasm_js` backend.
- **The page's listener reaches the core through the worker**: the core calls a `Listener`
  object (`turn`), which the worker makes around the page's Comlink proxy, never the proxy
  itself, which answers any property.
- **Replan from Now and Plan the Day are the app's screen, drawn** (`Proposal.tsx`): each change
  its line with a check box named by the core's `item` and described by the line, as a
  reminder's buttons are by its text; a new proposal is a new list (`shown`), whose first box
  takes focus. The browser's clock (`Intl`'s `hour12`) is told the app at start.
- **React Aria gives a component its default class only when no `className` is passed**, so
  a class of our own names the default too (`"react-aria-Dialog wide"`): passing `"wide"` alone
  left the Replan and block dialogs without a background, and destructive menu items unpadded.
- **A task row's check box is for a pointer alone** (`.done-box`): `aria-hidden`, never focused,
  its pointer events stopped so the row is not selected or opened behind it. It runs the row's
  Mark Done or Mark Not Done, which Space and the row's menu already offer to everyone else.
- **`npm run screens`** takes a screenshot of every screen into `screens/out`, light and dark, at a
  desktop and a phone width, for a visual review: the tests read the accessibility tree and never
  look at the page. The layout stacks to two columns below 64rem and one below 40rem.
- **The app runs in the worker beside the store** (`core/src/app.rs`, `src/session.ts`). The
  page sends what the person did and draws the screen that comes back, each line as the turn's
  `lines` word it, the capitals sentence case (`Event::Capitals`). The store worker runs as a task, and the
  network middleware hands each request to a runner on the browser's event loop
  (`core/src/app.rs`). Whatever the app leaves, at once or later, goes through one outbox to
  the listener the page gave (`listen`), a turn per batch; a call's reply is only the screen
  once the app is still, with nothing to say, for a field that keeps its own value. Every turn
  is applied one way (`apply`): drawn, its files saved, its notifications shown, focus landed
  if `landing` went up, then said, after focus so the move does not cut it off. Turns are
  numbered (`seq`), and an older screen is never drawn over a newer one. The one-second poll
  ticks the store, for another tab's or process's writes and new minutes, one tick at a time:
  a clock that jumps fires a burst, and queued before a slow worker they held up what followed.
  Reminders' looks are never skipped by the page; the app takes those asked for while one is
  under way as one after it.
- **Landing is judged against where focus was when the turn arrived**, never later: a turn can
  arrive while the person has already moved on. It lands on the main list's row in hand while
  focus is still on the same row of that list, or lost, or wherever React Aria put it when that
  row went; or, for a turn that closed a dialog, given back to where the dialog was opened from
  (`opener`, noted as each dialog takes focus); or after a command that goes somewhere
  (`send(event, { land: true })`). Each frame it waits asks again, so a move to another row, the
  details or the sidebar meanwhile stays where the person took it. It lands on the row in hand
  in the latest screen, which a move reported since may have changed.
- **Quick add opens at once and closes at once**: kept closed after Escape until asked for again,
  so the app's late answer to opening it does not open it a second time.
- **A form keeps its own value while its typing is on its way**, and takes the last answer's
  values once none is: an answer for earlier typing would otherwise undo later typing, and a
  kind's flags would never arrive.
- **A tree's `selectedKeys` is memoized by the key it holds**: React Aria says "… selected"
  whenever it is handed a new selection while the tree has focus, and a fresh array each draw
  said it five times after adding a block.
- **A command key waits for what is on its way** (`settled`): going to a place is a round trip
  now, and a key pressed straight after Go to Trash acts in the trash. Keys run in the order
  pressed, each after what the one before it sent (`commands` in `App.tsx`): keys that all waited
  on one settling ran on the screen before the first had changed it. A deferred command keeps
  trying each frame for a moment while its screen cannot act yet (a day still loading).
- **Playwright's clock does not reach the worker**, where the core reads the time. Tests that
  need a fixed day serve `worker.ts` with a shifted `Date` in front (`at` in
  `tests/planner.spec.ts`).

## Conventions

- `jiff`, not `chrono`. The civil/absolute distinction is load-bearing throughout.
- Timestamps are **millisecond precision**; use `lumenna_core::time::now()`. Automerge's
  timestamp scalar is milliseconds, so anything finer stops equalling itself once saved.
- All identifiers are UUIDv7, so `Ord` is creation order. Several repairs rely on this to
  pick "the newest" deterministically on every replica.
- Sibling ordering is fractional indexing on a single LWW string, never a list CRDT.
- Tests are named as the sentence they assert. Comments say *why*.
- `missing_docs` and `clippy::all` are warnings at the workspace level. Keep them at zero.
- **Run `git config core.hooksPath .githooks` once in every checkout.** Its `commit-msg` hook
  strips any `Claude-Session:` line: links to the conversations that made commits do not
  belong in this public history, which was rewritten once to remove 339 of them.

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

The toolchain is not pinned.
