# The core

Read when working in `crates/core/` (and on reminders, the planner and urgency in `crates/records`). The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

## Mutations and undo

`core::edit` computes an `Edit` (before/after record pairs) and `store` applies it. Core
writes nothing, so every rule is testable against plain structs.

- **What the app does for the person** (folding a legacy Inbox, import, restore) uses
  `Store::apply` and is not recorded; everything the person does is `Store::apply_recorded`.
- **Undo never rewinds**: Automerge gives history, not undo, and rewinding would discard
  concurrent remote changes. **Rebased**: only fields still holding what the edit set go back;
  the rest are kept and reported, so the stack never jams on a conflict.
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

## Recurrence

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

## Reminders

`core::remind::firings` works out every firing between two instants from the synced records
alone; each device asks of its own store and delivers its own, with no coordinator. The
surface's `reminders.rs` words them and answers the apps.

- **A due time or a block can be held to one place's time zone** (`set_task_zone`,
  `set_block_zone`): normally absent, so times float with the device; set, stored as the tzdb
  spells it, its times shown and its reminders fired at that zone's moment.
- **A civil time becomes an instant only in `remind::instant`**, by RFC 5545's rule: a time a
  daylight-saving change skips moves forward by the gap (2:30 becomes 3:30 on the spring
  day), a time that happens twice is the first. A block keeps its real length across a
  change: its end is its start plus its minutes. Expansion stays in UTC.
- **A target's own reminders replace the defaults** (`default_task_reminders`,
  `default_block_reminders`); a default fires under `remind::default_key`, derived from the
  target and trigger and fixed forever, so a dismissal of it syncs and holds everywhere.
- **What fires**: an open task at its due time or, date-only, the all-day hour; each block
  occurrence at its start or end, a cancelled one never; snoozed at the snooze's end;
  dismissed never. Windows are `(from, until]`, so checks in any steps fire each once, across
  the spring change too.
- **The text is the whole message** ("Deep work starts in 5 minutes — 3 tasks assigned, first
  is Review PR"). `missed` is the same said of the past, for the one summary of what came
  while nothing ran (`STALE_MINS`).
- **One process per device delivers** (`take_reminders`, a lock on `reminders.lock`): a
  running app or `lum notify` holds it and calls `deliver_reminders`, which remembers when it
  last delivered (`reminders-delivered`, a device setting) and goes back at most a day; over
  RPC, `reminder.deliver` takes it for its connection. A system that schedules notifications
  itself asks `upcoming_reminders(until)` instead.
- **A reminder is a phrase the core reads** (`parse_reminder`, `trigger_phrase`): "5m before
  start", "at end", "30m before" (a task's due time needs no saying), or a moment of its own,
  "tomorrow at 9am" (`ReminderAnchor::Absolute`, refused without a time or once past). The
  defaults, settings in the same phrases separated by commas, are counted phrases only
  (`parse_trigger`). A task with no due date takes no defaults: a reminder for it is a moment.
- **A system that schedules ahead also asks what it missed** (`reminders_missed`), on launch,
  on returning to the front and after a restart: iOS and watchOS keep what they were handed,
  so only what came past the window last handed over (`upcoming_reminders` records it) was
  missed (`kept`); Android and Wear OS lose their alarms on a restart, so they pass what they
  showed (`<key>@<occurrence>`) and the rest since the last ask was missed.
- **Who delivers, per client**: the resident desktop apps through `deliver_reminders`; iOS,
  watchOS, Android and Wear OS by scheduling `upcoming_reminders`; `lum notify` on a console;
  Emacs through `reminder.deliver`; BTSpeak's background service (`background.py`) through
  `Device::deliver_reminders`, whose `elsewhere` says another process has it, so ask again
  soon. The web's one tab asks `reminders_due` with a "since" of its own, and keeps
  "Reminders on this device" in local storage: a browser has no device-settings file.
- **A reminder of its own can fire on some devices only** (`Delivery`, `delivery_choices`,
  `deliver_reminder`): every device, only one, or all but one. The delivery is one nested map,
  replaced whole, so two devices changing it at once leave one whole choice, never a mix.
- **Dismissing and snoozing write a `ReminderAck`**, which syncs: best effort, since a sleeping
  device may still show it. "Reminders on this device" is a device setting and never syncs.

## The planner's rules

Urgency, suggestions, re-flow and the whole-day plan all keep these:

- **Pure, deterministic functions over the day's state**, in the core, property-tested.
- **A proposal, never a mutation**: a day that reorganises itself unannounced disorients,
  above all someone who cannot glance at it.
- **Every proposal can be said in full before it is accepted**: for this audience the
  description *is* the verification.
- **Constraints bind the planner, never the person.** A block's minimum, `anchored` and the day
  window say what may be *proposed*; no operation consults them, and someone can still shorten
  a break to five minutes or delete it.
- **No language model**: it cannot explain itself or be property-tested, and it would send the
  day to a server.

## Urgency and history

`core::urgency` scores a task, computed and never stored, after Taskwarrior's: additive terms,
the sum **multiplied** by the project's weight, so the order within a project holds and only
the scale between projects changes. `core::history` counts what the records already hold, and
one fact only the history holds: how often a due date was moved later by hand
(`Snapshot::postponed`, kept by the store as the core document grows, `store::postponed`; a
move made by the change that completed a repeating task is not counted).

- **Suggestions exclude nothing for not fitting** (`core::suggestions`): the open tasks a
  block's filter admits, by urgency, a task whose estimate fits the time left raised. Assign a
  Task offers them in that order, each `Choice::reason` said; Put in a Block offers only the
  blocks whose filter admits the task.
- **Planning against a dependency is allowed, and said** (`records::waits`): a task put in a
  block before the block holding a task it waits for, or while that task is in no block and
  not done, is told so in the assignment's notices, and Put in a Block gives each such block
  that reason; one in an earlier or the same block says nothing.
- **The day's own commands are the person's decisions, not proposals** (the surface's
  `day.rs`), each one undoable change: End Now and Add 15 Minutes (`EXTEND_MINS`) change a
  repeating block's day alone, a one-off itself; End the Day and Clear the Rest of the Day
  cancel later repeating blocks for today and trash one-offs; Carry Forward marks the old
  sitting Deferred.
- **A block's end offers Carry Forward** (`remind::falls_short`, `Firing::carry`): the first
  reminder at or after the end says it, or, with none, a firing of its own under `carry_key`,
  which scheduling devices re-arm as the sittings change. Its button runs
  `carry_rest_forward(<series>@<date>)`: every short sitting to its next block in one change.
- **What Should I Work On?** (`whatnow.rs`, `lum next`) is a Pick whose answer starts at once,
  timer running, in the block under way or a block of its own in free time: one change.
- **Re-flow moves the work first** (`core::reflow`, `records::replan`, the surface's
  `replan.rs`): a block's last sittings beyond its time left go to the next block of the week
  that takes them, else back to the list; then the walk from now, anchored and kept blocks
  fixed, a pushed block shrinking to its minimum keeping its end, then moving later. A block
  pushed against an anchored one shrinks to the gap before it when its floor allows. No block
  starts past midnight; a day planned past its window runs to midnight. Every change is a
  whole sentence in the clock the app passes (`TimeStyle`).
- **Applying proposes again** (`apply_replan`, `Replanned::next`): it refuses any change the
  day no longer matches and is one undo; a change taken alone can leave the day crowded, so
  what was not accepted becomes a pin (a block kept where it is, a sitting kept in its block)
  and the day is proposed afresh. A proposal carries its pins and clock, so a client hands it
  back whole.
- **Other options** (`Replan::alternatives`, at most three) each let one block not yet over
  shorten below its minimum (`Pin::Floor`, `ReplanPins::floors`: a work block to 15 minutes, a
  break to half and at least 10) or skip it, offered only where that does better (less unfit,
  then fewer minutes moved and shrunk), shortening first as the cheaper, each saying its cost.
  The minimum binds the main proposal, never an option.
- **The overrun warning** (`remind::overrun_at`, `Firing::overrun`, under `overrun_key`) comes
  before the clock says it: when a block's planned work first outruns its time by more than the
  free time after it, or `FEW_MINS`. Idle, that moment is fixed by the records; with a timer
  running, work and time shrink together, so it outruns from the timer's start or never.
- **The whole-day plan is greedy** (`core::dayplan`, the surface's `dayplan.rs`): each work
  block still to come takes the tasks its suggestions rank first until the estimates fill it;
  a task once a day, none already in a block that day, none blocked, one without an estimate
  counted as 30 minutes and said so; what is due by then and fits nowhere is named. Applying
  is one undo, refused where a task was done or put in that block since, and plans again
  (`DayPlanned::next`), each sitting turned down kept out of its block alone.
- **A task's history is said only when asked about that task** (`task_history`,
  `TaskDetail::history`), never in a list or a live region.
- **History is a place** (`Place::History`, `lists::history`, `lum history`): each task once,
  at its last completion or sitting in two weeks, newest first, its row a task's row, so the
  details, a row's actions and focus by identifier work as on any list.
- **The weights are a synced setting with a key per weight** (`Settings::urgency`), so
  concurrent edits of different weights both hold; said and read as one line,
  `urgency-weights`.
- **Task lists are most urgent first by default** (`Settings::task_order`, the synced
  `task-order`: `urgency` or `added`): siblings by urgency, a subtask kept under its parent,
  the order added breaking ties. Tasks have no order of their own, so nothing is lost. A
  parent's row counts its subtasks done, and the synced `show-completed-subtasks` shows the
  done ones under it in a list that leaves completed tasks out.
- **A list's query may end in `sort: urgency` or `sort: added`** (`parse_query`, `Sort`). A
  block's filter selects and never orders, so `parse_filter` refuses `sort:` rather than
  search for it.
