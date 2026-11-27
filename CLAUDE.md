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
apps/cli/       `lum`, `lum rpc` for clients that cannot link Rust, `lum mcp`, and `lum assist`.
apps/powershell/ the PowerShell module: a cmdlet per assistant operation, over one `lum rpc`,
                for Windows PowerShell 5.1 and PowerShell 7.
apps/btspeak/   the BTSpeak app, in Python, over the app in-process (crates/ffi).
apps/emacs/     the Emacs client, in Elisp, drawing the Crux app over `lum rpc` or the profile's
                socket, a screen per buffer.
apps/android/   Jetpack Compose over the Crux app's Kotlin bindings: the phone (app/), Wear OS
                (wear/), the HIMS BrailleSense 7 (braillesense/), the Orbit Speak and Player (orbit/),
                which draw no Compose and speak for themselves, what those two share (braille/: the
                dialog controller among it), and what every app shares (shared/); and the unofficial
                Orbit and BrailleSense SDKs (orbit-sdk/, braillesense-sdk/), each the device itself,
                depending on nothing of Lumenna's: git submodules, each its own repository under
                Apache-2.0 (github.com/emassey0135/orbit-sdk, …/braillesense-sdk). Clone with
                `--recurse-submodules`, or `git submodule update --init` in a checkout made before.
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
  where every capital also costs a braille cell. A name keeps its capitals in either: an
  acronym, "I", and Lumenna's own ("About Lumenna"). A row shows its `primary` actions by itself
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
  controls from them, never from a table of its own. **Which page a setting is on is the
  core's** (`settings_pages`, also `SettingsView::pages`): the pages in order with their titles,
  descriptions, captions and buttons, every setting on exactly one, so a new setting reaches
  every settings screen. A shell matches no key and keeps no page table: the page that is the
  devices screen says so (`SettingsPage::devices`), and the page a menu command opens is
  `keys::settings_page_of` (`settings_page_of`). **Which pages a device
  lists is the core's too**: `SettingsView::pages` holds only those with something on them here
  (a page's `keys` are the settings this device lists; General goes where the shell keeps nothing
  running and has its own clock), so a shell lists them all and filters none. What a shell can
  do is said once as it starts (`Event::Abilities`, `Abilities { files, backup_folder }`; a shell
  that never says can do both): one that saves and picks no files (a watch) lists no exports,
  imports or restores, one with no folder of backups (a browser) none of the backup schedule and
  folder settings. **About is the last page where a device has no About of its own**
  (`Abilities::own_about` false: a phone, a watch, the iPad, the web): a page of lines alone
  (`SettingsPage::lines`, `ShellWords::about_lines`: name, version, description, website, where to
  report a problem, publisher, licence), with no setting and no button, which a shell draws as
  lines to read. A desktop keeps its menu bar's About, and a braille device and Emacs Home's, so
  the page is not listed there.
- **The task details' words are the core's** (`details_words`; `DetailsView::lines`, also
  `Lines::details`: the state, a rule the form cannot say, what it waits for, its history). The
  buttons a details pane shows by themselves are the task's `primary` offered actions, the rest a
  menu away.
- **Every other fixed word a shell used to write is the core's** too, and **a shell has them all
  in one record**, `lumenna_app::words::ShellWords`, asked for once as it starts in its capitals
  and as it calls itself (`shell_words(capitals, ThisDevice)` over the FFI and the web's core,
  `app.words` over RPC): its panes' and lists' names, its dialogs' buttons, its file dialogs'
  kinds, a row's hint, a keyboard help's headings, the words of quick add, the details, the
  file and folder dialogs, notifications and the shortcuts from anywhere, and what the device
  calls itself (`this_device`, what it says as `PairingAs`). A word with parts is a function,
  since it is said with them (`could_not_read(what, reason)`, `press_to_add(key)`,
  `reminders_for(name)`, pairing's `PairingView::matching`); what changes with what is shown
  comes with the view. That includes what only one shell needs today (BTSpeak's menus and file
  prompts, the web's folder backups and tab notices, a watch's placeholders): a shell writes no
  sentence of its own, a key it names being a part. A word a shell finds missing is added to
  the record, never written in the shell. **The one exception is a device SDK's own words**:
  what the device itself says, as its own apps say it, belongs to the SDK that speaks for the
  device and not to Lumenna (the Orbit SDK's `OrbitWords`: its fields' echo and cell names, its
  Editor's selection, T9's modes; the BrailleSense SDK's edit box, cell echo and role words), so
  another app on the device says the same, and the core holds none of them. What Lumenna says
  there (its key help, its menus) stays the core's. A sidebar entry says its
  own folding (`hidden`, `expanded`, `level`), as a list's rows and a pick's lines (`Line::level`)
  do; a list a shell narrows or cuts out itself takes its levels from `lines::levels(depths)`
  (`levels()` over the FFI), so no shell counts them. **No shell joins two of the core's words
  into a line either**: a setting's row is `SettingView::line` ("Clock, 24-hour"), pairing's
  title and intro `PairingWords::heading`, a proposal's title and announcement
  `ProposalView::heading`, a change's line and what its check box says as it changes
  `ChangeView::line` and `toggled`, and with its reason `ChangeView::read`, what does not fit
  `ProposalView::unfit_lines`, where pairing stands and its question about the words
  `PairingView::status` and `question` (and with none open `DevicesView::pairing_status`), a
  settings page's title with its description `SettingsPage::line` and with its caption `heading`,
  a dialog's control as a row reads it `ControlShown::line` ("Clock, 24-hour", a check by its
  state's word) and a choice with why it is offered `option_lines`, for a device whose settings
  are rows (BTSpeak; a device SDK that works a dialog itself says a control in its own roles), a list's name with its
  readback `ListShown::heading` (`screen::heading`, `list_heading`, for a list a shell names
  itself), a reminder list's title with its announcement
  `ViewModel::reminders_heading`, a proposal's Apply with its hint `ProposalView::apply_line`, a
  row's or item's name with the keys that press its letter `key_help::with_press`, the task's fields read rather than edited
  `DetailsView::field_lines` ("Priority: High"), parts of a prompt (a refusal before what is
  asked) `screen::prompt` (`prompt_of`), several things the app said in a row, each a sentence
  ended once, the last keeping its full stop, `screen::said_together` (`said_together`), a line with its level after it `lines::with_level`
  (`line_with_level`), with its folding too `lines::with_state` and `detail_with_state`, a
  dialog's title and the lines before its controls `DialogShown::prompt`, a key help's line
  `DeviceKey::line`, and About's lines `ShellWords::about_lines`, which a device with no About
  page of its own shows as a list of lines, never one sentence. Whether a line entered whole
  ends in a name begun, for a device whose line cannot complete as it is typed, is
  `typing::ends_unfinished` (and `put_in` the completion chosen). A shell drawing a dialog's
  controls (`lumenna_app::dialog`) takes them apart instead: a label and a value, a refusal as a
  line of its own.
- **What Lumenna is is said once** (`[workspace.metadata.lumenna]` in the workspace's
  `Cargo.toml`: name, description, version, homepage, issues, publisher, the identifier `id` in
  freedesktop's and Windows' capitals and `package_id` lowercase for Apple's bundles and Android's
  packages, with the licence from `[workspace.package]`): `records`' build reads it into
  `words::metadata()` (`app_metadata()` over the FFI), which `lum`'s about, the About boxes and
  the autostart entry say. The Apple apps read their version, name and bundle identifier from it
  as they build (`apps/apple/metadata.sh`), and Android's Gradle its version, name and package.
  Every other platform file that must say it ahead (the desktop entry and metainfo, the MSIX and
  Inno Setup, Android's Gradle files and strings, the web's package and manifest, the PowerShell
  module, BTSpeak's pyproject, the apps' crate versions) is held to it by `records`' metadata
  test: change it there, then each file.
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
  Functions (Gemini) are declared from it and checked against `assistant_operations()` (and App
  Intents' entities, their properties and Open intents against `assistant_entities()`), so an
  operation added there reaches every assistant and none is worded twice. What another app sets
  up once and runs unattended (Tasker's actions) is `records::automation` over the same table:
  never an operation that `confirms`, every value as text the core types, and the answer as
  variables, so a platform words nothing. `lum assist` runs one from the command line and RPC's
  `assist` for a client that stays; the PowerShell module's cmdlets are written from
  `assist.list` (`apps/powershell/Write-Operations.ps1`, run again when the table changes) and
  checked against it.
- **A glance is the core's** (`records::glance`, worded by `lumenna_app::glance`; `App::glance`
  in the platform's clock, `Device::glance` without the screens): what a widget, complication,
  tile or Smart Stack shows, as a timeline of moments. A platform draws it and decides nothing.
  Which moment to show now and when to draw again are the core's too (`glance::current`,
  `redraw_at`, `logged`; `glance_current`, `glance_redraw_at` over the FFI).
  The widgets themselves are the core's too (`glance::widgets`, `glance_widgets()`): Now and
  Today, their names and descriptions in the picker and the parts each shows; a platform's
  files written ahead (Android's strings, Windows' manifest) are checked against them.
- **A control outside the app is the core's** (`records::controls`, `controls()`): Control
  Center's, a Quick Settings tile, a watch's button or complication. The timer's shows
  `Device::timer_control` (running, what runs or would start, else why nothing would) and a press
  is `Device::toggle_timer`: it stops the timer running, else starts on What Should I Work On?'s
  first choice. New Task is its launch shortcut. No platform decides what a press starts. A
  control that cannot reach its store reads the app's `timer_control_file` instead, as a widget
  reads the glance's.
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
  called (`Event::PairingAs`, sent once like the clock). A pairing code is taken as typed, spaces
  between its groups and all (`PairEvent::Code`, `lum pair`): no shell strips it.
- **Each screen's own actions are the core's** (`ViewModel::screen_actions`, `ScreenAction`,
  `screen_actions.rs`): what the place shown offers beside its rows, in order (New Task, the
  filter, New Project Inside, New Block, the day's moves, Undo, Redo; the trash and the history
  only Undo and Redo; the day's own actions, What Should I Work On? among them, are the day's
  `offered`, never a second time here),
  each with its title in the shell's capitals, its braille letter, whether it `asks`, whether it
  is the shell's own to run (`shell`: the filter field, Key Help, About, the reminders due list)
  and whether it is `enabled` now. Every one not the shell's runs as `Event::Key(id)`, so no shell
  hand-picks a toolbar: a desktop or a phone draws them as its buttons, a braille device lists
  them on its screen's menu after the row's actions. Their group is named `ShellWords::screen_actions`
  ("Commands"), an overflow menu `more_options`, and About's line is `ShellWords::about`.
  `Key("what-now")` goes to Today from any place and asks, as the launch shortcut does.
- **Home is for a shell without a sidebar or tabs** (`Event::Home`, `Key("home")`,
  `ViewModel::home`, `HomeView`): a braille device opens on it, and Emacs may. It is the places
  (the sidebar's entries, folded as the sidebar is, each with its letter: `HomePlace`), then
  Home's own actions (`HomeView::actions`, also `screen_actions` while it is shown): Reminders Due
  while any are due, New Task, Undo, Redo, What Should I Work On?, Sync Now, Settings, Key Help
  and About. Its row in hand is the sidebar's (`Hold(Side::Sidebar, key)`); `Key("open")` opens a
  place and folds or unfolds a heading, and a place's letter does the same as Enter on it; an
  action's letter runs it. New Task from Home starts an empty line (the Inbox), whatever place
  Home was opened over. Opening a place leaves Home; acting on one there (renaming it, making one)
  does not, the place beneath following it. A desktop or a phone never shows Home: its places are
  its sidebar or tabs, and its app-wide actions the menu bar's or the Browse tab's.
- **Where a phone and a watch put the places is the core's** (`lumenna_app::arrangement`,
  `place_arrangement(sidebar, watch, capitals)` over the FFI, from the view's own sidebar): a phone's
  tabs in order (`Tab`: a place, Browse or Settings, with its title), Browse's rows (every place
  not a tab, in the sidebar's order with its detail and swatch, and each group with the count of
  every place under it), each group's own screen (`GroupScreen`: its heading, whose `offered`
  holds its New, its places a level shallower with their levels said where they change, and what
  it says with none), and a watch's root list (`RootRow`), which is Home as a watch has it: New
  Task first, every place shown as the sidebar has it, then the rest of Home's actions in Home's
  order, each run as `Event::Key(id)`, but one that needs what the watch lacks (`arrangement::needs`,
  `watch_has`): a keyboard (Key Help), reminders listed in the app (Reminders Due: a watch gives
  them as notifications), an About beside Settings (a watch's is its Settings page), and syncing
  itself (Sync Now, only on a watch that does: `WatchKind::SyncsItself` for Wear OS,
  `ThroughItsPhone` for the Apple Watch). A new action on Home reaches the watches unless a rule
  leaves it out, which a test holds. A place added to the sidebar reaches every phone and watch
  with no shell edit; no shell picks places out of the sidebar by kind or titles them itself.
- **Settings offer the store's own actions** (`SettingsView::offered`: Erase Everything
  Deleted), a button each under `offered_caption`, handed back with
  `Event::ActOn(ActingOn::Settings, offered)`; the app asks its question as any row's.
- **What every command does is the core's** (`keys::what_runs`): every app, whatever key its
  platform calls Delete (the Mac's Delete, the iPad's Command-Delete, BTSpeak's chord), sends
  `Event::Key("delete")`, Enter `Key("open")`, a menu's Mark Done `Key("mark-done")`, and never a
  list of kinds of its own, so a command does the same everywhere: a row's action, the day's, or
  the app's own (quick add, a place, undo, a day). One the row lacks is refused in
  `not_offered`'s words; the details take `DetailsEvent::Key`. **Enter is `Key("open")` on every
  row of every platform**, never a choice per place: a task's details, a block's form, free
  time's new block, a cancelled day back, a trashed task restored, a device renamed, a project's
  section heading folded (an empty one renamed); on a place in the sidebar or on Home, the place
  opened (a heading folded). On a task, Enter runs its Edit
  Details, which asks for the task's form (`Question::Form`), the app showing that task's details
  as it asks: every shell opens its task form for
  it, as for any action asking that, `ScreenView::dialog` describing it to one that speaks for
  itself. What Enter does on a row, for a screen reader to name its default action (TalkBack's
  double tap), is `keys::runs_on("open", offered)` (`key_runs_on` over the FFI). The menu bar and the tray's menu
  are the core's (`keys::MENUS`, `TRAY`, `menus()`, `tray_menu()`), and each row or day command's
  name and whether it applies now are `ViewModel::commands` (Mark Not Done on a task done). A
  platform adds only what is its own (Cut, Copy and Paste).
- **A reminder due carries its buttons in order** (`Due::buttons`), and a button pressed holds
  the next reminder (`Side::Due`, `ViewModel::due_held`). An overrun's Replan is
  `DueEvent::Replan(id)` for one the app's list holds, and `DueEvent::ReplanFiring { key,
  occurrence }` for one the platform gave itself (`Device::deliver_reminders`, the system's
  notification): today, Replan from Now open, the warning dismissed once something is replanned.
  An overrun never sends `ProposalEvent::Replan`. A reminder dealt with elsewhere leaves the list
  once the store says so, at the next look for reminders as at the next tick; a summary of what was
  missed refuses Snooze in the core's words. **A button is handed back by its id**
  (`DueEvent::Press { id, button }`, `DueButton::id`): no shell maps `dismiss`, `snooze`, `carry`
  or `replan` to an event itself. A list of reminders due opening looks back over the last day by
  the app's own clock (`DueEvent::LookBack`), never by the shell's arithmetic.
- **A braille device's letters are the core's** (`lumenna_app::braille`): a letter per kind of
  action (`action_letter`), per settings page and button (`page_letter`, `button_letter`), per
  reminder's button (`due_letter`), per screen's own action and Home's place (`ScreenAction::letter`,
  `HomePlace::letter`, `command_letter`), each asked for over the FFI by what a shell already
  holds: a shell shows it beside the thing, and presses it with `Event::Letter`, which runs the
  row's action of that letter (the shared table's command where it has one, asking which where a
  row has several), else the reminder list's, the day's or the screen's own (on Home its places'
  and its actions' first), else says why; a letter of an action that is the shell's own
  (`ScreenAction::shell`) the shell runs itself. First-letter moving is `Event::FindLetter`
  (`first_letter` for a list a shell draws). **Which key presses a letter is one rule, the
  core's** (`lumenna_app::key_help`, by `KeyDevice`): on a BTSpeak a plain letter is the command
  and a capital (Dot 7 with the letter) moves by first letter, never Space with one; on a
  braille reader, the Orbit Speak and the BrailleSense, Space with the letter is the command,
  Enter with it where the device keeps Space's while the app is in front (the Speak's Enter is
  dot 8), and the letter alone moves; the Orbit Player presses none. `letter_press(device,
  letter)` says which, `press_name` how the device names the keys ("Space+U" on a reader,
  "Enter-C" on the BrailleSense, "dot 8 c" on the Speak, "u" on a BTSpeak) and `press_instead`
  what to press where Space reaches the app but Enter is the command. **A braille device's key
  help is one, made from that rule** (`key_help(device, view, words)`, `App::key_help_on` over
  the FFI, `KeyHelp`: its sections and the same as lines): the device's own keys (how it moves,
  opens, goes back, deletes and types, and how a row is found by letter, alone or as a capital),
  Home's places, the screen's own commands and the actions the rows shown offer, each by the
  keys that press its letter there; one whose letter has no press there is a menu away and left
  out. No shell writes a key help.
  A device reporting dots turns them into a letter by `braille_letter`, the core's own a to z and
  never the person's table: dot 7 or 8 in a chord is no letter. Erase Permanently has no letter. How the BrailleSense reads a line (its role in
  speech and on the cells, where its settings put it, a shortcut letter, a sign for a dialog or a
  submenu, a control's label and place) is the device's own convention, in its SDK
  (`deviceReading`, `apps/android/braillesense-sdk`), not the core's.
  A braille device's menu bar is the shared one with its letters (`braille::menu_bar`, over
  `keys::MENUS`), and what a letter gives where none is set is `braille::menu_shortcuts`. **Its
  first menu is the screen's own** (`braille::SCREEN_MENU`), ahead of File: named for the screen
  ("Tasks", "Today", a project's name, "Home"), its letter the first of that name free of the
  shared menus', holding the screen's `screen_actions` (Home's on Home) in order, each with its
  letter as a real hot key; the same commands stay under File, Edit and Day too. How it is opened
  is each device's: BTSpeak's M chord opens straight into it, Tab and Shift-Tab moving to the other
  menus as its editor's menu bar does; the BrailleSense's Space-M opens the bar on it. **A device
  whose one menu key is both the bar and a row's actions** (the Orbit's Space with M, as its own
  apps have it) **has the row's menu first** (`braille::menu_bar_on_row`, `braille_menu_bar_on_row`
  over the FFI, `ROW_MENU`): named for the row in hand, its `offered` actions in order, each with
  its `action_letter` and its `action` to hand back with `ActOn`, the menu's letter the first of the
  row's name the others leave free; on a place an item of another menu whose letter the row's
  action takes is no hot key (but the shared command that is that action), and on Home a row's
  action of one of Home's letters is none. The screen's own menu follows it. The bar holds
  every one of the screen's own actions (Home's on Home), each with its letter as a real hot key,
  and everything the menu bar offers but a window's or a pane's own is reachable without it: on
  Home, among a place's actions, as a row's or the day's, on a heading, or as a settings page or
  its button (both held by `screen_actions`' tests).
- **What a shell that speaks for itself needs is the core's** (`lumenna_app::screen`,
  `Turn::described`, `App::screen_view`): what is shown (a list with its name, the row in hand's
  line, its place, "3 of 12", and its actions with their letters; a question with its prompt,
  typed text, buttons; a form's name) and what changed since the turn before (`ScreenChange`:
  entered, a move only when `landing` rose, a question asked, a refusal). A refusal also marked
  on `Announce::refusal`, so a shell can mark it apart from what else is said. **A form or a
  dialog comes as its controls in tab order** (`lumenna_app::dialog`, `DialogShown`): the one the
  app has open is `ScreenView::dialog` (a question, the block form, quick add), and the two a shell
  opens itself are `App::task_form_on(screen)` and `App::settings_page_on(screen, page)`. Each
  control (`ControlShown`) says what it is in Lumenna's own terms (`ControlType`: a line, several
  lines, a choice, a check, a list, a button, a line to read), which each device maps to its own
  roles, with its label, value, options, hint, letter, whether it is the default or the Cancel,
  and what it sends (`ControlSends`). A shell builds no event for a control: it hands
  `control_event(sends, value, how)` what the person did (`ControlMove`: typed, set, pressed) and
  sends what comes back; a button that sends what a line or a list holds (`Answer`, `Pick`,
  `QuickAdd`) is pressed with that control's value. Escape is the dialog's `cancel`. A form asked
  for by a question (`Question::Form`, a task's Edit) is the task form, closing the question as it
  saves or cancels. What a dialog's button leaves to the shell (a file to pick, a backup into the
  folder, pairing, a folder setting) is `ControlSends::Shell(id)`.
- **Erasing permanently is never what deleting does**: a trashed task's Erase Permanently
  (`ErasePermanently`) is a menu away, after Delete from Trash, and no key runs it. It begins the
  store's history again without what it erased, on every device (`crates/store/CLAUDE.md`).
- **A question says when it asks for a day** (`Asking::day`), with the day a calendar opens on,
  so a shell offers its own picker beside the line without comparing questions. It says too
  which question it is (`Asking::number`: a new number is a new question, the same one refused
  is the same), what was typed in it (`typed`, where a refused field starts), and, when it asks
  how long, the usual lengths in words (`lengths`): a shell neither waits for the app to be still
  before asking nor keeps the text itself. A question open is never `Question::Immediate`: an
  action that asks nothing runs as it is handed back, so no shell answers one Yes itself.
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
- **So is whether its list has checkboxes** (`Event::Checkboxes`): the list's lines then leave
  "completed" to the checkbox.
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
PowerShell module, the GTK and Windows UI tests and a smoke set of iOS and Android UI tests block; the full iOS and
Android suites report without blocking, since a slow runner fails them where nothing is wrong.
A test that has only failed for a reason can join the smoke set. **A branch runs only what its
changes can break** (the `changes` job: a change to `crates/`, `Cargo.*` or `ci.yml` runs
everything, one to an app its own jobs, and the Rust workspace's for an app written in Rust);
master, pull requests and runs asked for run everything. A new app's jobs join a group there.
The suites that only report (the full iOS, Mac UI and Android suites), WebKit, and Rust on Intel
macOS run on master alone (`full`), since most wait for one of the five macOS runners: a branch
runs the builds and everything that blocks, Firefox included.

`crates/core/tests/rows.rs` snapshots the row projection (each row's place, folding, checked
state and the parts it carries; how a line is joined is the app's, tested there) in
`tests/snapshots/`, rewritten with `UPDATE_SNAPSHOTS=1` once a change is meant. `crates/store/tests/convergence.rs` checks replicas reaching the same state
from the same changes in different orders: the slowest suite, and the one worth running before
believing anything about merge.

**Release builds are whole-program** (`[profile.release]`: fat LTO, one codegen unit, symbols
stripped). **An artifact built beside an `rlib` gets none of it**, so `lumenna-ffi`, which the
Rust apps link and other platforms load, is built with `cargo rustc --crate-type …` where it
ships (the Apple and Android scripts; BTSpeak's through BoltFFI's `--cargo-arg`), and the web's
crate is a `cdylib` alone.

**`release.yml` builds every app for release, unsigned**, on master, a version tag or when
asked (`platforms`: `all`, or the jobs to build), and keeps each as an artifact for packaging
to build on: `lum` and the BTSpeak app on Debian 12 (the oldest glibc and Python the BTSpeak
devices run), `lum` universal on macOS, the Windows installer (the app and `lum`, Inno
Setup), the MSIX for the Store and a `lum` zip, the GTK app as an AppImage and a Flatpak
bundle, each on both architectures, the iPhone app with
its watch app for devices, the Mac app for both architectures, `lum` inside it, as a disk image,
Android's App Bundles and APKs, and the web client. Each is checked as a person would get it
(installed, unpacked or started) before it is kept. A new platform's build goes there too.

`[profile.dev]` sets `debug = "line-tables-only"` and `incremental = false` for the BTSpeak's
small disk. There, `cargo clippy --all-targets` and `cargo test` do not share artifacts, so
running both back to back can run out of space; `cargo clean` between them.

The toolchain is not pinned.
