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

The model is built: `Reminder` (targeting a task or a block series, anchored to a due date,
a block's start or end, or an absolute time, with an offset that may be after), `Delivery`
(every device, only these, all but these), `ReminderAck`, and the all-day hour. Nothing
creates, lists, schedules, delivers, dismisses or snoozes one, and the default task and
block reminders cannot be set.

- **The core schedules**: `reminders_due(since, now)`, over the FFI and RPC. The same
  scheduler serves hooks' time events and the planner's overrun warning.
- **Every device schedules its own notifications from synced data**, with no coordinator.
  Dismissing or snoozing writes a `ReminderAck`, which syncs; best effort, since a sleeping
  device may still show it. A device mute is local and never syncs.
- **Reminders attach to a series** and fire per occurrence; a cancelled occurrence
  suppresses them. A date-only due date fires at the all-day hour.
- **The content is the whole message**: "Deep work starts in 5 minutes — 3 tasks assigned,
  first is Review PR", never "Upcoming block".
- **Late start**: say anything under a few minutes stale, fold the rest into one summary,
  and let acks suppress what another device handled. Two resident processes on one machine
  must not both deliver.
- **What a block in a skipped or repeated DST hour does** must be decided first, once, in
  the core, where a civil occurrence becomes an instant: whether a 2:30 block on the spring
  day moves to 3:30, clamps to 3:00 or is skipped; which 1:30 an autumn one means; and how a
  block spanning the change counts its length and caps its timer. Never during expansion,
  or a routine loses a day each spring.

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

### Urgency

`urgency(task, now) -> f32`, computed and never stored, after Taskwarrior's. It orders block
suggestions, the default sort of task lists, and a `sort: urgency` clause the filter
language does not have yet. It never schedules anything by itself.

- Additive terms: due proximity (rising, and still rising while overdue, up to a ceiling);
  priority; age (small); partly worked (finishing beats starting); blocking other tasks;
  blocked (strongly negative: not actionable); already assigned today (mildly negative).
- **Project weight multiplies the sum.** An additive project term lets a heavy project's
  trivia outrank another project's emergency; multiplying keeps the order within each
  project and scales between them. Weights stay narrow (about 0.5 to 2), a sub-project's own
  weight replaces the inherited one, and labels have none.
- The coefficients are synced settings with good defaults, so ranking agrees across
  devices. `lum task show --urgency` breaks a score into its terms.
- If a task becomes permanent furniture at the top, add a staleness question ("this has
  been at the top for three weeks; is it real?") rather than a bigger formula.
- **Filters have no `sort` clause** (for `sort: urgency`, among others). Comparison
  operators stay out: the vocabulary grows by named predicates.

### History

**Per-task history from the records already kept**, first: "rescheduled 7 times", "three
sittings, 90 minutes logged against a 60-minute estimate", "created four months ago, never
assigned", and the remaining effort across sittings ("43 minutes worked, 17 left against
your estimate"), asked for, never in a live region. Repeated deferral says a task is badly
defined or secretly unimportant: support for executive function, not a statistic. Then a
history view in every app and `lum history`. Field-level history from Automerge (title and
due-date changes over time) later, with its own API.

### Suggestions for a block

`suggest_tasks(block, now, limit) -> [Suggestion { task, reasons }]`, behind "what should I
work on?" in a block, behind adding tasks to a block, and for filling free time.

- Candidates are the tasks the block's `task_filter` admits, ranked by urgency, with a task
  whose estimate fits the block's remaining time raised and one already assigned elsewhere
  today lowered, never excluded.
- **Reasons are required and are the accessible label**: "Suggested because overdue by two
  days, priority 1, and its 30-minute estimate fits your remaining 35 minutes."
- `task_filter` is stored and validated but evaluated nowhere. Today, "Assign a task" from a
  block offers every task, and the block chooser (`work_blocks`) offers every block that
  takes tasks; it should offer only blocks whose filter admits the task.

### Carrying unfinished work forward

At a block's end, a sitting with less logged than planned and its task not done is
**offered** onward: "Deep work ended. Essay draft: 40 of 90 minutes. Carry the remaining 50
to tomorrow's Deep work?" The destination is the next block whose filter admits the task,
with the same capacity check, and overfill is reported. Never carried automatically: silent
carries build a tomorrow nobody decided on, and the question is where "the estimate was
wrong" gets noticed.

### Day commands

End the day now, clear the rest of the day, replan from now; and on one block, end it now
or add fifteen minutes (a modified occurrence each, since occurrences have no lifecycle
state that could survive a merge). Each is the person's own decision and needs no proposal,
though ending early offers carry-forward. Skipping an occurrence is built.

### Re-flow when the day slips

`propose_reflow(day, now, pinned) -> Proposal { changes, unplaceable, alternatives }`.

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
- **Android**: pairing on the local network untried on a real phone (the emulator's NAT
  passes no multicast); the iPhone's two pairing tests not ported. The keyboard commands
  and the wide layout are tested in the emulator, not yet with TalkBack and a real keyboard.
- **GTK**: type-ahead in lists (the Windows and Mac lists jump to a row by its first
  letters; GTK's list view does not); shortcuts from anywhere without the GlobalShortcuts
  portal (an X11 key grab for Xfce and older GNOME); no tray on GNOME without the
  AppIndicator extension; "expanded" said twice on first focus of a row with subtasks.
- **Emacs**: priority faces, mapped to voices.
- **The web client**: making a task a subtask or moving it to the top level (the export
  exists; nothing calls it).
- **Every app**: icons of their own (GTK and Windows have none); a key for "go to now" on
  the web (Ctrl+T on Windows, GTK and Android, Cmd+T on the Mac, iPad and iPhone, `t` on
  BTSpeak).
- **Screen readers not yet tried by a person**: JAWS, Narrator, ChromeVox, Emacspeak and
  speechd-el. NVDA (Windows and the web), Orca, VoiceOver on the Mac and iPhone, TalkBack,
  Emacsvox and the BTSpeak have been. The braille short forms for states and roles are
  unchecked against BTBraille's tree-view convention.
- **Snapshot tests of the row projection** for task lists and the day: index and count,
  expanded and checked, states, speech and braille.

## 5. The watches

Both are full peers with every view, not companions of the phone.

- **watchOS** (SwiftUI; WatchKit only for haptics, the Crown and sessions): a full peer
  running the whole core with the phone off, with every view. **Spike first**: a throwaway
  crate with quinn/rustls, rusqlite, automerge and jiff, built with `cargo build -Z
  build-std=std,panic_abort --target arm64_32-apple-watchos` on a pinned nightly. Risks in
  order: the 32-bit pointer ABI; Iroh's crypto (`ring`, `aws-lc-rs`) failing to build;
  bundled SQLite cross-compiling (the system `libsqlite3` avoids it); watchOS policy against
  independent connections; memory, since Automerge loads whole documents. If Iroh or
  independent QUIC fails there, the watch becomes a phone accessory. `WatchConnectivity` is
  a battery-saving shortcut near the phone; complications through WidgetKit.
- **Wear OS** (Compose for Wear OS): a standalone full peer, the core through `cargo-ndk`,
  every view. Check rotary scrolling with TalkBack. The hardware is here to test on.
- **Toolchain**: unpinned; the watchOS spike decides the nightly.

## 6. Integrations

Each is a thin adapter over the command surface.

- **App Intents** (iOS and macOS; watchOS later): one implementation for Siri, Shortcuts,
  Spotlight, widgets, Control Center and the Action Button. `AppEntity` and `EntityQuery`
  for tasks, projects and blocks so Shortcuts can find things, not only make them;
  `AppShortcutsProvider` for Siri phrases with no setup. Add (in the quick-add grammar),
  complete, query by filter, today's plan, assign, start and stop. Hands-free capture is an
  accessibility feature. Check for an assistant-schema domain for tasks when building.
- **Android App Functions** (`androidx.appfunctions`, for Gemini) with AppSearch for
  retrieval; not the old App Actions.
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
- **Imports from other systems**: Todoist (whose API priority 4 is P1), then Jira, Linear
  and GitHub, which `ExternalRef` was shaped for.

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
