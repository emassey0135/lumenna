# Lumenna on BTSpeak and BTBraille

The device *is* the screen reader, so there is no accessibility API to map onto: speech and
braille are the entire output layer, and `/BTSpeak/Python/BTSpeak/dialogs.py` has already
made every accessibility decision, consistently, device-wide. This app draws the Crux app
(`crates/app`, the one Android and the web run) with that library, and almost nothing else.

## Running it

```
./build-core.sh
python3 __main__.py
```

`build-core.sh` builds the core as a Python extension (`crates/ffi` through BoltFFI's
Python target, `boltffi pack python`) and unpacks it into `native/`, which is not committed.
It needs `boltffi` (`cargo install boltffi_cli --version 0.31.1`) and a Python of 3.10 or
later with `pip`; the device has both, and builds it for itself, since it is aarch64 Linux.
`PYTHON` names another interpreter, and `--debug` builds without optimising.

From the User menu: an item whose action is `run python3 <checkout>/apps/btspeak/__main__.py`,
added with the device's own `BTSpeak.user_menu.add_item` (BT Code's "add to User menu" does
the same). Running `python3` on the file itself, not a shell script, is what lets the launcher
read the `blazie-flags` header — and a launch from a terminal shows the terminal's braille,
not the app's, so the menu is the way to use it.

`LUMENNA_PROFILE` picks a store, exactly as it does for the CLI, which is how you keep test
data out of the real profile.

## Self-voicing

`dialogs` speaks and brailles for itself, so brltty must not also read the screen — otherwise
everything is said twice. Two mechanisms cover that, and this app uses both:

- **`# blazie-flags: self-voice`**, on line 2 of `__main__.py`. The launcher reads it before
  starting the program and puts the setting back afterwards, and it reads *the program's own
  source* even when the menu action goes through an interpreter — so a `run python3
  .../__main__.py` entry finds it there. Keep it within the first ten lines, and keep prose
  that mentions the marker out of them: the reader would take that for a second flags line.
- **`host.push_app_context("lumenna", self_voice=True)` in `main()`**, popped in a `finally`.
  The header only fires when something launches the program *through the menu*; run from a
  shell or from BT Code's project runner, nothing has read it. The push is also what makes
  Lumenna the app in front: the device takes its braille table from that app, and without
  it the menus inherited BT Code's — computer braille, from its open `.py` file.

Restoring it matters more than setting it: the context and the flag are device-wide, so leaving it on would stop brltty reading the screen for whatever runs next. A
`finally` covers every exit except `SIGKILL`, which nothing can.

The one thing this changes elsewhere: with self-voice on, printing to the terminal is
silence. So the startup failure — a core not built yet, the likeliest first-run problem — is
said in a dialog rather than printed to stderr.

BT Code already has an option to add an app to the user menu, so this does not write a
`~/BTSpeak/user.menu` line itself. Nor does it have a `.menu` file in `/BTSpeak/Menus/`:
that is for apps shipped *with* the device, and this one is installed by the person using it.

## What it runs

The app and the store run in this process (`core.py`): the screens' state — which row is in
hand after a change, what is folded, the question open, what a form starts from and what
saving sends, the proposals, pairing — is the app's, and each change it makes arrives as a
turn, on a thread of its own, that the menus take in when they wake (`session.py`). A screen
here draws what the app shows, in the core's lines worded in this device's clock, and sends
what the person did.

Syncing runs while the app does. While it is closed, the background service keeps the device in
sync (see Background reminders and sync, below), turned on from Settings, so no one needs a
shell for it; `lum daemon install` does the same for someone who wants `lum`. Either way the app
waits its turn for the device's endpoint, and *Devices and sync* asks whichever holds it for a
round.

Nothing here parses a date, computes a state, or decides what completing a task does to its
subtasks. That is all the core's, so that every client behaves alike and none can drift.

## Files

| | |
| --- | --- |
| `build-core.sh` | Builds the core into `native/`. |
| `core.py` | The store and the app in this process: the turns, this device's clock and dates. |
| `rows.py` | The app's rows as menu items: the level where it changes, folding, the device's clock. |
| `session.py` | What every menu shares: sending to the app, live rebuilding, and what is said. |
| `actions.py` | Each row's actions, the core's, as its context menu, letters and delete keys; what the app opens answered with the device's dialogs. |
| `menus.py` | The main menu. |
| `tasks.py` | Task lists, what can be done to one task, quick add, and the trash. |
| `day.py` | The planner — blocks, sittings, free time and now, and the day's own actions — and the block series. |
| `proposals.py` | Replan from now and Plan the day: the core's proposal, checked and made. |
| `organise.py` | Projects, labels and saved filters. |
| `preferences.py` | Settings, devices, sync and pairing, backups, export and import. |
| `reminder_lists.py` | A task's or block's reminders, what came due, and the Reminders settings page. |
| `background.py` | The background service: keeping the device in sync and giving its reminders. |
| `reminders.py` | Giving reminders, as the background service does. |
| `service.py` | Turning that service on and off, only when asked. |
| `system/` | The service's systemd user unit, linked in from where it is. |

The menus are laid out as the phone's are — the day, tasks, projects, labels, filters, the
trash, settings — so the two can be described in one breath. A row's context menu offers
what the core says can be done to it, in its order and under its names; a letter does the
action of its kind (`actions.KEYS`), so it does nothing a row does not offer; the device's
delete keys delete; left and right fold. The main menu lists the core's places; settings
and priorities are worded as the core describes them.

No third-party Python beside the core. `dialogs` comes from the device.

## Two things worth knowing before changing it

**Speech and braille cannot diverge.** `DynamicMenuDialog.draw()` sets `content_text` and
`content_braille` from the same string. The ways round it are a compact level marker in the
title or a ~20-line subclass overriding the selection branch of `draw()`; this uses the marker, and
announces the level only where it *changes*, which is what a screen-reader tree view does.
`InteractiveSearchDialog` extends `ChoiceDialog`, so subclassing is an intended pattern when
the marker stops being enough.

**Completion is not inline.** `InputDialog` has no hook for completing as you type — Tab is
form navigation — so `assisted_input` offers candidates once the line is entered, whenever
the last word looks like a name that was started and not finished. Inline means subclassing
`InputDialog` to bind a key to the `complete` method, which is the same shape of change and
worth doing once the rest is in daily use.

## Tests

```
python3 -m unittest discover -s tests
```

The tests need the core built (`build-core.sh`, with `PYTHON` naming a Python of 3.10 or
later) and `lum` (`cargo build -p lumenna-cli`). `test_menus.py` and `test_reminders.py` drive
the menus against the app over a real store (`tests/harness.py`), and seed and check that store
through `lum rpc` (`tests/client.py`), as another process on the device would read it:
`tests/btspeak_stub.py` stands in for the device's `dialogs` and plays a script — open this
row, choose that, type this — failing at once if the app asks for something else, so each
test reads as the conversation a person would have. Pairing is among them, against a second
store. Every test runs in UTC, at fixed moments where the hour matters.

## Background reminders and sync

`background.py` is the one process that runs while the app is closed, as Blazie asks of a
background service, and does what the daemon and a resident desktop app do. It keeps the device
in sync (`Device::keep_in_sync`): it holds the device's endpoint, or waits its turn while another
process holds it and takes over when that one stops, and while it holds it it serves the command
surface at the profile's address, so the app's Sync now and `lum sync` run their round there. It
gives reminders (`reminders.py`), and it takes the automatic backups when they are due. The core
is built with the `rpc` feature for the serving.

`reminders.py` is the giving of reminders, in the process that owns an output. Every twenty
seconds it asks the core what came due (`Device::deliver_reminders`) and gives each: the notification sound, then
`host.say()` and `brl.push_message()`, so a reminder reaches the display too. What fires, its
words, and remembering what was given — so a restart does not say the day again, and what
came while it was stopped is one summary — are the core's; there is no reminder logic here.
The right to give them is one process's on the device, so the service and any other app never
both say one.

It is a systemd user service, and nothing of it is written under `/BTSpeak`, which the device
replaces on every release, or anywhere that needs an administrator. The unit,
`system/lumenna-background.service`, is used where it is: Settings, Background reminders and
sync (the app's device setting, `lumenna_records::background`, which the session carries out), turns it on by linking it into `~/.config/systemd/user` and its
`default.target.wants` (what `systemctl --user enable` makes), with a drop-in giving it the
app's profile, then starting it; off stops it and takes the three away, and stopping lets go of
the endpoint and of the right to give reminders. The links are the one record of whether it is
on. The device logs `pi` in at every start, so the person's units start with it; the unit runs
`background.py` beside
it through `%Y`, its own directory even through the link, and waits for `btspeak-ready.service`,
which a user unit cannot be ordered after. The tests link into a temporary directory and record
the `systemctl` commands rather than run them.

In the app, a task with a due date and every block offer Reminders (`r`): their list, Add
reminder by a phrase, Remove reminder. The main menu's Reminders (`r`) lists what came due
over the last day, to dismiss or snooze, or, at a block's end that left work short, to carry
it forward.
