# Roadmap

What Lumenna does not do yet, with the decisions already made about it, in the order it is
to be done. What is built is not listed: the code says how it works, and `CLAUDE.md` says
what is easy to get wrong.

Every client gets every feature. A platform may get it later, never a smaller version of it.

The order, and why:

1. **Safeguards first**: CI, now built: every app and the core on every push.
2. **Reminders**, the biggest gap in daily use, and the scheduler the planner's overrun
   warning and hooks' time events also need.
3. **The planner**, in dependency order: urgency and history, suggestions, carrying work
   forward and day commands, re-flow, the whole-day plan.
4. **Smaller gaps**, threaded through the two above.
5. **The watches**: the watchOS spike early, since it decides the toolchain and whether the
   watch can be a full peer; then both watch apps.
6. **Integrations**, once reminders and the planner exist for them to expose.
7. **Before others use it**: packaging, real erasure, installing the web client.
8. **The encrypted store and accounts**, which the web's reminders and syncing alone wait on.
9. **Only when needed**: performance and network extras.

## 1. Safeguards

- **CI** runs (`.github/workflows/ci.yml`). Still to come with packaging: release builds,
  signing, and publishing from it.

## 2. Reminders and time events

The core and the surface are built: when each reminder fires, across daylight-saving
changes; what it says; one process per device delivering; dismissing and snoozing, synced;
`lum reminder` and `lum notify`. Still to come:

- **Delivery in every app** (below), each over `deliver_reminders` or `upcoming_reminders`.
- **Setting reminders in every app**: a task's and a block's own, and the defaults in
  settings (a phrase such as `5m before start` for the defaults' setting).
- **Reminders at a fixed moment** exist in the model and the scheduler; nothing sets one.
- **The same scheduler for hooks' time events and the planner's overrun warning.**
- **Delivery to some devices only** (`Delivery`) has no way to be set.

**Who delivers.** Whatever process owns the output, and never the daemon: on a Linux desktop
the tray app does it better, without one no notification service is listening, and a system
unit has no session bus.

- **macOS, Windows, GTK**: the resident app. Windows toasts need an app identity (AUMID);
  Linux uses the notification portal.
- **iOS and watchOS**: local notifications, at most 64 pending, so a rolling window of about
  48 hours, re-armed on foreground, background refresh and sync.
- **Android**: `SCHEDULE_EXACT_ALARM` (Android 13 and later) for time-critical reminders,
  degrading if refused. Wear OS mirrors the phone, with duplicates suppressed.
- **Console Linux**: `lum notify`, a foreground process started from the shell profile,
  writing to the controlling terminal (which speakup and BRLTTY read), and `spd-say` when
  speech-dispatcher runs.
- **BTSpeak**: a small resident Python service, apart from the sync daemon: code in
  `/BTSpeak/Services/`, its unit in `/BTSpeak/Systemd/` (as `btspeak-calendar-reminders`
  does), a `Settings/lumenna-reminders.values` toggle. The notification sound, then
  `host.say()` and `brl.push_message()`; acks so a restart does not re-announce the day. No
  reminder logic in Python.
- **Emacs**: announcements while it runs, through `notifications.el` or `message`, as a
  convenience only.
- **The web**: see the web client below.

## 3. Planning

The planner is four capabilities: urgency, suggestions for a block, re-flow when the day
slips, and a whole-day plan. None is built. Per-task history comes with urgency: it is
cheap, and its facts ("rescheduled 7 times") feed suggestions' reasons. What they will read
already exists: block minimum lengths and compression floors, the `anchored`,
`accepts_tasks` and `counts_capacity` flags, the day window, block task filters, project
weights, and the day's timeline with its free time.

### Rules for all of it

- **Pure, deterministic functions over the day's state**, in the core, property-tested.
- **A proposal, never a mutation.** An accepted proposal is applied through the ordinary
  operations, so it can be undone and it syncs. A day that reorganises itself unannounced is
  disorienting, above all to someone who cannot glance at it.
- **Every proposal can be said in full before it is accepted**: "Deep work moves from 9:00
  to 9:45; Email shortens from 30 to 15 minutes; Review PR does not fit and moves to
  tomorrow." For this audience the description *is* the verification.
- **Constraints bind the planner, never the person.** A block's minimum length, `anchored`
  and the day window say what may be *proposed*; no operation consults them, and someone can
  still shorten a break to five minutes or delete it. The minimum is hard in the main
  proposal, soft in alternatives (which may break it if they say what it costs), and absent
  for anything the person does directly.
- **No language model.** It cannot explain itself or be property-tested, and it would send
  the day to a server.

### Urgency and history

Built: the score and its terms (`task_history`, `lum task history`), and per-task history
from the records already kept, on a task's own screen. Still to come:

- **The coefficients as synced settings**, so ranking agrees across devices; today they are
  `Coefficients::default()`.
- **Ordering by urgency**: what block suggestions rank by, and a `sort: urgency` clause the
  filter language lacks (filters have no `sort` clause at all; comparison operators stay out).
  Whether a task list's *default* order becomes urgency rather than the person's own is a
  decision to make with the lists' tree in mind.
- **A history view in every app** beyond the task's own screen, and `lum history`.
  Field-level history from Automerge (title and due changes over time) later, with its own
  API.
- If a task becomes permanent furniture at the top, a staleness question ("this has been at
  the top for three weeks; is it real?") rather than a bigger formula.

### Suggestions for a block

Built: `core::suggestions`, behind Assign a Task, each choice's `reason` its spoken
description; and Put in a Block offering only the blocks whose filter admits the task.
Still to come:

- **"What should I work on?"** from a block under way, and for filling free time: the same
  ranking, asked for from the day.
- **Every app showing a choice's `reason`** as its description (accessible description,
  hint, or a second line).

### Carrying unfinished work forward, and day commands

Built (`day.rs`, `lum end`, `extend`, `end-day`, `clear-day`, `carry`): End Now and Add 15
Minutes on the block under way, End the Day and Clear the Rest of the Day, and Carry Forward
on a sitting left short of its planned time, each one undoable change. Still to come:

- **Offering it at the block's end**: "Deep work ended. Essay draft: 40 of 90 minutes. Carry
  the remaining 50 to tomorrow's Deep work?", as the block-end reminder's own action, with
  the next block whose filter admits the task chosen first and overfill reported. Never
  carried automatically: the question is where "the estimate was wrong" gets noticed.
- **Replan from now**, which is re-flow, below.

### Re-flow when the day slips

Built (`core::reflow`, `replan.rs`, `lum replan [--keep] [--skip] [--apply [1,3]]`, RPC
`replan` and `replan.apply`): the walk, moving the work first, pins, the proposal said in
the app's clock, applied whole or per change against the day as it is now, one undo. Still
to come: the Replan from Now day action and each app's screen for it (the action is held
back until they exist), alternatives, the overrun warning on the scheduler, and staying
silent. As designed:

- **Three situations, kept apart.** Assigned work exceeding the block is a plan that was
  wrong when made, predictable in advance. A block running past its end, and a block that
  never happened because the one before ran through it, are slips.
- **Say it before the clock does**: fire when the remaining assigned work first exceeds the
  block's remaining time. A sighted person sees that on a timeline; a list must compute and
  say it. It runs on the reminder scheduler, anchored to block ends.
- **The walk**: forward from now. Anchored blocks are fixed and routed around. A movable
  block with slack above its minimum shrinks; one at its minimum moves later. A break's
  minimum is its whole length by default, so a break moves rather than being squeezed to
  three minutes; a work block may shrink to about half, never below 15 minutes. Nothing
  moves before now or past the day window. What does not fit is `unplaceable`, and said: the
  planner cannot create time.
- **Move the work before moving the day.** Deferring a task out of an overrunning block
  touches no commitment; a block is a commitment, an assignment only a plan.
- **One concrete proposal**, accepted whole, per change, or refused. Alternatives sit behind
  "other options" and name their cost ("or skip the 3:00 break: your only break today").
  Never a menu of strategies.
- **Re-entrant**: the person's decisions ("skip the 3:00 break", "keep this here") come back
  as pinned constraints and the proposal is recomputed, never applied on stale assumptions.
  Skipping is a cancelled occurrence and "end at 4" a modified one, both of which exist.
- **An anchored block already under way** (a class that started ten minutes ago) is a
  conflict to report (stop now, or miss it), never a re-flow that pretends it moved.
- **Stay silent** when the overrun fits the gap after it, is under a few minutes, or was
  dismissed for this block.

### The whole-day plan

`propose_day(date, now, pinned)`: walk the day's work blocks in order, run suggestions for
each, and assign greedily until the estimates fill it. One proposal, naming what did not
fit, pinned the same way. Greedy and editable is worth more than optimal packing. Always
optional.

### Smaller planning gaps

- **Planning against a dependency** is allowed without a word: task B can go in a block
  before the one holding the task it waits for.
- **Sittings cannot be reordered** within a block once assigned.
- **Skipped and Deferred sittings** exist in the model and nothing sets them.

## 4. Smaller gaps

A day or less each, for between the larger pieces.

### Tasks, blocks and text entry

- **Reordering tasks among siblings.** New tasks go last and only "move to top" exists. The
  fractional `order` key supports any position; ties break by identifier.
- **Completed subtasks**: a setting to show them, and a completion count on the parent row.
- **Time zones** on tasks and blocks are stored and nothing can set them. Normally absent,
  so times float; set only for something tied to a real place.
- **Colours** for projects and saved filters exist in the model with no way to set them.
  Block icons likewise. Colour never carries meaning alone.
- **Verbosity is stored and read by nothing.** Terse ("Review PR, overdue") or full (the
  whole sentence), applied by the core so every target behaves alike.
- **Week start is stored and read by nothing**: week-relative phrases, the block chooser's
  coming week.
- **Quick add for blocks**, in the same grammar: `block "Deep work" 9-11am weekdays`.
- **The block editor** should move a start date that does not match the repetition to the
  first real occurrence ("every Monday, starting Tuesday" never occurs on its start date).

### Completion

The core's `complete` works everywhere; what is missing is how each app offers it.

- **Every field written in the quick-add or filter language completes**, not only quick add
  and the task list's filter: the saved-filter editor and a block's "Tasks from" field have
  it in Emacs only.
- **Down arrow opens it on every keyboard.** Missing on the Mac (Option-Escape or F5 only,
  and it opens unasked after `#` and `@`, which GTK chose not to do because a menu
  interrupts someone typing a name straight through: decide), on iPad and Android hardware
  keyboards, and Ctrl+Space on the web. The web's filter field has none.
- **iOS and Android** offer a strip that changes as one types. The design was an
  accessibility action on the field ("Show completions") opening a modal list, which has
  clear edges where a strip leaves one unsure whether it is there right now. Decide, then
  make both alike.
- **BTSpeak**: a key inside the input dialog (Ctrl-L, dots 7-8 plus L, once checked against
  global BRLTTY chords) completing at the cursor, never opening on `#` or `@` by itself.
  Needs an `InputDialog` subclass; today choices come after Enter, for the last word only.
- **The shell**: `lum completions` is static. clap_complete's dynamic mode would complete
  project, label and filter names from the store.

### Each app

- **iOS**: custom rotors (overdue, running, next block).
- **iPad**, found by its UI tests, each skipped there until fixed:
  - ⌘Z right after quick add closes does not reach the list (it does from the list itself).
  - A block's Delete swipe action does not appear to the test in Browse's Blocks list.
  - The audit fails contrast on the task form's About heading, which looks like the
    headings that pass.
  - In the simulator, the first tap on a button of an alert with an untouched text field
    only ends editing (Skip when assigning a task). Try it on a real iPad; VoiceOver, which
    activates rather than taps, is not affected.
  - ⌘F never reaches the app in the simulator; try it on a real iPad too.
  - On the 11-inch iPad simulator in portrait, the first ⌘, after launch opens the system's
    Settings app instead of ours (the menu bar does replace that item); try a real iPad.
  - In CI the iPad suite takes over half an hour on the runner and times out where it
    passes locally, so its job reports without blocking: make it fit the runner (fewer
    simulator round trips, or splitting it across jobs), then make it block again.
- **Android**: pairing on the local network untried on a real phone (the emulator's NAT
  passes no multicast); the iPhone's two pairing tests not ported. The keyboard commands
  and the wide layout are tested in the emulator, not yet with TalkBack and a real keyboard.
- **GTK**: type-ahead in lists (the Windows and Mac lists jump to a row by its first
  letters; GTK's list view does not); shortcuts from anywhere without the GlobalShortcuts
  portal (an X11 key grab for Xfce and older GNOME); no tray on GNOME without the
  AppIndicator extension; "expanded" said twice on first focus of a row with subtasks.
- **Emacs**: priority faces, mapped to voices.
- **Every app**: icons of their own (GTK and Windows have none).
- **iOS UI tests that fail only when the simulator delivers a key out of order**: ⌘K (the K
  arriving before the ⌘, so VoiceOver's focus test sees nothing done) and ⌘F on CI. They
  are in the full suites, which only report.
- **Braille apart from speech**: rows could show the core's braille short forms ("ovd")
  while speech says the word. On the web through `aria-braillelabel`, which JAWS and NVDA
  read; on Windows only through JAWS scripts or an NVDA add-on, since UI Automation gives a
  control one name, so only if braille users there ask.
- **Screen readers not yet tried by a person**: JAWS, Narrator, ChromeVox, Emacspeak and
  speechd-el. NVDA (Windows and the web), Orca, VoiceOver on the Mac and iPhone, TalkBack,
  Emacsvox and the BTSpeak have been. The braille short forms for states and roles are
  unchecked against BTBraille's tree-view convention.
- **Snapshot tests of the row projection** for task lists and the day: index and count,
  expanded and checked, states, speech and braille.

## 5. The watches

Both have every view and work with the phone away, each with a whole store of its own.

- **watchOS** is built (`apps/apple/Watch`): the whole core without Iroh, since watchOS allows
  no sockets outside an audio session (TN3135; even a WebSocket counts), syncing with its
  iPhone over WatchConnectivity, which the phone passes on. Still to come, in this order:
  - **On a real watch**: TN3135 is not enforced in the simulator, nor is memory, and
    Automerge loads whole documents.
  - **Syncing out of reach**: queued for when the phone is next reachable
    (`transferUserInfo`, which needs the exchange to carry changes without a reply), and
    background refresh (`WKApplicationRefreshBackgroundTask`).
  - **Complications** through WidgetKit, and the running timer on the watch face.
  - **Later, if it matters**: syncing far from the phone through a relay of our own that the
    watch polls over plain HTTPS, the one networking watchOS allows.
- **Wear OS** is built (`apps/android/wear`): a standalone full peer over the phone's shared
  module, pairing and syncing itself. Still to come: on the real watch, rotary scrolling with
  TalkBack and pairing over the network; tiles and complications, with the widgets.
- **Toolchain**: unpinned.

## 6. Integrations

Each is a thin adapter over the command surface.

- **App Intents** (iOS and macOS; watchOS later): one implementation for Siri, Shortcuts,
  Spotlight, widgets, Control Center and the Action Button. `AppEntity` and `EntityQuery`
  for tasks, projects and blocks so Shortcuts can find things, not only make them;
  `AppShortcutsProvider` for Siri phrases with no setup. Add (in the quick-add grammar),
  complete, query by filter, today's plan, assign, start and stop. Hands-free capture is an
  accessibility feature. Check for an assistant-schema domain for tasks when building.
- **A URL scheme** beside them, `lumenna://add?text=…` (one task per line, in the quick-add
  grammar), so a Drafts action, or any app's x-callback-url, sends text in as tasks.
- **Android App Functions** (`androidx.appfunctions`, for Gemini) with AppSearch for
  retrieval; not the old App Actions. The same verbs as App Intents: add, what is next,
  start and stop a sitting's timer, complete. Through Gemini they reach the phone, the watch,
  the car and audio glasses alike.
- **A "Today" shortcut** on the iOS and Android icons, beside New Task.
- **Widgets**: WidgetKit (iOS, macOS) and Glance (Android): today's blocks and what is due.
- **PowerShell module** wrapping `lum --json`; the same language as Windows hooks.
- **`lum mcp`**, in the same binary so versions on a machine cannot drift. Tasks and the day
  are resources, operations are tools, plus "undo last action". stdio, and Streamable HTTP
  (not HTTP+SSE) as a daemon listener, since Home Assistant usually runs elsewhere:
  loopback by default, a bearer token required off loopback, no TLS (a reverse proxy or
  Tailscale). Read-only unless writes are turned on. Say plainly that a cloud-hosted model
  sends what it reads off the device; the point is a fully local voice pipeline.
- **Hooks**, `lum hook add|list|test|rm`:
  - **Reactive, never interceptive**: a change can arrive already committed from an
    unreachable device, and convergence cannot be vetoed. Hooks fire after a change is
    durable, see before and after, may make further changes, and cannot report failure to
    the person who made the change (log it; show it in sync status).
  - Change events: task created, completed, uncompleted, updated, deleted; assignment
    created, removed; timer started, stopped; block edited. Time events, from the scheduler:
    block started, ended, upcoming(offset); day start, end; task due. A time event past its
    hook's maximum lateness (a few minutes by default) is dropped and logged: Do Not Disturb
    40 minutes late is wrong.
  - `Hook { event, filter, action, max_lateness_secs }`, the filter in the filter language.
  - **Local only, never synced**: a hook running `ssh` belongs to one machine, and synced
    hooks would be remote code execution for anyone who can write the document. On one
    machine only the sync-lock holder runs them.
  - The event is JSON on stdin in `lum --json`'s shape, common fields in the environment.
    argv is run directly, without a shell unless asked. Windows: `pwsh -NoProfile
    -ExecutionPolicy Bypass -File` (without Bypass, unsigned scripts fail silently).
    Android: a broadcast, `com.lumenna.EVENT`, for Tasker and the like.
  - With nothing resident, hooks run late over the backlog; there is no ordering across
    devices; an always-on peer is the best host.
- **Calendar import**, read-only first (so the planner knows about real meetings), through
  `ExternalRef` (EventKit, CalDAV, ICS URL), which exists in the model only; `lum calendar
  add|list|rm`. Read-only must be enforced on edit. Two-way sync is a separate project.
  Importing an `.ics` file first, which Google Calendar exports and which needs no API.
  Google Calendar's API is open to any project, but its scopes are sensitive: an app used by
  the public needs Google's OAuth verification (no paid audit), and an unverified one is
  limited to 100 people and warns them. PKCE serves a desktop app that cannot keep a secret.
- **Imports from other systems**: Todoist first, through its public API and a personal API
  token the person copies from Todoist's settings, so nothing needs registering (check the
  current API version when building; its priority 4 is P1). Then Jira, Linear and GitHub,
  which `ExternalRef` was shaped for.

## 7. Before others use it

- **Packaging**: a `lumenna` alias for `lum`, a Homebrew formula (`brew services start
  lum`), a winget manifest, Flatpak and AppStream for GTK. A command-line user has no app to
  prompt for updates, so packages matter most there.
- **Fields this build does not know, everywhere**: kept by edits and by the store's own
  rewrites, but not yet by a record restored by undoing a delete from the trash, nor by JSON
  export and import, which carry only what the model holds. Keeping them means the model
  carrying what it cannot read.
- **Real erasure.** Erasing a task removes it from the current state only: its content stays
  in the history and in backups, and undo brings it back. Truly erasing means rebuilding the
  document without it, losing all history, every device syncing afresh, and clearing the
  undo table's copies. Rare and expensive, so it says so: "Permanently erase — rebuilds your
  database and re-syncs all devices."
- **Property tests**: back up and restore a randomly generated store; an export never holds
  a deleted task (a privacy guarantee, not formatting).
- **An always-on peer**, documented: `lum daemon` on hardware the person controls. A VPS
  works, but say the provider can read the data, and make no encryption claim: unattended
  boot and provider-proof encryption conflict.
- **Installing the web client as a PWA**: a manifest and service worker;
  `navigator.storage.persist()`, because OPFS can be evicted and the device key with it;
  offline loading; a badge for what is due; Background Sync to send pending changes when the
  connection returns; File System Access as a backup destination.
- **Browsers for the web client**: only Chromium is tested. Safari, Firefox (OPFS sync access
  handles there) and ChromeVox are unchecked; an IndexedDB fallback for browsers without sync
  access handles is optional.

## 8. The encrypted store and accounts

Storage for change chunks that are encrypted before they leave a device, addressed by hash
and served on request. It never runs Automerge and never holds a key, so it is encrypted at
rest by construction. Clients find what they lack by exchanging hash lists. It must store,
not just forward: forwarding helps only when two devices are already awake. It is what a web
client alone syncs through, what optional hosting the operator cannot read would be, and
perhaps where attachments live. Call it the *encrypted store* (`lum store`), never a "blind
store" or a "relay" (Iroh's word). It must be cheap to run for many people. Meanwhile, the
sync layer must gain no assumption that makes a hash-addressed, opaque-chunk transport hard
to add.

**Keys.** One `account_key`, 32 random bytes made once, encrypts chunks. The password is
never the key: changing it would mean re-encrypting every append-only chunk and breaking
other devices until done. The key is stored in wrappers, each `{kind, salt, kdf params,
wrapped key, label}` with its own salt and KDF, so each credential rotates alone and only the
password is stretched:

- **Password**: Argon2id, about 64 MiB, t=3. Required: joining with no other device
  reachable needs something memorable.
- **Recovery phrase**: 12 BIP39 words (128 bits) through HKDF, made at setup and never
  offered again, for when every device is gone. A phrase, not a QR code: it can be read
  aloud, copied or saved to a file (offer all three), and its checksum catches
  transcription errors. Never shown or accepted in a browser.
- **Key file**: a random wrapping key through HKDF, for unattended setups. Not the account
  key on disk, so a leaked file can be revoked. `label` tells them apart.

**Authenticating.** `auth_value = HKDF(account_key, "store-auth")`; the store keeps only its
hash. Derived from the key, not the password, so it never changes and a password change is
invisible to the store. `account_key` and `auth_value` live in `local_state` and never sync;
the password is never kept. Joining with a password: fetch the wrappers by account name
without authenticating, run Argon2id, unwrap the key, derive `auth_value`, authenticate,
fetch chunks.

**The key never goes in a document**: Automerge history is permanent, so it would survive any
rotation. It reaches a new device over the pairing channel after the words are confirmed, or
by unwrapping from the store.

**Joining by account.** `lum store add <url>`. A device that signs in can read and write
through the store and learns every device from the device list, then dials the first one
awake for the ordinary word comparison. Until then it is pending: not in the device list,
no direct connections. An unexpected prompt elsewhere is the alarm that someone enrolled with
the password. A store is never paired with; it has no device identity.

**Rotation.** A credential (new password, key file or phrase) is re-wrapped with a fresh salt
and its wrapper replaced: cheap, local, needing no old password, from any enrolled device;
the normal recovery path, offered before the phrase. Rotating `account_key` itself is what
truly revokes a stolen device, and means re-encrypting everything and moving every other
device to the new key: **unsolved**. Keep saying which is which: unpair a device you
replaced, rotate the key for one that was stolen. A lost `auth_value` makes the store
disposable: a device with the key makes a new account and uploads again.

**Accounts.** A plain username the person chooses. Wrappers are fetchable without
authenticating, so offline guessing is possible regardless; a random identifier would only
get written down beside the password. No email: the store never sends mail, a reset is
impossible by construction, and an address is personal data for nothing. Billing, if any,
stays with the payment processor.

**Passwords.** At about 10⁶ guesses a second offline: 40 bits takes days, four words decades,
five words about 10⁵ years. Offer a generated five-word passphrase, and make accepting it the
easy path. A chosen password has a floor, estimated zxcvbn-style and stated in words ("about
six days to guess"), never a coloured bar. Rate-limit wrapper fetches per account.

**Say plainly**, once, at setup: against a hostile operator, the password is the only
protection. "We cannot read your tasks" is true; "we learn nothing" is not (identity, chunk
counts and sizes, timing, which devices connect when).

What the web client gains from it:

- **Reminders**: only as an installed PWA, through a classic service-worker push (not
  Declarative Web Push, which needs plaintext at the sender). The wake-up carries no
  content: the service worker syncs, works out what is due, and shows it, within Chrome's
  budget for pushes that show nothing. The sender is an awake peer that can decrypt, or the
  encrypted store sending a content-free wake-up on a schedule the person chose to give it
  (off by default, the trade-off stated).
- **Keys**: the account key as a non-extractable WebCrypto key.
- **Verifiability**: subresource integrity and a published bundle hash, and a one-time plain
  notice at sign-in that a web client is harder to verify than a native one.

## 9. Only when needed

- **Watching the WAL** (inotify, FSEvents, `ReadDirectoryChangesW`) instead of polling
  `data_version` every second; keep the poll where watching is unreliable, such as network
  filesystems.
- **Closed years**: a finished year's document is history. Leave it out of the default sync
  set and fetch it on demand; the always-on peer keeps every year. Nothing is ever
  discarded: pruning history would break offline replicas.
- **Sync status**: live reachability per device. `Device.last_seen` is written at pairing
  and never again; status comes from this device's own record, so leave the synced field.
- **A relay of one's own**: `node.rs` always uses n0's relays.
- **Pairing across networks by a spoken code**: the cross-network code is the pairing key,
  fine to paste and too long to read aloud. An optional rendezvous service could hold it
  under a short code; the words still confirm both sides, and pasting must keep working.
- **Unattended provisioning**: a long pre-shared token in a headless device's config file in
  place of the words. Everything a person does still goes through the words.
- **Wake-up push** (optional, can be turned off): a silent APNs push when a device changes
  something, on top of background refresh, never instead; Web Push with VAPID for an
  installed web client. The push server learns timing, so say so, and batch with jitter. A
  sync wake-up needs no data; a reminder does, which is why the encrypted store can never
  send reminders.
- **The SQLite read model**, only if queries become slow or full-text search is wanted: a
  rebuildable projection (flattened tasks with depth and path, date indexes, FTS5, block
  occurrences for a window), recording the Automerge heads it reflects, filled in the same
  transaction as the changes, producing the same core structs. Filters would then compile
  partly to SQL, with the rest evaluated in memory over what remains.
- **The web's WASM** is 8.7 MB (3 MB compressed), mostly Iroh; worth trimming.
- **The web takes no automatic backups**; it asks for downloads. Revisit if a browser path
  appears; a backup must live outside the live store.

- **An Outlook add-in** for turning an email into a task, only if email import is wanted.
  Office add-ins are web pages Office loads from a URL and cannot reach the local socket, so
  it would need a localhost endpoint in the desktop apps.

## Open questions

- **Rotating `account_key`** after a device is stolen (above).
- **A visual, proportional timeline**, Tiimo or Structured style, beside the list every day
  view is now: perhaps the most valuable addition for people with ADHD.
- **Location-based reminders**: mobile only (iOS caps regions at 20, so a rolling window;
  Android's Play review makes it a later submission; impossible on watchOS). Places entered
  by "use my current location", not address search.
- **Attachments**, which would rest on the encrypted store as a hash-addressed blob store.
- **Templates** for tasks and projects; try "duplicate this subtree" first.
- **Paid hosting** of an encrypted store, and whether any app is ever paid (decide before an
  App Store launch).

## Decided against

- **Alexa, AppleScript, WSH/COM**: a cloud assistant cannot read what the cloud cannot
  decrypt; Shortcuts and `lum --json` cover scripting.
- **A hosted MCP endpoint**, for the same reason, and TLS inside `lum mcp`.
- **Interceptive or synced hooks**, and hooks on iOS, watchOS, Wear OS and the web.
- **A language model in the planner, optimal packing, a strategy menu, automatic
  carry-forward.**
- **A daemon that delivers reminders, or that coordination requires.**
- **Pruning Automerge history**; saved per-peer sync state.
- **A second pairing mechanism** (a PAKE, short typed codes): one mechanism for everything
  a person does.
- **An email or random identifier for accounts; a QR code for recovery.**
- **Comparison operators in filters.**
- **A TUI** for `lum`.
- **A projected view for display glasses**: Gemini, through App Functions, serves glasses
  as it serves the phone; what glasses add to an app like this is speech.
- **Scripts for braille display keys**: the apps' keyboard commands already reach them
  through each screen reader's keystroke emulation, and a key that is hard to send from a
  braille keyboard is fixed in the app, for everyone.
