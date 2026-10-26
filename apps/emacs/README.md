# Lumenna in Emacs

A client of `lum rpc` in Emacs Lisp, using only what ships with Emacs 29 and later:
`jsonrpc.el` for the connection, and `completing-read` for choosing a project or label. It
draws Lumenna's app, as every other client does: the app runs in `lum rpc` (or the daemon or a
desktop app it reaches), each Lumenna buffer one of its screens. Like the BTSpeak app it
decides nothing itself; which row is in hand, what an action asks, every rule, date and
announcement is the app's.

## Running it

```elisp
(add-to-list 'load-path "~/lumenna/apps/emacs")
(require 'lumenna)
(keymap-global-set "C-c l" lumenna-command-map)   ; any prefix you like
```

Then `C-c l t` is Today, `C-c l a` adds a task, and `C-c l ?` lists the rest.
`M-x lumenna` lists the places; `M-x lumenna-today`, `lumenna-tasks` and `lumenna-add` go
straight there. `lum` is found on `exec-path`, or set `lumenna-lum-program`.

The client connects to the daemon's socket (`<profile>/lumenna.sock`) when one answers, and
otherwise starts `lum rpc` itself, which stops when Emacs does, and which keeps this device in
sync while Emacs runs once it is paired. Either way changes made elsewhere reach the app,
whose screens every open Lumenna buffer redraws. `lumenna-profile`, or
`LUMENNA_PROFILE` in Emacs's environment, picks a store, as it does for the CLI.

To keep this device in sync while Emacs is closed, turn on Sync in the background in
`lumenna-settings`: it installs `lum`'s sync daemon as a service that starts at login (`lum
daemon install`), and turning it off removes it again.

## Using it

Every list is a read-only buffer with one item per line, said in words: a task reads
`buy milk, due tomorrow, p1, overdue`, never as columns or symbols. A subtask is indented
and is an outline level deeper, so TAB folds it and a screen reader that announces outline
levels says the depth.

In every list:

| Key | |
|---|---|
| `n`, `p` | next, previous line |
| RET | the line's main action: open a task, a project's tasks, change a setting |
| `.` | every action this line offers, by name, as every Lumenna app offers them |
| TAB | fold or unfold the subtasks |
| `g` | refresh |
| `u`, `y` | undo, redo — and Emacs's own `C-/` and `C-?` do the same here |
| `?` or `h` | every key this list has, one per line; RET on one runs it, `q` goes back |
| `L` | the places |
| `q` | leave the list |

`M-x imenu` goes to an item by name, with completion.

The same commands are in the menu bar, for whoever prefers it: each list has a Lumenna
menu of its own, grouped as `?` groups them, and Tools has Lumenna's commands from anywhere.
In a terminal, `F10` (or `M-x tmm-menubar`) opens the menu bar as a list to choose from.

What a line offers is decided by Lumenna's core, the same on every app: a task in the trash
offers Restore, Delete from Trash and Erase Permanently, the Inbox only Move Down and Weight. Each key below
runs the line's action of that kind, so a key does nothing the line does not offer, and
says what the line does offer instead.

Tasks: `a` add, `/` search, `c` Mark Done or Mark Not Done, `e` Edit Details (one field at
a time), `b` Put in a Block, `m` Move to Project, `s` Make Subtask Of, `t` Move to Top Level,
`w`/`W` Wait For and Stop Waiting, `d` Move to Trash; in the trash `r` Restore and `d`
Delete from Trash. Erase Permanently, which takes a task out of every device's history too, is
no key's: choose it by name with `.`. Adding is one line, written as you would say it (`call the
bank tomorrow at 3pm p1 #Home @calls 15m`); TAB completes a project or label, `C-c C-r` says how
the line is understood so far (the date, priority, project and labels, as the other apps
show under the field), and a line naming a project that does not exist is refused rather
than added. `C-c C-r` in a filter says what it means and how many tasks it matches.

The day: `[` and `]` previous and next day, `t` today, `j` go to a day, `a` add a block.
On a block, `e` changes one field at a time — its name, times, kind, repetition, notes,
whether it takes tasks, counts toward capacity or is anchored, its shortest length, the
filter its tasks come from, its last day and its colour — `i` Assign a Task, `x` Cancel
This Day, `o` Restore This Day. On a sitting, `s` starts its timer, pauses it, or resumes
it; `S` stops it, ending the sitting; `l` sets its planned length; `m` logs minutes; `e`
opens its task. `d` deletes a block or unassigns a sitting. On the block under way, `N` is
End Now and `+` Add 15 Minutes; `f` is a short sitting's Carry Forward, or a block's Carry
the Rest Forward.

The day's own actions are its first line's (`.` there offers them all) and these keys
anywhere in the day: `P` Plan the Day, `r` Replan from Now, `w` What Should I Work On?
(each suggestion's reason beside it as it completes), `E` End the Day and `C` Clear the
Rest of the Day. Plan the Day and Replan from Now open a buffer of the core's proposal, a
change per line, each checked: `SPC` or `RET` unchecks one, `C-c C-c` makes what is checked,
and the core's next proposal, around what was left unchecked, takes its place until nothing
more is proposed; `q` cancels. Under Other options, `RET` or `o` on one shows the core's
proposal for it in place. A task's own buffer ends with its History.

Projects, labels and saved filters each have their own list (`a` add, `r` rename, `d`
delete, `M-p`/`M-n` reorder, and the rest in `?`). `M-x lumenna` lists the places as every
app's sidebar does, each project, label and filter under its heading. Settings, devices and
pairing are in `lumenna-settings` and `lumenna-devices`; Settings' last line, Erase Everything
Deleted, empties the trash and every deleted thing's history on every device, asking first.

## Reminders

While Emacs runs, `lumenna-reminders-mode` gives this device's reminders: every twenty
seconds, and on coming back to Emacs, what came due is said in the echo area (or on
Emacsvox's notification lane, with its alarm cue; Emacspeak plays its alarm icon) and, where
Emacs has D-Bus and a notification service, shown as a desktop notification whose buttons
are the core's Dismiss and Snooze. It starts on the first connection unless
`lumenna-reminders-start-automatically` is nil, and gives way to whatever else on the device
already gives reminders — the Mac, GTK or Windows app, or `lum notify` — so a reminder is
never said twice. What came while nothing ran is one summary. `lumenna-disconnect` stops
them until Lumenna connects again.

`C-c l r` (`lumenna-reminders-due`) lists what came due and was not dealt with, as the app
lists it on every device: `d`
dismisses one and `z` snoozes it, on every device, and `c` carries forward what a block's
end left short, where the reminder offers to, and `r` opens Replan from Now where a block's
work outruns its time (each notification has its button too); RET
opens what it is about. On a task with
a due date or a block, `R` lists its reminders: `a` adds one by a phrase (`5m before start`,
`at end`, `30m before`), asked again with the core's sentence if it cannot be read, and `d`
removes one of its own. The defaults for every task and block, and Reminders on this device,
are in Settings.

## Speech

Three layers, each used only where it can be:

- **Plain** — what every screen reader in Emacs hears, speechd-el included: ordinary text
  on ordinary lines, and each change's announcement from the core in the echo area.
- **Voices** (`lumenna-voice.el`) — a face for headings, finished, overdue, quiet, now and
  a running timer, mapped to voices with `voice-setup-add-map`, which Emacspeak and
  Emacsvox share. Under Emacspeak, completing, deleting and starting or stopping a timer
  also play an auditory icon.
- **Emacsvox** (`lumenna-emacsvox.el`, loaded when Emacsvox's aural layer is) — each line
  carries semantic facts (a task, block or sitting, and its states), and each change is
  submitted on the notification lane as an event (`lumenna-task-completed`,
  `lumenna-deleted`, `lumenna-timer-started`…). Its sounds are a module fragment's rules, so
  they are changed in Aural Home like any integration's, and `C-e E` explains a line. With
  Emacsvox the Emacspeak icons are not played, so nothing sounds twice.

## Tests

ERT, against a real `lum rpc` on a scratch profile, driving the commands as a person would:

```
cargo build -p lumenna-cli
emacs --batch -Q -L apps/emacs -l apps/emacs/test/lumenna-test.el \
  -f ert-run-tests-batch-and-exit
```

`LUM` names another binary. The Emacsvox layer has its own file, which loads Emacsvox's
aural layer (not its speech server) from `EMACSVOX`, else `~/emacsvox/lisp`, and is skipped
where it is not found:

```
emacs --batch -Q -L apps/emacs -L apps/emacs/test \
  -l apps/emacs/test/lumenna-emacsvox-test.el \
  --eval '(ert-run-tests-batch-and-exit "^lumenna-emacsvox-")'
```
