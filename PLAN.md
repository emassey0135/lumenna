# Lumenna — Project Plan

A task manager and day planner, built accessibility-first, with a shared Rust core and
native user interfaces on every platform.

**The name.** *Lumenna* is Quenya for "upon the hour" — `lúmë`, a defined span of time, in the
allative case; the phrase survives elided as *lúmenn'* in Gildor's greeting to Frodo. It names
the half of this app that nothing else does well: not a list, but a specific stretch of a
specific day, and the attention given to it.

Written without the diacritic. Tolkien's `ë` exists to stop English readers silencing the
vowel, but text-to-speech ignores diacritics and users drop them when typing, so it would cost
more than it bought. **The binary is `lum`** — three characters for something typed dozens of
times a day, which matters more on a braille keyboard than a QWERTY one. Ship `lumenna` as a
symlink for discoverability.

Conceptually: Todoist (projects, labels, priorities, subtasks, recurrence, quick add)
fused with Structured (a timeline of blocks across the day), where the join between the
two halves is that tasks are *assigned to blocks* — repeatedly, across multiple sittings,
until they're done.

---

## 1. Principles

1. **Native controls everywhere.** Cross-platform UI toolkits approximate accessibility;
   native controls have it built in and decades of screen reader testing behind them.
   This is the single reason the architecture looks the way it does.
2. **Fat core, thin UIs.** Anything that can live in Rust does. A UI layer should read as
   a mapping from core view models onto platform widgets, with as little logic as possible.
3. **Accessibility is a core concern, not a UI concern.** Structural semantics — level,
   position, set size, expansion, state — are computed once in Rust and consumed
   identically by every platform.
4. **No server ever sees user data.** Peer-to-peer sync between the user's own devices.
   Relays forward ciphertext when hole punching fails; they can't read it.
5. **Keyboard-first, always.** Every action reachable without a pointer, on every platform.
   No mouse-only affordances, no gesture-only affordances.
6. **The accessibility constraint is a design asset, not a limitation.** Designing for speech
   and braille forces reduced working-memory load, explicit rather than implied state,
   predictable low-surprise interfaces, announcing what changed, and low-friction capture
   before a thought evaporates. Those are the same properties **executive-function support**
   needs — someone with ADHD who looked away benefits from the same design as someone who
   cannot see. When a decision is genuinely balanced, the more explicit and less implicit
   option is usually right for both.

   The same constraint produces things sighted power users want anyway: typed entry instead
   of pickers (quick add is Todoist's most-praised feature), information density in text, and
   native controls rather than Electron.

   *Known divergence:* time-blindness support is normally addressed **visually** — Tiimo's
   visual timers, Structured's timeline ring. This design would not arrive at that naturally,
   and it is the highest-value deliberate addition for that audience. See §21.

---

## 2. Architecture

```
                    UI layer (per platform, native)
 CLI  Emacs  BTSpeak  GTK4  Win32  AppKit  UIKit  Compose  Wear  SwiftUI  Web
  |      |        |       |      |       |      |       |      |      |      |
  +------+--------+-------+------+-------+------+-------+------+------+------+
                              |
        COMMAND SURFACE (Rust, typed): one definition of every
        operation (§12). UniFFI exports it to Swift/Kotlin; CLI,
        GTK and Win32 link it; JSON-RPC serialises it for
        Emacs/BTSpeak
                              |
    +-------------------------+--------------------------+
    |                      CORE (Rust)                    |
    |  domain model | queries & filters | scheduling      |
    |  recurrence   | quick-add parser  | a11y projection |
    +-----------------------------------------------------+
    |  STORE: Automerge changes in SQLite (truth)          |
    |         -> optional SQLite read model                |
    +-----------------------------------------------------+
    |  SYNC: Automerge sync protocol over Iroh QUIC        |
    +-----------------------------------------------------+
```

### Language and binding strategy per platform

- **Linux / GTK4** — pure Rust via `gtk4-rs`. No FFI; the UI links the command surface
  directly.
- **Windows / Win32** — pure Rust via `windows-rs`. No FFI. Standard common controls only
  (`SysTreeView32`, `SysListView32`, native `HMENU`), because custom-drawn controls mean
  writing UI Automation providers by hand.
- **macOS / AppKit, iOS / UIKit** — Swift, with **UniFFI**-generated bindings to the core.
  Not `objc2`: every sample, doc, and accessibility protocol conformance you'll need is
  written for Swift, and an Xcode project is required for submission regardless.
- **Android / Wear OS — Jetpack Compose**, with UniFFI-generated Kotlin bindings, built
  via `cargo-ndk`.
- **watchOS** — SwiftUI (the only supported option). See §16.9 for the Rust toolchain
  caveat.
- **CLI** — pure Rust.
- **Emacs, BTSpeak / BTBraille** — no FFI at all. JSON-RPC, in Elisp and Python respectively,
  to a running `lum sync-daemon` over its socket or to a spawned `lum rpc` over stdio —
  identical protocol either way (§8).

### Workspace layout

```
crates/
  core/        domain types, filter evaluation, scheduling, a11y projection. No I/O.
  store/       Automerge change store in SQLite + optional read model
  sync/        Iroh transport, pairing, Automerge sync loop
  parse/       parsers: quick add, filter queries, shared date grammar
  surface/     the command surface (§12): every operation, typed, once
  ffi/         the library the Swift and Kotlin apps link; re-exports surface's UniFFI
apps/
  cli/         one binary, subcommands: add/list/..., `daemon`, `rpc`, `mcp`, `pair`
  linux/       GTK4
  windows/     Win32
  emacs/       Elisp package; voices for Emacspeak/Emacsvox, facts for Emacsvox
  btspeak/     Python app, .menu file, reminder service, systemd unit
  apple/       Xcode project: macOS, iOS, watchOS targets, sharing Swift code
  android/     Gradle project: phone + Wear OS modules
```

The CLI, the daemon, the RPC server, and the MCP server are **one executable with
subcommands**, git-style.
Beyond code reuse, this makes *local* version skew impossible: separate executables could put
a v3 daemon and a v2 CLI against the same store on one machine. GUIs stay separate binaries —
different toolkits — but link the same core crates.

`store/` also owns multi-process coordination — WAL watching, `data_version` checks, and the
sync leader lock (§8). No process is privileged: `lum sync-daemon` is simply whichever
participant happens to be headless, running the same code path as the tray app.

---

## 3. Data model

Automerge documents are authoritative. SQLite is a rebuildable index, never a source of
truth. All identifiers are **UUIDv7** — time-sortable, good index locality, no coordination
needed for uniqueness across devices.

### 3.1 Document layout

Automerge loads and syncs whole documents, so sharding is a modeling decision:

- **`core`** — tasks, projects, labels. Small, needed on every device including the watch.
- **`blocks-<year>`** — block series, exceptions, and assignments for one calendar year.
  Bounds memory on constrained devices and stops history growing without limit in a single
  document.
- **`devices`** — the paired device roster.

Cross-document references are bare UUIDs. There is **no referential integrity across
documents** — an assignment in `blocks-2026` may reference a task the local `core` doc
hasn't seen yet. The materializer must tolerate dangling references rather than assume
they can't occur.

### 3.2 Task

```
Task {
  id:            TaskId          // UUIDv7
  title:         String          // LWW
  notes:         Text            // Automerge Text, character-level merge
  parent_id:     Option<TaskId>
  project_id:    ProjectId       // always set; Inbox is a real Project
  priority:      u8              // 1 = highest .. 4 = none
  labels:        Set<LabelId>
  depends:       Set<TaskId>     // must complete before this one can start
  due:           Option<Due>
  estimate_mins: Option<u32>
  order:         String          // fractional index
  external:      Option<ExternalRef>
  created_at:    Timestamp
  deleted_at:    Option<Timestamp>
}
```

`external` mirrors the field on `BlockSeries` (§3.6). Blocks import from calendars; tasks
will eventually import from team systems — Jira, Linear, GitHub issues. Both are later work,
but the field is nearly free now and unpleasant to retrofit into a CRDT afterwards.

`title` is a plain last-write-wins string; character-level merge isn't worth the overhead
on a short field. `notes` is Automerge `Text` so long-form editing merges properly.

`priority` uses 1 as highest. Todoist's REST API inverts this (their p1 is API priority 4);
convert at the boundary if importing.

`estimate_mins` is what makes multi-sitting work legible: remaining effort is the estimate
minus the sum of logged time across assignments.

`depends` is a flat set, deliberately minimal — no lag times, no start-to-start variants, no
critical path. Personal task management does not need project-management machinery, and
subtasks already cover most of what people reach for dependencies to express.

**What justifies it here is the planner, not the list.** In a pure list app dependencies are
marginal. But §10.5 places tasks into a day, and scheduling B into the morning when A sits in
the afternoon is not a suboptimal plan, it is an impossible one — an error nothing else in the
model can catch. Dependencies also give `blocked` and `ready` (§6.2) their meaning, and those
are the most useful of the computed states; without `depends` they would have to be dropped.

*Deliberately excluded:* dependencies across the parent/child boundary are allowed but not
encouraged, and nothing in the UI will suggest them. A subtask that blocks its own parent is
almost always a modelling mistake by the user, but forbidding it costs a validation rule that
CRDT merge could violate anyway.

`deleted_at` is a **product feature** (Trash, undo), not a sync mechanism. Automerge handles
deletion natively as a CRDT operation; tombstones are not needed for convergence.

### 3.3 TaskCompletion

```
TaskCompletion {
  id:              CompletionId
  task_id:         TaskId
  completed_at:    Timestamp
  occurrence_date: Option<Date>     // which occurrence, for recurring tasks and their subtasks
  cascaded_from:   Option<TaskId>   // set if caused by a parent's cascade
}
```

Completion is a separate record because a recurring task is a *single task whose due date
advances*, not a generated series. A recurring task accumulates completions over time; a
one-off task has exactly one.

**A subtask of a recurring task recurs with it.** Its completion names the nearest recurring
ancestor's current occurrence, and counts only while that occurrence is current — so the
checklist under a weekly review is open again each week, while every past week's ticks stay
on record. A subtask with its own recurrence keeps its own dates.

`cascaded_from` exists because "completing a parent completes its subtasks" is a user
setting (default on). If you complete a parent, the cascade completes three subtasks, and
you then uncomplete the parent, a subtask you'd independently finished last week must not
be uncompleted along with the rest. The uncomplete path only reverses completions it caused.

Because the cascade is a setting that can change, completion is **stored per task and
applied at write time**, never derived at read time — deriving would retroactively rewrite
history when the setting flips.

### 3.4 Project and Label

```
Project {
  id, name, parent_id: Option<ProjectId>, color,
  order: String, archived: bool, is_inbox: bool,
  weight: f32,                    // urgency multiplier, default 1.0
  deleted_at
}

Label {
  id, name, color, order: String, deleted_at
}
```

Inbox is a real `Project` record with `is_inbox`, not a null `project_id`. This keeps
ordering, view settings, and queries uniform.

**`weight` is not a second priority, and must never be called one.** Task priority answers
"how much does this one item matter"; project weight answers "how much does this whole area
matter right now" — the state where everything in `#thesis` outranks everything in `#chores`
for a fortnight. Users who meet two fields both named *priority* immediately ask what a P1
task in a low-priority project means, and there is no good answer. Two names, two questions.

It is a **multiplier on urgency** (§10.3), not an additive term, so a heavy project lifts its
whole contents proportionally rather than swamping due dates. Keep the range narrow — roughly
0.5 to 2.0 in the UI — because anything wider lets one project dominate every ranking and the
user then distrusts the whole feature.

**Weight inherits down the project tree** unless a sub-project sets its own. Setting `#thesis`
heavy should not require touching each of its chapters.

`Label` deliberately has no weight. Labels are cross-cutting context (`@laptop`, `@errand`),
not statements of importance, and weighting them produces tasks whose urgency depends on how
many labels someone happened to attach.

#### Labels are entities, created implicitly

Two models exist and the difference is not cosmetic. Taskwarrior and `todo.txt` treat tags as
**plain strings on the task**: `+home` exists because something wears it and vanishes when
nothing does. Todoist treats them as **records**, referenced by id. This model is the second —
`Task.labels` is `Set<LabelId>`, not `Set<String>` — for three reasons:

1. **Rename works.** Renaming `@work` to `@office` updates one record. With strings it is a
   rewrite of every task carrying it, which in a CRDT is a large multi-object change where a
   concurrent edit can leave the rename half-applied.
2. **A label list can exist at all** (§16.1). A set derived from whatever tasks happen to
   mention it cannot carry colour or ordering, and cannot be curated.
3. **§6.3 already promises** *"unknown label 'lapto' — did you mean 'laptop'?"*. That check is
   only possible against a **closed set**. With free strings there is no such thing as an
   unknown label, and every typo silently becomes a new one that quietly splits a filter's
   results. This reason alone settles it, because the commitment is already made elsewhere.

**But creation is implicit.** Typing `@errand` in quick add for a label that does not exist
creates it — no dialog, no trip to a management screen. Being an entity is an implementation
fact the user should never have to think about during capture.

Implicit creation and typo detection pull against each other, and the resolution is
**confirm-on-new, never prompt-on-known**: an unrecognised label is reported with its nearest
match before it is created — *"new label 'lapto'; did you mean 'laptop'?"* — and confirming
proceeds. Silence would let typos accumulate; a prompt on every label would make capture
miserable.

**Projects do not auto-create**, and the asymmetry is deliberate. A project has a parent,
ordering, archive state, and a weight — structure that wants a decision. `#newthing` in quick
add is treated as an unknown project and reported, not created.

**Deletion is a soft-delete of the `Label` record and touches no tasks.** `Set<LabelId>`
entries pointing at a deleted label simply project as absent, which §3.1's
tolerate-dangling-references rule already requires. Undo is free, and a large multi-task write
is avoided entirely. Note the consequence: a later label with the same *name* is a different
record, and old tasks do not acquire it.

**Merge is a first-class operation**, precisely because implicit creation makes near-duplicates
inevitable. Merging `@lapto` into `@laptop` rewrites the affected tasks' sets and soft-deletes
the loser. Cheap with entities; impossible with strings, where the two tags were never
distinguishable from intent in the first place.

```
SavedFilter {
  id, name, query: String, order: String, color, deleted_at
}
```

`query` is stored as **text, never as a resolved date range** — see §6.2. The same language
appears inline in `BlockSeries.task_filter` (§3.6).

### 3.5 Due and Recurrence

```
Due {
  date:       civil::Date
  time:       Option<civil::Time>
  timezone:   Option<TimeZone>
  recurrence: Option<Recurrence>
}

Recurrence {
  rrule:           String   // RFC 5545
  from_completion: bool
}
```

`time` is optional because "Tuesday" and "Tuesday at 3pm" are genuinely different states.

`timezone` is normally **absent**, meaning the due time floats — 3pm wherever you are.
Set it only when a task is anchored to a real place, such as a call in another region.

`from_completion` distinguishes Todoist's `every day` (advance from the scheduled date, so
it can fall behind and show as overdue) from `every! day` (advance from actual completion).

### 3.6 Time blocks

```
BlockSeries {
  id:              SeriesId
  title:           String
  notes:           Text
  kind:            BlockKind
  flags:           BlockFlags
  start_time:      civil::Time
  duration_mins:   u32
  min_duration_mins: Option<u32>     // how far re-flow may compress it; default per kind
  start_date:      civil::Date
  end_date:        Option<civil::Date>
  rrule:           Option<String>     // absent for one-off blocks
  timezone:        Option<TimeZone>
  color, icon
  task_filter:     Option<String>     // scopes auto-suggestions; see §10
  external:        Option<ExternalRef>
  deleted_at
}
```

`task_filter` is a saved filter expression scoping which tasks the core will suggest for
this block — "this is my `#work` block, don't offer me personal errands." It is what makes
auto-suggestion useful rather than noisy, and it promotes the filter query language from a
nice-to-have to a load-bearing component.

**Duration, not end time.** It can't be negative, can't be inverted, and survives DST
transitions cleanly.

**`min_duration_mins` is what stops re-flow destroying a block to save the schedule.** When
the day slips, §10.1 may shrink later blocks to absorb the overrun — but shrinking is only
sometimes acceptable, and which case you are in depends entirely on what the block is *for*. A
work block cut from 90 to 60 minutes still does work. A fifteen-minute break cut to four
minutes is not a break; it has been deleted while appearing to survive, which is worse than
being told it does not fit.

So the default comes from `BlockKind`:

- **Break** — minimum equals its duration. **Incompressible by default**, because a shortened
  break is a failed break. It moves instead.
- **Work** — compressible to roughly half, floored at fifteen minutes; below that the context
  switch costs more than the block yields.
- **Event** — irrelevant; anchored blocks are never re-flowed at all.

Per-block override for the real cases: a two-hour work block that is useless under an hour, a
"walk the dog" break that genuinely can be five minutes instead of twenty.

**Wall-clock local by default.** `timezone` is normally absent: a 9am block is 9am after
you fly to another continent. Set it only for genuinely anchored blocks.

**One type for both one-off and recurring blocks**, distinguished by `rrule`. This avoids
two near-identical shapes and lets a one-off block become recurring without changing type.

#### BlockKind and BlockFlags

Kind exists only where the *core* behaves differently. Everything else is presentation.

The behaviourally meaningful axes are orthogonal, so they're flags:

```
BlockFlags {
  accepts_tasks:  bool   // can tasks be assigned into it
  counts_capacity: bool  // contributes to "hours available for work today"
  anchored:       bool   // fixed in time; cannot be shifted when the day slips
}
```

`BlockKind` is a preset that expands to a default flag set, and the flags remain
individually overridable:

- **Work** — accepts tasks, counts as capacity, movable. The core concept.
- **Break** — no tasks, no capacity, movable. Meals, rest, sleep, commute.
- **Event** — no tasks, no capacity, **anchored**. Meetings, appointments, classes.

Presets cover the common path; overrides cover the real world. A commute is a Break, but
someone who works on the train wants it to accept tasks. "Protected admin time" is anchored
*and* accepts tasks, which no preset covers.

`anchored` is the flag that matters most operationally — it is what lets the core do anything
sensible when the day runs late (§10.1), by determining which blocks can absorb a slip.

There is deliberately no `Custom(String)` kind. A free-text kind the core can't reason about
is a title with extra steps.

#### Occurrences have no lifecycle state

**Blocks are not timers.** Nothing is ever started or ended by the user. A block occupies its
scheduled window and leaves it, and whether that window contains `now` is **derived from the
clock, never stored**.

Three reasons, the last decisive:

1. A punch clock is friction people forget, and a forgotten "end block" leaves data worse than
   no data — an occurrence that appears to have run for nineteen hours.
2. Actual effort is already recorded in the right place. `BlockAssignment.running_since` and
   `accumulated_mins` (§3.7) are where "I worked on this for forty minutes" lives. A
   block-level actual-start would be a second clock that could disagree with the first, and
   then something has to decide which one is true.
3. **A lifecycle state machine cannot survive CRDT merge.** Two devices starting the same
   occurrence, or one ending it while another extends it, produce states with no sensible
   merge and no correct answer. Derived-from-clock has nothing to converge, so it cannot
   conflict.

*Ending early and running over are still real*, and they already have a representation: they
are **edits to the occurrence, not lifecycle transitions**. Finishing at 10:40 instead of
11:00 is a `Modified` exception with a shorter duration — honest, because you genuinely
changed that day's schedule, and it should persist and sync like any other change. The UI
offers "end now" and "add fifteen minutes"; those buttons write exceptions.


#### BlockException

```
BlockException {
  series_id:     SeriesId
  original_date: civil::Date
  action:        Cancelled | Modified { start_time?, duration?, title?, kind?, flags? }
}
```

Sparse exceptions. Unmodified occurrences are **never stored** — they're expanded from the
rule at read time. Only edits and cancellations become records. Without this, a daily
routine generates thousands of rows and every sync becomes a bulk transfer.

#### ExternalRef

```
ExternalRef {
  provider:    EventKit | CalDAV | IcsUrl
  external_id: String
  read_only:   bool
  last_synced: Timestamp
}
```

A day planner that doesn't know about real meetings will schedule work on top of them and
report fictional capacity. Calendar import is close to table stakes for this category.
Read-only import first; two-way sync is a separate project.

### 3.7 BlockAssignment — the join between the two halves

```
BlockAssignment {
  id:             AssignmentId
  block_ref:      OneOff(SeriesId) | Occurrence(SeriesId, civil::Date)
  task_id:        TaskId
  planned_mins:   Option<u32>
  accumulated_mins: u32                    // completed timer runs, plus manual entry
  running_since:  Option<Timestamp>        // Some(_) iff a timer is currently running
  status:         Planned | InProgress | Worked | Skipped | Deferred
  order:          String
  created_at:     Timestamp
}
```

**Live timers: store a fact, never a counter.** Elapsed time is derived
(`accumulated_mins + (now - running_since)`), so nothing ticks in storage and nothing has to
be written every second. Pausing folds the running interval into `accumulated_mins` and
clears `running_since`.

This matters because running state is the most conflict-prone thing in the model. Two
devices starting a timer on the same assignment is last-write-wins on `running_since`, which
is harmless. The real failure mode is an *orphaned* timer — started on a phone that then
died — so cap derived elapsed time at the block's duration and prompt for confirmation
rather than silently logging fourteen hours.

Manual entry writes `accumulated_mins` directly; the timer is optional, not mandatory.

**Accessibility constraint:** elapsed time must never live in a live region or announcement
channel. A continuously updating value would make a screen reader announce constantly and
render the screen unusable. Expose it as *polled on demand* — a keystroke or button that
reports "43 minutes elapsed, 17 remaining against your estimate."

Two properties make the central requirement work:

**Completion lives on the Task; the assignment has its own status.** A task worked on
Monday but not finished gets `status: Worked` on Monday's assignment and a *fresh*
assignment on Tuesday. Both persist. Over time this accumulates a genuine work history —
how many sittings something took, where estimates were wrong — which is data Structured
discards.

**`block_ref` is an enum** because a recurring block's occurrence has no stable identifier
until it's excepted. Assigning a task to next Tuesday's instance of a daily focus block
means referencing `(series_id, 2026-08-25)`. This is the main reason the exception model
must exist rather than being an optimization.

**No uniqueness constraint on `(task_id, date)`.** A task can be assigned to multiple days
concurrently — planning three sittings for a long essay up front is a first-class use case,
not an accident to prevent.

### 3.8 Reminder

```
Reminder {
  id:       ReminderId
  target:   Task(TaskId) | Block(SeriesId)
  anchor:   Due | BlockStart | BlockEnd | Absolute(Zoned)
  offset:   i32          // minutes; negative = before, 0 = at, positive = after
  delivery: AllDevices | OnlyDevices(Set<NodeId>) | ExceptDevices(Set<NodeId>)
  deleted_at
}
```

One shape covers every case asked for. A task takes any number of reminders anchored to its
due datetime. A block takes any number anchored to either its start or its end, before or
at either. Positive offsets come free and are genuinely useful — "five minutes after this
block ends, log what you did."

**Anchoring rules:**

- `Due` requires the task's `due.time` to be set. For date-only due dates, resolve against a
  settings-level default hour (§3.10), otherwise "30 minutes before" has no referent.
- `BlockEnd` is meaningless for tasks; validate at construction.
- Reminders attach to the *series*, not to occurrences, and fire for each expanded
  occurrence. Cancelled occurrences (`BlockException { action: Cancelled }`) must suppress
  their reminders — easy to miss, and very annoying when missed.

**Delivery defaults to `AllDevices`.** Per-reminder overrides handle the "only notify me on
my work laptop" case. Because every device holds the same synced reminder data, each one
independently schedules its own local notifications. There's no coordinator and no
"which device owns this reminder" election — the all-devices default is what makes the
architecture simple, not just the behaviour desirable.

### 3.9 ReminderAck

```
ReminderAck {
  reminder_id:     ReminderId
  occurrence_date: civil::Date
  action:          Dismissed | Snoozed { until: Zoned }
  acked_at:        Timestamp
  acked_by:        NodeId
}
```

If a reminder fires on five devices and you dismiss it on one, the other four should stop
nagging. That's what this record is for, and it propagates over the ordinary sync channel.

Be honest about the limit: propagation is **best-effort**. A device that is asleep or
offline will still show a stale notification, and there is no way around that in a
peer-to-peer design without a central broker. Dismissal is eventually consistent, like
everything else here.

### 3.10 Settings

Synced, since these are user preferences rather than device state:

```
Settings {
  cascade_complete_subtasks: bool            // default true
  verbosity:                 Terse | Full    // announcement detail, §13
  default_task_reminders:    Vec<Trigger>    // offered in the UI for new tasks
  default_block_reminders:   Vec<Trigger>    // e.g. [BlockStart -5min]
  all_day_reminder_hour:     civil::Time     // anchor for date-only due dates
  day_window:                (Time, Time)    // waking hours, bounds re-flow
  week_start:                Weekday
}
```

Device-specific overrides (such as muting all reminders on one machine) live in local-only
state (§3.12), not here.

### 3.11 Device

```
Device {
  node_id:   IrohNodeId    // ed25519 public key
  name:      String
  platform:  String
  paired_at: Timestamp
  last_seen: Timestamp
}
```

### 3.12 Local-only state (never synced)

- **The device's own Iroh private key.** Its public half is published in `devices` and syncs;
  the private half never leaves the machine.
- **`account_key` and the derived `auth_value`** (§8) — shared by all your devices, but
  transferred over the pairing channel rather than through a document, because Automerge
  history is permanent and keys must be overwritable. Caching `auth_value` is what stops the
  store asking for a password on every request.
- Iroh peer sync state and Automerge document heads
- The SQLite read model itself
- Current view, selection, scroll position
- The undo/redo stack (§9) — per device, saved locally, never synced
- **Tree expansion / collapse state** — deliberately per-device. Syncing it means the watch
  collapsing a project on the desktop.

### 3.13 CRDT hazards to handle explicitly

**Cycles in the tree.** A parent pointer is the right representation, but nothing stops
device A moving X under Y while device B moves Y under X. After merge you have an orphaned
cycle, and a naive tree walk hangs. Detect cycles when materializing into SQLite and break
them by reparenting to root. Kleppmann's *"A highly-available move operation for replicated
trees"* is the principled treatment; Automerge does not provide a move primitive. The same
applies to nested projects.

**Cycles in the dependency graph.** `Task.depends` (§3.2) is a second graph over the same
nodes, and it has the same hazard for the same reason: A makes X depend on Y, B makes Y depend
on X, and after merge `ready`/`blocked` evaluation never terminates. It needs its own repair,
not the tree's — reparenting to root is meaningless here. Detect the cycle when projecting,
then **drop the edges that close it**, preferring to drop the most recently created (the
`created_at` of the depending task breaks ties deterministically, so every replica drops the
same edge without coordinating). Surface it: a task silently losing a dependency is worse than
being told. Unlike the tree, an unrepaired dependency cycle degrades gracefully if evaluation
is written with a visited set — everything in the cycle simply reads as blocked forever — so
this is a correctness-of-meaning fix rather than a hang fix.

**Ordering.** Do *not* use an Automerge list of children on the parent. List CRDTs handle
insertion within one list well, but reparenting between lists produces duplicates or losses.
Use **fractional indexing**: `order` is a string sorted between its neighbours. Single LWW
field, survives reparenting, identical mechanism for tasks, projects, and assignments. Break
ties on identical keys by UUID.

### 3.14 Derived: the SQLite read model

Mirrors the above, plus:

- `task_flat` — the tree flattened with `depth` and a materialized path, cycle-repaired
- due-date and scheduled-date indexes
- FTS5 index for search
- expanded block occurrences for a bounded date window

It records which Automerge heads it was built from, so staleness is detectable, and it is
droppable and rebuildable at any time.

---

## 4. Time handling

Use **`jiff`**, not `chrono`. The distinction between civil (wall-clock) datetimes and
absolute instants is load-bearing throughout this app, and `jiff` models it properly.

Rules:

- Block times are **civil** — date plus time, no zone. Storing them as UTC instants is the
  classic bug in this category: a 9am block silently becomes 8am or 10am across a DST
  boundary.
- Timestamps that record *when something happened* (`created_at`, `completed_at`,
  `last_seen`) are absolute instants.
- Quick-add parses relative to the user's current `Zoned` datetime, never UTC.
- Watch for the missing and ambiguous local times that DST transitions create; decide
  explicitly how a block scheduled in a skipped hour behaves.

---

## 5. Recurrence

Two separate systems that happen to share RRULE syntax:

**Recurring tasks** — one task whose `due` advances. Not a generated series. This is the
opposite of how calendar events work, and getting it backwards is the standard mistake.

**Recurring blocks** — a series definition plus sparse exceptions, expanded on read. This
*is* how calendar events work.

Use the `rrule` crate for parsing and expansion. Note that it takes RFC 5545 strings, not
prose — English-to-RRULE has no mature Rust implementation and is written by hand as part
of the quick-add grammar (§6).

---

## 6. Text entry: quick add and filters

Both features parse user-typed text into structured queries, both need the same date
grammar, and both need position-aware errors and completion. They share one parser stack
(§6.3) and one completion API, surfaced through different affordances per platform (§6.4).

### 6.1 Quick add

Todoist-style quick add is the highest-leverage feature in the whole product for a screen
reader user: typing a full task specification beats navigating any date picker on any
platform.

Grammar sketch:

```
review PR tomorrow 3pm p1 #work @laptop
       ^          ^        ^  ^     ^
       title      date     pri proj label
```

Sigil elements (`#project`, `@label`, `p1`–`p4`) are trivial to parse. Only the date phrase
is genuinely fuzzy.

**Write the parser rather than adopting one.** Existing crates (`interim`, which notably
supports a `jiff` backend, plus `chrono-english`, `two_timer`, `parse_datetime`) expect
their input to *be* a date expression, not to contain one. Quick add needs to know **which
span** of the input the date consumed so it can be stripped from the title — that's the
actual requirement, and no crate exposes it. Use **`chumsky`** (§6.3), optionally delegating
the residual date phrase to `interim`.

Writing it also gives you recurrence parsing, which nothing off the shelf provides.

**The core returns a structured parse preview, not just a task**: title text, date span with
its resolved absolute value, project, labels, priority, plus a ready-to-announce
confirmation string.

- Always expose the **resolved absolute** datetime, never just the phrase. "Friday" is
  ambiguous — this Friday or next — and the resolution must be announceable.
- On ambiguous or partial parses, say so explicitly. Never silently fold an unrecognized
  token into the title; a swallowed date is invisible until the task fails to fire.
- **An unknown `@label` is a new label; an unknown `#project` is an error** (§3.4). Both are
  reported in the preview with a nearest match — *"new label 'lapto'; did you mean
  'laptop'?"* — but only the label proceeds on confirmation.

A sighted user gets live inline highlighting as they type. That channel doesn't exist here,
so the preview API replaces it.

### 6.2 Filter query language

A filter is a **boolean expression over predicates**. Used in two places, with one
implementation: user-facing saved filters, and `BlockSeries.task_filter` (§3.6) scoping
which tasks auto-suggestion will offer.

**Decision: full boolean** — `&`, `|`, `!`, and parentheses. This is Todoist's level, it is
what users arriving from Todoist expect, and it covers essentially all real queries.

```
#work & (p1 | overdue) & !@waiting
```

#### Predicate families

- **Set membership** — `#work` (project), `@laptop` (label)
  - `#work` matches **exactly** that project.
  - `##work` matches that project **and all descendants, to unlimited depth** — the full
    transitive closure, never depth-limited. There is no `###`.
  - Evaluating `##` walks the project tree, so it depends on the cycle repair in §3.13; an
    unguarded closure walk over a cyclic tree loops forever. In SQL it is a recursive CTE,
    the same machinery subtask predicates need (§6.2 Evaluation).
- **Scalar** — `p1`–`p4`
- **Date range** — `overdue`, `today`, `7 days`, `due before: friday`, `due after: ...`
- **Computed states** — `recurring`, `subtask`, `blocked`, `ready`, `running`, `no date`,
  `no label`, `no project`. See below.
- **Text search** — `search: invoice`

#### Computed states are not labels

Taskwarrior calls these *virtual tags* and spells them exactly like real tags — `+OVERDUE`
next to `+home` — distinguished only by a capitalisation convention it does not enforce. That
is elegant when tags are your only such axis. It is the wrong move here, and the grammar above
already avoids it: labels are `@laptop`, computed states are bare words. Keep it that way.

Three reasons, in ascending order of how much they matter:

1. A user who sees `blocked` sitting where `@waiting` sits will try to remove it, and cannot.
2. `@` completion (§6.4) should offer the user's own labels, not a fixed vocabulary mixed in
   among them. In a screen reader you cannot distinguish the two by styling, only by where
   they appear — so they must appear in different places. Bare-word states complete from their
   own list, on their own trigger.
3. **They are already modelled.** §13's Row projection carries `state: Vec<State>` for the
   accessibility layer — "overdue", "blocked", "running" is exactly what a screen reader must
   announce after the title. Computed states in the filter language and `State` in the
   accessibility projection are one concept with two surfaces, and they should share one
   definition in the core rather than drifting into two lists that disagree about whether a
   task with a running timer is "active" or "started".

That third point is the design constraint: **adding a computed state means adding a `State`
variant**, and anything a filter can select on is something a screen reader can announce.

`blocked` and `ready` come from `Task.depends` (§3.2) — `blocked` when any dependency is
incomplete, `ready` when none are and the task is otherwise actionable. They are the most
useful states in the set and the reason dependencies earn their place in the model.

#### Predicates this project needs that Todoist has no equivalent for

These come from the planner half, and they are what make `task_filter` worth having:

- `assigned: today` — has a block assignment on a date
- `unassigned` — has no sitting ahead: nothing planned or under way in a block today or later.
  A task worked on last week and not finished, or one whose planned sitting was missed, is
  unassigned again — those are exactly what auto-suggestion has to find
- `started` — has accumulated timer minutes but is not complete
- `no estimate` — auto-suggestion cannot rank what it cannot size

"Work tasks not already assigned today" is the canonical block filter and needs two of them.

Deliberately kept out of the grammar: general comparison machinery (`estimate < 30m`,
`created after: -30 days`). Extend the *vocabulary* with named predicates (`no estimate`,
`long`) rather than the *grammar* with operators. Small grammar, growing dictionary.

#### Evaluation

The AST is the stable interface; evaluation strategy can change behind it with no
user-visible effect.

**Start by interpreting over the in-memory documents** — walk the AST per task, O(n) per
query. Correct, obvious, and works with no read model at all, which is what §18 assumes.

**Compile to SQL later** if it becomes slow. Not everything compiles cleanly: text search
needs an FTS join and subtask predicates need a recursive CTE. The realistic end state is
hybrid — push cheap predicates into SQL, evaluate the rest in memory over the reduced set.

#### Dates

**Store the query as text, never as a resolved range.** A saved filter containing `today`
must mean today *at evaluation time*. Resolving at save time produces a filter that silently
rots overnight.

**Reuse the quick-add date grammar** (§6.1) — `due before: next friday` parses exactly as
`next friday` does there, anchored to the user's current `Zoned` datetime. Two subtly
different date parsers in one app is a bug generator.

#### The name ambiguity

Project and label names contain spaces. In `#My Project p1`, is `p1` part of the name or a
priority predicate?

Do both: **greedy-match against known names** for convenience, and support **quoting**
(`#"My Project"`) as the escape hatch for a project genuinely named "work p1".

#### Storage

```
SavedFilter { id, name, query: String, order: String, color, deleted_at }
```

Query text lives in the `core` document and syncs like everything else.

### 6.3 Shared parser infrastructure

**Use `chumsky` for both parsers**, over `winnow`, for one reason: `chumsky` reports **expected-token sets at a position**, and completion depends on
exactly that.

Three requirements drive the choice, and all three are accessibility requirements that a
sighted implementation can afford to skip:

**Errors must be precise and speakable.** Not "syntax error" but *"unknown label 'lapto' at
position 12 — did you mean 'laptop'?"* A sighted user sees a squiggle under the offending
token; here the position and the token must be **in the message text**.

**The core must return a human-readable rendering of the parsed query.** For
`#work & (p1 | overdue) & !@waiting`, return *"tasks in Work, that are either priority 1 or
overdue, and not labelled waiting."* Same principle as the quick-add preview (§6.1) and for
the same reason: a mis-parsed filter shows wrong results **silently**, and wrong results are
invisible. Without a readback there is no way to know the query didn't mean what you thought.

**Completion is a parser feature, not an afterthought.** Completing `#pro` to project names
and `@la` to labels is worth a great deal everywhere, and it requires the parser to answer
"what token kinds are valid here?"

Also: **announce the result count before the list.** "17 tasks", then the rows.

#### The completion API

One function, affordance-agnostic. Triggering and presentation are per-platform (§6.4):

```
complete(text, cursor) -> Completions {
  replace_span: (start, end),
  candidates:   Vec<Candidate { text, kind, label }>,
}
```

`kind` matters for announcement: *"project Work"* tells you what you are inserting where a
bare *"Work"* does not. The candidate count belongs in a core-provided announcement string,
consistent with the readback above.

### 6.4 Completion affordances per platform

**Do not bind Tab as the primary trigger on any GUI platform.** Tab is focus traversal, and
screen reader users depend on it more than sighted users do. Capturing it inside a text field
means losing the ability to Tab out — available everywhere (`DLGC_WANTTAB` via
`WM_GETDLGCODE` on Win32, a key controller in GTK4, `doCommandBySelector:` intercepting
`insertTab:` in AppKit) and a real regression on all of them.

#### Desktop — down arrow, via a real combobox

The point is not the key binding but the control type: make quick-add and filter fields
**genuine comboboxes**, so expansion state, candidate count, and active descendant are
reported by the platform rather than reimplemented by you.

- **Windows** — expose UIA `ComboBox` with the ExpandCollapse pattern; NVDA and JAWS then
  announce "expanded, list, 1 of 5" with no custom work. *Investigate* `IAutoComplete2` with
  `ACO_AUTOSUGGEST` — the system's own autocomplete, used by Explorer's address bar, so
  maximally screen-reader-compatible; but its `IEnumString` source may not suit
  position-dependent candidates. Check before committing.
- **GTK4** — `GtkEntryCompletion` is deprecated in 4.10+, so this is a `GtkPopover` with a
  `GtkListView` plus `gtk_accessible_update_relation` setting `ACTIVE_DESCENDANT` and
  `CONTROLS`. The combobox pattern spelled out manually.
- **macOS** — `NSTextField` has built-in completion through the delegate's
  `control:textView:completions:forPartialWordRange:`, triggerable with `complete:`. Native
  and VoiceOver-aware.

Ctrl+Space as a secondary binding is free and familiar from IDEs.

#### Mobile — a custom action opening a modal list

A **custom accessibility action** on the field ("Show completions") that opens a **modal
list**. Nothing renders below the field while typing.

Discoverability is handled by the platform: VoiceOver announces "Actions available"
automatically whenever a custom action exists, and TalkBack advertises its local context menu
the same way. So the action does not need teaching.

A modal list also has clearer boundaries than a persistent one — you know where you are, and
focus returns to the field predictably on pick or dismiss. A list appearing and disappearing
below the field as you type creates an "is it there right now?" ambiguity that costs more
than it gives.

**Do not use `ExposedDropdownMenuBox`.** It renders through `Popup`, and Compose popups have
a history of TalkBack problems: focus not entering the popup, expanded state unannounced,
items unreachable. `menuAnchor()` is meant to wire the semantics but the gaps are reported
often enough not to build on it untested. A plain `AlertDialog` or full-screen list avoids
`Popup` entirely, and dialogs have far better-established TalkBack behaviour. Same shape on
iOS: custom action, modal list.

Wire `UIKeyCommand` for down-arrow as well — external keyboards are common among blind iOS
users, and the desktop idiom should work when one is attached.

#### CLI and Emacs — Tab is correct here

No focus traversal to lose in a shell prompt or minibuffer. `clap_complete` dynamic
completions for the CLI; `completing-read` for Emacs, which inherits whatever vertico or
consult setup the user already has. The best completion experience of any target, for free.

#### BTSpeak — an explicit control-key trigger

Tab is taken by `request_form` field navigation, and `request_input` has **no completion
hook** to extend. So completion is a two-step flow: a key closes the input dialog, opens a
`request_choice` list, and inserts the selection back into the field.

**Do not auto-trigger on `#` or `@`.** Typing a project name straight through is common and
faster; an automatic popup interrupts it.

Use an explicit control key instead — on the braille keyboard, dots 7-8 plus a letter.
**Ctrl-L** ("list") is free in the dialog key map, where Ctrl-K and Ctrl-D are LineDelete and
Ctrl-X/C/V are clipboard. *Verify it is not intercepted by a global brltty chord* before
committing; only the dialog-level table was checked.

One key, **context-determined**: the trigger never needs to know whether a project, label,
date, or saved filter is expected. It calls `complete(text, cursor)` and the core decides
from cursor position, with `kind` supplying the announcement — "project Work", "label
laptop". `#`, `@`, and anything added later are covered by the same binding.

A choice list is idiomatic for the device regardless — it is menu-driven throughout.

---

## 7. Sync

**Automerge** for the data layer, **Iroh** for transport. They compose cleanly because
Automerge's sync protocol is transport-agnostic by design.

### Mechanism

Open an Iroh bidirectional QUIC stream under a custom ALPN (`lumenna/sync/0`), then pump
length-prefixed `automerge::sync` messages until both sides report nothing outstanding.
`generate_sync_message` / `receive_sync_message` handle delta negotiation. The integration
is on the order of a hundred lines.

Prefer the raw `automerge::sync` API over the `automerge-repo` crate: the Rust version of
`automerge-repo` has been less stable than the JS one, and the low-level API is small and
settled.

### Why Automerge rather than iroh-docs

- `iroh-docs` is last-write-wins per `(author, key)` over blob values. Two devices editing
  different fields of one task means an edit vanishes.
- Automerge has a proper list CRDT, so reordering and moves converge correctly.
- n0 narrowed Iroh's scope to the connectivity core; docs, blobs, and gossip moved to
  separate, less-supported repositories.

Iroh remains the right choice for the hard part — authenticated, encrypted, NAT-traversing
device-to-device connections.

**v1.0 (June 2026) froze the wire protocol**, so any two v1 endpoints interoperate regardless
of minor version or language binding. That matters more here than it would elsewhere: §9
establishes that cross-version coexistence is permanent under rolling per-platform releases,
and this removes the transport layer from the list of things that can skew.

### Pairing and enrollment

#### Two secrets, answering two different questions

Keeping these apart is what stops the web client looking like a special case, and almost every
question in this area comes from treating them as one thing.

| | What it answers | Where it comes from |
| --- | --- | --- |
| **Device keypair** | *Which devices are mine.* | An Iroh `NodeId` generated on the device. **Never typed, never leaves it.** Membership in `devices` is the trust boundary. |
| **`account_key`** | *Can this actor read store contents.* | Random 32 bytes, generated once; a password, phrase, or key file only **unwraps** it (§8). Needed **only to reach an encrypted store**; devices syncing peer-to-peer never use it. |

#### Three enrollment flows, identical on every platform

The web is not a special case in any of them. Which one you use depends on what is reachable,
not on what you are running.

**1. Device to device — the normal one.** Needs another of your devices online.

Two questions have to be answered, and keeping them apart is what makes this small:

- **Where** — which peer to dial. mDNS answers it on a local network; off-network it takes a
  `NodeId`, 32 bytes. Nothing else belongs here: resolving a `NodeId` to an address is
  discovery's job, and Iroh 1.0's framing is *dial keys, not IPs*.
- **Who** — that this is the device you meant and its owner agreed. mDNS cannot answer this;
  an attacker on the network can advertise too.

#### Confirmation by comparison, not by typing

The *who* half needs no secret transferred at all when both devices can speak. After the key
exchange, **each side derives a few words from the shared secret and the handshake transcript,
both display them, and the human compares and confirms on both.** Matching words mean there is
nobody in the middle — an interposed attacker necessarily produces two different secrets and
therefore two different word lists. This is the Signal safety-number, ZRTP, and Bluetooth
numeric-comparison pattern.

**Nothing is typed, and nothing is dictated.** For this project that is a large win: comparing
three spoken words across two devices is easy by ear, works identically on a watch where
typing is worst, and is symmetric — neither device has to be the one "showing", so there is no
better-keyboard rule to remember.

Use a **phonetically distinct word list**, the magic-wormhole and PGP-wordlist approach, chosen
so words survive a synthesiser and a braille display as well as a phone line.

**Typing a `NodeId` does not remove the need for this**, and the reason is worth being exact
about. Dialling a `NodeId` authenticates **one direction**: the joiner now knows it reached
that key. The device being joined knows nothing — it sees an inbound connection from an
unfamiliar key asking to pair, and a `NodeId` is not secret. Anyone who overheard or
shoulder-surfed it could dial during the same window, and a device that accepted automatically
would hand itself to whoever won the race.

So the words are load-bearing in every case: with mDNS they authenticate both directions, and
with a typed `NodeId` they authenticate the one the typing did not. **You type one identifier,
never two, and the comparison closes the other side.**

#### Headless peers need no special mechanism

A Raspberry Pi has no screen, but pairing is something a human does — and anyone pairing a Pi
is at a terminal on it. `lum pair` prints the words and asks yes or no, the same comparison as
anywhere else. Headless is not unattended.

That leaves **unattended provisioning** as the only genuine exception: a Pi built from a
config file with nobody present. There, put a long pre-shared token in the config. No PAKE and
no short code — the machinery for making short secrets safe exists to serve humans reading
them aloud, and a config file can hold 32 bytes without complaint.

*So there is exactly one pairing mechanism for everything a person does*, which is worth more
than the flexibility a second one would buy.

#### Getting the `NodeId` across, without depending on a service

Only the *where* half ever needs transferring, and often not even that. Three modes, in order
of how little they depend on:

| | Needs | Dictatable |
| --- | --- | --- |
| **Same network** — mDNS finds the peer, words are compared, nothing is transferred at all | **Nothing external.** No relay, no rendezvous, no discovery service. Unavailable in a browser, which has no mDNS (§16.12). | N/A — nothing to dictate |
| **Off-network** — the `NodeId` moves by copy-paste, file, or message; words confirm | Discovery, plus the relay Iroh already needs | Not really: 32 bytes |
| **Off-network, dictated** — the `NodeId` is published to a rendezvous keyed by a spoken code | A rendezvous service | Yes |

**The same-network path is the common case and costs nothing**, because pairing usually happens
with both devices in your hands — a new laptop beside the desktop, a phone beside the watch.
That is Bluetooth-style pairing at its best: both devices say the same three words, you agree
they match, and you confirm. No infrastructure, and nothing typed.

**The rendezvous therefore earns its keep in one narrow case**: two devices not on the same
network, where 32 bytes cannot be copied and must be spoken — in practice, dictating down a
phone line. If it is down, unreachable, or no longer hosted, pairing falls back to moving the
`NodeId` by hand and nothing is lost but that convenience. Build it so its removal is a
degradation, never a failure, and treat the same-network path as the one that must always
work.

**2. Account and password — needs nothing but a reachable encrypted store.** Self-hosting means
typing a URL as well, which is unremarkable. This is the flow for "I have no other device
awake", and it is **not a web mechanism** — a fresh laptop at 2am with the phone dead is the
same situation as a browser on a work machine.

Username plus password: the KDF unwraps `account_key`, which yields the auth value (§8), and
the device fetches and decrypts. **What it decrypts includes the `devices` document**, so
it now holds every one of your devices' `NodeId`s without anyone typing one.

**So this flow ends in the same word comparison as the others.** Rather than trusting itself in
on the strength of a password, the new device dials the first known peer that comes online and
runs the ordinary confirmation — no `NodeId` entered, because it already has them all. Which
gives the rule the whole section reduces to:

> **You type a `NodeId`, or find one by mDNS, or type a password. Never two of them.
> Every path ends by comparing words.**

Until that confirmation happens the device works — it has the key, so it reads and writes
through the store — but it is **not written into `devices`**, gets no direct peer connections,
and shows as pending. An unexpected prompt on your phone is then exactly the alarm you want:
*something enrolled with your password.*

*Be honest about what this does and does not buy.* With a store in play the password is a
**second trust boundary alongside the `devices` document**, and a weaker one: anyone holding it
can already read and write store contents, because the store checks an auth value and nothing
else. Word comparison governs direct peer connections and membership — it does not put the
password back in the bottle. Its real value is the alarm.

**3. Recovery phrase — the escape hatch, not a routine step.** For the password forgotten with
no device remaining. **Twelve words, not twenty-four:** 128 bits is ample for a personal task
manager, it halves the transcription burden, and BIP39's checksum catches errors at entry
either way (§8). Nobody should ever type this during ordinary enrollment; if they are, one of
the flows above has failed.

#### The encrypted store is not a paired device

Worth stating plainly, because "how do I pair with my store?" is a natural question with no
answer. It has no `NodeId` in `devices`, holds no replica, and runs no Automerge (§8). You do
not pair with it — you **authenticate** to it, with the account identifier and auth value. So a
headless self-hosted store needs no pairing UI at all: `lum store add <url>` from any client,
and the store accepts whoever presents a valid auth value for that account.

The **always-on peer** (a Pi, §8) is the opposite: a genuine paired device with a full replica,
enrolled the same way as any other device. Two things that both live in §8, only one of
them is a peer.

#### Transitivity, trust, and revocation

**Pairing is transitive, and this is the whole point.** The `devices` document syncs like any
other, so pairing a new watch with your laptop publishes it to every device already in the
set. The phone, the desktop, and the Pi all learn about it without a second confirmation.
Pairing *n* devices takes *n − 1* operations, not *n(n−1)/2* — with five devices that is four
pairings instead of ten, and the quadratic version would be unusable long before
anyone reached five.

**Membership in the `devices` document is the trust boundary**, and it is the only one. A
`NodeId` learned from that document is trusted; a `NodeId` learned any other way — mDNS
discovery, an inbound connection attempt, a device nobody confirmed — is not. This is what the
earlier "never sync to a peer merely because it learned a NodeId" rule was protecting, stated
in terms of where trust actually lives rather than as a prohibition.

Transitivity adds no exposure. Any paired device can already read and write everything, so a
device that can be added to the set could equally have been handed the data directly. That any
paired device may pair another is likewise correct for a single-user system.

**Unpairing is not revocation, and the difference must not be glossed.** Removing a device
from `devices` stops the others syncing with it in normal operation, but the removed device
keeps every byte it already holds, and — being a CRDT — nothing physically prevents a
malicious one from writing itself back in. Removal also only takes effect on devices that
receive it, best-effort like everything else.

Real revocation means rotating **`account_key` itself** so the old device can no longer decrypt
what it fetches — the expensive rotation of the two in §8, and the one still open. So: unpair
for a device you replaced, rotate for one that was stolen. Say which is which in
the UI rather than letting "remove" imply more than it does.

### Wake-up push (optional)

iOS and watchOS won't hold background connections. A content-free push service solves this:
when a device makes a change, it triggers a silent APNs notification (`content-available`)
that causes other devices to sync. The server learns only that *something* changed.

- iOS silent pushes are throttled and explicitly best-effort. Pair with `BGAppRefreshTask`
  and sync-on-foreground; treat push as an optimization, never a guarantee.
- The push server does learn timing metadata — which device, how often. Not content, but
  worth disclosing. Batching with jitter blunts it.
- Android is far more permissive: foreground services and `WorkManager` cover this without
  a third party.
- **Web Push** (§16.12) covers an installed web client, and is the easiest of the three:
  **VAPID requires no platform enrollment**, so the device making the change sends the wake-up
  itself and no intermediary service is needed. RFC 8291 encrypts the payload, so the push
  service learns endpoint and timing but not content.

- Entirely optional and disableable.

Note the asymmetry between the two uses of push, because it decides what the encrypted store can
and cannot do. A **sync wake-up** is triggered by a device that just made a change — it is
awake by definition and needs to decrypt nothing to say "something moved". A **reminder** is
triggered by the clock, and knowing one is due requires reading the data. That is why the
encrypted store can never be a reminder sender however convenient it would be (§16.12).

### Honest framing

Relay servers are still involved when hole punching fails. They forward ciphertext and
cannot read it. The accurate claim is *"no server ever sees your data,"* not *"no servers."*

---

## 8. Local storage and multi-process coordination

Several processes may want the same data on one machine — the GTK app in the tray, a CLI
invocation, an Emacs subprocess. **This is solved without *requiring* a daemon or a system
service.** Coordination is a property of the store itself, so the design degrades correctly to
nothing running at all — nothing need be registered with systemd, launchd, or the Windows
service manager. A daemon is *available*, is advisable on some platforms, and is required on
BTSpeak (see *Background sync without a GUI* below); nothing in this section depends on one.

### Storage layout

A single SQLite file per profile, holding three kinds of thing:

```
changes(rowid, doc_id, hash, data)   -- append-only Automerge change chunks
snapshots(doc_id, heads, data)       -- periodic compaction (Automerge save())
<read model tables>                  -- tasks, blocks, assignments, indexes, FTS
read_model_meta(doc_id, heads)       -- which document state the projection reflects
```

**A document *is* its changes.** There is no separate "document" record — loading the change
chunks reconstructs it. Snapshots exist only to bound load time, since replay is linear in
the number of changes: past a threshold of a few thousand, write a fresh snapshot and delete
the changes it subsumes.

**History cannot be pruned, and does not need to be.** Automerge changes reference their
dependencies by hash, so discarding old operations breaks the DAG for any replica that has
not merged past them. Safe pruning would require every device to agree on a fully-merged
common prefix and drop it simultaneously — impossible when a device may be offline for
months. `save()` compacts the *representation*; it does not discard operations.

Nor is it a problem at this scale. A heavy user generating ~100 operations a day produces
roughly 2 MB a year compressed — around 20 MB over a decade, fine even on a watch. The cost
that matters is **load time**, which snapshots and year-sharding already bound.

**Archive closed shards out of the default sync set.** Once `blocks-2023` is closed it is
read-only history; devices fetch it on demand rather than carrying it. Nothing is discarded.

Automerge chunks are stored as **opaque blobs**. SQLite replaces the *file*, not the
representation, so there is no structural conversion — the gain is transactional appends and
multi-process locking instead of hand-rolled file locking.

The read model is a strictly **one-directional projection**: Automerge to SQLite, never back.
SQLite is never a source of truth for anything.

### Startup

1. Open SQLite in WAL mode.
2. Load the `core` document: its snapshot, then `load_incremental` for changes after it.
   `blocks-<year>` documents load **lazily**, only when that year is viewed — this is what
   keeps the watch viable.
3. Compare document heads to `read_model_meta`. Equal in the normal case, so the projection
   is used as-is. Rebuild only on mismatch.
4. Record the highest `changes.rowid` as this process's watch cursor.

### Writes

Mutate the in-memory document, append the change bytes, apply the resulting patches to the
read model — **all in one SQLite transaction**, so the projection cannot diverge from the
truth.

### Cross-process change notification

SQLite has no cross-process notification — `sqlite3_update_hook` and `wal_hook` fire only for
your own connection. So:

- Watch the `-wal` file with the `notify` crate (inotify / FSEvents / `ReadDirectoryChangesW`).
- On a filesystem event, check **`PRAGMA data_version`**, which changes when another
  connection has modified the database and is nearly free to read.
- If it moved, read `changes` rows past the cursor, `load_incremental` them, take the
  resulting **patches**, apply them to the read model, and refresh the UI.

Fall back to polling `data_version` on a one-second timer where file watching is unreliable
(network filesystems). Either way it is imperceptible.

This is the same path whether a change originated locally, from another local process, or
from Iroh sync — which is a good sign the design is right.

### Sync leader election — an optimization, not a requirement

When processes **share a store**, an **advisory file lock** decides which one runs the Iroh
endpoint. Whoever holds it syncs; everyone else reads and writes the store normally. When the
leader exits, the next process that wants the lock takes over. The tray-resident app holds it
all day; a one-shot CLI invocation grabs it briefly if nothing else is running.

**Processes that cannot share a store still work.** Each simply becomes its own peer with its
own `NodeId` and its own replica, and Automerge converges regardless of replica count. The
data is small and local discovery is fast, so the overhead is negligible.

This matters because it **decouples the architecture from packaging**. A sandboxed
distribution — Mac App Store, or Flatpak, where the GUI's container is unreachable from a
separately installed CLI — degrades to independent replicas rather than breaking. Neither
store is ruled out by §8.

Three costs, all UX rather than technical:

- The device list gets noisier; one machine can appear twice.
- A CLI install on a machine that already has the GUI needs pairing of its own, which reads
  as odd.
- If two resident instances ever coexist on one machine, reminders must be deduplicated or
  they fire twice.

Shared store with a lock is the better arrangement. Independent replicas are an acceptable
fallback, not a failure.

### Device identity is per-store, not per-process

The device keypair lives **in the store**. Every process using that store *is* the same
device; the lock (above) only decides which one currently listens on the network.

This resolves what would otherwise be a mess: a CLI falling back to its own Iroh endpoint
because no daemon is running is not a second device, just the same device without a daemon
holding the endpoint. Identity proliferation happens only with genuinely **separate** stores —
the sandboxed-install case — where independent replicas are the intended behaviour anyway.

### Three roles, easily conflated

All three are the same binary (§2), and confusing them is the single most likely way for this
design to be described inconsistently.

- **In-process core** — any client that can link Rust (the CLI, GTK4, Win32, AppKit, iOS,
  Android, the watches) opens the store itself. Reading and writing data involves no IPC.
- **`lum rpc`** — a stdio JSON-RPC server, one per client, for clients that *cannot* link
  Rust: Emacs and the BTSpeak app. Opens the store exactly as a linked client does, watches
  the WAL, pushes updates. No Iroh.
- **`lum sync-daemon`** — resident and headless. Holds the sync lock, runs the Iroh endpoint,
  keeps the store fresh. It **also** serves the same JSON-RPC surface (§12) over a **Unix
  domain socket** (Linux, macOS) or **named pipe** (Windows).

**The daemon's two jobs are independent.** *Being resident* is what CLI-only and BTSpeak users
need. *Serving the socket* is a convenience that saves an RPC client from spawning a second
process. Neither implies the other, and a claim about one is not a claim about the other.

**One protocol, two transports.** Socket and stdio carry an identical command surface, so an
RPC client opens one pipe or the other and is otherwise unaware of which it got.

**The fallback rule differs by client class.** There is no single rule, and stating one is how
this got muddled:

| Client class | Normal path | When no daemon is running |
| --- | --- | --- |
| **Linked** — CLI, GUIs, mobile, watches | Open the store directly | Unchanged: take the lock, run Iroh in-process |
| **RPC** — Emacs, BTSpeak | Connect to the daemon socket | Spawn `lum rpc` over stdio |

Linked clients never need the socket for *data*. They use it only for operations that must
reach whoever currently holds the endpoint — `pair`, `sync-status`, force-sync — and perform
those locally when nothing answers (*Pairing without a GUI*, below).

**Nothing requires the daemon, with one platform exception.** BTSpeak has no tray and no
session GUI, so something must be resident there to keep **sync** running (§16.11). That is a
statement about **residency**, not transport: the BTSpeak UI speaks the same JSON-RPC either
way, and spawns `lum rpc` if the service happens to be stopped. Reminder *delivery* is a
separate process again — see below.

### The daemon never delivers reminders

**Delivery belongs to whatever process owns an output device.** The daemon owns none by
definition — it is headless — so it is never the thing that announces. This is a hard rule,
not a setting, and it means there is no deployment-dependent special case to get wrong.

Every delivering process is a **resident client**:

| Platform | Delivers |
| --- | --- |
| Windows, macOS, Linux desktop | The tray / menu-bar resident app (§16.2) |
| BTSpeak | A small Python service — `host.say()` plus a sound (§16.11) |
| Console-only Linux — speakup, BRLTTY, no session | `lum notify`, below |
| iOS, Android, watchOS, Wear OS | The OS, from pre-scheduled local notifications (§11.2) — this path is not involved at all |
| The always-on peer (a Pi or VPS) | Nothing. Correct by construction, not by configuration |

**Scheduling is a core function, not a daemon function.** `reminders_due(since, now)` is
available to every client, over FFI and over RPC. Any resident process drives it off the WAL
watch it already has (*Cross-process change notification*, above). The daemon may push
due-reminder events to RPC subscribers as a convenience, but nothing depends on that — polling
`reminders_due` on a 30–60 second timer is fine at this data size, and is already the
convention on BTSpeak.

A subscriber that starts late — the tray app was restarted, the console user just logged in —
asks what fired while it was away. Announce anything under a few minutes stale, fold the rest
into a summary, and let `ReminderAck` (§3.9) suppress what another device already handled.

**`lum notify`** is the console-Linux delivery client: a foreground process started from a
shell profile that subscribes and speaks. Writing to the controlling tty is the universal
mechanism there — speakup and BRLTTY both read the console — with `spd-say` available when
speech-dispatcher is running. This is the same shape as the BTSpeak Python service, in Rust.

*Why not let the daemon do it on Linux anyway?* Desktop notifications are D-Bus
(`org.freedesktop.Notifications`) rather than X or Wayland, so a `systemctl --user` unit can
usually reach the session bus at `$XDG_RUNTIME_DIR/bus` — the daemon **could** technically
call it. It still shouldn't. When a graphical session exists, the tray app exists and is the
better deliverer; when one doesn't, the bus has no notification service listening and the call
goes nowhere. A *system* unit — the BTSpeak pattern, `User=pi` — has no session bus at all
without guessing at `XDG_RUNTIME_DIR`. Every branch either duplicates a better path or fails,
which is what makes this a rule rather than a trade-off.

The always-on peer is also the one deployment that holds **all** documents including every
year shard — §3.1's sharding exists for memory, and nothing there is memory-constrained.

### Background sync without a GUI

A tray-resident app holds the lock and syncs all day (§16.2). **CLI-only users have no tray**,
so they need a way to keep sync running. Required on BTSpeak (§16.11), which has no tray at
all; optional but worth offering everywhere else.

Provide `lum daemon install` / `uninstall` so nobody hand-writes a service definition:

- **Linux** — a `systemctl --user` unit, **plus `loginctl enable-linger $USER`**. Without
  lingering the service stops when the last session ends, which defeats the purpose. This
  belongs in the command, not in documentation nobody reads.
- **macOS** — a `~/Library/LaunchAgents` plist. Shipping a Homebrew formula also makes
  `brew services start lum` work, which is what Mac CLI users will reach for first.
- **Windows** — a Task Scheduler entry at logon. No administrator rights needed, unlike a
  real Windows service.
- **BTSpeak** — its own convention: a system-level unit with `User=pi`, which sidesteps
  lingering entirely.

Auto-updating package managers matter more for the CLI than for GUI targets, since a CLI-only
user has no app to prompt them and no store to update them. Homebrew and winget are the
mechanism (§17).

On Windows and macOS the tray-resident app covers most users, so the daemon is genuinely
optional there. **On Linux it matters much more**: users running console-only with speakup or
BRLTTY and no X or Wayland at all, and users of desktops with no system tray, have nothing to
be resident.

### The always-on peer

A headless daemon answers the one structural weakness of pure p2p: **two devices must be
online simultaneously to reconcile.** An always-on peer removes that — the phone syncs to it
whenever it wakes, the desktop syncs whenever it opens, and the two never need to overlap.

It is **not a server**. It is another paired device running the same binary and holding your
own replica, so it changes neither the protocol nor the trust model, and it is entirely
optional.

**Recommended deployment: hardware you control** — a Raspberry Pi at home. Same always-on
benefit, no trust problem; the threat model becomes physical theft, which is tractable.

**A VPS works but cannot be secured against the provider.** Encryption at rest does not
rescue it: a key file sits in the same snapshot, a vTPM is controlled by the hypervisor, and
full-disk encryption with remote unlock survives snapshots but not a RAM dump. **Unattended
boot and provider-resistant encryption are fundamentally in conflict** — anything the machine
can use to decrypt itself without a human is available to whoever holds the image. Support
VPS deployment, and document plainly that the provider can read the data. Do not paper over
it with an encryption claim that does not hold.

### The encrypted store

**Named for what it holds.** Not "blind store", the earlier working name: for an
audience of blind users, blindness as a metaphor for ignorance is the wrong word to build
into the product. Not "untrusted store" either, which describes the threat model rather than
the thing, and invites the reasonable question of why anyone would use one; that it needs no
trust belongs in the sentence explaining it. In the CLI it is simply `lum store`.

**Not "relay"** — Iroh already uses that word for its hole-punching fallback servers, and this
is a different thing. More importantly it must genuinely **store**: a device that only forwards
live traffic helps solely when two devices are already online, which is the problem it exists
to remove.

An **encrypted store** persists change chunks — encrypted before they leave your devices —
addressed by hash, and serves them on request, never running Automerge and never holding a
key. Clients exchange hash lists to find
what they are missing, losing Automerge's efficient delta negotiation in favour of naive set
difference, which is acceptable at this data size.

Encryption at rest is therefore **inherent** here, not an added feature: chunks arrive
encrypted and are stored as they arrived.

**Not v1, but a recorded dependency rather than an idea.** Three things rest on it:

1. **Optional paid hosting** (§21) — an always-up sync peer for people who cannot or will not
   self-host, where *the operator cannot read the data*. Open source is what makes that claim
   verifiable rather than a promise.
2. **A web client** (§16.12) — not a prerequisite, since a browser *is* an Iroh peer over a
   relay, but the store is what lets one sync when no other device happens to be awake.
3. **Attachments**, if ever (§21) — a persistent hash-addressed blob store is exactly the
   primitive they need, so the cost of that feature drops sharply once this exists.

So the sync layer must not acquire assumptions that make a hash-addressed opaque-chunk
transport hard to add later. As a service it also has to be cheap to run for many users, not
merely workable for one person's three devices.

### Encryption and key management

Required by the encrypted store, and the hardest security work in the project.

#### One account key, several wrappers

The password must not *be* the encryption key. If it were, changing it would mean re-encrypting
everything the store holds, and the store holds append-only chunks — so a password change would
be an O(data) operation that also breaks every other device until it finished. That is the
mistake this structure exists to avoid.

```
account_key          random 32 bytes, generated once, never changes
   ├── Wrapper { Password, salt_a, argon2id_params, wrapped, label }
   ├── Wrapper { Phrase,   salt_b, hkdf,            wrapped, label }
   └── Wrapper { KeyFile,  salt_c, hkdf,            wrapped, label }

auth_value = HKDF(account_key, "store-auth")   — store keeps only hash(auth_value)
```

**`account_key` is what actually encrypts chunks.** Everything else is a way of getting at it.

**Each wrapper is self-contained, with its own salt and its own KDF**, and both parts matter.
Separate salts let credentials rotate independently — a shared salt would mean changing the
password silently invalidated the phrase and the key file. Separate KDFs because only the
password needs stretching: it is the one low-entropy input, so it gets Argon2id, while a BIP39
seed and a key file already carry full entropy and need nothing slower than HKDF. Making a key
file go through Argon2id would cost seconds and buy nothing.

`label` exists so a person with three key files can tell which is which before revoking one.

**All three are the same mechanism, not separate keys** — which is why they coexist, why any
one can be replaced without touching the others, and why a fourth would be nearly free:

- **Password** — required, because enrolling with no other device reachable needs something
  memorable (§7, flow 2). The one low-entropy input, hence Argon2id.
- **Recovery phrase** — BIP39, **twelve words**, generated at setup and not offered later.
  128 bits is ample and it halves what has to be transcribed or read aloud. The accessible
  choice deliberately: the usual alternative is a QR code, unusable without sighted help,
  whereas a phrase can be **read aloud, copied, or saved to a file** — support all three — and
  its **checksum catches a transcription error at entry** rather than when someone is already
  locked out.
- **Key file** — a high-entropy wrapping key, *not* `account_key` written to disk, which would
  make a leaked file impossible to revoke without re-encrypting everything. The natural fit for
  unattended provisioning (§7).

Randomness comes from `getrandom`, which wraps every platform's CSPRNG. Nothing to design.

*Enrolling with a password:* fetch wrappers by account identifier → `Argon2id(password,
salt_a)` → unwrap → `account_key` → derive `auth_value` → authenticate → fetch chunks.

#### Deriving `auth_value` from `account_key`, not from the password

This is what makes password changes trivial rather than delicate. If the auth value came from
the password, changing the password would change it, the store would have to be told, and the
store would have to *authorise* that change — presenting the old auth value, or worse, the old
password. Every one of those is a step that can fail halfway and leave a device locked out.

Deriving it from `account_key` removes the problem instead of solving it. `account_key` never
changes, so **`auth_value` never changes**, so a password change is invisible to the store.
HKDF is one-way, so a store holding the value learns nothing about `account_key` — and it
should keep only `hash(auth_value)`, so a database leak does not immediately grant access.

**Both live in local state**, alongside `account_key` (§3.12). Re-deriving `auth_value` per
request would mean keeping the password around or prompting for it, and Argon2id is
deliberately slow — the whole point of caching it is that you never type a password to reach
your own store.

#### So changing the password is cheap, and needs no old password

Re-derive a wrapping key from the new password and a fresh salt, **re-wrap `account_key`
alone**, replace that one wrapper. Nothing is re-encrypted, the store's view is unchanged, and
no other device is disturbed.

It needs only `account_key`, which every enrolled device holds — so *"change password"* works
from any device you are signed in on, with the old one forgotten. That is the ordinary recovery
path and the UI should point at it before mentioning the phrase.

*And if even the auth value is lost:* the store is **disposable**. An account there is a
namespace and a hash, nothing more, so a device holding `account_key` can create a fresh
account and re-upload. You lose the server-side copy and nothing else. Worth knowing, because
it means no store-side state is ever load-bearing.

**A forgotten password is only fatal if every device is also gone.** The phrase is for that
terminal case alone — the store cannot reset what it cannot decrypt, so nothing else helps
there, which is why it has to exist from the start rather than be offered later.

#### A username is the account identifier, and the password is what matters

**Wrappers are fetchable by account identifier without authenticating**, and they have to be:
unwrapping is how you *get* the key that produces the auth value. So anyone holding the
identifier can attack the password offline.

The tempting fix is a random identifier. **It defends against the wrong adversary.** The threat
this whole design exists for is a **malicious or breached store operator** — and they hold the
database, so the identifier buys nothing against them. It only slows a third party who has
neither the database nor your identifier, which is not who we are worried about. Against the
operator, offline resistance rests entirely on password entropy and the KDF.

It also costs more than it looks. A random identifier is not memorable, so it gets written
down — and it gets written down *next to the password*, which means it adds no independent
secret. Worse, it is paid exactly in the scenario the store exists for: reaching your data from
a machine with nothing else to hand.

**So: a plain username, chosen by the user, and put the effort into the password instead.**

**Not an email address**, and not because it would be less secure — because there is no reason
to hold one. The store never sends mail: password reset is impossible by construction, there
are no notifications, and recovery is the phrase. An email would be personally identifying
data collected for nothing, with a validation flow that serves no purpose and a breach that
leaks more than it needs to.

That matters for the claim this store makes about itself. The honest framing below concedes
that the host learns *account identity*, connection timing, and chunk counts. If that identity
is a handle you invented, it is much weaker than an email tied to your real name. Collecting
less is the cheapest privacy improvement available here, and it removes a GDPR surface (§21
q6) rather than mitigating one.

*If paid hosting ever happens*, billing needs a contact address — but that belongs to the
**payment processor**, not the store. Keeping them separate means the store's database and the
billing database share no identifier, which is a better arrangement than the one an email
would have forced.

Rough numbers for Argon2id at sensible parameters (64 MiB, t=3), assuming a well-funded
attacker at ~10⁶ guesses/second:

| Password | Offline search |
| --- | --- |
| Human-chosen "strong" (~40 bits) | days |
| Four-word passphrase (~51 bits) | decades |
| Five-word passphrase (~64 bits) | ~10⁵ years |

The gap between the first row and the third is far larger than anything an identifier
contributes, which is where the design effort belongs:

- **Generate a five-word passphrase by default**, and make accepting it the path of least
  resistance. You already generate a BIP39 phrase, so this is a familiar interaction.
- **Enforce a floor** on user-chosen passwords with `zxcvbn`-style estimation, and state the
  estimate in words rather than a coloured bar — *"about six days to guess"* is a sentence a
  screen reader can read and a person can act on.
- **Rate-limit wrapper fetches** by identifier. Useless against the operator, cheap, and it
  does close off casual third-party enumeration.

*The honest framing to show the user:* against a hostile store operator, your password is the
only thing protecting your data. That is true of every zero-knowledge service, it is why the
generated passphrase is the default, and it is worth one clear sentence at setup rather than
being left implied.

#### `account_key` does not live in the documents

It reaches a new device over the **pairing channel** — authenticated, after word comparison
(§7) — or by unwrapping from the store. It is local state (§3.12), never document content.

The reason is specific to this design: Automerge history is permanent (*Storage layout*,
above), so a key written into a document is in that document's history **forever**, surviving
any later rotation. Keys belong somewhere overwritable, and this also avoids two sources of
truth for one secret.

#### Two rotations, and only one of them is cheap

Worth separating, because "rotation" has been used for both:

- **Credential rotation** — changing a password, replacing a key file, regenerating a phrase.
  Re-wrap `account_key`. Cheap, local, no coordination.
- **`account_key` rotation** — needed when a device is *stolen* rather than replaced (§7), since
  the old device holds the key itself. This means re-encrypting everything the store holds and
  ensuring every remaining device picks up the new key. Expensive, requires coordination, and
  **remains the open question here.**

**Honest framing:** *"we cannot read your tasks"* is true. *"we learn nothing"* is not. The
host still sees account identity, chunk counts and sizes, timing, and which devices connect
when. State that, because this product's audience is exactly the audience that checks.

### Pairing without a GUI

**Pairing runs on an endpoint of its own, not the device's** — so it never has to route
through the daemon at all. Each `lum pair` opens a short-lived endpoint under a key minted for
that one pairing. The words authenticate that connection; only once they are confirmed on both
sides does each device say which device key it is, and those keys go into `devices`. Two
things follow:

- **The daemon needs no prompt.** It holds the device endpoint and keeps syncing, headless,
  while the person pairs from a terminal beside it. Nothing has to carry words and a yes/no
  between a headless process and a terminal.
- **Trust is unchanged.** A device key enters `devices` only by being stated over a channel a
  person confirmed — never by being seen on the network.

```
lum pair                 # on both devices, on one network: they find each other by mDNS
lum pair <code>          # off-network: dial the code the other device's `lum pair` printed
lum sync                 # one round now; asks the daemon to, if one is running
lum sync status          # sync_status() (§9), as sentences
lum sync-daemon          # hold the endpoint, keep syncing, serve the socket
lum device list|rename|unpair
lum store add <url>      # enrol against an encrypted store, §7 flow 2 (not built yet)
```

Each pairing prints the comparison words and asks yes or no — the same confirmation every
other platform shows, which is why a headless peer needs no special mechanism (§7).

The code a person types off-network is the pairing session's key, not the device's: good for
one pairing, then gone. That is still *one identifier, never two*, and the words still close
the other direction.

`--local-only` on any of these binds with no relay and no lookup service: the local network
and nothing outside the building. Useful where the n0 infrastructure is unwelcome, and what
the tests use.

### The read model is optional for v1

The change store is not optional; it is how documents persist and how processes coordinate.
The projection is an optimization. With a few thousand tasks, filtering and sorting the
in-memory Automerge document directly is fast enough, and queries go through core functions
either way — so adding it later changes no interfaces. Full-text search is the one thing
likely to pull you back toward it early.

---

## 9. Durability: backup, export, import, schema

### Backup — sync is not backup

**Peer-to-peer sync is replication.** Replicated corruption is still corruption, replicated
deletion is still deletion, and losing every device loses everything. Todoist users have
server-side backups they never think about; ours would have none.

**Every client implements this, not just `lum sync-daemon`.** The daemon is optional on most
platforms, so anything that only it does effectively does not exist. A resident daemon may
run backups on a schedule; a one-shot CLI invocation or a GUI launch runs one if the last is
older than the configured interval. Same code in `store/`, triggered opportunistically.

- Dated snapshot of the store, last N retained, pruned oldest-first.
- Written outside the live database directory so a corrupting bug cannot take both.
- Automerge `save()` output rather than a raw file copy — self-describing and version-checked.

**Never default to a cloud-synced directory.** A backup written to `~/Documents` or
`~/Desktop` can land inside iCloud Drive, OneDrive, or Dropbox, putting local-first data on
someone else's server and quietly defeating the premise. Default to a platform data directory,
and if the user chooses a synced path, say so plainly rather than silently complying.

**Backups contain full history.** An Automerge document retains its change log, so a backup
holds every task ever created — including every one deleted. These files are materially more
sensitive than the live view suggests, and that should be stated where the user chooses a
location, not buried in documentation.

**Permanent erasure.** Since history cannot be pruned (§8), "delete" honestly means "hidden
from current state." That is a fine default, but there must be a way to make something
genuinely gone — someone will eventually put something in a task title they need erased. The
only mechanism is **rebuilding the document without it**, after which every device re-syncs
from scratch and all history is lost. Expensive and rare, and the UI should say so:
*"Permanently erase — rebuilds your database and re-syncs all devices."*

### Export and import

Two distinct things, with different sensitivity, kept visibly separate in the UI:

**Current-state export** — what most people mean by "export". Present state only, no history:

- **JSON** — structured, machine-readable
- **Markdown / org** — tasks, human-readable
- **iCalendar** — blocks, so a planner day can be read by anything

**Full-fidelity backup** — the change log, for disaster recovery and device migration. This is
the round-trip format, and the one that contains deleted tasks (above).

Conflating the two is how someone emails a "task list" that turns out to contain everything
they ever deleted.

Import accepts either. **Import of our own output must be tested, not assumed.** A disaster-recovery path that is
never exercised does not work. Round-trip property tests belong in the same suite as the
convergence tests (§14).

There is also a trust argument. Data lock-in costs blind users more than sighted ones,
because migrating apps means relearning an entire interface rather than importing rows. A
credible export story is a genuine differentiator, and it is what the todo.txt audience
actually cares about.

### Undo

**Automerge does not provide undo.** It provides history, but undo is not rewinding the
document — rewinding would also discard concurrent remote changes. Undo means computing and
applying an **inverse change**.

So: every core mutation returns its inverse, and a bounded undo stack holds them. Redo
likewise.

- The stack is **local-only and per device** (§3.12). Undo is not a synced concept. It is
  saved in the profile's SQLite file rather than held in memory, because a one-shot CLI
  process has no session to hold it in; every process on the device shares it, so `lum undo`
  in a terminal reverses what the BTSpeak app just did.
- **Rebased on the current state.** An entry may be undone after other edits, local or
  merged from another device, so only the fields the edit changed — and still hold what it
  set — go back. A field changed since is kept and announced, never silently overwritten.
- Multi-level.
- **Announceable**: *"Undid: completed Review PR."* Never a silent state change.

This matters more here than in a sighted app. A sighted user notices "wait, that moved"
immediately; without that channel a mis-keystroke can go unnoticed for minutes, by which
point the context for recovering is gone.

### Sync status

**Silent sync failure is the worst failure mode a peer-to-peer app has** — you believe you
are synced, you are not, and you discover otherwise by losing work.

```
sync_status() -> {
  is_leader:    bool,
  last_success: Option<Timestamp>,
  peers:        Vec<{ node_id, name, reachable, last_seen }>,
  last_error:   Option<String>,
}
```

Surfaced as **text, not an icon**. A cloud glyph with a slash through it communicates
nothing.

### Schema versioning

Automerge has no schema concept. Adding optional fields is safe; renaming, restructuring, or
changing a field's type is not.

**This is worse in peer-to-peer than client-server**, because there is no server to gate on
and no way to force an update. A device running last year's build will happily keep writing
the old shape into documents the new build reads.

- `schema_version` per document.
- Forward-compatibility rule: ignore unknown fields, and **never drop unknown keys when
  writing back** — otherwise a stale client silently destroys newer data on every edit.
- Migrations run only when all devices in the `devices` document report a version at or above
  the threshold.

Design this before v1 ships any real data.

**Rolling per-platform releases make cross-version coexistence permanent, not transitional.**
Platforms ship independently as each is ready (§18), so there is no moment when everyone runs
the same build. A user will run a six-month-old iOS build alongside a fresh Windows one,
indefinitely, and there is no mechanism to force an upgrade. Mobile auto-update covers most
users, but not those who disable it, cannot run the current OS, or are out of storage.

Two consequences:

- The "never drop unknown keys when writing back" rule stops being a precaution and becomes
  the only thing preventing **silent data loss between your own releases**. A stale client
  round-tripping a newer document must preserve what it does not understand.
- **Sync robustness matters far more at the second public platform than the first.** Until
  then, sync bugs affect only you. Sync status (above) earns its place at exactly that point:
  a user with mismatched builds needs to see that something is not reconciling.

Since the `devices` document already carries a `schema_version` per peer, the app can **detect
version skew directly and tell the user** — "your iPhone is running an older version" — which
is more useful than any update-check mechanism, and works on platforms with no auto-update at
all.

---

## 10. Planning and auto-scheduling

Four distinct core capabilities, all **pure functions over the day's state**. Each returns
a *proposal*, never a mutation. The core never silently rewrites the plan — for a screen
reader user especially, a day that reorganised itself without announcement is disorienting
and erodes trust in the whole app.

### 10.1 Slip absorption (re-flow)

Triggered when the current time is past where the plan says you should be, or on demand.

```
propose_reflow(day, now, pinned: Vec<Constraint>)
    -> Proposal { changes: Vec<Change>, unplaceable: Vec<BlockRef>, alternatives: Vec<Alt> }
```

Walk forward from `now`. **`anchored` blocks are fixed points** — this is what that flag was
introduced for. Movable blocks shift later, routing around the anchors, compressed if
necessary down to their own `min_duration_mins` (§3.6). Nothing may be moved earlier than `now`, and
nothing may be pushed outside `Settings.day_window`.

When it doesn't fit — and it often won't — say so explicitly. `unplaceable` names what falls
off the end so the user decides what to drop or defer, rather than the core silently
truncating the day.

#### Three situations, routinely conflated

- **The work overruns the block.** Assigned work exceeds the time left in it. Nothing is late
  yet — this is *predictable in advance*, and is the interesting case.
- **The block overruns the day.** You are still working past its scheduled end, and the next
  block should already have started.
- **A block never happened.** The previous one ran straight through it.

Only the last two are slips. The first is a plan that was wrong when it was made.

#### Detect it before the clock does

**A sighted user reads this off the timeline for free.** Forty minutes of work in a block with
twelve minutes left *looks* wrong — the bars don't line up — and they notice without anyone
deciding to tell them. Reduced to a list, that information is simply absent unless the core
computes it and says so.

So the check is **predictive, not reactive**: fire when remaining assigned work first exceeds
remaining block time, not when the end time passes. This is the single most valuable thing the
planner does for the audience in Principle 6, and it is nearly free — the reminder scheduler
already runs, and `BlockEnd` is already a reminder anchor (§3.8).

#### When the block itself runs late

**The user is never shown a menu of strategies.** "Would you like to move, compress, or defer?"
is a bad question in any modality and an awful one in speech — it asks someone to simulate an
algorithm in their head. The core produces **one concrete proposal** in terms of actual
changes — *"Break moves to 3:15. Deep work moves to 3:30. Review PR does not fit and moves to
tomorrow."* — which is accepted wholesale, accepted per change, or rejected. Alternatives
exist behind an explicit "other options", not up front.

So what follows is the algorithm's preference order, not a dialogue.

**One global choice, then per-block behaviour.** The global choice is real and worth
surfacing: *move the work, or move the day.* Deferring a task out of an overrunning block
touches no commitments; shifting every subsequent block rearranges the day. Prefer moving work
— a block is a commitment about the shape of your day, an assignment is only a plan for one
task.

Once the day must move, each subsequent block is handled **by its own constraints**, not by a
global strategy:

| Block | What re-flow does |
| --- | --- |
| `anchored` | Nothing. Fixed point; the walk routes around it. |
| Compressible, has slack above `min_duration_mins` | Absorb what it can by shrinking. |
| At its minimum — including any `Break` by default | **Move later, never shrink.** |

Your fifteen-minute break after a thirty-minute overrun therefore **moves**, and is never
squeezed to three minutes, because `min_duration_mins` for a Break defaults to its full
duration (§3.6). That is not a special case in the algorithm — it falls out of the block
describing itself.

**"Compress" means shrink a block's duration in place to absorb overrun** — a 60-minute work
block becoming 45 so the rest of the day still fits. It is the quietest intervention available
and the easiest to do damage with, which is exactly why the floor is per-block rather than
global.

**The day has a fixed length, and the planner cannot create time.** When moving and
compressing both run out, something leaves the day. Say so; `unplaceable` already carries it.
A plan that silently truncates is worse than one that admits it does not fit.

**An anchored block already in progress is not a scheduling problem.** If your class started
ten minutes ago and you are still working, no rearrangement helps — the only real choices are
to stop now or to accept missing it. Report the conflict; do not propose a re-flow that
quietly pretends the class moved.

#### Unfinished work when a block ends

This is not a failure state. **Multi-sitting work is the premise of the app** (§3.7), so an
assignment ending incomplete is the ordinary case, not an exception to handle.

At block end, if an assignment has `accumulated_mins` below its `planned_mins` and the task is
not complete, offer to carry it forward — *"Deep work ended. Essay draft: 40 of 90 minutes.
Carry the remaining 50 to tomorrow's Deep work?"* Find the target by walking forward for the
next block whose `task_filter` admits the task, which is what that field was for.

**Offer, never do it silently.** Auto-carrying quietly produces a tomorrow that fills up
without anyone deciding it should, and the first time a user discovers a week of automatic
decisions they stop trusting the planner entirely. The prompt is also the moment where the
honest answer is often "this estimate was wrong", which no automatic carry can notice.

Carrying forward can itself overfill the destination day. Run the same capacity check and
report it rather than stacking silently.

#### Constraints bind the planner, never the user

This is the rule that keeps good defaults from turning into a cage, and it is easy to violate
by accident the first time someone writes a validator.

`min_duration_mins`, `anchored`, `day_window` and the rest describe **what the algorithm may
propose**. None of them describes what the user may do. A break is incompressible *to
re-flow*; the user can still shorten it to five minutes, skip it entirely, or delete the
occurrence. Nothing in the mutation API consults these fields.

Three levels, and they must not be collapsed:

| | `min_duration_mins` is |
| --- | --- |
| The primary proposal | **Hard.** Never propose below it. |
| Alternatives | **Soft.** May be offered, and must say what it costs. |
| Direct user action | **Absent.** Not a constraint at all. |

#### Amending a proposal, and why it re-runs

Accept-or-reject per change is not enough on its own. A user who says *"actually, just skip
the 3:00 break"* has made a decision the proposal did not contain, and the rest of the day is
now different: fifteen minutes just came free.

So a proposal is **re-entrant**. Decisions feed back in as `pinned` constraints and the
proposal is recomputed — keep this block where it is, drop that one, this task moves to
Thursday instead. Without that, accepting an amended proposal applies a plan computed against
assumptions that no longer hold, and the day quietly stops adding up. The same mechanism gives
§10.5 its pinning for free.

**Skipping needs no new machinery.** "Skip this break" is a `Cancelled` `BlockException` for
that occurrence (§3.6); "end at 4 instead of 5" is a `Modified` one. Both already exist, both
sync, and both are undoable.

**Offer the constraint-violating alternative, but name its cost.** The primary proposal moves
your break. `alternatives` may carry *"or skip the 3:00 break and everything else stays where
it is"* — genuinely often what someone wants — as long as it is labelled: *"this is your only
break today."* Silently proposing it would be wrong; refusing to offer it is paternalism.

#### Day-level commands, independent of any proposal

Ending work early is a **decision**, not a response to a notification, and it must be
reachable when nothing has gone wrong and nothing has prompted you:

```
end day now          # cancel or truncate what remains
skip block <ref>     # this occurrence only
clear rest of day
replan from now      # discard the current shape, re-flow from this moment
```

Each is a user-initiated verb that may *produce* a proposal — ending the day early leaves
assignments needing somewhere to go, so offer the carry-forward above — but none of them
requires one to already exist.

#### When not to propose at all

Proposing on every slip is noise, and noise is what gets a planner ignored. Stay silent when
the overrun fits in the gap that follows it, when it is under a threshold (a few minutes), or
when the user has already dismissed a proposal for this block. **A five-minute overrun into
twenty minutes of free time is not a problem and must not be announced as one.**

### 10.2 Task suggestions for a block

Triggered when the user asks "what should I work on?" while in or opening a work block.

```
suggest_tasks(block_ref, now, limit) -> Vec<Suggestion { task_id, reasons: Vec<Reason> }>
```

Candidates are scoped by the block's `task_filter` (§3.6), then ranked by **`urgency`**
(§10.3), with two adjustments that only make sense in the context of a specific block:

- **Fit** — whether `estimate_mins` fits the block's *remaining* time. A task that fits is
  worth more here than the same task in the abstract.
- **Already assigned elsewhere today** — deprioritise, never exclude. Concurrent assignment is
  legitimate (§3.7); recommending it twice in one day is just unhelpful.

**`reasons` is not decoration.** Suggestions the user can't interrogate are suggestions they
won't trust, and the reason string is also the accessible label: "Suggested because overdue
by two days, priority 1, and its 30-minute estimate fits your remaining 35 minutes." A
sighted user might infer ranking from visual position and colour; here the justification has
to be in the text.

### 10.3 Urgency — one ranking function, several consumers

One scoring function, three consumers: block suggestions (§10.2), the default sort of any
task list, and `sort: urgency` in the filter language. Taskwarrior's is the model — a single float per task
from tunable coefficients, computed rather than stored.

```
urgency(task, now) -> f32
```

Additive terms, then one multiplier:

| Term | Contributes |
| --- | --- |
| Due proximity | Rises approaching the due date; keeps rising while overdue, with a ceiling so a task forgotten for a year cannot outrank everything permanently |
| Priority | p1..p4, flat |
| Age | Small, positive — surfaces the quietly-rotting rather than letting it sink forever |
| Partially worked | `accumulated_mins > 0` — finishing beats starting |
| Blocking others | Something other tasks depend on matters more than its own priority says |
| Blocked | Strongly negative. A blocked task is not actionable and should not be suggested |
| Already assigned today | Mildly negative — legitimate, but don't recommend it twice |
| **Project weight** | **Multiplies the sum** (§3.4) |

The multiplier is what answers "how do projects enter the score". Additive project terms let a
weighted project's trivia outrank another project's genuine emergency; multiplying preserves
the ordering *within* each project and scales between them, which is what someone setting
`#thesis` heavy actually means.

**Coefficients are settings, and the defaults are the product.** Ship values that behave
sensibly for someone who never opens the settings, expose them for those who will, and keep
them in synced `Settings` (§3.10) so ranking doesn't differ per device — a suggestion list
that reorders between your laptop and your phone reads as a bug.

**Urgency never auto-schedules anything by itself.** It orders proposals; §10.6's constraint
that the planner proposes and never silently mutates is unaffected.

**Expose the arithmetic.** `lum task show <id> --urgency` should break the score into its terms.
This is the same principle as §10.2's `reasons`: an opaque number that reorders someone's day
earns distrust quickly, and the breakdown is also what makes the coefficients tunable by
anyone other than the author.

*Known failure mode, worth watching:* age terms and overdue terms compound, so a task that is
both old and long overdue can pin itself to the top of every list and become furniture. The
ceiling on due proximity handles the common case. If it still happens, the answer is a
staleness prompt — "this has been at the top for three weeks, is it real?" — not a bigger
formula.

### 10.4 History as a feature

Since history is permanent (§8), expose it rather than apologising for it. Two tiers, and the
cheaper one is the more valuable:

**Domain history — already first-class in the model.** `TaskCompletion` (§3.3) and
`BlockAssignment` (§3.7) are ordinary queryable records. They answer the questions people
actually have:

- *"This task has been rescheduled 7 times."*
- *"Three sittings, 90 minutes logged against a 60-minute estimate."*
- *"Created four months ago, never assigned to a block."*

The first is a genuine signal, not a statistic. A repeatedly-deferred task is usually badly
defined or secretly not important, and surfacing that is exactly the executive-function
support principle 6 points at. Structured discards this data; Todoist's activity log is a
premium feature and is not connected to planning.

**Document history — the Automerge layer.** Every field change with timestamp: title edits,
due-date changes. Available, but needs its own API and is more expensive to query.

Build the domain views first; they answer the interesting questions and need no Automerge
internals. Expose per-task change history later.

### 10.5 Whole-day auto-plan (optional, and a small increment)

A greedy pass over §10.2, not a new mechanism:

```
propose_day(date, now, pinned: Vec<Constraint>) -> Proposal
```

Walk the day's work blocks in time order. For each, run `suggest_tasks` scoped by that
block's `task_filter` (§3.6), and assign greedily until estimated time fills the block.
Return the whole thing as one proposal.

**The scoping mechanism already exists.** `BlockSeries.task_filter` is a filter expression per
block — "this is my `#work` block" — which is what makes per-block candidate selection work
without adding anything to the model.

Optimal bin-packing with soft constraints is the *ambitious* version and is not worth
building. Greedy plus a proposal the user edits is worth far more and costs almost nothing on
top of 8.2.

Same rules as everything else in §10: it proposes, never mutates, and it names what did not
fit rather than silently dropping it.

**Always optional.** Some days you want the machine's suggestion; most days you do not.

### 10.6 Design constraints

- All four are deterministic and headless, so they're property-testable in Rust with no UI.
- Never mutate. The UI applies an accepted proposal through the ordinary mutation API.
- **Scheduling constraints live in the proposal layer, never in the mutation API.** If
  `min_duration_mins` or `anchored` can make a user's own edit fail, the design has gone
  wrong. Good defaults must not become a cage, and the moment a validator starts consulting
  planner fields is the moment they do.
- Proposals must be announceable as text before acceptance, in full — "Deep work moves from
  9:00 to 9:45; Email shortens from 30 to 15 minutes; Review PR does not fit and moves to
  tomorrow."

#### How Structured handles this, and why not to copy it

Worth knowing precisely, since it is half the premise of this app.

- **Dragging on the timeline** is the primary mechanism — you slide a task down and the day
  reshapes. Purely visual and gestural, with no non-visual equivalent. This is the single
  clearest example of the §1 accessibility argument: the *feature* is fine, the only *interface*
  to it is a drag on a proportional canvas.
- **Replan** (paid) reviews *past* unfinished tasks — check off, delete, reschedule, or push to
  inbox by swiping. Note that it is retrospective, run over yesterday's misses, rather than
  live slip absorption. Both are worth having and they are different features; §10.1 is the
  live one, and the carry-forward prompt above is the nearest equivalent to Replan.
- **Structured AI** (4.0, January 2025) does the rescheduling with an LLM — sleep in, and it
  rearranges your day.
- **Free time is highlighted** with suggestions for filling it — independent confirmation that
  surfacing gaps matters, though again delivered as visual emphasis rather than something you
  can navigate to.

The AI approach is the one to deliberately diverge from, and not on ideology. **It cannot
explain itself, it cannot be property-tested, and it requires sending your day to a server** —
which contradicts §7's premise directly. Determinism matters more here than in a sighted app,
too: when you cannot glance at the result and see that it looks right, the description *is*
the verification, and a description you cannot trace back to a rule is not verification at
all. §10.1's rules produce a proposal that can be read out, checked, and reproduced.

Where Structured is straightforwardly ahead: it has a five-year head start on defaults, and
its free-time suggestions are a good idea worth taking.

---

## 11. Reminders and notifications

The data model is §3.8 through §3.10. This is delivery.

### 11.1 Every device notifies

The model and its rationale are §3.8 (delivery defaults, no coordinator) and §3.9 (dismissal
is best-effort). The consequence for *delivery* is what matters here: each device schedules
its own local notifications from synced data, so this section is a set of per-platform
scheduling problems and nothing else. There is no cross-device protocol to design.

**One target breaks that rule**, and it is the only one: the web client cannot schedule
anything locally, so its reminders arrive as a remote wake-up and the service worker decides
what to show on arrival (§16.12). Every other target schedules for itself.

### 11.2 Platform delivery constraints

These are real limits that shape the scheduling code, not incidental details:

- **iOS and watchOS cap pending local notifications at 64.** A daily block with three
  reminders is 21 per week on its own. You cannot pre-schedule everything. Maintain a
  **rolling window** — schedule the next ~48 hours, re-arm on foreground, on background
  refresh, and on sync. Prioritise by fire time when the window overflows.
- **Android** has no hard cap but needs `SCHEDULE_EXACT_ALARM` (Android 13+) for
  time-critical reminders, and Doze will otherwise defer them. Request it, and degrade
  gracefully if refused.
- **Linux** via the desktop notification portal, from the tray-resident app — so reminders
  are only as reliable as the session. Console-only users (speakup, BRLTTY, no X or Wayland)
  have no session and no portal; `lum notify` writes to the tty instead (§8). The daemon
  delivers in neither case.
- **Windows** via toast notifications, which require app identity registration.
- **Wear OS** mirrors phone notifications by default; suppress duplicates deliberately
  rather than by accident.
- **Web** has no local scheduling at all — no working equivalent of a calendar trigger exists,
  so a wake-up must be pushed from outside and the service worker evaluates what is due when it
  arrives. Requires installation as a PWA, and either a decrypting peer or a scheduled
  content-free wake-up from the encrypted store. The one target where reminders are conditional
  (§16.12).

### 11.3 Notification content is the whole message

A sighted user glances at a notification and opens the app for detail. Hearing a truncated
notification is a strictly worse experience — you've been interrupted and still don't know
why.

So notification bodies must be **self-sufficient**: task title, due time, project, and for
blocks the kind and what's assigned. "Deep work starts in 5 minutes — 3 tasks assigned,
first is Review PR" beats "Upcoming block."

---

## 12. Automation and integration

App Intents, App Functions, Shortcuts, PowerShell — these are **the same small set of
operations** exposed through different platform mechanisms. So the core defines one named,
typed **command surface**, and every binding is a thin adapter over it.

This is not new work. The CLI and `lum rpc` already need exactly this surface; automation
names it and reuses it.

**The surface is typed Rust, defined once, in `crates/surface`.** An object holding the open
store, with one method per operation, taking and returning plain records. UniFFI exports it
as-is, so Swift and Kotlin get native classes and structs and the compiler checks both sides
of the boundary; the CLI and the GTK and Win32 apps call it directly. JSON-RPC is an adapter
that serialises the same records for the clients that cannot link Rust (Emacs, BTSpeak),
not the definition. An earlier draft carried JSON-RPC requests over the FFI so that there
would be only one surface; that bought nothing a typed surface does not, at the cost of
encoding on every call and errors that only show up as a failed decode at run time. A web
client would take the same records through wasm with generated TypeScript types.

Every record a method returns carries an `announcement` — the one composed sentence, where
core composed one (§13) — and `notices`, things worth saying that are not the answer. Rows
stay components.

```
add_task(text) -> Task            // quick-add grammar, §6.1
complete_task(id)
query_tasks(filter) -> Vec<Task>  // filter language, §6.2
todays_plan() -> Vec<Block>
assign_task(task, block)
start_timer(assignment) / stop_timer(assignment)
```

**Retrieval is first-class, not an afterthought.** Most integrations only let an assistant
*create* things; being able to ask what is due is half the value.

**Voice invocation is a genuine accessibility feature here, not a gimmick.** Hands-free,
eyes-free capture is the one case where "Hey Siri, add task" beats every UI in this document.

### Apple — App Intents (iOS, macOS, watchOS)

One implementation serves Siri, the Shortcuts app, Spotlight, widgets, Control Center, and
the Action Button, across iOS **and macOS**. This is the highest-leverage integration in the
plan.

- **`AppEntity`** with an `EntityQuery` for Task, Project, and Block — this is what makes
  retrieval work, so Shortcuts can *find* things rather than only create them.
  `EntityStringQuery` for search.
- **`AppShortcutsProvider`** for Siri phrases that work with no user setup.

**Apple Intelligence:** App Intents *are* the integration surface; there is no separate API.
The one extra thing to check is **assistant schemas** (`@AssistantIntent`, `@AssistantEntity`)
— Apple-defined typed schemas per domain that let the system reason about app actions in a
standardized way. *Whether a task-management schema exists publicly needs verifying at
implementation time.* If it does, conform to it; if not, plain App Intents still deliver Siri
and Shortcuts. Given how much the Apple Intelligence and Siri timeline has shifted, treat any
specifics here as needing re-checking rather than settled.

### Android — App Functions

`androidx.appfunctions` is the current framework and the right target for Gemini integration,
with **AppSearch** as the retrieval companion. The older App Actions path (`shortcuts.xml`
plus built-in intents) is the legacy route.

### App icon shortcuts

Cheap and high-value on both mobile platforms — "Add task" and "Today" belong here.

- **iOS** — `UIApplicationShortcutItem`. Reached under VoiceOver by double-tap-and-hold.
- **Android** — `ShortcutManager`, static or dynamic. Long-press on the launcher icon.

### Widgets

Quick overview without launching, on the same footing as the watch complication:
**WidgetKit** on iOS and macOS, **Glance** on Android. Today's blocks and what is due.

### Windows — a PowerShell module

There is no third-party equivalent to Shortcuts or App Intents worth targeting on Windows.
WSH and COM automation would mean implementing a COM interface for an audience that has moved
to PowerShell. A thin module wrapping `lum --json` is idiomatic and nearly free.

### MCP server — `lum mcp`

An **MCP server** exposing the command surface to any MCP-capable client: Home Assistant's
Assist, Claude Desktop, local LLM front-ends, coding assistants.

Cheap by construction. MCP speaks JSON-RPC over stdio, which `lum rpc` already does, and MCP
tools map onto the command surface above almost one-to-one — so this is another subcommand of
the same binary, not a new subsystem. Tasks and the day plan fit MCP *resources*; mutations
fit *tools*.

**Both transports.** *stdio* for clients that spawn the server themselves (Claude Desktop,
most local clients), and **Streamable HTTP** — the current spec's remote transport, which
replaced HTTP+SSE — for clients on another machine. The latter matters concretely: Home
Assistant usually runs on its own box, so stdio alone would force HA and the daemon onto the
same host. Architecturally it is just another listener on the daemon.

HTTP rules: **bind to loopback by default**, require a bearer token for any non-loopback bind,
and do not implement TLS termination — a reverse proxy or Tailscale is what people will use
anyway.

*Distinction worth keeping:* HTTP transport to **your own** daemon is fine, because plaintext
never leaves your machine. Offering a **hosted** MCP endpoint as a service would be the Alexa
problem again — the server would need the key.

**It runs locally, where the key already is.** That is the property a cloud assistant cannot
have (see below), and it makes the fully-local voice loop possible: wake word → local speech
recognition → local model → MCP → daemon, with nothing leaving the house. Home Assistant
supplies every piece except the last.

Two design requirements:

- **Read-only by default, writes opt-in.** An assistant that can complete tasks can complete
  the wrong ones, and MCP has no notion of undo. Expose **"undo last action"** as a tool —
  nearly free, since §9 provides inverse operations regardless.
- **A cloud-hosted model sees whatever it reads.** Connecting Claude or ChatGPT rather than a
  local model means task data leaves the device. That is the user's explicit choice, not an
  architectural violation, but it must be stated rather than implied given what the rest of
  the app promises.

### Hooks — reactive, never interceptive

Taskwarrior fires `on-add` / `on-modify` scripts **synchronously, inside the mutating
process**, and lets them rewrite or veto the change before it commits. That model cannot be
ported here, and the reason is worth stating plainly because someone will try.

**A change can arrive already committed, from a device that is no longer reachable.** A task
added on a phone in a tunnel merges three hours later; there is nothing to veto and nowhere to
send a rejection. CRDT convergence is not negotiable after the fact — that is the property the
whole sync design is built on.

So hooks here observe rather than gate:

- They fire from the **change feed** (§8's WAL watch), the same stream that drives UI
  refresh, after the change is durable.
- They see before-and-after, and may make further changes as a consequence. A hook cannot stop
  a task from being created; it can add a label to it a moment later.
- They cannot report failure to the user who made the change. Log it, surface it in
  `lum sync status`, and accept that this is a batch-processing surface.

#### What fires them

Two classes, and conflating them is a mistake — **half of these are not changes at all**.

**Change events** come from the change feed, and are free:

```
task.created / task.completed / task.uncompleted / task.updated / task.deleted
assignment.created / assignment.removed
timer.started / timer.stopped
block.edited            # the series or an exception changed
```

**Time events** come from the clock, and need the scheduler:

```
block.started / block.ended / block.upcoming(offset)
day.start / day.end
task.due
```

A block starting at 09:00 mutates nothing. No document changed; time merely passed. So these
fire from the **same scheduler that drives reminders** (§11) — `reminders_due` and
`blocks_due` are the same shape of query — and they inherit its constraint: they only fire
where something is resident, and if nothing was, they fire late.

**Late is worse for hooks than for reminders.** A reminder announced ten minutes late is still
useful. "Turn on do-not-disturb when my focus block starts," fired forty minutes into the
block, is actively wrong. So **time events carry a staleness window** — a per-hook maximum
lateness, defaulting to a few minutes, past which the event is dropped and logged rather than
run. Change events have no such window; a task completed yesterday is still completed.

#### Every hook is filtered

```
Hook { event, filter: Option<String>, action, max_lateness_secs }
```

`filter` is a §6.2 expression, evaluated against the subject of the event. This is most of
what makes the feature usable — nobody wants a script on *every* completion, they want one on
completions in `#server` — and it costs nothing, because the filter language, its parser, and
its completion UI all already exist. It also means hook configuration gets the same `@`/`#`
completion as everywhere else, which matters on the platform where you'll actually write them.

**Which device runs them?** This is the question the design has to answer, and the answer is
that it never needs an election: **hooks are local-only configuration** (§3.12) and do not
sync. A hook that runs `ssh` or touches a local file belongs to one specific machine, which is
also how anyone would think about it — the alternative, synced hooks, means every device tries
to run a script that exists on one of them.

There is a second reason, and it is the stronger one. **Synced hooks would be remote code
execution across every device you own.** Anything that could write to the document — a
compromised peer, a tampering store operator if the encryption ever failed — would get a shell
everywhere. Local-only configuration removes that entire class of attack rather than
mitigating it, and it is worth accepting the mild inconvenience of configuring hooks per
machine to keep it.

**The device that runs a hook need not be the device where the thing happened.** This is what
rescues the feature on mobile: the change feed reaches every device, so completing a task on
your iPhone fires the hook on your Pi. Time events work the same way — the schedule is synced,
so the Pi knows your 09:00 block starts at 09:00 regardless of which device created it.

Within a single machine two resident processes may both see the change (a tray app and a
daemon). **Only the sync-lock holder runs hooks** (§8), which reuses machinery that already
exists rather than inventing a second election.

#### Actions, per platform

The action is a command line plus an environment. Event data arrives as **JSON on stdin** —
structured, no shell-escaping hazards, and the same versioned shape as `lum --json` (§15) — with
a few common fields also in the environment for one-liners.

- **Linux, macOS, BTSpeak** — argv, executed directly. No shell unless the user asks for one.
- **Windows** — `pwsh -NoProfile -ExecutionPolicy Bypass -File`. PowerShell is the right target
  and there is no close second: Power Automate Desktop is heavy, cloud-tied, and poorly
  exposed to screen readers. It also gives the automation story symmetry — §12 already
  proposes a PowerShell module wrapping `lum --json` for calls *inbound*, so the same language
  serves both directions. `-ExecutionPolicy Bypass` is not optional; the default policy blocks
  unsigned local scripts and would otherwise make every hook fail silently on a fresh machine.
- **macOS Shortcuts** — needs **no special support**. `shortcuts run "Focus On"` is a CLI, so
  the generic action already covers it. Note that this is a different judgement from §12's
  rejection of AppleScript, and not a contradiction: that was about exposing an inbound
  scripting API, this is about spawning a process.
- **Android — broadcast an intent, don't integrate with Tasker.** Send
  `com.lumenna.EVENT` with the event JSON as extras. Tasker catches it with an *Intent
  Received* profile, and so do Automate, MacroDroid, and Easer. Targeting Tasker's own
  `ACTION_TASK` API instead would mean a permission, a dependency on one paid app, and
  nothing for users of the others. A generic broadcast is roughly twenty lines and serves
  everyone. This is the one platform where the action is not a subprocess, because Android
  does not let an app exec one.
- **iOS — not possible, and worth being direct about it.** Shortcuts is one-way: it calls
  *into* apps through App Intents, and no third-party app can trigger a Shortcut unattended.
  `shortcuts://run-shortcut` requires foreground and visibly switches apps, which is not
  automation. The compensation is real, though: App Intents (§12) already make Lumenna
  queryable, so a user's own time-triggered Shortcut can *pull* — and because hooks run
  wherever something is resident, anything they do on their phone still fires hooks on their
  desktop or Pi.
- **watchOS, Wear OS** — nothing. Neither is ever the resident device.

#### Consequences, stated so they don't surprise anyone

- If nothing is resident, hooks run late — on next launch, over the backlog since last seen.
  Correct behaviour for change events; time events past their staleness window are dropped.
- Ordering across devices is not guaranteed. Two resident machines see merged changes in their
  own order, so a hook must not assume it observes a global sequence.
- **The always-on peer is the best place for most of them**, and this is where the feature
  earns its keep: a Pi that is always up is exactly the right host for "when a task in
  `#server` completes, run the deploy script" — and, given the mobile limitations above, it is
  the only host that catches everything.

Cheap to build — the change feed, the reminder scheduler, the filter evaluator, and subprocess
spawning all already exist — and it is the automation surface MCP is bad at. MCP serves an
assistant asking questions; hooks serve a script reacting to events. Different jobs, both
worth having.

### Alexa — deliberately skipped

Not because of the Alexa+ transition, though that is real and independently sufficient. The
durable reason is architectural: **a cloud assistant cannot read data the cloud cannot
decrypt.** Siri and Gemini integrations work here because App Intents and App Functions run
*on the device*, alongside the key. An Alexa skill runs in Amazon's cloud and would need
either the decryption key handed to a hosted service — defeating the encrypted store entirely —
or a tunnel from Amazon to the user's own peer, which is a great deal of setup for a small
feature.

Alexa Lists is the lighter alternative some apps use, and has the same plaintext problem plus
a flat string model that would discard projects, priorities, and blocks.

The voice case that matters is already covered on-device by App Intents, and the home case is
covered locally by MCP (above).

### AppleScript — deliberately skipped

An `.sdef` plus a scriptable AppKit object model is real work for a shrinking audience.
Shortcuts covers GUI automation and `lum --json` covers scripting, so nothing is lost that
the CLI does not already provide.

---

## 13. Accessibility as an architectural concern

The core emits a **row projection** for every list-shaped view, computed once:

```
Row {
  id, kind,
  depth:     u32,
  index:     u32,             // position within its sibling set
  count:     u32,             // size of that set
  expanded:  Option<bool>,
  checked:   Option<bool>,
  title:     String,          // verbatim content: task title, block name
  role:      Role,            // structured, not a string
  state:     Vec<State>,      // overdue, completed, recurring, running...
  value:     Option<String>,
  hint:      Option<String>,
}
```

**Emit components, not a composed sentence.** Speech and braille compose them differently,
and pre-flattening into one `label` string forces one channel to accept the other's
conventions.

- **Speech** spells roles and states out: *"2:00 PM, one hour, Deep work, work block, three
  tasks assigned."*
- **Braille** uses conventional short forms for roles and states — the same convention the
  BTBraille tree view uses — but renders `title` **verbatim**.

**Titles and names are never abbreviated for braille.** Braille users get everything speech
users get; abbreviating content is information loss. Long labels also cost nothing when
scanning, because panning to the next braille *window* and moving to the next *line* are
separate operations — you move between items without reading through the current one. This
holds across BRLTTY, NVDA, JAWS, VoiceOver, and TalkBack.

Only roles and states are abbreviated, and only because those abbreviations are conventions
the reader already knows.

These fields map one-to-one onto every platform's accessibility API:

- GTK4 — `posinset`, `setsize`, `level` on `AccessibleRole::ListItem` / `TreeItem`
- Win32 — `SysTreeView32` reports this natively from the control's own structure
- AppKit — `NSAccessibilityOutline` / `Row` / `Cell` roles
- UIKit — `accessibilityContainerType`, element ordering, custom actions
- Compose — `collectionInfo`, `collectionItemInfo`, `stateDescription`
- Web (§16.12) — `aria-level`, `aria-posinset`, `aria-setsize`, `aria-expanded` (literally
  the same names)

Computing this once prevents eleven subtly different announcements for the same list, and
makes the semantics snapshot-testable in Rust rather than only observable through a screen
reader on each platform.

### The day view, as the worked example

The planner is where a list-shaped UI sits furthest from the visual metaphor, so its shape is
specified once here rather than eight times in §16.

**A two-level list.** Blocks in time order, assignments nested beneath each. `depth`, `index`,
and `count` carry the structure; nothing is conveyed by position on screen.

**Gaps are rows**, and this is the easiest thing in the whole design to leave out. A visual
timeline renders free time as empty space, and a sighted user reads "I have forty-five minutes
at 10:15" without anyone choosing to tell them. A list of blocks does not contain that fact at
all. So free time is a **first-class row** — *"Free, forty-five minutes, 10:15 to 11:00"* —
navigable, and the natural place to hang "schedule something here". Emit it above a threshold
of a few minutes, or a two-minute seam between meetings becomes noise.

That generalises into the rule this section exists to state: **anything a sighted user reads
from empty space or spatial proportion must become an explicit row or an announced value.**
Duration is the other instance — a timeline draws a two-hour block twice as tall as a one-hour
block, so duration belongs in the label, which the cross-cutting rules below already require.

**"Now" is a position, not a highlight.** Colour-shifting the current block is invisible here.
Emit `now` as its own row between the blocks it falls between, announce current/past/upcoming
in each block's `state`, and provide a **"go to now"** command bound consistently everywhere.
Opening the day view should land focus there by default, not at midnight.

**Open with a summary row.** *"Thursday 4 September. Six blocks, four hours of work capacity,
three tasks assigned, one overdue."* A sighted user gets that gestalt from a glance at the
shape of the day; without it, building the same picture costs a full traversal.

#### Creating and filling blocks

- **Creating** is a form: title, start time, duration, kind, recurrence, and optionally
  `task_filter` — which gets the same `#`/`@` completion as everywhere else (§6.4). Extend the
  quick-add grammar to blocks rather than inventing a second one: `block "Deep work" 9-11am
  weekdays` parses with the §6.1 date grammar plus a time range.
- **Filling works from both directions**, and both are needed. From the task: "assign to a
  block" opens a picker of upcoming blocks whose `task_filter` admits it. From the block: "add
  tasks" opens §10.2's suggestions, already ranked and already scoped by that filter. The first
  is what you reach for with a task in hand; the second is what you reach for when sitting
  down to work.
- **Recurrence edits ask the standard question** — this occurrence, or the series? — and the
  answer maps onto `BlockException` versus `BlockSeries` (§3.6). Never guess.

### Verbosity

Because the core generates announcement text, terseness is a **core setting**, not a
per-UI one (§3.10). Some users want *"Review PR, overdue"*; others want the full sentence
every time. One setting, honoured identically on all eleven targets.

### Cross-cutting UI rules

- Never encode meaning in colour alone — priority, overdue state, block kind, completion.
  Always carry a text equivalent.
- Kind and state belong in the announced label: *"2:00 PM, one hour, Deep work, work block,
  three tasks assigned."* The words "work block" tell you which actions exist before you
  open the menu.
- Every gesture-only or pointer-only affordance needs a keyboard and screen-reader
  equivalent. Swipe actions become custom accessibility actions.
- Announce state changes explicitly (task completed, task moved to block, day re-planned).
- Manage focus deliberately after mutations. When completing an item causes a list to
  re-sort, focus must land somewhere predictable rather than wherever the framework decides.

---

## 14. Testing

- **Core is headless and fully testable** — the reason for the fat-core design pays off here.
- **Export/import round-trip property tests** (§9) — generate a random store, back it up,
  restore into an empty one, assert equivalence; and separately assert that a *current-state*
  export contains no deleted task, which is a privacy guarantee rather than a formatting
  detail. This proves the disaster-recovery path works and, incidentally, that the model has
  no unreachable state.
- **Property-based convergence tests** (`proptest`): apply random operation sequences to N
  simulated replicas in different orders, assert identical materialized state. This is the
  only practical way to gain confidence in the tree-cycle and ordering logic.
- **Golden tests for the quick-add parser** — a corpus of input strings and expected parses,
  including deliberately ambiguous ones.
- **Snapshot tests for the accessibility row projection.** Catches most a11y regressions
  once, in Rust, rather than eight times on the eight targets with an accessibility API.
- **Manual screen reader testing per platform is unavoidable**: Orca, NVDA, VoiceOver on
  macOS / iOS / watchOS, TalkBack on Android and Wear OS. Test continuously, not at the end.

---

## 15. The CLI (first target)

Built first, and kept permanently — it's a real platform, not scaffolding. It's usable
within weeks rather than months, it's fully accessible by construction, and it forces the
core API to be complete before any GUI can paper over gaps in it.

**Libraries:** `clap` v4 (derive) for commands, `clap_complete` including dynamic
completions so project and label names complete for real, `rustyline` for an optional REPL
mode, `anstyle`/`anstream` for styling.

**Deliberately no TUI.** `ratatui` and friends repaint regions and reposition the cursor;
terminal screen readers track the cursor and read what changes, which makes full-screen
TUIs genuinely hostile. Linear, append-only, one-item-per-line output is what works — a
large part of why `todo.txt` and Taskwarrior have the following they do.

Consequences:

- No box-drawing tables. Unicode borders are noise read aloud. Plain aligned columns for
  lists, `key: value` lines for detail views.
- `--json` from day one: scriptable, and a debugging window into the core.
- Numbered choices and typed input over arrow-key selection menus.

### Completeness, for two reasons

**Anything a GUI can do, the CLI can do.**

**It is a first-class interface.** Some people simply prefer text — `edbrowse` exists because
that preference is real and long-standing, not because those users' GUIs are broken. Add
scripting, and the CLI is a product in its own right.

**And it is a test.** If something is only possible in a GUI, that is business logic that
leaked out of the core, violating principle 2. CLI coverage is the cheapest checkable proxy
for core coverage there is — far easier to audit than reading eleven UI implementations looking
for logic that should not be there.

Treating it only as a test would harm it: a test optimises for coverage, a product optimises
for use. Concretely:

- **Short IDs.** UUIDv7 is 36 characters — miserable to type and worse to dictate. Assign small
  integers to the rows of the last listing, as Taskwarrior does, so `lum task done 3` works. Accept
  UUID prefixes too, git-style.
- **`--json` output is a compatibility contract**, not a debugging convenience. People will
  script against it; give it a version and do not reshape it casually.
- **Aliases** for frequent operations, and output density worth actually tuning.
- **`--help` is an accessibility surface**, not generated boilerplate. It is the primary
  discovery mechanism for anyone who cannot skim a GUI.

Command surface:

```
# tasks — every noun is a subcommand group, so the shape is `lum <noun> <verb>`
lum task add "review PR tomorrow 3pm p1 #work @laptop"
lum task list [<expr>] [--json]          # full filter language, §6.2
lum task show <id>
lum task edit <id> [--due ...] [--priority ...] [--estimate 45m] [--notes -]
lum task done <id> / lum task undone <id>
lum task rm <id> / lum task restore <id> # trash, not erasure
lum task move <id> --parent <id> | --project <id>
lum task search <text>

lum task depend add <id> --on <id> / rm  # §3.2; `lum task list blocked`

# organisation
lum project add|list|rename|archive|rm
lum project weight <id> <n>              # §3.4 urgency multiplier
lum label   add|list|rename|merge|rm    # merge repairs typo duplicates, §3.4
lum filter  add|list|rm                  # saved filters

# planning
lum plan [date]                          # blocks and assignments
lum plan reflow                          # §10.1 proposal
lum plan suggest --block <ref>           # §10.2
lum plan auto [date]                     # §10.5
lum day end|clear|replan                 # day-level commands, §10.1
lum block  add|list|edit|rm
lum block  skip <ref>                    # this occurrence only
lum assign <task> --block <ref> [--minutes 45]
lum unassign <assignment>
lum start|pause|stop <assignment>        # timers, §3.7

# reminders, history
lum remind add|list|rm
lum history <id>                         # §10.4

# data and sync
lum export [--format json|md|ics]        # current state, no history
lum backup / lum restore <file>          # full fidelity
lum import <file>
lum calendar add|list|rm <url>           # ICS/CalDAV subscription, §3.6
lum task erase <id>                      # permanent, rebuilds the doc — §9
lum pair [<node-id> | --id]              # §7; words are compared either way
lum store add|list|rm <url>              # encrypted store, §8
lum device list|rename|unpair            # §3.11
lum sync / lum sync status
lum daemon install|uninstall|start|stop
lum notify                               # foreground: speak reminders on a console system
lum hook add|list|test|rm                # local-only automation, §12
lum rpc / lum mcp                        # servers, §8 and §12

# meta
lum undo / lum redo
lum config get|set
```

`--json` on every read command. An optional `rustyline` REPL for interactive use.

**Audited against the model, not written from memory.** Five capabilities existed in §3 with
no way to reach them from a terminal, which is exactly the failure the completeness rule is
supposed to catch: **dependencies** (`depends` was filterable but not settable), **project
weight**, **device management** — you must be able to unpair a lost device without a GUI —
**permanent erasure** (§9), and **calendar subscription**, which is not the same operation as
importing a file. Re-run this audit whenever §3 gains a field.

---

## 16. Platform UIs

Every UI is a thin mapping over core view models. None contains business logic, date
arithmetic, or recurrence handling.

### 16.1 Required views

§15's completeness rule for the CLI has a counterpart here. The per-platform sections below
cover *how* each toolkit is used; this one fixes *what* must be built, because a target that
ships with no label list is not otherwise something anyone notices for a month.

Every full GUI target implements all of these. Each maps to model sections that already exist,
and none needs core work beyond what §3–§13 specify.

| View | Covers | Notes |
| --- | --- | --- |
| **Day** | §13, §3.6, §3.7 | The planner. Gaps and "now" as rows. |
| **Task list + filter entry** | §6.2, §6.4 | The filter field is a combobox with completion, not a search box. |
| **Task detail / edit** | §3.2 | Including notes, estimate, labels, dependencies. |
| **Project tree** | §3.4 | Navigable hierarchy, plus `weight`. |
| **Label list** | §3.4 | Labels are a first-class axis; they need their own list, not just a picker inside task edit. Rename, recolour, reorder, **merge**, delete. |
| **Saved filters** | §3.4 | Create, rename, reorder, delete — not only run. |
| **Block editor** | §3.6 | Kind, flags, recurrence, `task_filter`, `min_duration_mins`. |
| **Assignments + timers** | §3.7 | Start, pause, stop, manual entry; elapsed polled on demand, never announced live. |
| **Reminders** | §3.8 | Per-task and per-block, with the per-reminder device override. |
| **Settings** | §3.10 | Including verbosity and urgency coefficients. |
| **Devices + sync status** | §3.11, §9 | Pair, unpair, rename, and the textual sync state. |
| **Trash + undo** | §3.2, §9 | Restore, and permanent erase behind a warning. |
| **History** | §10.4 | Per-task: sittings, reschedules, logged time. |
| **Backup / export / import** | §9 | In every client, not only the daemon. |

**No capability exemptions, and no partial targets.** Two targets have notes below, but every
exclusion in them is a platform fact — somewhere a file cannot go, a process that cannot run,
a secret that need not be introduced. None is a scope decision.

**Effort determines order, not scope.** Platforms ship independently as each is ready (§18),
so there is no shared "v1" whose budget they compete for and no moment when everyone runs the
same build (§9). Building one target fully does not make another smaller; it makes it later.
So the honest response to "this target is a lot of work" is to sequence it accordingly, never
to ship a diminished version of it.

#### Watches implement everything

**Wear OS and watchOS carry the full view list**, including settings, project and label
management, block authoring, device management, backup, and permanent erase. Exempting the
authoring views because text entry is hard on a watch would be wrong twice over.

- **Most of it is not typing.** Reordering, deleting, recolouring, merging, toggling settings,
  unpairing a device, confirming an erase — none of these involve a keyboard. Excluding them
  because they sat in the same *view* as something that does is a category error.
- **The rest is possible anyway.** Dictation, Scribble, the on-screen keyboard on newer Apple
  Watch hardware, and the completion API (§6.4) all work. A filter expression is unpleasant to
  compose on a wrist; unpleasant is a reason to do it elsewhere by choice, not a reason for the
  capability to be absent.

The principle is the same one as §10.6's: **constraints belong to what the app proposes, not to
what the user may do.** Nobody has to author a saved filter on a watch. Someone occasionally
will, usually because it is the only device to hand, and that is exactly the moment a
capability exemption would fail them.

This is consistent with the rest of the design rather than an exception to it. §16.9's *full
peer, not a companion* is the same claim; the small screen is not a constraint for a VoiceOver
user, and a device that runs the whole core has no technical reason to hide half of it.

**Treat "let the paired phone do it" as a red flag** wherever it appears. A watch goes on walks
and hikes without a phone, and those are the moments when the wearer most needs the thing to
work. The phrase is only ever correct when the platform genuinely removed the capability —
watchOS region monitoring (§21 q2) is the one instance in this plan — and then it belongs in
the text as a platform gap, named as such, not as a decision.

*Honest cost:* this is real work — fourteen views on two more platforms — and it is the
largest single addition breadth has cost so far. Some views will be plainer than their desktop
equivalents. Plainer is fine; absent is not.

What remains a genuine platform limit, and only one: **export produces a file, and watchOS
gives it nowhere useful to go.** Automatic backups (§9) still run on the watch like every
other client. Manual export is available but will usually be done elsewhere, because the
platform affords no good way to retrieve the result — a missing destination, not a missing
feature.

#### The web client — a full target, warned rather than restricted

The obvious move is to withhold device management and permanent erase here, because the server
ships the JavaScript that decrypts and a hostile operator could serve code that steals the
password. The premise is true. **The conclusion is security theatre**, for a reason worth
recording so it is not re-derived later:

**A malicious bundle already has the password**, because the user typed it in to log in. From
the password the KDF yields the encryption key (§8), so that bundle can read everything and
issue any call the API accepts — including unpairing devices — regardless of which buttons the
honest UI chooses to render. Hiding a view restricts the legitimate user and inconveniences
the attacker not at all. Any mitigation that only removes UI is not a mitigation.

**And the difference from native is degree, not kind.** A compromised release pipeline can ship
a backdoored desktop binary; a compromised source repository can tag a malicious release. Both
end with the user running code the author did not write. Bitwarden — the closest comparable
threat model — does not restrict its web vault, and in fact puts *more* there than in its
native clients.

So the trust argument justifies no restriction at all. Neither does effort: **the web client is
a full target**, carrying the same view list as every other. It is genuinely expensive — an
eleventh target in a sixth language, and the only one that cannot exist until the encrypted store
does (§8) — but per the rule above, expense decides when it is built, not how much of it is.
Someone using a locked-down work machine is not a lesser user, and a browser is the one place
where "install our app instead" is not available as an answer.

**One exclusion survives, and it is not about hiding a button.** Never display or accept a
**recovery phrase** in the browser. The distinction is real: the password is necessarily
present in that environment, so excluding UI cannot protect it — but the recovery phrase need
never enter the browser at all, and a bundle cannot capture a secret that was never typed into
it. That is a reduction in what a compromise yields, not a reduction in what the UI offers.

**What the warning should actually say**, since disclosure is doing the work the restrictions
were pretending to do:

> Web code is re-delivered on every load, so it can be changed without you noticing, and can be
> targeted at one account for one session leaving nothing behind. A native release is a durable
> artifact that can be downloaded, compared against the published source, and checked by
> anyone. That is the difference, and it is real.

Say it once, plainly, where someone decides whether to sign in. Then let them decide.

*Constructive follow-through:* the answer to both halves is verifiability rather than
restriction — **reproducible builds** for native releases, and subresource integrity plus a
published bundle hash for web. Neither closes the hole; both make tampering detectable, which
is the most any of this can honestly claim.

Still limited on **capability** rather than trust, because a browser tab is not resident:

- **Reminders are conditional**, not excluded. Installed as a PWA the client gets Web Push.
  A decrypting peer — a Pi, a running desktop — can send them; the encrypted store cannot, since
  it cannot know a reminder exists. But the store *can* emit a **content-free scheduled
  wake-up**, letting the service worker decrypt locally and decide what to show, which needs
  no peer and leaks only timing. Off by default. See §16.12.
- **Hooks** stay out. A service worker cannot execute a program.

**Timers do work**, and it is worth noting why: §3.7 stores `running_since` as a fact rather
than ticking a counter, so a timer started in a browser keeps accumulating after the tab is
closed and stops correctly from a phone. That falls out of the data model rather than needing
anything web-specific.

Emacs and BTSpeak are *not* exempt from anything. Both are cheap precisely because they are
lists and forms over the same RPC surface, and both are someone's primary device.

### 16.2 Tray residency (all three desktop platforms)

Desktop apps minimize to the tray or menu bar rather than exiting. This is what makes
reminders work at all — a reminder for a task added on your phone cannot fire on the desktop
if no desktop process exists. Sync-on-launch covers everything else.

A resident app also normally holds the sync lock (§8), so it is the de facto sync process
without being a service. **No systemd unit, no launchd plist, no Windows service, no user
setup.**

- **Linux** — GTK4 dropped `GtkStatusIcon`, so this is StatusNotifierItem over D-Bus via
  `ksni`. Note that GNOME supports SNI only through an extension; degrade gracefully.
- **Windows** — `Shell_NotifyIcon`, straightforward in `windows-rs`.
- **macOS** — `NSStatusItem`, plus `LSUIElement` if it should run without a Dock icon.

**Accessibility caveat, and it is a real one:** tray and menu-bar items are poorly exposed to
screen readers — Orca and StatusNotifierItem especially. The tray must never be the only way
back to the window. Provide a global hotkey to summon it, and make relaunching the app focus
the existing instance rather than starting a second one.

**Global quick capture.** A second hotkey opens a bare quick-add field from anywhere, accepts
one line of the §6.1 grammar, and dismisses. The app is already resident so this is nearly
free, and "capture before you lose it" is the workflow a task manager lives or dies on. It is
also the desktop counterpart to app-icon shortcuts and Siri capture (§12).

Desktop widgets — WidgetKit on macOS, and whatever the tray affords elsewhere — give the same
quick overview as the watch complication.

### 16.3 Linux — GTK4 (`gtk4-rs`)

Pure Rust, links the core directly.

**Verify before building anything:** `GtkListView` and `GtkColumnView` replaced the
deprecated `GtkTreeView`, and Orca support for them has historically lagged. Build a
throwaway with fifty nested rows and confirm Orca reports level, position, and expansion
state correctly. If it doesn't, that reshapes this target — possibly toward the deprecated
`GtkTreeView`, possibly toward custom `AccessibleRole` work.

Menu bar via `GMenu`, keyboard shortcuts via `GtkShortcutController`, F6 pane traversal
implemented explicitly.

### 16.4 Windows — Win32 (`windows-rs`)

Pure Rust. **Standard common controls only.** `SysTreeView32` is the gold standard for
screen reader behaviour on Windows: mature MSAA/UIA reporting, decades of NVDA and JAWS
special-casing.

The moment anything is custom-drawn, you own its UI Automation provider implementation.
Avoid it.

Native `HMENU` menu bar, standard accelerator table, F6 pane traversal implemented
explicitly (neither Win32 nor any other framework provides it free — the dialog manager
handles Tab, arrows, Esc, and mnemonics only).

*Considered and rejected:* WPF renders everything itself and exposes UIA peers. It's
excellent for non-native, and heavily tested because Visual Studio uses it, but virtualized
WPF `TreeView` has a history of UIA problems at scale — broken item enumeration, wrong
`posinset`/`setsize`. That's precisely the case a deep subtask tree exercises. WinForms
(genuinely native controls, C# ergonomics) is the fallback if raw Win32 proves too costly.

### 16.5 macOS — AppKit (+ SwiftUI leaves)

`NSOutlineView` for the task tree. It's what Finder and Mail use, with correspondingly
mature VoiceOver behaviour, and it's the whole reason for choosing AppKit here.

`NSMenu` for the menu bar with `validateMenuItem:`, `keyViewLoop` for explicit focus order
and F6 traversal, `NSSplitViewController` for panes.

SwiftUI via `NSHostingView` for leaf content only — settings, task detail, quick-add sheet.

*Why not SwiftUI throughout:* macOS SwiftUI has incomplete `Table` VoiceOver support,
inconsistent `OutlineGroup` / `DisclosureGroup` expansion announcements, and noisy nested
group announcements from deep view hierarchies.

### 16.6 iOS — UIKit shell + SwiftUI content

UIKit owns structure: navigation, the collection view, focus, and keyboard handling.

The decisive factor is **deterministic VoiceOver focus**. `UIAccessibility.post(notification:)`
puts focus exactly where you specify. SwiftUI re-renders the view tree on state change and
focus can silently jump — which is exactly what happens when completing a task causes the
list to re-sort. That's the single most common SwiftUI accessibility complaint and it lands
squarely on this app's core interaction.

Also from UIKit: `UIAccessibilityCustomRotor` (substantially more capable than SwiftUI's
`accessibilityRotor`), `UIAccessibilityCustomAction` for swipe equivalents,
`accessibilityContainerType`, and real `UIKeyCommand` support — Bluetooth keyboard use is
common among blind iOS users and SwiftUI's `.keyboardShortcut` is thin on iOS.

**`UIHostingConfiguration`** (iOS 16+) renders SwiftUI content inside
`UICollectionViewCell`s: UIKit structure and focus behaviour, declarative cell content.
This is the intended blend.

*In practice:* the accessibility audit (`performAccessibilityAudit`) flagged every `Text`
hosted this way as not supporting Dynamic Type, and as clipped when the size changed at run
time. Rows that are text, a symbol and a chevron are exactly what `UIListContentConfiguration`
draws, and it passes, so the task list uses that. `UIHostingConfiguration` remains the
option for a row UIKit's configurations cannot express — and such a row should be checked
against the audit before it ships.

*And in forms:* SwiftUI's form parts needed help to pass the same audit — a `LabeledContent`
control is not named by its visible label, the inline picker's checkmark and the placeholder
grey fail contrast, and a section footer stops growing with Dynamic Type. Each has a small
replacement in the iOS app. Bottom toolbars do not survive a tab bar at all: the floating bar
covers them, and VoiceOver's activation lands on the tab beneath.

SwiftUI for detail forms, settings, and sheets — where its state binding eliminates
"UI out of sync with model" bugs, a class of bug that's *worse* for screen reader users
because a stale label is announced confidently and looks like truth.

*Mixing caveat:* accessibility modifiers applied *to* a `UIViewRepresentable` are
unreliable. Set properties on the underlying view inside `makeUIView` instead.

### 16.7 Android — Jetpack Compose

Compose over the View system, despite the View system's slightly greater accessibility
maturity, for one reason specific to this app: **Android has no native tree control.** There
is no `NSOutlineView` or `SysTreeView32` equivalent. The subtask hierarchy becomes a flat
list with indentation and manually-supplied level metadata in *either* framework, which
removes Views' main advantage. Compose's semantics API is more expressive, and it's where
Google is investing.

Budget explicit semantics work — Compose defaults are frequently wrong where RecyclerView's
were right:

- `collectionInfo` / `collectionItemInfo` on `LazyColumn` items, or TalkBack won't announce
  "item 3 of 40"
- `traversalIndex` and `isTraversalGroup` for reading order — the existence of these APIs
  tells you the default order isn't reliable
- `stateDescription` for completion and block status
- custom accessibility actions for every swipe affordance
- explicit accessibility focus management around dialogs and bottom sheets, historically buggy

Test with TalkBack continuously. Compose punishes deferring this more than any other target.

### 16.8 Wear OS — Compose for Wear OS

**Ordinary target with one prerequisite: acquiring the hardware.** Building a screen reader
interface against a device you cannot test on is how something subtly broken ships — but that
is a build-order constraint, not a reason to rank this below the others. Compose for Wear
shares most of the Android work, and on this timeline the device is likely to arrive well
before its turn.

`androidx.wear.compose` is the official toolkit; the XML and `WearableRecyclerView` path is
legacy. Compose is *less* contested here than on phones.

**Full capability, same as watchOS** (§16.1) — the whole view list, not a companion subset.
Wear has the easier time of it: an ordinary Android build via `cargo-ndk`, no ILP32 ABI, no
tier-3 target, and nothing resembling watchOS's network policy, so direct peer sync is
expected to work rather than hoped for.

Rust builds normally for Wear via `cargo-ndk` — it's ordinary Android on ARM — so a fully
standalone Wear app is practical.

Verify rotary input (`rotaryScrollable`) **with TalkBack active**; screen reader gesture
handling and rotary scrolling can interact badly.

### 16.9 watchOS — SwiftUI

SwiftUI is the only supported option; Apple deprecated the storyboard-based WatchKit UI, and
WatchKit survives only as a device-services framework (haptics, Digital Crown, session
management). This is fine — watch screens are simple enough that SwiftUI's focus-management
weaknesses barely surface.

**Design intent: a full peer, not a companion.** For a VoiceOver user the watch isn't an
information-poor device — it's the same linear audio stream as the phone on a smaller radio.
The small screen is not the constraint it is for sighted users, so the watch app should run
the complete core: SQLite, Automerge, all query, filter, recurrence, and scheduling logic.
Every operation works with the phone off.

*This is a claim about the core, not about the view inventory.* §16.1 omits the authoring
views — writing a filter, defining a block — because text entry is hard on a watch, not
because anything is unavailable. Everything remains reachable; some things are simply
unpleasant to type there.

**Rust toolchain caveat.** `arm64_32-apple-watchos` is a **tier 3** target: no prebuilt
standard library, so it needs nightly plus `-Z build-std=std,panic_abort` and the `rust-src`
component. `std` itself is fully supported — tier 3 means untested and unshipped, not
degraded. Pin the nightly in `rust-toolchain.toml` and upgrade deliberately.

Risks, in order:

1. **`arm64_32` is an ILP32 ABI** — 32-bit pointers on a 64-bit ARM core, so `usize` is 32
   bits. Crates assuming `usize == u64`, doing pointer-integer punning, or shipping
   hand-written assembly frequently have no path for it. This worries me more than tier-3
   status.
2. **Iroh's crypto stack is the likeliest hard blocker.** Iroh → `quinn` → `rustls` → `ring`
   or `aws-lc-rs`, both carrying C and assembly. `ring` has historically struggled with
   unusual Apple targets.
3. **`rusqlite`** with `bundled` compiles SQLite from C via `cc-rs`, needing correct watchOS
   sysroot and target flags. Link the system `libsqlite3` instead to sidestep it.
4. **watchOS networking policy** may matter more than the toolchain — the system strongly
   prefers routing through the paired phone and aggressively manages independent connections.
5. **Memory budget** is tight, and Automerge loads whole documents. This is a concrete reason
   for the per-year block sharding in §3.1.

**Direct Iroh is the goal, and `WatchConnectivity` is the optimization** — not the other way
round. The watch is a peer in the `devices` set like any other (§7), so when the phone is dead
or left at home it must reconcile over Wi-Fi or cellular with the desktop, the Pi, or the
encrypted store. `WatchConnectivity` is the cheap path when the phone happens to be nearby, chosen
for battery rather than because it is the only route.

*This raises the stakes on risks 2 and 4 above.* If Iroh's crypto stack will not build for
`arm64_32`, or watchOS's network policy refuses independent QUIC, the watch cannot be a full
peer — it becomes a phone accessory, and that is the one place in this plan where a toolchain
question decides a product question. Transport stays pluggable so a fallback exists, but the
fallback is a real loss and should not be planned around. **Run the spike early** (below).

Complications via WidgetKit.

**De-risking step, worth doing early and cheaply:** a throwaway crate with the intended
dependency set (`quinn`/`rustls`, `rusqlite`, `automerge`, `jiff`) built with
`cargo build -Z build-std=std,panic_abort --target arm64_32-apple-watchos`. This answers
every question above concretely in about a day. Requires a Mac with Xcode and the watchOS SDK.

### 16.10 Emacs — for Emacspeak and speechd-el

Likely the cheapest UI in the plan and plausibly the best one for a blind power user. It
skips both the FFI *and* the platform accessibility API.

**Transport: JSON-RPC, driven by `jsonrpc.el`** (in-tree since Emacs 28, proven under Eglot).
Per §8, connect to the daemon socket if one answers; otherwise spawn `lum rpc` over stdio —
started on demand, dies with Emacs, needing nothing but the binary on `PATH`. `jsonrpc.el`
abstracts the two cases, so this is a choice of constructor rather than two code paths. Either
way the server participates in the same store and lock as every other local process, and does
the WAL watching, so Emacs receives **pushed** updates rather than polling.

Shelling out to `lum --json` per command also works and is a fine starting point; move to
the subprocess when you want live refresh.

*Rejected: dynamic modules* (`emacs-module.h` via the `emacs` crate). Panics aren't the
problem — the crate catches unwinds at the boundary and converts them to Emacs errors. The
problem is that a blocking call freezes Emacs's single thread, so any network operation
hangs the editor. It also wouldn't avoid the multi-process coordination question.

**The accessibility model is different from every other platform here.** In Emacs there is no
accessibility API — semantics come from major mode, text properties, and faces. Emacspeak
infers structure from those. The Row projection (§13) still maps cleanly, just through text
conventions: `depth` becomes indentation, `checked` becomes a leading marker, `title` is the
line itself.

Design:

- `lumenna-mode` derived from `special-mode`, line-oriented, one task per line, with a `lumenna-id`
  text property per line so commands know their target.
- Lean on **outline structure** for the subtask tree. Emacspeak's org-mode and outline support
  is among its most developed areas, so folding, level announcement, and navigation come
  nearly free rather than being hand-built.
- `completing-read` for project and label selection — with vertico and consult this is
  genuinely better than any picker on any other platform in this plan.
- `transient.el` for command menus if a magit-style interface is wanted. *As built: not
  used.* A transient's screen reader has to follow transient's own window, and under
  Emacs 31 Emacsvox's could not; `?` lists a buffer's keys in an ordinary buffer instead,
  one per line, from the same definition that binds them.

**Emacspeak** rewards deliberate support: define faces for priority, overdue, and completion
so voice-lock maps them to distinct voices, and ship an `emacspeak-lumenna.el` advising commands
to speak confirmations, with auditory icons on completion. Plausibly worth contributing
upstream.

*As built:* no advice. Writes go through one function that runs a hook and hands the core's
announcement to a replaceable announcer, so the Emacspeak layer is a face→voice map
(`voice-setup-add-map`, which Emacsvox shares) plus auditory icons on that hook, and an
Emacsvox layer puts semantic facts on each line and submits each change as an event whose
sound is a module-fragment rule. Under Emacsvox the icons are left off, so nothing sounds
twice.

**speechd-el** is largely free — ordinary text in an ordinary buffer, with `message` for
feedback.

Costs: Elisp becomes an additional language, and Emacs is not a reliable deliverer of
reminders — it may not be running. Per §8 the daemon will not deliver them either, so
reminders on a Linux desktop come from the tray app and on a console-only system from
`lum notify`. While Emacs *is* running it can announce alongside them via `notifications.el`
or `message`, deduplicating through `ReminderAck` (§3.9); treat that as a convenience, not the
delivery path.

### 16.11 BTSpeak / BTBraille — Python + curses dialogs

Probably the **cheapest UI in the plan**, cheaper even than Emacs. The device is aarch64
Linux on Python 3.11, so the Linux core binary already runs on it unmodified.

Like Emacs, there is no accessibility API to map onto — the device *is* the screen reader,
and speech and braille are the entire output layer. Unlike Emacs, the dialog library has
already made every accessibility decision, consistently, device-wide.

#### The toolkit

`/BTSpeak/Python/BTSpeak/dialogs.py` — a mature curses dialog library:

- **`dynamic_menu` / `DynamicMenuItem`** — the workhorse. Callable titles, `shortcut`,
  `action`, `left`/`right` arrow handlers, per-item `hotkeys`, `dependency` for conditional
  visibility, `hint`, `refresh_interval`.
- **`request_form` / `InputField`** — multi-field forms, Tab/Shift-Tab navigation, per-field
  `validate` plus `format_hint`, `required`, and field types `text`, `bool`, `file`,
  `multiline` (opens nano).
- **`request_date`** — already does flexible parsing via `dates.parse_flexible_date`:
  `today`, `tomorrow`, `+3`, weekday and month names. Returns ISO, `""` to clear, `None` on
  cancel.
- `request_choice`, `request_input` (with history and braille grade handling),
  `request_confirmation`, `show_message`, `view_lines`, `request_file`, `interactive_search`,
  `runActivity` for progress.

Braille-aware throughout — computer versus literary braille spans, back-translation, grade
announcement.

#### Packaging

A Python package the user installs, not a script shipped with the device — so `apps/btspeak/`
in this repository, with a `pyproject.toml` carrying `[tool.btcode]` metadata, and BT Code
handling installation and the user-menu entry. The app is its own menu, built with
`dynamic_menu`, rather than a `.menu` file in `/BTSpeak/Menus/`.

That is the whole integration surface: `# blazie-flags: self-voice` on the entry point, and
`lum` on `PATH`.

The flags comment is read by the launcher, which restores the setting afterwards — but only
when the program is launched *through a menu*. An app that is also run from a shell or from
BT Code's project runner should set `host.set_self_voice(True)` itself and restore it in a
`finally`, as `read-bible` does. The flag is device-wide, in `/run/BTSpeak/`.

*(This section originally specified a `/BTSpeak/Menus/` file listing `lum -a today` and the
like. That shape is right for an app shipped with the device; this one is downloaded, so
BT Code's existing "add to user menu" option does that job and the app should not.)*

#### Transport

JSON-RPC, same as Emacs (§16.10) and for the same reason — pushed refresh on sync rather than
stale lists. Shelling out to `lum --json` works as a first step.

The difference from Emacs is that on BTSpeak the daemon is **required** and therefore normally
running (§8), so the socket is the expected path and spawning `lum rpc` is the exception —
the reverse of the desktop case. Connect to the socket; fall back to spawning if the service
is stopped, so a failed unit degrades the app to working-but-not-syncing rather than broken.

#### The subtask tree

`left`/`right` are already bound to left-arrow/Dot7 and right-arrow/Dot8, so **expand and
collapse need no custom key handling**, and `dependency` returning `False` for collapsed
children removes them from the list. Emulate a screen-reader tree view: announce the level
when it changes, and let `+`/`-` (via `hotkeys`) expand and collapse.

Do **not** convey depth through indentation — it doesn't work in speech.

*Caveat:* in `DynamicMenuDialog.draw()`, both `content_text` (spoken) and `content_braille`
derive from the same `get_title()` string, so speech and braille cannot diverge with the
stock library. Either put a compact level marker in the title (zero custom code, and braille
tree views conventionally show level indicators anyway), or **subclass `DynamicMenuDialog`**
and override the selection-change branch of `draw()` to prepend the level to speech only —
about twenty lines. Subclassing is an intended pattern here; `InteractiveSearchDialog`
extends `ChoiceDialog`.

Start with the marker.

#### Reminders

The device already does what §11 specifies. `btspeak-calendar-reminders.service` runs
`/BTSpeak/Services/calendar-reminders` as a resident 60-second polling loop, and delivery is
simply:

```python
play_sound()          # ffplay on notification.ogg
host.say(message)     # to the synth via brltty
```

**There is no notification framework to integrate with — you speak.** So the rolling-window
scheduler and the 64-notification cap of §11.2 are both irrelevant here; this is the simplest
delivery target in the plan.

**Which process fires them?** A Python one, and per §8 never the daemon. `host.say()` and
`brl.push_message()` are Python APIs, so this is the platform where the general rule —
delivery belongs to whoever owns an output device — is most obviously right. The delivery
service polls `reminders_due` over JSON-RPC (or takes pushed events, if the daemon is the
thing it connected to), speaks, and writes the `ReminderAck`. No reminder *logic* in Python;
no synth handling in Rust.

Two units, then: `lum sync-daemon` for sync, and the Python reminder service for delivery.
That matches how the device already separates `btspeak-calendar-reminders` from everything
else, so it will look native to anyone reading the system.

Three things to copy and one to improve on:

- Unit in `/BTSpeak/Systemd/`, implementation in `/BTSpeak/Services/` — an established pattern
  with roughly twenty existing examples.
- A `Settings/lumenna-reminders.values` file yields an on/off toggle in the device settings UI
  automatically, with `# @prompt` supplying the question text.
- Keep announcement in Python so `host.say()` and `brl.push_message()` are used correctly.
- **Improve on:** their `triggered_reminders` is an in-memory set, so restarting the service
  re-announces the day. `ReminderAck` (§3.9) already handles this properly. Their
  `# TBD: braille` also means calendar reminders never reach the braille display — push to
  both.

#### Awkward parts

- The **timeline** has no visual metaphor here, but blocks as a time-ordered list lose
  nothing.
- **Live refresh** via `refresh_interval` costs battery; prefer RPC push, or refresh on menu
  entry.

### 16.12 Web — an ordinary peer, over a relay

**A browser is a real Iroh peer.** Iroh compiles to `wasm32-unknown-unknown` and runs in the
browser as of v0.33, with iroh-gossip alongside it, and **v1.0 (June 2026)** froze the wire
protocol so any two v1 endpoints interoperate regardless of version or language binding. So
the web client gets its own `NodeId`, joins the `devices` set by ordinary transitive pairing
(§7), and syncs Automerge directly with your other machines. No special case.

**With one restriction that shapes everything else: browser connections are always relayed.**
The sandbox cannot send UDP to an arbitrary address, so hole punching cannot be ported and
every connection goes over a **WebSocket to a relay**. The relay cannot decrypt any of it —
this is the same relay path §7 already describes as the fallback when hole punching fails,
made permanent. What is lost is directness, not privacy.

**Which means the encrypted store is no longer a prerequisite** — it is the same convenience it is
for every other device. Relays forward; they do not store. So a web client alone reaches your
other devices **only while one of them is online**, exactly the structural weakness §8's
always-on peer exists to remove. Open the tab while your desktop is on and it syncs; open it
at 2am with everything asleep and it does not. That trade-off is the one every device in this
plan already makes, and the web client stops being exceptional.

**Pairing is therefore ordinary too** — the §7 flows, unchanged, ending in the same word
comparison. With another device online a browser enrols by `NodeId` and **never sees the master
password**, which is a real reduction in what a hostile bundle could capture. With nothing else
awake it uses account-and-password, exactly as a fresh laptop in the same situation would.

**One platform difference, and it is the mDNS path.** A browser cannot do local discovery —
there is no mDNS in the sandbox and nothing equivalent — so the zero-typing option available
everywhere else does not exist here. Web pairing always involves typing *something*: a
`NodeId`, or a password. Worth knowing when judging the flow, and it is the one place the web
client is meaningfully clumsier rather than merely different.

*The quick-look case is worth calling out*, since it is the one the web is uniquely good for:
someone checking their day on a borrowed machine, not installing anything, with IndexedDB
likely to be cleared afterwards. That is flow 2, once, and it is the Bitwarden web vault
experience — acceptable precisely because the alternative is no access at all.

*Two wrinkles worth writing down.* The browser's device keypair lives in IndexedDB, so storage
eviction loses the identity and forces re-pairing — which is a further argument for the
persistent-storage grant below. And relay-only means bandwidth flows through someone's relay:
n0's public ones by default, or your own, since relays are self-hostable and this is the one
piece of infrastructure worth being deliberate about depending on.

**Structural caveat, and it is not fixable.** The server serves the JavaScript that performs
decryption, so a compromised or hostile operator could serve code that exfiltrates the
password. Subresource integrity narrows this; nothing closes it. This is the standard and
valid criticism of Bitwarden's web vault. Doing the crypto inside the Rust/WASM core rather
than in JS shrinks the surface but does not remove it.

**The web client is therefore less verifiable than a native client, by construction.** Disclose
that where someone signs in (§16.1) — but disclosure is the remedy, not a reduced feature set:
restricting the UI cannot constrain a bundle that already holds the password.

Build notes: `rusqlite` does not target `wasm32-unknown-unknown`, but the read model is
optional (§8), so the browser build can simply scan the in-memory document and persist change
chunks to IndexedDB. Automerge itself is proven on WASM — the JS library *is* this Rust
implementation compiled for the browser. If the read model turns out to matter (full-text
search is the likely trigger), SQLite's own WASM build backed by **OPFS** is the escape hatch;
it is a different binding than `rusqlite`, not an impossibility.

#### Install it as a PWA

Cheap — a manifest and a service worker — and it changes what the client is capable of rather
than only how it looks. Worth doing for the storage guarantee alone, before any of the rest.

- **Persistent storage** (`navigator.storage.persist()`) is the unglamorous one that matters
  most. Un-persisted IndexedDB is **evictable under storage pressure**, and in a local-first
  app the browser holds change chunks that may not be anywhere else yet. Eviction is data
  loss. Installed apps are generally granted persistence; uninstalled tabs are not.
- **Offline operation** via the service worker cache, which the premise demands: the app must
  work when the encrypted store is unreachable, exactly as every native client does.
- **Web Push** — see below.
- **Badging** for what is due today, the same role as the watch complication.
- **One-shot Background Sync** to flush pending changes when connectivity returns (Chromium).
- **File System Access** (Chromium) gives backup and export somewhere real to go, which is
  otherwise the browser's weakest point and, notably, the same gap watchOS has.

#### Reminders: conditional, not impossible

**There is no local scheduling on the web** — the
Notification Triggers proposal (`showTrigger` / `TimestampTrigger`) never reached Baseline,
remains Chromium-only and experimental, and on desktop fires only while Chrome is running.
Do not build on it; revisit if it ever ships broadly.

**Web Push does work, but not from the encrypted store.** "The web client needs a peer" and "the
web client needs the encrypted store" look like one requirement and are two.

**The encrypted store cannot send a reminder, by construction.** It holds encrypted chunks
addressed by hash and never holds a key (§8), so it cannot know a reminder exists, let alone
when it is due. Anything that fires a reminder must be able to **decrypt**, which the store
specifically is not. Building the store so it could would destroy the only property it has.

So the sender is **a peer that decrypts and happens to be awake**: a Raspberry Pi, a desktop
with the tray app running, or in principle a phone. In practice the Pi, since a phone is
usually not executing at the moment its own local notification fires, and a desktop is only on
when it is on.

Payloads are end-to-end encrypted (RFC 8291), so the push service sees endpoint and timing but
not content — the same disclosure the APNs wake-up carries (§7).

*One genuine advantage over APNs:* **VAPID needs no platform enrollment.** Any party holding
the subscription endpoint and the key pair can send, so a peer sends directly and no
intermediary service is required at all. The APNs path (§7) needs a registered sender; this
one does not.

**Which leaves a gap when nothing of yours is awake** — a user whose only devices are a phone
and a locked-down work Chromebook. That is real, and the next subsection closes it at a price
worth naming explicitly.

#### The store can close that gap, and needs no plaintext to do it

**The push payload never has to carry the title and body.** In classic Web Push the push
service relays opaque ciphertext, the browser decrypts it, and hands the bytes to the service
worker's `push` event — and it is the **service worker** that calls `showNotification()`. The
displayed text is constructed by your code, in the browser, after delivery. Nothing upstream
ever sees it.

Which means the store does not need to send content at all. It can send a **content-free
wake-up on a schedule**, and the service worker does the rest:

1. Store emits an empty push at time T.
2. Service worker wakes, syncs from the store, and decrypts locally with the non-extractable
   `CryptoKey` in IndexedDB (above) — service workers are same-origin and can reach both.
3. It computes what is actually due *now* and calls `showNotification()` with locally derived
   text.

This is better than relaying pre-encrypted payloads, which was the obvious design and the
worse one. **There is nothing to go stale**: a reminder cancelled or a task completed since the
schedule was registered simply is not shown, because the decision is made at fire time against
current data rather than baked in hours earlier.

It also removes the peer requirement entirely. The **web client registers its own schedule**
whenever it is open — "wake me at these times over the next 48 hours" — so no Pi and no
desktop need be involved. That is the same rolling-window pattern §11.2 already specifies for
iOS, reused.

**The cost is timing metadata, and it is the whole cost.** The store learns *when* your days
have events. Never what they are, never their titles, never which task. Whether that is
acceptable is the user's call and should be a setting, defaulting off, with the trade stated in
the words above — this audience is entitled to decide rather than have it decided.

*Two implementation wrinkles worth knowing before committing.* **Declarative Web Push**
(Safari 18.4+) puts title and body in a fixed JSON payload the browser renders **without**
running a service worker — convenient, but it requires the sender to hold plaintext, so this
scheme must use the classic service-worker path and not that one. And Chrome enforces a
budget on pushes that display **no** notification; a wake-up that legitimately finds nothing
due must handle that rather than assume silence is free.

#### No, the web app cannot schedule them itself

This is the difference from iOS and it is worth stating flatly, because everything else
follows from it. iOS schedules a `UNCalendarNotificationTrigger` and the **OS** fires it with
the app not running (§11.2). The web has no working equivalent:

- **Notification Triggers** is the exact analogue and never became Baseline — Chromium-only,
  experimental, and on desktop it fires only while Chrome is running (above).
- **Periodic Background Sync** is Chromium-only and coarse, roughly twelve-hourly. Useless for
  a reminder at 14:55.

**So every wake-up on the web must arrive from outside.** There is nothing to schedule
locally, only something to arrange remotely.

#### And sync wake-ups are largely pointless here

Your instinct was to push on every sync. That turns out to be the wrong half of the problem,
and skipping it makes the design smaller.

On iOS a silent push matters because it keeps the local store fresh so that a **locally
scheduled** notification fires with current data. On the web there are no locally scheduled
notifications, so the reminder wake-up **does its own sync** on the way to deciding what to
show. The sync-wake-up's entire job has been absorbed.

What is left for a sync push is making data fresh for a tab that nobody has opened — which
opening the app does anyway. Against that: every push that displays no notification spends
Chrome's silent-push budget, and a sync push displays nothing by definition. Pushing on every
sync would burn the budget on the one thing that does not need it, and then reminders start
being dropped.

**So: reminder wake-ups only.** Sync happens when the app is opened, and inside a reminder
wake-up.

#### The flow, concretely

1. **While open**, the client syncs, computes its reminders for a rolling window (§11.2's
   pattern, reused), coalesces any within a minute or two of each other, and registers those
   times with the store: *"wake me at these instants."* No content, no titles.
2. **At each time** the store emits an empty push to the subscription endpoint.
3. **The service worker wakes**, syncs from the store, decrypts locally, and evaluates what is
   due *now* — so a task completed in the meantime is silently correct.
4. It calls `showNotification()`, or handles the nothing-due case within the budget rule above.
5. **Before finishing, it re-arms** — extending the registered window from wherever it now
   stands.

Step 5 is what makes the scheme self-sustaining. Without it the horizon expires whenever the
app goes unopened for longer than the window; with it, every wake-up pushes the horizon
forward, exactly as §11.2 re-arms on foreground and on sync. A client that is neither opened
nor woken for the whole window does go quiet — a Pi re-registering on its behalf covers that,
if one exists, and otherwise it is the honest limit of a client that cannot run.

**Hooks remain out.** A service worker cannot execute a program, and no API changes that.

#### Key storage

**IndexedDB, not `localStorage`** — which is synchronous, string-only, and capped around
5–10 MB, so the change chunks want IndexedDB anyway.

Better than storing key bytes at all: derive a **non-extractable `CryptoKey`** through WebCrypto
and store the handle. Script can decrypt with it and cannot read it out, which defeats XSS and
anyone who copies the profile directory. It does not defeat a malicious bundle, which can
simply use the key — but nothing does, and this is the same posture as a platform keychain.

#### It depends heavily on the platform

| | Install | Web Push | Periodic sync | File System Access |
| --- | --- | --- | --- | --- |
| **ChromeOS, Chrome/Edge desktop** | Yes | Yes | Yes (~12h, engagement-gated) | Yes |
| **Android Chrome** | Yes | Yes | Yes | Partial |
| **macOS Safari** | Add to Dock | Yes | No | No |
| **iOS Safari** | Home screen, manual | Yes — **only** once added to the home screen | No, and no timeline | No |
| **Firefox** | Desktop install dropped | Yes | No | No |

Periodic Background Sync is Chromium-only and coarse; treat it the way §7 treats iOS silent
push — an optimisation, never a guarantee.

#### ChromeOS may be the best case, not the fallback

A Chromebook can run the Android build, so web there looks redundant. It probably is not.

**ARIA is the closest match to the Row projection of any target** — §13 already notes that
`aria-level`, `aria-posinset`, `aria-setsize`, and `aria-expanded` are literally the same
fields. A `role="tree"` with arrow-key navigation, headings for section jumping, and landmarks
gives ChromeVox the full desktop screen-reader idiom. The Android build gives it a
touch-oriented UI in a container whose accessibility is bridged into ChromeVox rather than
native to it, which is historically the weaker path.

Two smaller points in the same direction: managed and school Chromebooks often have the Play
Store disabled entirely, and Compose for Wear-and-phone is built around touch semantics that a
keyboard-and-screen-reader user on a laptop form factor does not want.

So the web client is plausibly the *preferred* ChromeOS client rather than the fallback —
worth verifying with ChromeVox early, since it changes how much that target is worth.

Accessibility is entirely under your control here, and semantic HTML maps one-to-one onto the
Row projection: `aria-level`, `aria-posinset`, `aria-setsize`, `aria-expanded` are literally
the same fields (§13).

**A full client, not a read-only view.** The audience is people on locked-down work machines
and Chromebooks where nothing can be installed — for whom this is not a convenience alongside a
native app but the only access there is. Shipping them a cut-down version would target the
reduction precisely at the users with no alternative.

Sensible *build order* still applies: today's blocks, checking things off, and quick capture
are the first things worth having and the first things to write. That is sequencing, not a
capability ceiling (§16.1).

---

## 17. Build and tooling

### Languages

Eleven UI targets on six languages, because each language covers several:

- **Rust** — core, store, sync, quick-add, CLI, `lum rpc`, `lum sync-daemon`, GTK4, Win32
- **Swift** — macOS AppKit, iOS UIKit, watchOS SwiftUI
- **Kotlin** — Android and Wear OS Compose
- **Emacs Lisp** — the Emacs UI
- **Python** — the BTSpeak app and its reminder service
- **TypeScript** — the web client (§16.12); the core still comes from Rust via WASM

SQL is embedded in Rust rather than standing alone. Build-system material (Gradle's Kotlin
DSL, Xcode configuration, an `xtask`) adds no language you write the app in.

The toolkit knowledge concentrates the same way — AppKit and UIKit are close cousins, Compose
is the same framework on phone and watch — so each additional target after the first in a
family is much cheaper than the target count suggests.

### Platforms

- **Linux, Android, Wear OS** build on Linux. `cargo-ndk` for Android targets.
- **BTSpeak / BTBraille** is aarch64 Linux; the ordinary Linux core build runs on it
  unmodified.
- **Windows** requires a Windows machine.
- **macOS, iOS, watchOS** require a Mac with Xcode. Core ships as an XCFramework built from
  the Rust workspace; UniFFI generates the Swift package.
- Pin the toolchain in `rust-toolchain.toml` (nightly, for the watchOS target).
- Sessions do not migrate between machines — keep durable project context in a committed
  `CLAUDE.md` alongside this document so a session on any machine starts with the same
  architectural decisions.

---

## 18. Sequencing

Not a strict order, but dependencies are real:

**Foundation** — core domain model, Automerge change store in SQLite (§8), cycle repair and
fractional ordering, property-based convergence tests. Nothing else is safe to build on an
unproven store. **Skip the read model initially** — scan the in-memory document until that is
measurably too slow.

**Recurrence** — easiest to test headless, and the place where getting the model wrong is
most expensive to correct later.

**Parsers** — quick add and filter queries together (§6), since they share `chumsky`, the
date grammar, and the completion machinery. Needed by every UI, easiest to test headless, and
the filter language must exist before block-scoped suggestions (§10.2) are useful. The CLI is
where both get exercised hardest.

**CLI** — first usable product, and the completeness test for the core API (§15). Build it
broad rather than minimal; a capability the CLI cannot reach is a gap in the core, not in the
CLI.

**Sync** — Automerge over Iroh, pairing flow. Best proven with two CLI instances before any
GUI is involved.

**First GUI** — whichever platform is used daily. Establishes the row-projection-to-native-
widget pattern the other GUI targets follow.

**Remaining desktop and mobile UIs** — each is largely independent once the pattern exists.

**Watch targets** — after the `-Z build-std` spike answers the toolchain question, and after
`WatchConnectivity` gives a transport that doesn't depend on it.

**Reminders** — the data model is cheap; delivery is per-platform work that lands with each
UI. The rolling-window scheduler (§11.2) is shared logic and belongs in the core.

**History views** (§10.4) — nearly free once assignments and completions exist, and the
defer-count signal is worth surfacing early because it changes how you use the app.

**Auto-scheduling** — §10.1 and §10.2 after the block model is in daily use. Building them
against a planner you haven't lived with yet means guessing at the ranking heuristics. §10.5
follows immediately after, being a greedy loop over §10.2 rather than new machinery.

**Backup and export** (§9) — early, and before the store holds anything you would mind
losing. Cheap to build, and the round-trip test is what proves the model is complete.

**Undo** (§9) — inverse operations are easiest to write alongside the mutations themselves
rather than retrofitted across a finished command surface.

**Automation bindings** (§12) — after the command surface stabilises. App Intents first: one
implementation covers Siri, Shortcuts, Spotlight, and widgets across iOS and macOS. **MCP is
the cheapest** of the lot — a thin mapping over a surface that already speaks JSON-RPC over
stdio — and unlocks local voice via Home Assistant.

**Calendar import** — needed before the planner's capacity numbers can be trusted, but not
before the planner works at all.

**Team system import** (Jira, Linear, GitHub) — later still, and strictly one-way to begin
with.

**Web client** (§16.12) — no longer gated on anything but the WASM build of the core, since
iroh runs in the browser over a relay. Not scoped down for arriving late (§16.1); it is a full
target. Worth verifying against ChromeVox early, because if it is the better ChromeOS client
than the Android build, that changes its priority rather than its size.

---

## 19. Risks to retire early

1. **Iroh's crypto stack on `arm64_32`** — the cheapest test with the largest consequence in
   the plan. One day's spike, and it decides whether the Apple Watch is a full peer (§16.9) or
   a phone accessory. Nothing else here lets a toolchain question settle a product question.
2. **Orca support for `GtkListView`/`GtkColumnView`** — determines the shape of the Linux UI.
   Fifty nested rows in a throwaway app.
3. **CRDT tree convergence** — property tests over random operation orderings, before any
   UI depends on the store.
4. **watchOS networking policy** for direct peer connections — may invalidate the standalone
   sync ambition independently of whether Rust compiles.
5. **Compose semantics on deep nested lists** — verify TalkBack reports level and position
   correctly before committing the Android tree design. While there, confirm dialog-based
   completion lists (§6.4) behave under TalkBack, since Compose `Popup` cannot be trusted for
   this.

---

## 20. Decisions made

Recorded so they don't get relitigated:

- **The persistent store for encrypted chunks is the *encrypted store*** (§8), not the
  *blind store* it was first called and not the *untrusted store*. Blindness as a figure for
  not knowing is the wrong word in a product built for blind users, and "untrusted" names the
  threat model instead of the thing.
- **Named Lumenna, binary `lum`** (header). Rejected: *Asar* — best meaning of the candidates,
  but `asar` is already Electron's archive-format CLI, and it sits close to Asana in the same
  product category. *Carmë* — a near-homophone of "karma", and a mishearing that lands on a
  common English word is unrecoverable in a project distributed by word of mouth, where a
  mishearing that lands on a non-word is not. *Lúmë* — reads as "loom". *Coranar* — a year is
  the wrong granularity, and it front-loads "corona".
- **Single-user, multi-device.** Not shared projects with collaborators. The day-planner half
  points this way — a timeline of *your* day isn't a shared artefact. Team task systems (Jira,
  Linear, GitHub) arrive later as **imports**, on the same footing as calendar import, via
  `Task.external` (§3.2). This keeps sync, pairing, and access control simple.
- **Completing a parent completes its subtasks** — a setting, defaulting on (§3.3).
- **Tasks may be assigned to multiple days concurrently.** Planning three sittings for a long
  essay up front is a first-class case, not an error to prevent (§3.7).
- **Reminders fire on every device by default**, with a per-reminder override (§3.8).
- **The daemon never delivers reminders** (§8). Delivery belongs to a process that owns an
  output device — a tray app, the BTSpeak Python service, `lum notify` on a console. Being
  headless, the daemon owns none, so the always-on peer stays silent by construction rather
  than by configuration.
- **The planner auto-schedules assistively** — proposals, never silent mutation (§10).
- **Task dependencies are in, minimally** (§3.2) — a flat `depends` set, no lag times or
  critical path. Justified by the planner rather than the list: only dependencies can stop
  §10.5 producing an impossible ordering, and they are what give `blocked`/`ready` meaning.
- **Urgency is one function with several consumers** (§10.3), with **project `weight` as a
  multiplier** rather than an additive term, and never named "project priority" (§3.4).
- **Labels are entities, created implicitly** (§3.4). Records rather than strings, because
  rename must be one write and because §6.3's "did you mean 'laptop'?" needs a closed set —
  but typing `@errand` creates one, with confirm-on-new to catch typos. **Projects do not
  auto-create.** Deletion is a soft-delete touching no tasks, and **merge** exists because
  implicit creation makes near-duplicates inevitable.
- **Computed states are not labels** (§6.2). Taskwarrior spells virtual tags like real tags;
  here `@laptop` and `blocked` are different syntactic classes, they complete from different
  lists, and each computed state must have a matching §13 `State` variant — so anything a
  filter selects on is something a screen reader announces.
- **Hooks are reactive, local-only, and run by the sync-lock holder** (§12). Taskwarrior's
  interceptive `on-add` model cannot survive CRDT sync, where changes arrive already
  committed from devices that are gone. Local-only is also what stops hooks being remote code
  execution across every device. They fire on **both tasks and blocks**, are filtered with the
  §6.2 language, and the device that runs one need not be the device where it happened.
- **Live timers, storing `running_since` rather than a counter** (§3.7).
- **Block occurrences have no lifecycle state** (§3.6). Blocks are not timers: they end when
  their scheduled window ends, "current" is derived from the clock, and ending early or
  extending writes a `Modified` exception. A start/stop state machine has no correct CRDT
  merge, and effort is already recorded on the assignment.
- **Overrun detection is predictive, not reactive** (§10.1) — warn when assigned work first
  exceeds remaining block time, not when the end time passes. This is information a sighted
  user reads off a timeline for free.
- **Move work before moving blocks** when a day slips (§10.1). A block is a commitment about
  the shape of the day; an assignment is only a plan for a task.
- **Re-flow produces one proposal, not a menu of strategies** (§10.1). Asking a user to choose
  between "move, compress, or defer" makes them simulate the algorithm; the core picks and
  shows concrete changes, with alternatives behind an explicit request.
- **Constraints bind the planner, never the user** (§10.1, §10.6). `min_duration_mins` and
  `anchored` describe what may be *proposed*; the mutation API never consults them. The user
  can always skip the break the planner refused to shorten.
- **Proposals are re-entrant** (§10.1). Amendments feed back as `pinned` constraints and the
  proposal recomputes, so accepting a modified plan never applies one computed against
  assumptions that no longer hold.
- **Compression floors are per-block, defaulted per kind** (§3.6, `min_duration_mins`).
  Breaks are incompressible by default and move instead — a break shortened to nothing has
  been deleted while appearing to survive.
- **Re-flow is deterministic, not an LLM** (§10.6). Structured 4.0 uses AI for this; that
  cannot explain itself, cannot be property-tested, and requires sending your day to a
  server. When you can't glance at a result to check it, the explanation *is* the
  verification.
- **Gaps are rows in the day view** (§13), as is "now". Anything a sighted user reads from
  empty space or spatial proportion becomes an explicit row or an announced value.
- **Filter queries are full boolean** — `&`, `|`, `!`, parentheses — parsed with **`chumsky`**,
  which also parses quick add (§6.3).
- **Braille renders titles verbatim**; only roles and states get conventional short forms
  (§13). Braille users get everything speech users get.
- **Backup, export, and import live in every client**, not only the daemon, since the daemon
  is optional on most platforms (§9).
- **AppleScript is skipped**; Windows automation is a PowerShell module over the CLI (§12).
- **Open source**, and free on every platform. Beyond ethos: it makes the encrypted store's
  privacy claim verifiable, and it is insurance against the abandonment risk that ends most
  widely-used unpaid accessibility software.
- **Automerge history is permanent and exposed as a feature** (§10.4), not pruned. Pruning is
  unsafe in p2p and unnecessary at this data size (§8). "Permanently erase" exists as a
  heavyweight escape hatch (§9).
- **CLI, daemon, RPC server, and MCP server are one binary** with git-style subcommands (§2) —
  which also makes local version skew impossible.
- **MCP rather than an Alexa skill** (§12), over both stdio and Streamable HTTP. MCP runs
  locally where the key is; a cloud assistant cannot read data the cloud cannot decrypt.
- **The CLI is a complete interface** (§15) — anything a GUI can do, it can do. It is both a
  first-class product for people who prefer text, and a test of core completeness: GUI-only
  capability means logic escaped the core.
- **GUIs have a required view list** (§16.1), the counterpart to §15's completeness rule for
  the CLI. **The watches are not exempt** — Wear OS and watchOS carry the full list,
  including settings, device management, and permanent erase. Most of what looks like
  "authoring" involves no typing at all, and what does is possible by dictation or on-screen
  keyboard; a capability exemption fails precisely when the watch is the only device to hand.
  **The web client is not exempt either.** Trust-based restrictions there would be theatre —
  a malicious bundle already has the password, so hiding views constrains only the honest
  user — and effort is not a reason either, since **effort determines order, not scope**
  (§16.1). Warn plainly, ship reproducible builds and SRI, and never put a recovery phrase in
  a browser: the one exclusion that reduces what a compromise yields rather than what the UI
  offers. Reminders and hooks remain out because a browser tab is not resident.
- **Pairing confirms by comparing words, never by typing a code** (§7). Both devices derive a
  few words from the completed key exchange, display them, and the human agrees they match and
  confirms on both — which rules out anyone in the middle, since an interposed attacker
  produces two different word lists. It is symmetric, so no device has to be the one
  "showing", and on a local network **nothing is transferred at all**. Required even when a
  `NodeId` was typed: dialling authenticates one direction only, and automatic acceptance
  would hand the device to whoever dialled first. Use a **phonetically distinct word list**,
  the magic-wormhole and PGP approach, so words survive a synthesiser and a braille display.
- **Three enrollment flows, one input each, all ending in word comparison** (§7): mDNS on the
  same network, a typed `NodeId` off it, or **username and password** when only an encrypted store
  is reachable. Never two of the three. The password flow is not a web mechanism — a fresh
  laptop with a dead phone is the same situation as a browser on a work machine — and it still
  confirms, because decrypting the account already yields every `NodeId`. That confirmation
  catches the *enrollment*, not the compromise: its value is the alarm.
- **Headless is not unattended, and the rendezvous is optional in the strongest sense** (§7).
  Whoever pairs a Pi is at a terminal on it, so it compares words like anything else; only
  genuinely unattended provisioning needs a pre-shared token, and a config file holds a long
  one. The rendezvous earns its keep solely for dictating a `NodeId` to someone on another
  network — without it that 32 bytes moves by copy-paste, and its absence is a degradation,
  never a failure.
- **Pairing is transitive; unpairing is not revocation** (§7). The `devices` document syncs, so
  adding a device publishes it to the whole set — *n−1* pairings, not *n(n−1)/2* — and
  membership in that document is the only trust boundary. But a removed device keeps what it
  holds and can write itself back, so a **stolen** device means rotating `account_key`, not
  removing a row.
- **The encrypted store is authenticated, not paired** (§7). No `NodeId`, no replica, no Automerge,
  so a headless self-hosted store needs no pairing UI at all. The always-on peer is the
  opposite: a real paired device, enrolled exactly like any other.
- **The password is not the encryption key** (§8). A random `account_key` encrypts everything
  and never changes; password, recovery phrase, and key file are **three wrappers around it**,
  each with its own salt and KDF so they rotate independently — Argon2id for the password,
  HKDF for the two that already carry full entropy, and the phrase is twelve BIP39 words rather
  than twenty-four. **`auth_value` derives from `account_key`**, so it never changes and a
  password change is invisible to the store: no old-password check, no authorised update, no
  half-applied state. Both are cached in local state and reach a new device over the **pairing
  channel**, never through a document — Automerge history is permanent, and keys must be
  overwritable.
- **The account identifier is a username** (§8) — not a random string, which would defend
  against the wrong adversary since the operator already holds the database, and not an email,
  which would be identifying data collected for a store that never sends mail. **Password
  entropy is the whole defence**, so generate a five-word passphrase by default and enforce a
  floor on chosen ones.
- **The always-on peer runs on hardware you control** (§8). VPS is supported but cannot be
  secured against the provider, and that is documented rather than papered over.
- **No app-level encryption at rest.** Platform encryption covers the realistic threats —
  file-based encryption tied to passcode or biometric on mobile, FileVault, BitLocker or LUKS
  on desktop — and app-level encryption adds little against a running machine that file
  permissions do not already cover. That §8's key management exists anyway does not change it:
  the untrusted-host problem is the encrypted store's job, and on iOS the strict
  `NSFileProtectionComplete` mode that would actually help **breaks background sync**, which
  is worth more. The real exposure is backups and exports (§9), which is a file-location and
  history-disclosure problem rather than a cryptographic one.
- **Wear OS is a normal target**, gated only on acquiring the device (§16.8) — do not build a
  screen reader UI against hardware you cannot test on. Every other target is dogfooded, which
  is what makes the breadth viable.
- **Release blind-first, keep the door open.** Distribution is easiest in the blind community,
  feedback is highest-quality there, and BTSpeak has no competition at all. Broadening later
  costs only "don't ship anything that looks broken" (§21 q1) — and single-user-multi-device
  is what protects the option, since shared projects would make a blind-first app unadoptable
  by mixed teams.
- **Unfinished assignments are offered forward, never carried silently** (§10.1). Multi-sitting
  work is the premise, so an incomplete assignment is the ordinary case rather than an error.
  The core proposes a fresh assignment on the next block whose `task_filter` admits the task.
- **No start or defer dates** (§3.5). Assigning a task to a future block already says when you
  will work on it, and says more than a date does — which block, for how long, alongside what.
  `depends` (§3.2) covers "not until X is finished". A start date would be a second mechanism
  for an intent the planner already expresses, and two ways to say one thing is how a task
  manager becomes confusing.
- **No sub-blocks.** A nested block is either a block — schedule it — or a task inside a block
  — assign it. Nesting would cost a traversal level in the day view, whose two-level shape
  (§13) is exactly what makes a timeline navigable as a list, and buys no capability the model
  lacks. Same reasoning as sections, below.
- **No separate archive state for tasks.** Completion already retains a task, keeps it
  queryable, and feeds history (§10.4); projects keep `archived` because a project is a place
  you navigate to and a task is not. "Not doing this, not deleting it" is a label or an
  archived project. A third lifecycle state would need excluding from every query that forgets
  it — the `is_header` failure mode below, in another guise.
- **Completed subtasks hide by default**, with a setting to show them, and a parent carries a
  completion count. The count is not decoration: a sighted user reads progress off a
  half-struck-through list at a glance, and there is no equivalent to glance at here.
- **Todoist import gets built** (§9). The import path exists regardless, so the adapter is
  incremental rather than new machinery — and it pins down the priority inversion (§3.2) and
  the recurrence-syntax mapping early, while being the migration route for the audience most
  likely to switch.
- **No sections within projects.** Labels cover cross-cutting grouping, sub-projects cover
  location, subtasks cover "a chunk with pieces", and the block/assignment model absorbs the
  sequencing that sections carry in Todoist. A fourth grouping axis costs a mandatory
  traversal level in every project view for no capability that isn't already available.
  `##project` (§6.2) pays the resulting filter tax.

  *What would reverse this:* repeatedly creating sub-projects purely to group, never
  navigating to them independently. If so, add a separate `Header` entity ordered among
  tasks — **not** an `is_header` flag on `Task`, which would leak into every query that
  forgets to exclude it.

---

## 21. Open questions

**The audience decision — deferrable, but name it:**

1. **Build a visual timeline view?** This is the actual fork between a blind-first release and
   one a sighted audience could adopt, and it is worth separating from "how much visual
   polish."

   Polish is cheap: native controls supply platform-correct typography, spacing, dark mode,
   high contrast, focus rings, and icon sets (SF Symbols, Material Symbols, Adwaita, Segoe
   Fluent) for free. The gap between "looks broken" and "looks like a competent native app"
   is perhaps 5–10% overhead — layout hierarchy and empty states, so "no tasks today" reads
   as intentional. Win32 is the exception and needs deliberate work to not look dated.

   The timeline is not cheap. A sighted user **cannot** use a day planner as a list; the
   proportional timeline with duration-scaled blocks and a current-time indicator *is* the
   interface for them. That is a custom-drawn control per platform, and custom drawing means
   owning its accessibility implementation too — the expensive intersection. Call it 20–30%
   of a platform's UI effort, and it need not be built on all of them.

   Nothing in this plan forecloses it: blocks carry start times and durations, the core
   returns structured data rather than rendered rows, and every UI is a thin mapping. A
   timeline could be added to one platform later without touching the core. **Defer the
   decision; keep the door open by not shipping anything that looks broken.**

**Would change features but not the architecture:**

2. **Location-based reminders?** Todoist has them. The model in §3.8 is purely time-anchored;
   adding them means a `Location` anchor variant plus a `Place` record so coordinates are
   named and reusable. That part is small.

   *The value is the ordinary one*, and not worth dressing up as an accessibility argument:
   someone forgets a task, and arriving somewhere reminds them. Knowing you have arrived is
   not the problem a blind user has — that is a GPS app's job, not this one's. The case is the
   same case everyone has.

   **It is a mobile-only feature.** Everything else either cannot do it or gains nothing:

   | | |
   | --- | --- |
   | **iOS** | `CLMonitor` (17+) or `CLLocationManager` region monitoring. **20 regions maximum** — a shared system resource, not a per-app allowance you can grow. Needs `.authorizedAlways`. The system relaunches a terminated app to deliver a crossing, which is the good part. |
   | **Android** | Play Services `GeofencingClient`, **100 geofences per app per user**. Needs `ACCESS_FINE_LOCATION` **and** `ACCESS_BACKGROUND_LOCATION`; on Android 11+ the latter cannot be granted in the dialog and sends the user out to Settings. Play Services dependency, so no de-Googled or F-Droid build. |
   | **Wear OS** | Yes, standalone. Play Services geofencing runs on the watch itself, and Wear hardware has GPS — so a watch on a walk with the phone left at home still fires. |
   | **watchOS** | **Not possible.** Region monitoring is unavailable on watchOS; Core Location there gives location updates but no geofencing. The phone dependency here is Apple's, not a design choice, and there is no clean way around it — keeping location alive via a workout session is not something a task manager should do. |
   | **macOS** | The cheapest of the desktops by a wide margin: **the same Core Location API as iOS**, so the code is shared with that target rather than written again, Apple's Wi-Fi positioning database is healthy, and the tray app (§16.2) is already resident to receive a crossing. |
   | **Windows** | WinRT `Windows.Devices.Geolocation.Geofencing` exists, but it is a **separate code path** through COM interop that shares nothing with the Apple work, the location service is frequently disabled on desktops, and positioning without GPS is weaker than Apple's. |
   | **Linux** | Worst of the three, for two independent reasons. **GeoClue has no geofencing API at all** — you would poll position and compute crossings yourself. And since **Mozilla Location Service shut down in June 2024**, its Wi-Fi backend is dead by default; distributions ship it disabled and the replacements (BeaconDB, Positon) need configuration or API keys. What is left is IP-based positioning, which is meaningless at a 100-metre radius. |
   | **Web** | None. Geolocation requires the page to be open. |

   *Note what that table does **not** say.* Wear OS is listed standalone deliberately: someone
   on a short walk or a hike wears the watch and leaves the phone behind, which is exactly when
   an arrival reminder matters. "Let the paired phone handle it" is the wrong default
   everywhere in this plan (§16.1) — watchOS is an exception only because Apple removed the
   capability, and it should be recorded as a platform gap rather than absorbed as a design.

   **Three costs dominate, and none of them is code:**

   - **Play Store review is the real risk.** Background location requires a declaration form,
     and Google restricts it to apps where it is *core functionality*. One optional feature in
     a task manager is a hard case to argue, and a rejection blocks **the whole app**, not the
     feature. So: not in the initial Play submission. Adding it to an established app later is
     a far smaller risk than launching with it.
   - **The permission is the most scrutinised tier on both platforms**, and both nag afterwards
     about background use. Expect a substantial fraction of users to decline, so it has to
     degrade to nothing gracefully rather than half-work.
   - **iOS's cap is 20 *places*, not 20 reminders**, which is tighter than it sounds. It needs
     the same prioritised rolling window as the 64-notification cap (§11.2) — machinery that
     already exists, at least.

   **One problem is specific to this app: entering a location.** A map picker is useless
   non-visually, so the natural affordance is address search — and geocoding an address sends
   it to Apple or Google, which contradicts §7 directly. **"Use my current location" avoids
   that entirely**, is the better accessible affordance anyway, and is what someone standing in
   the pharmacy would reach for. Saved `Place` records make it reusable afterwards. If address
   search is ever added, say plainly that the address leaves the device.

   *Verdict shape:* small model change, modest code, disproportionate non-code cost, two
   platforms only. Reasonable later; not before the Play Store submission.

3. **Attachments.** Files or images on tasks need content-addressed blob storage and transfer,
   which the encrypted store now specifies almost exactly: a persistent hash-addressed encrypted
   blob store (§8). **The cost objection has largely dissolved**, so this is no longer a
   technical question but a scope one — do tasks want files on them at all, and is that this
   app or a link to one? Decide it when someone wants it.

4. **Task and project templates** — recurring structures, like a packing checklist
   instantiated per trip. Recurrence (§5) handles a task that returns; it does not handle a
   *subtree* you want a fresh copy of.

   *Worth noting before building anything ambitious:* "duplicate this subtree, optionally
   re-anchoring its dates" is most of the value and needs no template entity at all. Reach for
   a stored `Template` record only if duplication proves insufficient in practice.

5. **Paid hosting as the revenue model.** The app is **open source and free**; optional paid
   hosting would provide an always-up encrypted store (§8) plus the silent-push trigger (§7), for
   people who cannot or will not self-host. This is an *addition*, never a feature removed from
   the free app — the Bitwarden shape.

   In its favour: what is charged for is what actually costs money (servers, bandwidth, APNs),
   so recurring pricing is honest rather than rent on written software. Open source makes the
   "we cannot read it" claim verifiable. Comparable services run $2–5/month.

   Open: whether to do it at all. It brings a business entity, tax handling, terms of service,
   and probable GDPR data-processor obligations even for opaque blobs — and paying customers
   expect support, which is what actually consumes time.

   **The one deadline:** free-to-paid is a hard transition and looks like a bait-and-switch;
   paid-to-free is painless. So if the *app itself* were ever to be paid on mobile, that must
   be decided before the App Store launch. Hosting as a separate add-on has no such constraint
   and can arrive whenever.
