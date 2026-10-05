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
crates/surface/ the command surface (§12): the `Lumenna` object, one method per operation.
crates/ffi/     the library the Swift and Kotlin apps link; re-exports surface over UniFFI.
crates/sync/    Iroh endpoints, the document sync session, and pairing (§7).
crates/desktop/ what the Windows and GTK apps decide alike: row wording, flat rows as a
                tree, the sidebar's places, work-block choices, device wording, the profile.
apps/cli/       `lum` — the first target, and a permanent one.
apps/btspeak/   the BTSpeak app, in Python, over `lum rpc`.
apps/emacs/     the Emacs client, in Elisp, over `lum rpc` or the daemon's socket.
apps/apple/     the iOS and macOS apps over the generated bindings, Shared/ between them,
                and build-core.sh.
apps/windows/   the Win32 app, linking the surface directly.
```

The other GUI apps in §2's layout do not exist yet.

### The command surface

`crates/surface` is every operation, once, typed. `Lumenna` holds the open store; its
methods take text identifiers and plain records and return records (`types.rs`). It is
the only place operations are implemented. The CLI calls it, `lum rpc` serialises what it
returns, and UniFFI exports it unchanged to Swift and Kotlin (the `uniffi` feature, built
by `crates/ffi`) — no JSON crosses the FFI.

- **Every returned record carries `announcement` and `notices`** (the `Announced` trait).
  The announcement is the one composed sentence, and only because core wrote it — an
  `Edit`'s description or a quick-add readback. Rows stay components (`role`, `state`,
  `title`, `value`) because speech and braille assemble them differently (§13).
- **Wording stays client-neutral.** Nothing in the surface names a `lum` command or flag;
  the CLI adds those (`lum task restore` after a trash, `--force`, `lum stop … --minutes`).
- **Identifiers are text**: a whole UUID or a prefix that names one record. A bare number is
  refused as a row number. Row numbers are the CLI's: `Profile::row` turns them into
  identifiers before calling in.
- **What every client's forms share is here too, as free functions** (`form.rs`):
  `task_fields` and `task_edit` (a form's fields from a task, and the `TaskEdit` holding only
  what changed — sending an unchanged field reverts a concurrent edit elsewhere),
  `project_reference`/`label_reference` (`#"Home Office"`, the parser's own quoting), and
  `sitting_status`. Swift sees `taskEdit(task:fields:)` and so on. A client does not keep
  its own copy of any of them.
- **Every method refreshes first** (`Lumenna::with`), so an answer is never staler than the
  last change on disk.
- **Reshaping a record breaks `--json`, `lum rpc`, and the Swift and Kotlin types at
  once.** That is the point: they cannot drift apart.
- `uniffi.toml` names the Swift module `LumennaCore`. Methods named after Swift keywords
  (`import`, `open`) come out backticked, and work.

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
- **`api.rs` is an envelope, not the surface.** Every command returns a `Response`: the
  contract version and an `Outcome` tagged `result`, wrapping a surface record. `render`
  writes it as prose or as JSON, and neither computes anything the other does not have.
  The sync payloads live here too, because syncing is the CLI's for now. A command
  reachable only by reading prose off stdout is not on the surface.
- **`--json` is versioned** and shaped for scripts, not derived from the model — §15 calls it
  a compatibility contract. Reshaping `api.rs` or a surface record is a breaking change.
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
- **Driving sync is the surface's** (`crates/surface/src/sync.rs`, the `sync` feature):
  pairing through a `PairingPrompt` callback, `sync_now`, status, devices, and the
  `SyncService` loop that both the daemon and the iOS app run. The CLI adds only the
  terminal prompt, the daemon's socket and signals, and asking the daemon.
- **One endpoint per device**, decided by an advisory lock on `<profile>/sync.lock`. A
  `SyncService` holds it; `sync_now` takes it for a round, hands the round to this store's
  service, or returns `LumennaError::SyncElsewhere` so the CLI asks the daemon over the socket.
- **The service shares the operations' connection**, so it notices local edits by
  `Store::version()` (every document's heads); `refresh` alone cannot see a connection's own
  writes. Arrivals are signalled as soon as a session's changes are in the store.
  The device key lives in the store's `local_state` table and never syncs.
- **The sync session is lockstep per document**: both sides send one frame (a message, or an
  empty frame for nothing) then read one; both empty ends the document. Both sides see the
  same two frames, so both stop together. Sync state is per session, not saved.
- **Local discovery is standard DNS-SD** (`crates/sync/src/discovery`), an Iroh address
  lookup of our own: services `_lumenna._udp` (devices) and `_lumenna-pair._udp` (pairing
  sessions), TXT `id` plus `a0`… addresses. Apple (macOS and iOS) goes through the system
  responder's C API, which on iOS is Bonjour and needs no multicast entitlement — only the
  types under `NSBonjourServices`. Linux uses Avahi over D-Bus (zbus) when it runs: beside
  Avahi, `mdns-sd` announced but heard nothing, not even itself (seen on the BTSpeak).
  `mdns-sd` covers Windows and Linux without Avahi. Iroh's own mDNS was
  replaced because no standard browser can see it: no PTR record, every TTL zero.
- **Bonjour reports an instance once per interface**, and a gone device's record can linger.
  So an instance is resolved once, on its own thread, and lost only when gone from every
  interface; dials to heard pairing sessions give up after three seconds.
- **Tests that pair take turns** (`ONE_PAIRING_AT_A_TIME`): waiting sessions on one network
  find each other, so two tests pairing at once pair with each other.
- **A session joining by code does not advertise on mDNS.** Otherwise the waiting session
  dials it back and two connections each wait for the other to speak.
- The daemon serves the RPC surface on `<profile>/lumenna.sock` (Unix); a `sync` request
  there is passed to the daemon's own endpoint through a hook, not dispatched.
- Device records are inline (`Record::INLINE`): both sides of a pairing write them.
- Tests use `Network::LocalOnly` with explicit addresses: offline, no outside server.
- `lum daemon install` (`service.rs`) writes a systemd user unit plus linger on Linux, a
  system unit through `sudo` on BTSpeak, a LaunchAgent on macOS, a logon task on Windows.
  The service always gets `--profile`, and a non-default profile gets a tagged name so
  each profile can have one. The macOS and Linux paths have been run for real (Linux on an
  Arch VM); BTSpeak's and Windows' text is unit-tested, their commands are not.

### `lum rpc`

The same surface over JSON-RPC on stdio, for clients that cannot link Rust — Emacs (§16.10)
and the BTSpeak app (§16.11). One server per client, no Iroh; syncing is the daemon's job,
and §8's *one protocol, two transports* means the daemon will serve exactly this over a
socket.

- **A method builds the same `Command` and calls the same `dispatch`**, which calls the
  surface. There is one implementation of every operation, so RPC cannot drift from the CLI
  or from the apps that link it.
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
- **`pair` replies when the pairing ends**, from a thread of its own, and talks in between
  through `lumenna/pairing` notifications: `{code, name}` while it waits to be found, then
  `{words}`. The client answers with `pair.confirm {match}` or gives up with `pair.cancel`;
  the server keeps answering everything else meanwhile. One pairing at a time.
- **`complete` and `preview` have no command line** — completion is a keystroke-rate
  question and a process per keystroke is not an answer. They are surface methods
  (`complete_text`, `preview_task`), so linked apps call them directly, and they are why the
  BTSpeak app speaks a protocol rather than shelling out.
- **`block.choices` is the block chooser's rule** (`Lumenna::work_blocks`): the coming
  week's work blocks, as components, which every app offers when a task is put in a block
  from the task itself. Each app only words them; none decides which days or kinds.

**Not built yet:** the encrypted store and its account key (§8), wake-up push, Windows
named pipes, reminders, hooks, auto-scheduling.

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
- **`main()` pushes an app context** (`host.push_app_context("lumenna", self_voice=True)`,
  popped in a `finally`), as the stock apps do. The device takes its braille table from the
  app it believes is in front; without the push that stayed BT Code, whose open `.py` file's
  table made every menu computer braille. The push also sets self-voice, and the pop
  restores it, the app in front and its help — all device-wide. `# blazie-flags: self-voice`
  on line 2 of `__main__.py` still matters for the menu launcher, which reads it before
  starting the program. With self-voice on, printing to the terminal is silence, so startup
  errors are dialogs and the spawned server's stderr goes to `rpc.log`.
- **No `.menu` file of its own.** It is a line in `~/BTSpeak/user.menu`,
  `Lumenna: run python3 <checkout>/apps/btspeak/__main__.py`, added with the device's
  `user_menu.add_item`, as BT Code does. `connect.find_lum` finds `lum` in that checkout's
  `target/` when it is not on `PATH`, which a menu launch's is not.

### The Emacs client

`apps/emacs/` — Elisp over `jsonrpc.el`, built-in libraries only (§16.10), and no transient. Tests are ERT
against a real `lum rpc`, answering the minibuffer by rebinding the reading functions:
`emacs --batch -Q -L apps/emacs -l apps/emacs/test/lumenna-test.el -f ert-run-tests-batch-and-exit`.

- **A client and nothing else**, like the BTSpeak app. A row is said by joining its
  components in words (`lumenna-describe`); nothing is computed.
- **The socket, else a child.** `lumenna-profile-directory` repeats `lum`'s own rule for the
  default profile (the `directories` crate's local data directory), because the socket is
  found by path before there is anything to ask.
- **Depth is an outline level, not just indentation.** `outline-regexp` matches every line
  and `outline-level` reads the `lumenna-level` text property, so folding and Emacspeak's
  level announcements come from outline mode. Indentation is there for the eye.
- **A list's keys are defined once** (`lumenna-define-keys`), which binds them, builds
  the list's Lumenna menu (the global keys go under Tools), and keeps them for `?`: an ordinary buffer, one key per line, RET running it back in the list.
  Not a transient menu: Emacsvox reads a transient by asking whether its command was
  called interactively, and Emacs 31's transient runs each command inside a wrapper of
  its own, so moving through one was silent. A test checks every listed key is bound. The
  global keys are a prefix map, `lumenna-command-map`.
- **Booleans come back as `:json-false`**, which is non-nil. Test them with `lumenna--true`.
- **Spans are bytes**: completion converts both ways (`lumenna--byte-offset`).
- **Writes go through `lumenna-write`**, which refreshes every Lumenna buffer, runs
  `lumenna-changed-functions`, and hands the announcement to `lumenna-announce-function`.
  No advice anywhere: §16.10's `emacspeak-lumenna.el` advising commands became those hooks.
- **Three speech layers, each skipped where it would double up.** Plain echo-area text
  (speechd-el and everyone); faces mapped with `voice-setup-add-map`, which Emacspeak and
  Emacsvox share, plus Emacspeak auditory icons; and under Emacsvox, facts on each line and
  events on the notification lane whose sounds are a module fragment's rules — then the
  icons are not played, and the echo-area copy is shown with `emacsvox-speak-messages` nil.
- **The Emacsvox layer is tested against Emacsvox itself** (`test/lumenna-emacsvox-test.el`,
  skipped without it): its registry validates, each event resolves to its cue, a row
  carries its facts, a change is one notification. Emacsvox is at `~/emacsvox` on the Mac
  and the BT Braille. Rules are one per role and event, not per method — `task.rm` and
  `task.erase` share one, and a rule each played the sound twice. A fact's `:content`
  labels the object in Emacsvox's history and tuning tools; the line is still what is said.

### The iOS app

`apps/apple/` — Swift, a UIKit shell with SwiftUI forms (§16.6), calling the surface
through the generated `LumennaCore.swift`. `cd apps/apple && xcodegen` makes the project from
`project.yml`; the `.xcodeproj` and `Generated/` are build output and not committed.

- **Xcode builds the core itself.** A pre-build phase runs `build-core.sh`, which builds
  `lumenna-ffi` for the platform being built, in a clean environment (Xcode's SDK variables
  break Cargo's host build scripts), then regenerates the bindings from a host build.
- **The core is linked statically, by the `.a`'s full path.** Cargo builds a dylib beside it
  for `uniffi-bindgen`, and `-llumenna_ffi` picks the dylib: the app then loads it from the
  Mac's disk, which works in the simulator and crashes on a phone at launch. A post-build
  phase fails the build if either binary references `liblumenna_ffi`. A launch is not
  proof it ran: check the process is still alive, or the crash logs
  (`xcrun devicectl device info files --domain-type systemCrashLogs`).
- **Every list cell sets its own accessibility label and value.** Left alone, a list cell's
  label is its text *and* secondary text, so a value of the detail says it twice.
- **The cell owns what VoiceOver says**, assembled from row components in `RowSpeech`.
  Depth is said only where it changes.
- **Rows are `UIListContentConfiguration`, not SwiftUI.** §16.6 suggests SwiftUI in a
  `UIHostingConfiguration`; the accessibility audit flagged every hosted `Text` as not
  supporting Dynamic Type and as clipped when the size changed at run time. The stock
  configuration passes. SwiftUI stays for forms.
- **Text entry wraps** (`LineEntry`, a `UITextView` where Return submits). A one-line field
  scrolls sideways at large text sizes and the audit flags it as clipped. Its placeholder is
  the VoiceOver hint.
- **A SwiftUI `TextField`'s title is only a placeholder**: once there is text, VoiceOver reads
  the value and never the field's name. Forms use `field(_:text:)` in `TaskDetailView`, which
  labels the field and hides the visible name so it is not a second stop.
- **Focus after a mutation is chosen, not left to UIKit**: the same row if it is still
  listed, else whatever now holds its position. Then the core's announcement is queued
  behind the focus change, so neither cuts the other off.
- **Swipe actions are the only actions.** UIKit offers a list cell's swipe actions to
  VoiceOver, Switch Control and Full Keyboard Access itself; custom actions set on the cell
  are *added* to those, so setting both lists each one twice. A swipe action's title is its
  spoken name, so it says what it does ("Mark Done", not "Done").
- **`TZ` is set from `TimeZone.current`** at launch and on return to the foreground.
  jiff finds the zone through `TZ` or `/etc/localtime`, and the sandbox is no place to
  rely on the second. A UI test checks that "today" is today where the phone is.
- **A UI test names a fresh store** through `LUMENNA_TEST_PROFILE`, a directory under the
  app's temporary directory. The store otherwise lives in Application Support.
- **Operations run on the main thread.** They take milliseconds against local SQLite, and a
  view never shows a state the store has moved past. The automatic backup is the exception.
- **Four tabs** (Today, Tasks, Browse, Settings), each its own navigation stack. **No bottom
  toolbars**: inside a tab bar the floating bar sits where a toolbar's buttons are, so a tap
  on Undo landed on the Today tab — for VoiceOver too. Actions go in the navigation bar or
  the content.
- **Settings is a short list of pages**, and pushed forms set `hidesBottomBarWhenPushed`.
  Rows scrolled under the glass tab bar fail the contrast audit at any scroll position, and
  a long form costs a VoiceOver user a swipe per row anyway.
- **Sync runs while the app is active** (`Core.startSyncing` in `sceneDidBecomeActive`,
  stopped on entering the background); arrivals post `Core.changed`, which every view
  reloads on. Pairing is by code on iOS — local discovery needs Apple's multicast
  entitlement — through `PairingViewController`, whose `PairingPrompt` blocks the pairing
  thread on a semaphore while the words are asked on the main thread.
- **Stock controls unless there is a reason.** What remains in `Common/FormParts.swift` each
  has one: `NamedRow` (a SwiftUI `TextField` is not named by text beside it; the field
  carries the name and the visible name is hidden, as Apple's UIKit forms do — combining the
  row instead lost the text-field role and read an empty field's placeholder as its value),
  `example(_:)` placeholders (the system placeholder grey fails contrast, and a placeholder
  repeating the name says nothing), and `FormParts.caption` for header and footer text (the
  system grey fails contrast). Give a control an empty title when it has an accessibility
  label, or VoiceOver hears the name twice.
- **The tint is `UIColor.lumennaTint`**: system blue is about 4:1 on white. Set on the window
  for UIKit **and with `.tint` on every SwiftUI form**, since SwiftUI's accent does not take
  the window's tint — that, not the control, is why stock picker checkmarks failed contrast.
  Red text uses `.warningLabel`, never `systemRed`. The app's colours have Increase Contrast
  variants (`accessibilityContrast == .high`), as the system's do.
- **The audits collect every issue and fail once** (`audit()` in the UI tests); left alone,
  the audit stops at the first. Exemptions, all narrow: "potentially inaccessible text" on
  screens built from `NamedRow` (`namedRows: true`), whose hidden names are deliberate; the keyboard's own
  `TUIPredictionViewCell`, and "partially unsupported" Dynamic Type on SwiftUI nodes, which
  the audit reports even for a stock button; `testSettingsPagesAtTheLargestTextSize` keeps
  whole-page screenshots at the largest size showing them scale fully. To see what an unnamed finding
  is, run with `AUDIT_ATTACH=1` (findings left unhandled, so Xcode attaches a screenshot)
  and `xcrun xcresulttool export attachments`.
- **Iroh needs `SystemConfiguration` and `Network`** linked, and the Rust C code must see
  `IPHONEOS_DEPLOYMENT_TARGET` (build-core.sh sets it in its clean environment). A C object
  built without it targets the SDK's own version and the linker warns; heed it, since such
  code can fail on older iOS.
- **Run UI tests with `-collect-test-diagnostics never`.** On any failure xcodebuild
  otherwise collects diagnostics from the simulator, which hangs for its full ten-minute
  timeout after the tests have finished. Two tests take 45 seconds with it off and over ten
  minutes with it on:

  ```
  cd apps/apple && xcodebuild -project Lumenna.xcodeproj -scheme Lumenna \
    -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
    -collect-test-diagnostics never test
  ```

### The macOS app

`apps/apple/Mac/` — AppKit, with SwiftUI only for leaf forms (task detail, block form,
settings pages), per §16.5. `Shared/` holds what both Apple apps compile: `Core`, `Clock`,
`RowSpeech`, the wording helpers, and **`Shared/Forms/`: the task, block and settings forms,
models and views both**. Each app supplies only presentation — a hosting controller, its
pickers (`TaskFormHost`), its file panels (`SettingsModel` extensions). Where the platforms
need different answers it is an `#if os` in `FormParts.swift` (`Named`, `Labelled`,
`ChoiceSection`) or at the one row that differs, so a fix to a form lands on both.
Section titles are `FormParts.heading`, a header to VoiceOver's heading commands. Scheme
`LumennaMac`.

- **Not sandboxed, and shares `lum`'s profile** (`~/Library/Application Support/lumenna`):
  the app and the command line on one Mac are one device with one store. `Core` polls
  `refresh` once a second there, since `lum` is another process writing to it.
- **Resident in the menu bar, and the device's sync process** (§16.2): closing the window
  hides it, `NSStatusItem` and the Dock bring it back, and it syncs for as long as it runs.
  No daemon or service is needed or used.
- **Global shortcuts through Carbon's `RegisterEventHotKey`**, which needs no accessibility
  permission: Control-Command-L shows the window, Control-Command-K opens quick add, both
  changeable in Settings (this Mac's own defaults). Never Control-Option: it is VoiceOver's
  modifier. The recorder warns of clashes with VoiceOver, VOCR (Control-Command S, P, and L, I while it navigates a scan;
  Shift-Control-Command more) and macOS (Control-Command Q, F, D, Space).
- **Three panes** (`NSSplitViewController`): a source-list sidebar of places, the list
  (`NSOutlineView` for tasks and the day), the task's details. F6 and Shift-F6 move between
  them. Actions are in the row's context menu (VO-Shift-M), the menu bar, and keys on the
  outline itself (Space completes, Delete trashes, Return opens), from one `TaskActions`.
- **⌘Z undoes typing while a field is being edited**, and otherwise the store (`undoChange:`).
- **The core has a record called `Timer`**, so Foundation's is named in full.
- **SwiftUI form controls go inside `Named`**: on macOS a `LabeledContent`, since a bare
  `TextField("Title", …)` in a form showed its name as separate text and left the field
  unnamed, and labelling it as well made it "Title, Title". On iOS the opposite (see the
  iOS app's `NamedRow` note). A growing text field draws its name in grey on macOS, under
  contrast, so Notes is a `TextEditor` there.
- **SwiftUI is hosted through `HostedForm`** (`Mac/Core/HostedForm.swift`): the hosting view
  steps out of VoiceOver's tree and its name goes to the form's scroll area, so a page is
  one named scroll area rather than a group around one. It has to be the hosting view's
  own override; set from outside, it goes on reporting itself as a group.
- **A table cell recolours itself on selection** (`TwoLineCell.backgroundStyle`): the app's
  quiet grey on the selection highlight fails contrast.
- **The window's minimum size is set** after its content: the split view otherwise shrinks
  it to almost nothing, and a frame saved that small is replaced.
- **UI tests enter text by pasting** (`enter(_:into:)`), restoring the clipboard after. The
  machine they run on has VoiceOver on, and XCUITest's synthesized keystrokes reached it —
  it opened its Item Chooser and swallowed the rest. Windows are found by identifier
  (`main`, `settings`): the first window can be Siri's, and tabs retitle Settings.
- **The Mac audit judges only the window in front** (or the open sheet): the system dims
  the rest, and dimmed text fails contrast without being what anyone reads. Also excused:
  the Touch Bar (above the screen's top), SwiftUI pop-ups' "Action is missing", and a
  "Parent/Child mismatch" with no element at all.
- UI tests need macOS to have authorized UI automation for Xcode once.

### The Windows app

`apps/windows/` — Rust over `windows-rs`, linking `lumenna-surface` directly (§16.4: no FFI),
binary `lumenna.exe`, at parity with the Mac app. Everything Win32 is in `src/win/` behind
`cfg(windows)`; shortcuts (`shortcut.rs`) build and are tested on every platform. How rows
are worded (`speech`), flat rows into a tree (`outline`), the sidebar's places (`places`), the
week's work blocks (`choices`), device and export wording (`devices`) and the command line
(`profile`) are `crates/desktop`'s, shared with the GTK app. The task form's diff is the
surface's (`task_edit`, below), as it is every client's.

- **Building on ARM64 Windows needs clang on `PATH`** for `ring` (under Iroh's TLS). Visual
  Studio ships one: `VC\Tools\Llvm\ARM64\bin`. Nothing else is needed — the manifest
  (Common Controls 6, per-monitor DPI v2) is embedded by the MSVC linker from `build.rs`,
  with no `.rc` file.
- **Stock controls only.** Every list is a `SysTreeView32` (`win/tree.rs`), so level,
  position, set size, expansion and checkboxes are the control's to report; an item's text is
  the row's components joined, with exactly those left out. Dialogs and Settings' property
  sheet are in-memory templates run by the dialog manager (`win/dialog.rs`); the file
  dialogs are the shell's own (`win/system.rs`).
- **Names and the live region go through Dynamic Annotation** (`IAccPropServices`,
  `win/a11y.rs`), never a provider of our own. The status line is a polite live region:
  `EVENT_OBJECT_LIVEREGIONCHANGED` after setting its text is how a change is announced. A
  dialog in front hides it, so each settings page and the pairing dialog has its own.
- **An empty static is named after the label made just before it**, so a page's status line
  is made first (placed at the bottom) — or it would read the page's footer twice.
- **A reload updates a tree in place when its rows are the same ones** (same keys, same
  order), so the one-second refresh and arriving syncs do not make the screen reader read
  the focused row again. Otherwise it rebuilds, and the selection goes back to the same key
  or the row now at its position.
- **Nothing that rebuilds a tree runs inside one of its notifications.** Space, Delete and
  double-click go through `App::defer` (a posted `WM_DEFERRED`). `Tree::busy` marks
  notifications sent by a refill, which the views ignore.
- **What can be done to a task is in one place** (`win/task_actions.rs`): the list's context
  menu, the Task menu and the details pane's buttons, acting on the task in hand — the list's,
  a sitting's on the day, or the details' when focus is there — as the Mac's `TaskActions`.
- **IsDialogMessage runs over the whole main window**, with the panes `WS_EX_CONTROLPARENT`,
  so Tab, mnemonics, Enter (`IDOK`) and Escape (`IDCANCEL`) work everywhere; menu command
  ids are never 1 or 2 for that reason. Field mnemonics avoid the menu bar's letters (F E V
  T D H): the dialog manager gives Alt+letter to a field first. F6 is explicit
  (`App::move_pane`).
- **A tree view passes a keyboard context menu up altered**: `WM_CONTEXTMENU` arrives naming
  the pane, at (−2, −2). So a request from a pane is taken as from the keyboard, for what has
  focus there, and right-clicks are handled from `NM_RCLICK` instead.
- **Never disable the control that has focus** without moving focus first: it leaves focus
  nowhere (found in the pairing dialog).
- **The Notes field gives Tab and Escape back** (`WM_GETDLGCODE` in `detail.rs`), and panes
  ignore `WM_CLOSE`, which a multi-line edit sends its parent on Escape.
- **Settings apply as they are made**, as the Mac's do (a text field when it is left, or the
  sheet closes), so the sheet has one button, Close. The global shortcuts and opening at
  sign-in are this PC's: `HKCU\Software\Lumenna\Shortcuts` (0 is off) and the Run key,
  which starts it with `--background` (no window) and `--profile` for a profile not the
  usual one. Shortcuts are unregistered while new keys are chosen, so pressing the old ones
  reaches the hotkey control.
- **Resident, as the Mac app is**: closing hides; the tray icon, Control+Alt+Shift+L (show)
  and Control+Alt+Shift+K (quick add from anywhere) bring it back; starting it again shows
  the running instance (a mutex and window class named from the profile path). It shares
  `lum`'s profile and syncs while it runs. The first launch triggers the firewall prompt,
  since the sync endpoint listens.
- **Completion is a popup menu**, decided (§6.4): Down or Ctrl+Space in the filter and
  quick-add fields. Items lead with the name ("Work, project") so a letter finds them, and
  the first is highlighted by a Down queued before the menu opens — marked with a reserved
  bit of the key data, so if the menu does not take it the field swallows it rather than
  opening the menu again.
- **UI tests** (`tests/ui.rs`) start the real app on a store of their own, drive it through
  its message queue and assert on native UI Automation — what NVDA and Narrator are given.
  They open windows and take the foreground, so they are ignored by default and take turns:
  `cargo test -p lumenna-windows --test ui -- --ignored` (about 40 seconds). A posted key
  needs the app active and not minimized; `Automation::activate` sees to both.
- **`examples/inspect.rs`** is the same automation by hand: `cargo run -p lumenna-windows
  --example inspect -- "- Lumenna"` prints the tree; `--post` takes steps (keys, `text:`,
  `cmd:<menu id>` for a Ctrl shortcut, `context`, `focus:Name`, `select:Name`, `invoke:Name`,
  `dump`) and reports focus and the status line after each. Match `- Lumenna`, not
  `Lumenna`: a terminal or folder named after the checkout matches that too. `focus:`
  selects the field's text as tabbing in does; focus moved by UI Automation alone leaves the
  caret at the start, and typing goes in front of what is there. The managed
  `System.Windows.Automation` in Windows PowerShell is no substitute: x64 under emulation, it
  saw every control as an unnamed pane.
- **Not built yet**: what no other client has either (history, reminders), and an icon of
  its own.

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
