# The Emacs client

Read when working in `apps/emacs/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

`apps/emacs/`: Elisp over `jsonrpc.el`, built-in libraries only, drawing the Crux app, which
runs in the process serving RPC (`app.*`). Each Lumenna buffer is one of its screens; a buffer
opened from another — a proposal from a day, a reminder list from a list — draws the same
screen (`lumenna--attach`), and the places' buffer `*Lumenna*` is screen 0. Tests are ERT
against a real `lum rpc`, answering the minibuffer by rebinding the reading functions:
`emacs --batch -Q -L apps/emacs -l apps/emacs/test/lumenna-test.el -f ert-run-tests-batch-and-exit`.
CI byte-compiles every file first with warnings as errors (bar `lumenna-emacsvox.el`, which needs
Emacsvox), so the tests run the compiled code, as `package.el` installs it.

- **The socket, else a child.** `lumenna-profile-directory` repeats `lum`'s rule for the
  default profile, because the socket is found by path before there is anything to ask. A
  socket left by a dead daemon does not answer; then a `lum rpc` is spawned.
- **A buffer draws its screen's turns** (`lumenna/turn`, `lumenna--turned`): its view and the
  lines the app words, point kept on its row, and moved to the row in hand only when the
  view's `landing` went up (`lumenna--redraw`). What a turn says goes to
  `lumenna-announce-function`, `lumenna-changed-functions` first with the action that led to it.
- **A command waits for the app and answers what it opens** (`lumenna--follow`): it sends its
  events (`lumenna-send`), waits until the app is still (`app.still`), then asks each question
  the screen has open in the minibuffer, a refusal asking again with why, fills in the block
  form, and opens what the app opened as a screen of its own (a proposal, a reminder list,
  `lumenna-followed-hook`). The row is put in hand only as it is acted on (`ActOn`), never as
  point moves. A form a question names (`lumenna-define-form`) is the client's: a task's
  details, a screen of its own holding the task (`Details(Show)`).
- **Settings' own buttons are rows** (`lumenna--store-rows`, from `settings.offered`): RET or
  `.` hands the action back naming `Settings`, the row's `:on`, which `lumenna--acting-on`
  takes before any list's name for a row. Never a key: erasing is a line chosen, then asked.
- **Depth is an outline level**: `outline-level` reads the `lumenna-level` text property, for
  Emacspeak's level announcements; the folding is the app's (TAB runs Collapse or Expand).
- **A list's keys are defined once** (`lumenna-define-keys`), which binds them, builds the
  menu, and lists them for `?` in an ordinary buffer. A row's key runs the row's action of
  that kind (`lumenna-define-action`, `lumenna-act-kind`); `.` offers them all by title, and a
  key the row does not offer refuses with the app's sentence (`Run`, `lumenna--refuse`). A key
  of the shared table is the core's whole: `d` sends `Key("delete")` on the row put in hand
  (`lumenna-press`), and what it runs (`keys::what_runs`) is never listed here; so do RET on the
  day (`open`) and a task's `b`, `m`, `s` and `w` (`lumenna-define-command`). In a task's own
  buffer the command goes to the details' task (`Details(Key)`), and a refusal there is the
  details' (`Details(Run)`). `W` stays the row's own action, one Stop Waiting per task waited
  for, so the person chooses which. Sounds key on the
  action's `subject/kind`; a key's is learnt from the question it opens (`asking.action`), so an
  immediate one plays none until a turn names what ran. Not transient: Emacsvox reads a transient by asking
  whether its command was called interactively, and Emacs 31's transient wraps each command, so
  moving through one is silent.
- **A task's priority is its face** (`lumenna-priority-1` to `-3`, from the row's `priority`),
  read in a voice of its own (`lumenna-voice-map`).
- **A colour is two spaces in it** (`lumenna--swatch`), before a row's name and in a pick's
  annotation: blank to speech and braille, which hear the colour's word in the line, its light
  or dark value by the frame's `background-mode`, and nothing where the display has no colour.
- **Booleans come back as `:json-false`**, which is non-nil: test them with `lumenna--true`.
- **The clock is Emacs's**: `display-time-24hr-format`, told the app as it changes
  (`app.clock`), which words every time in it.
- **Three speech layers, each skipped where it would double up**: echo-area text; faces
  through `voice-setup-add-map` plus Emacspeak auditory icons; and under Emacsvox, facts on
  each line and events on the notification lane, whose rules are one per role and event (a
  rule per method plays the sound twice). The Emacsvox layer is tested against Emacsvox
  itself (`test/lumenna-emacsvox-test.el`, skipped without it).
- **The day's own actions are its heading's**: the heading is the day's summary row, so `.`
  on it offers the plan's `actions`; `P`, `r`, `w`, `E` and `C` run them from anywhere in the
  day.
- **A proposal is the app's screen, drawn as checkable lines** (`lumenna-proposal.el`): SPC
  checks one (`Proposal(Check)`), C-c C-c applies, and the app's next proposal takes its place
  until it closes. `lumenna-proposal--now` fixes the moment for tests (`PlanFrom`). A
  reminder's `replan` button, and `r` in the due list, open Replan from Now.
- **Its background setting is `lum`'s sync daemon** (`BackgroundKind::SyncDaemon`, Sync in the
  background, `lumenna-settings.el`), since Emacs is not open all the time: `lum --json daemon
  status`'s `service` `installed` is told the app as `Background(Offered)` at connecting and as
  Settings opens, and a turn's `background` runs `lum daemon install` or `uninstall`
  asynchronously, the state read back answering the app, or `Failed` with what `lum` said; the
  sentence is the app's. A status asked before the last install or uninstall is not told
  (`lumenna--daemon-changes`): the app would take it for the answer. Tests stand a script in
  for `lum daemon` (`lumenna--daemon-program`), so nothing is installed.
- **Reminders are given through `reminder.deliver`** (`lumenna-reminders-mode`), which takes
  this device's reminder lock and says what came due; the app is then handed the same moments
  (`Due(Look)`, from just before the earliest firing to the moment delivered at), and lists,
  says and gives notices of what it finds (`ViewModel.due`). The due list draws that list from
  screen 0, each notice is a desktop notification, and every button is a `DueEvent`, so which
  are still listed is the app's. `lumenna-disconnect` stops the mode (`lumenna-disconnected-hook`),
  whose timer would connect again at once; connecting starts it again. A task's or block's
  reminder list is the app's.
- **A new connection draws every buffer's screen again** (`lumenna--leave-screens`,
  `lumenna--reopen-screens`): the new app has none of the old one's screens, so each buffer that
  opened a screen sends the events that opened it (`lumenna--opened-with`) to the same number,
  screen 0's buffers simply attach, and a buffer drawing what the app opened on another's (a
  proposal, a reminder list) is closed.
- **A choice's reason is its completion annotation**, never part of the line chosen.
- **Tests seed the store through its own methods** (`lumenna-test--seed`), then tick the app
  and wait for it, as another client's write would reach it.
