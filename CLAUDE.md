# Lumenna

A cross-platform task manager and day planner: Todoist's model (projects, labels,
priorities, subtasks, recurrence, quick add) joined to Structured's timeline of time blocks.
The defining feature is that **tasks are assigned to blocks repeatedly across multiple
sittings** until they are done.

`ROADMAP.md` lists what is still to build, with the decisions already made about it.

This file holds what every part shares. Each app, and each crate with rules of its own, has a
`CLAUDE.md` beside its code with what is easy to get wrong there: `apps/*/CLAUDE.md`, and
`crates/app`, `core`, `host`, `parse`, `store` and `sync`. Read the one for the code being
changed before changing it.

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
                open) as events in and view models out; every write an effect. No host. Also
                how every line reads (`speech`, `lines`), the shared keyboard table (`keys`),
                flat rows as a tree (`outline`) and device wording (`devices`), for every app.
crates/bridge/  the app beside its store: Crux middleware answering the app's store effects.
crates/ffi/     the app for the Apple apps, Android and BTSpeak through BoltFFI, and for Windows
                and GTK linked as Rust: `App`, its turns, and `Device`, the store for what runs
                without the screens. The web's is apps/web/core, through BoltFFI too.
crates/sync/    Iroh endpoints, the document sync session, and pairing.
apps/cli/       `lum`, `lum rpc` for clients that cannot link Rust, and `lum mcp`.
apps/btspeak/   the BTSpeak app, in Python, over the app in-process (crates/ffi).
apps/emacs/     the Emacs client, in Elisp, drawing the Crux app over `lum rpc` or the profile's
                socket, a screen per buffer.
apps/android/   Jetpack Compose over the Crux app's Kotlin bindings: the phone (app/), Wear OS
                (wear/), and what they share (shared/).
apps/apple/     the iOS, macOS and watchOS apps over the Crux app's Swift bindings, Shared/
                between them.
apps/windows/   the Win32 app, over the Crux app (crates/ffi, linked as Rust).
apps/gtk/       the Linux app, GTK 4, drawn from the Crux app.
apps/web/       the web client: the core in WASM (core/), TypeScript and React Aria over it.
```

## The command surface

`crates/host` is every operation, once, typed. `Lumenna` holds the open store; its
methods take text identifiers and plain records and return records (`lumenna_records`:
`types.rs`, and `actions.rs`, `form.rs`, `fields.rs` and `words.rs` named below). The CLI
calls it, `lum rpc` serialises what it returns, and the bridge answers the Crux app's writes
with it; `Device` (`crates/ffi`) exports what runs without the screens over BoltFFI. No JSON
crosses the FFI.

- **Every returned record carries `announcement` and `notices`** (the `Announced` trait).
  The announcement is one composed sentence, and only because the core wrote it: an
  `Edit`'s description or a quick-add readback. Rows stay components (`role`, `state`,
  `title`, `value`) because speech and braille assemble them differently.
- **Composed phrases are the core's, as parts**: `PlanBlock::details`,
  `PlanAssignment::details` and `DeviceView::status`, built in `words.rs`. An app adds its
  own time format and title and joins the parts as its screen reader wants; it never
  composes them itself.
- **A time of day goes out as `HH:MM`, for the app's clock.** A task row's `due` is words
  without the time ("due tomorrow") and `due_time` the time, which each app says as its
  platform's setting has it.
- **Every row's actions are the core's** (`actions.rs`): which apply now, in order, their
  spoken names, `destructive`, and the `Question` each asks (immediate, a confirmation, a line
  of text, a pick, a choice of answers, or the client's own form). What a pick offers is
  `Lumenna::choices`, so no client offers a task as its own parent or a wait that loops.
  `ActionKind` is generic across subjects so a key means one thing on every row (Delete is
  `Delete`), with `Subject` saying what it is done to. Only `Question::Form` is the client's:
  the task and block forms. Never decide in a client which actions a row has. A key pressed on
  a row without that action is the app's to refuse (`Run`, `RunDay`, `DetailsEvent::Run`), in
  `not_offered(kind, subject, …)`'s words.
- **A view's rows name their actions in short** (`lumenna_records::Offered`): kind, subject,
  title, sentence, destructive, primary, `other`, and `asks` (`Nothing`, `Something`, or
  `Form`, the shell's own). A list's rows name theirs by place in `ListView::offered` (each
  `Item::actions`); every other row in a view carries its own `offered`. Host records keep
  `actions` whole, for `lum --json` and RPC. A shell hands one back with
  `Event::ActOn(ActingOn, Offered)`, naming its row: `Row(key)` of the place shown, `Held(side,
  key)` in a side list, `Day`, `Details`, or `Reminders`. The app puts that row in hand, then
  runs its action as it has it now, or refuses one it no longer offers (`not_offered`) or a row
  gone (`no_longer_here`). A `Form` one is opened by the shell for the row's own record. A shell
  cannot send `Perform` (`ShellEvent` leaves it out): a whole action is the app's own, so
  nothing a shell holds can run against a record as it was.
- **What every client's forms share is the core's**, as free functions in `form.rs`:
  `task_fields`/`task_edit`, `block_fields`/`block_edit`/`new_block`, `parse_weight`, the
  `#"Home Office"` references. `task_edit` and `block_edit` hold only what changed: sending an
  unchanged field reverts a concurrent edit elsewhere. A client never keeps its own copy.
- **The forms' and screens' words are the core's** (`fields.rs`): `task_form()` and
  `block_form()` give each field's label, hint, example, control and options;
  `pairing_words(this_device, local)` every sentence and button of pairing; `proposal_words(kind)`
  Replan from Now's and Plan the Day's; `go_to_day_question`, `new_filter_questions` and
  `length_question` what the apps' own commands ask. A field marked `one_day` is what one day of
  a repeating block can change. A new block starts at `new_block_start(date)`. A day or a time a
  client offers its own picker for (`FieldKind::Date`/`Time`, a time setting, a question whose
  `Asking::day` is set) keeps the line first, the picker beside it filling it (ISO, `HH:MM`);
  what the picker says where the platform's control does not is `picker_words()`. A pick and a
  text question name their button (`yes`); "OK" is never the answer. A mnemonic or access key is
  the platform's to add; the words are not.
- **Each action has a `sentence` and a `primary`.** Capitals follow the platform: Title
  Case (`title`) on Apple, Windows, GTK and Emacs; sentence case (`sentence`, and
  `sentence_case()` for a fixed button or question) on Android, Wear OS, the web and BTSpeak,
  where every capital also costs a braille cell. A row shows its `primary` actions by itself
  and offers the rest a menu away; every client still offers every action.
- **Lines are joined in one order everywhere** (`lumenna_app::speech`), the clock and day
  words being the app's: a block "<start> to <end>, <title>, <details>"; a sitting "<title>,
  <details>"; free time "<title>, <details>, <start> to <end>"; now "<title>, <time>"; a
  cancelled day "<start>, <title>, <details>"; a block to pick "<day>, <start> to <end>,
  <title>"; anything else to pick "<title>, <detail>"; the day's heading "<day>. <summary>". A
  Blocks-list row is a task row: `due` ("every weekday") said with `due_time` ("at 9:00 AM"),
  then `value`. An empty listing says `Rows::empty`.
- **A setting describes itself** (`Setting::title`, `kind`, `options`, `syncs`, `hint`), and
  the priorities are `priorities()`: a settings screen or a priority picker builds its
  controls from them, never from a table of its own.
- **Colours are the core's palette** (`lumenna_core::colour`, `records::colours`): eleven named
  colours, each with a value for a light background and one for a dark, both of 3:1 contrast.
  A project, label or saved filter takes one by its Colour pick (`colour_choices`, "No colour"
  first), a block by its form's choice; a name outside the palette is refused. What is drawn
  carries a `Swatch`: a client draws it beside the name in the platform's light or dark
  appearance, never in place of it, since the name is said and shown too. A stored name the
  palette lacks is said, not drawn.
- **Terse announcements** (the synced `verbosity`, `terse` or `full`) leave out what the person
  knows already, never what the change was. Rows are components either way.
- **Every assistant does what `lum mcp` does, from one table** (`records::assistant`): each
  operation's names, description, category, keywords, parameters and what it does to the
  store, run through one dispatcher (`Lumenna::assist`, over the FFI `Device::assist`). MCP's
  tools are built from it; App Intents (Siri, Shortcuts, Apple Intelligence) and Android App
  Functions (Gemini) are declared from it and checked against `assistant_operations()`, so an
  operation added there reaches every assistant and none is worded twice.
- **A glance is the core's** (`records::glance`, worded by `lumenna_app::glance`; `App::glance`
  in the platform's clock, `Device::glance` without the screens): what a widget, complication,
  tile or Smart Stack shows, as a timeline of moments. A platform draws it and decides nothing.
- **What a platform's search finds is the core's** (`records::search`: `search_documents` for an
  index such as Spotlight's or AppSearch, `search_results(text)` for one that asks as the person
  types, such as GNOME Shell or KRunner): each a `SearchDocument` opened by its `launch`
  (`task:<id>`, `project:<name>`, …) through `Event::Launch`. No platform decides what matches.
- **The ways in from outside are the core's** (`records::launch`, `Event::Launch(id)`): the
  icon's shortcuts, the Dock's menu, the Jump List, the desktop entry's actions, the manifest's
  shortcuts, in its order and words; the files written ahead are checked against it.
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

`crates/app` is the screens as a Crux 0.20 app; `crates/bridge` runs it beside a store. Every
platform's app is drawn from it entirely, Emacs over RPC; only the CLI still calls the host
directly (8805659 is the last commit before the move). How the app works inside is
`crates/app/CLAUDE.md`; this is its contract with every shell.

- **The app never depends on the host.** It holds the snapshot the host last sent (`Watch`)
  and works out from it, with `lumenna-records`, everything that is a function of the store:
  the listings, the sidebar, the day, a task's details, a reminder list, completion and the
  quick-add preview, what a pick offers, What Should I Work On?, and the planner's proposals.
  Every write is a `HostRequest`, whose answer comes back carrying the store as the write left
  it. What only the host can answer is asked of it the same way: settings, devices, reminders
  due, backups and exports.
- **What is left for a shell** is drawing, saying (`Shell::Announce`, and `Heading` for a day
  newly shown), saving a file, showing a notification and keeping Lumenna running in the
  background. Each batch the app leaves arrives through the layer's callback, and a shell draws
  it as it comes, never waiting on the thread it draws on. Every native shell uses screen 0
  alone for now (`Event::On(screen, event)` addresses another).
- **Keeping Lumenna running in the background is a device setting whose value is the
  platform's** (`records::background`, key `background`). A shell that can says so with
  `BackgroundEvent::Offered { kind, on }` at start and whenever it changes; the app lists it,
  worded for its `BackgroundKind` (`background_words`), hands what the person chose to the shell
  (`Effect::Background`, a `Turn`'s `background`), and says the core's sentence once the shell
  answers `Offered`, or its `failed` and the platform's reason after `Failed`. A shell that keeps
  nothing running (the web, the phones) never sends it, and nothing is offered.
- **What every app decided for itself is the app's now**: the row in hand after a change (the
  same row, else whatever holds its place); a place gone back to, which opens on the row in
  hand and the filter it was left with; folding, the sidebar's too (Expand and Collapse are row
  actions; a folded row comes marked `hidden`); the question open, a refusal staying in it;
  where a place's action leads (renamed, made, deleted); the day shown and today opening on
  now; the details' draft, saved as only what changed; the quick-add line and its completion;
  the block form; the proposal screens; settings, a refused line kept beside why; pairing; when
  to start syncing; reminders due and their buttons; and what to say.
- **What a shell used to ask the core for itself is the app's too.** Put in a Block's pick ends
  in the app's Another Day (`ANOTHER_DAY`), which asks Go to Day's question and offers that
  day's blocks. Pairing's words come with the Devices view, worded as the shell says it is
  called (`Event::PairingAs`, sent once like the clock).
- **Settings offer the store's own actions** (`SettingsView::offered`: Erase Everything
  Deleted), a button each under `offered_caption`, handed back with
  `Event::ActOn(ActingOn::Settings, offered)`; the app asks its question as any row's.
- **What Delete and Space do to a row is the core's** (`keys::runs`): every app, whatever key
  its platform calls Delete (the Mac's Delete, the iPad's Command-Delete, BTSpeak's chord), sends
  `Event::Key("delete")` and never a list of kinds of its own, so the key does the same
  everywhere.
- **Erasing permanently is never what deleting does**: a trashed task's Erase Permanently
  (`ErasePermanently`) is a menu away, after Delete from Trash, and no key runs it. It begins the
  store's history again without what it erased, on every device (`crates/store/CLAUDE.md`).
- **A question says when it asks for a day** (`Asking::day`), with the day a calendar opens on,
  so a shell offers its own picker beside the line without comparing questions.
- **A list's first row is in hand as soon as it opens**, so the details show it.
- **The view says when focus is the app's to move** (`ViewModel::landing`, a count): it moves
  when a turn wrote or moved the row in hand, and never for `Event::Focus`, the person's own
  move. A shell lands focus on the row in hand when the count goes up and leaves it alone
  otherwise, so focus never jumps because a sync or a minute redrew the list.
- **The lists beside the main one have their rows in hand too** (`held.rs`: the sidebar, a
  reminder list, the devices). A shell reports a move there with `Event::Hold(side, key)` (a
  sidebar row by its entry's `key`, a reminder by `id`, a device by `node_id`). A change made
  from one lands there; it stays where changes land only while the person goes on acting from
  it, and anything else hands landing back to the main list.
- **A shell follows the app's own moves between places**: when the app goes on from the place
  shown (a rename, a merge, a delete), the screen showing it goes too.
- **A shell's clock is the shell's** (`Event::Clock`), for the sentences the core composes
  whole, and when to look for reminders. A shell whose clock is pinned for tests says the
  moment proposals are made against (`Event::PlanFrom`).
- **So are its capitals** (`Event::Capitals`): a shell that writes sentence case says so and
  the view comes cased (`capitals.rs`).
- **A sidebar entry carries its `title` and `detail` beside its one line** (`SidebarEntry`,
  `text`), so a sidebar that shows two lines never cuts the line apart; `SidebarEntry::key_of`
  names a row by its kind alone.
- **A view's lines are worded once** (`lumenna_app::lines`, over the `speech` functions): the
  list's rows, the day's, a pick's choices, the devices. A `Line` is its first part (`title`),
  the rest (`detail`), the two joined (`text`), for a list that shows two lines and one that
  reads one, and `braille` where a display shows it apart from speech. The platform gives only
  its locale's formatting of a time and a date (`speech::Formats`); "Today", "Tomorrow", and
  when to give the year are `speech::Spoken`'s.
- Flow tests are `crates/bridge/tests/flows.rs`: events in, view models and sentences out,
  against a real store; a stand-in middleware takes the network's requests, and the test
  answers them.

## The core and the store

`core` holds **materialized** records and knows nothing about Automerge; `store` hydrates
documents into `Snapshot` and translates edits back into Automerge operations, so every rule is
testable without a CRDT. Nothing validates on construction that merge could violate: dangling
references and cycles must load, not be refused. What is easy to break is in
`crates/store/CLAUDE.md` and `crates/core/CLAUDE.md`; these four apply to any change that
writes:

- **Writes take the version the user edited.** `put(record, before)` writes only the fields
  that differ; a fresh `before`, or `None` on an update, writes every field and silently
  reverts a concurrent edit from another device.
- **Everything a person does is an `Edit` through `Store::apply_recorded`**, or it cannot be
  undone. Undo applies the edit's other side, rebased, and never rewinds.
- **Never drop what you cannot read**: builds of different versions sync with each other for
  good, so a field a newer build added must survive every write. **A field added to the model
  must be added to `export/json.rs`**, or the export round-trip test fails.
- **A new container, collection, or record keyed by something two devices can produce on their
  own** needs the care in `crates/store/CLAUDE.md` (deterministic changes, `Record::INLINE`),
  or one device's writes vanish on merge.

The planner (urgency, suggestions, Replan, Plan the Day) only ever proposes: an accepted
proposal goes through the ordinary operations, every proposal can be said in full before it is
accepted, and no language model is involved.

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
  belong in this public history.

## Commands

```
cargo test --workspace
cargo clippy --workspace --all-targets
PROPTEST_CASES=20000 cargo test -p lumenna-core --test properties
```

CI (`.github/workflows/ci.yml`) builds each mobile app once and tests it on several devices in
parallel jobs that share the build. **Two tiers**: the Rust tests, the builds, the clients, the
GTK and Windows UI tests and a smoke set of iOS and Android UI tests block; the full iOS and
Android suites report without blocking, since a slow runner fails them where nothing is wrong.
A test that has only failed for a reason can join the smoke set. **A branch runs only what its
changes can break** (the `changes` job: a change to `crates/`, `Cargo.*` or `ci.yml` runs
everything, one to an app its own jobs, and the Rust workspace's for an app written in Rust);
master, pull requests and runs asked for run everything. A new app's jobs join a group there.

`crates/core/tests/rows.rs` snapshots the row projection (each row's place, folding, checked
state, speech and braille) in `tests/snapshots/`, rewritten with `UPDATE_SNAPSHOTS=1` once a
change is meant. `crates/store/tests/convergence.rs` checks replicas reaching the same state
from the same changes in different orders: the slowest suite, and the one worth running before
believing anything about merge.

**Release builds are whole-program** (`[profile.release]`: fat LTO, one codegen unit, symbols
stripped). **An artifact built beside an `rlib` gets none of it**, so `lumenna-ffi`, which the
Rust apps link and other platforms load, is built with `cargo rustc --crate-type …` where it
ships (the Apple and Android scripts; BTSpeak's through BoltFFI's `--cargo-arg`), and the web's
crate is a `cdylib` alone.

**`release.yml` builds every app for release, unsigned**, on master, a version tag or when
asked, and keeps each as an artifact for packaging to build on: `lum` and the BTSpeak app on
Debian 12 (the oldest glibc and Python the BTSpeak devices run), `lum` universal on macOS, the
Windows installer (the app and `lum`, Inno Setup), the MSIX for the Store and a `lum` zip, the
GTK app as an AppImage and a Flatpak bundle, each on both architectures, the iPhone app with
its watch app for devices, the Mac app for both architectures, `lum` inside it, as a disk image,
Android's App Bundles and APKs, and the web client. Each is checked as a person would get it
(installed, unpacked or started) before it is kept. A new platform's build goes there too.

`[profile.dev]` sets `debug = "line-tables-only"` and `incremental = false` for the BTSpeak's
small disk. There, `cargo clippy --all-targets` and `cargo test` do not share artifacts, so
running both back to back can run out of space; `cargo clean` between them.

The toolchain is not pinned.
