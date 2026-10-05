# Lumenna in Emacs

A client of `lum rpc` in Emacs Lisp, using only what ships with Emacs 29 and later:
`jsonrpc.el` for the connection, `outline-minor-mode` for the subtask tree,
`completing-read` for choosing a project or label, and `transient` for the menus. Like the
BTSpeak app it decides almost nothing itself; every rule, date and announcement is the
core's (§16.10).

## Running it

```elisp
(add-to-list 'load-path "~/lumenna/apps/emacs")
(require 'lumenna)
(keymap-global-set "C-c l" #'lumenna-dispatch)   ; any key you like
```

`M-x lumenna` lists the places; `M-x lumenna-today`, `lumenna-tasks` and `lumenna-add` go
straight there. `lum` is found on `exec-path`, or set `lumenna-lum-program`.

The client connects to the daemon's socket (`<profile>/lumenna.sock`) when one answers, and
otherwise starts `lum rpc` itself, which stops when Emacs does. Either way changes made
elsewhere arrive as a push and every open Lumenna buffer refreshes. `lumenna-profile`, or
`LUMENNA_PROFILE` in Emacs's environment, picks a store, as it does for the CLI.

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
| TAB | fold or unfold the subtasks |
| `g` | refresh |
| `u`, `y` | undo, redo |
| `?` or `h` | this list's menu, naming every key |
| `L` | the places |

Tasks: `a` add, `/` search, `c` done or not, `e` edit a field, `b` assign to a block, `m`
move to a project, `s` make a subtask, `t` back to the top level, `w`/`W` wait for another
task or stop, `d` to the trash. Adding is one line, written as you would say it (`call the
bank tomorrow at 3pm p1 #Home @calls 15m`); TAB completes a project or label, and a line
naming a project that does not exist is refused rather than added.

The day: `[` and `]` previous and next day, `.` today, `j` go to a day, `a` add a block.
On a block, `e` edit, `i` assign a task, `x` cancel this day only, `o` put it back. On a
sitting, `s` starts or stops its timer, `l` sets its planned length, `m` logs minutes.
`d` deletes a block or takes a sitting off it.

Projects, labels and saved filters each have their own list (`a` add, `r` rename, `d`
delete, `M-p`/`M-n` reorder, and the rest in `?`). Settings, devices and pairing are in
`lumenna-settings` and `lumenna-devices`.

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

`LUM` names another binary.
