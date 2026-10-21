# Roadmap

What Lumenna does not do yet, with the decisions already made about it, in the order it is
to be done. What is built is not listed: the code says how it works, and `CLAUDE.md` says
what is easy to get wrong.

Every client gets every feature. A platform may get it later, never a smaller version of it.

The order, and why:

1. **Safeguards first**: CI, now built: every app and the core on every push.
2. **Reminders**: built in every app; what is left is small.
3. **The planner**: built in every app (urgency and history, suggestions, carrying work
   forward and day commands, re-flow, the whole-day plan); what is left is small.
4. **Smaller gaps**, between the larger pieces.
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

Built: when each reminder fires, across daylight-saving changes; what it says; one process
per device delivering, and every app delivering or scheduling them (CLAUDE.md, "Who delivers,
per client"); a task's and a block's own reminders and the defaults, set in every app;
dismissing, snoozing, Carry Forward and the overrun warning's Replan, synced; `lum reminder`
and `lum notify`. Still to come:

- **Hooks' time events on the same scheduler**, once hooks exist (Integrations). The
  planner's overrun warning already runs on it.
- **The web's reminders** wait on the encrypted store (below); today one open tab delivers.

## 3. Planning

The planner is four capabilities, all built in every app: urgency, suggestions for a block,
re-flow when the day slips, and a whole-day plan. Per-task history came with urgency, and its
facts ("rescheduled 7 times") feed suggestions' reasons. The sections below keep the design
each was built to, and say what is still to come.

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
from the records already kept, on a task's own screen in every app. Still to come:

- **A History place in every app**, beside Trash, listing what was done by day (tasks
  completed, sittings worked, with their times), and `lum history` printing the same
  (decided 2026-10-20). Field-level history from Automerge (title and due changes over time)
  later, with its own API.
- If a task becomes permanent furniture at the top, a staleness question ("this has been at
  the top for three weeks; is it real?") rather than a bigger formula.

### Suggestions for a block

Built: `core::suggestions`, behind Assign a Task and What Should I Work On?, each choice's
`reason` its description in every app; and Put in a Block offering only the blocks whose
filter admits the task.

### Carrying unfinished work forward, and day commands

Built (`day.rs`, `lum end`, `extend`, `end-day`, `clear-day`, `carry`): End Now and Add 15
Minutes on the block under way, End the Day and Clear the Rest of the Day, and Carry Forward
on a sitting left short of its planned time, each one undoable change; the carry offer at a
block's end, a button on every app's notification; What Should I Work On? (`lum next`).

### Re-flow when the day slips

Built (`core::reflow`, `replan.rs`, `lum replan [--keep] [--skip] [--apply [1,3]]`, RPC
`replan` and `replan.apply`): the walk, moving the work first, pins, the proposal said in
the app's clock, applied whole or in part (the rest pinned and proposed again) against the
day as it is now, one undo; Replan from Now on the day, a screen in every app. Still to
come: nothing of the design below; every part is built. As designed:

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

Built (`core::dayplan`, `dayplan.rs`, `lum plan-day [date] [--leave] [--apply [1,3]]`, RPC
`day_plan` and `day_plan.apply`), Plan the Day on the day, a screen in every app. As designed: `propose_day(date, now, pinned)`: walk the day's work blocks in order, run suggestions for
each, and assign greedily until the estimates fill it. One proposal, naming what did not
fit, pinned the same way. Greedy and editable is worth more than optimal packing. Always
optional.

### Smaller planning gaps


## 4. Smaller gaps

A day or less each, for between the larger pieces.

### Tasks, blocks and text entry

- **Block icons** exist in the model with no way to set them, and nothing shows one: decide
  what an icon is (a symbol's name, an emoji) and where it is said before adding a field.
- **Verbosity** shortens a reminder (its detail left out), the quick-add readback (the date
  once, as itself) and adding or scheduling a block (no time or day). Rows are components
  either way; find any other announcement that runs long under terse as it is met.
- **The block chooser's coming week** is the seven days from today, not the week from the
  week-start setting (which week-relative phrases now follow); say which it should be.

### Completion

The core's `complete` works everywhere, and the app marks the lines in its language
(`Asking::syntax`, `FormField::syntax`). Done: the web, Windows, Android and the Apple apps
complete the list's filter, a saved filter's query and a block's Tasks from, opening only when
asked (Down, Ctrl+Space; the Mac no longer opens on `#` or `@`), and the phones and the Mac
have both the strip and a list behind a Show Completions action. `lum`'s shell completion offers
names from the store. Still to come:

- **GTK**: the query questions and Tasks from (the Linux session has it).
- **BTSpeak**: Control-L (dots 7-8 with L) inside the input dialog completes at the cursor,
  through the dialog's own `hotkeys` (no subclass needed); no dialog or device key claims it.
  Tested against the stub only: try it on the device, in computer and in literary braille.
- **Emacs** completes in the minibuffer already; nothing to add.

### Each app

- **iOS**: custom rotors (overdue, running, next block). Tried (2026-10-20) as SwiftUI's
  `accessibilityRotor` on the task list and the day: VoiceOver lists the rotor, but VO-Command-Down
  steps to none of its entries, with a namespace, the list's own ids or an explicit `.id` alike,
  where the actions rotor steps fine. A SwiftUI `List`'s rows are made lazily; try a UIKit
  `UIAccessibilityCustomRotor` on the hosting view, or a `LazyVStack`, and test it with VoiceOver
  (`VoiceOverUITests` steps a rotor by keyboard).
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
  - Since the lists became SwiftUI, a test under load (CI, or three simulators at once) now
    and then tapped a sidebar place a second after showing the sidebar, and the app never said
    its animations were done: every step after waited a minute. The app was idle (a spindump
    showed no work). UI tests now launch with animations off (`LUMENNA_NO_ANIMATIONS`), and the
    first full run after had no stall; what was left animating was never found, and is worth
    finding if anyone sees the sidebar misbehave.
  - In CI the iPad suite takes over half an hour on the runner and times out where it
    passes locally, so its job reports without blocking: make it fit the runner (fewer
    simulator round trips, or splitting it across jobs), then make it block again.
- **Android**: pairing on the local network untried on a real phone (the emulator's NAT
  passes no multicast); the iPhone's two pairing tests not ported. The keyboard commands
  and the wide layout are tested in the emulator, not yet with TalkBack and a real keyboard.
- **GTK**: no tray on GNOME without the AppIndicator extension; "expanded" said twice on first
  focus of a row with subtasks. (Type-ahead and shortcuts without the portal are done; the X11
  grab is unchecked with Orca under a real X11 window manager.)
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
  unchecked against BTBraille's tree-view convention, and two collide: `blk` is both a block's
  role and a task's blocked state (`crates/core/tests/snapshots/`).

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
  - **Quick add's UI test** (`testQuickAddReadsBackThenTheTaskIsListedAndOpens`) finds no
    readback saying "priority 1" after the line is entered, and failed before the lists moved
    onto the phone's models too: find whether the readback or the test is wrong.
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

- **Reordering tasks among siblings** (decided 2026-10-20): task lists are most urgent
  first, or in the order added (`task-order`, `sort:`), so a task has no place of its own to
  move to. Projects, labels, saved filters and a block's sittings are reordered.
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
