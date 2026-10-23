# The BTSpeak app

Read when working in `apps/btspeak/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

`apps/btspeak/`: Python, on the device's own `dialogs` library, drawing the Crux app, which runs
in the same process beside the store: `crates/ffi` built by BoltFFI's Python target
(`build-core.sh`, `boltffi pack python`, with the `rpc` feature, needing Python 3.10 or later
with pip) into `native/`, not committed. `README.md` gives running it, its files and the tests
(`python3 -m unittest discover -s tests`). The library is only on a BTSpeak
(`/BTSpeak/Python/BTSpeak/`); elsewhere the tests run on `tests/btspeak_stub.py`, with the core
built and `target/debug` on `PATH` for `lum`. On the Mac the system's Python is 3.9: build and
test with a newer one (`PYTHON=…`).

- **A shell and nothing else.** No date parsing, no computed states, no folding of its own:
  each screen sends what the person did (`Session.send`, which waits until the app is still and
  says what it said) and draws `view()` with the core's `lines()`. Turns arrive on the app's
  thread (`core.py`) and queue; a menu wakes on them (`restless`), and what the app said that no
  action asked for is said as it settles. Seeing another process's writes is `tick`, every five
  seconds (`TICK`).
- **A row's context menu is its actions in short** (`actions.py`): the rows' actions merged into
  one list of commands that keeps each row's order, each offered where its row has it, a
  letter per kind (`KEYS`), the delete keys for `DELETES`; a letter the row does not offer runs
  there and the app says why. Running one is `ActOn`, naming the row (`acting_on`), which the
  app puts in hand; `follow` then answers whatever the app opens, one after another, with the
  device's dialogs: a question (a refusal staying in it), the block form, a proposal, a reminder
  list. The task form registers in `actions.FORMS`.
- **Speech and braille come from one string** (`DynamicMenuDialog.draw()` sets both from it),
  but where the app shows a line apart in braille (`Line::braille`) it is the menu item's
  `braille_title`, as the device's own mail list shows its statuses short. Every short form is
  the core's (`words::block_braille`, `sitting_braille`, `Plan::summary_braille`). A list row's
  kind is never shown: speech never says it. A menu's title has no braille of its own on the
  device (T-Chord says and pushes the spoken title), so the day's menu answers T-Chord itself
  (`session._menu`, a `DynamicMenuDialog` subclass, as the device's file manager does) with the
  summary short.
- **Depth is never indentation**: it says nothing in speech. The app says the level only where
  it *changes*, against the previous *visible* row, so folding keeps it right.
- **A menu reopens on the app's row in hand** (`live_menu`'s `focus`), and is drawn after it is
  built, the arrival flag cleared then (`session.settle`): building sends (a place opened), and
  a flag left set draws every screen twice.
- **Completion is Control-L inside the line** (dots 7-8 with L, `tasks.completing_key`): the
  device's `InputDialog` takes `hotkeys`, and a hotkey is handed the dialog, so it completes at
  the cursor and puts the choice in the field; no dialog or device key claims it. In literary
  braille the field holds braille, so it completes the line's end. A last pass after Enter
  (`assisted_input`) still offers what would finish the last word.
- **`main()` pushes an app context** (`host.push_app_context("lumenna", …, self_voice=True)`,
  popped in a `finally`). The device takes its braille table from the app it believes is in
  front; without the push, BT Code's `.py` table makes every menu computer braille. The push
  and pop are device-wide. `# blazie-flags: self-voice` on line 2 of `__main__.py` is for
  the menu launcher, which reads only the first ten lines: keep prose naming the marker out of
  them. With self-voice on, printing is silence, so startup errors are dialogs.
- **It is a line in `~/BTSpeak/user.menu`** (`BTSpeak.user_menu.add_item`, or BT Code's own
  option), which the app never writes itself; not a `.menu` file, which is for apps shipped with
  the device.
- **The app syncs while it runs** (`App::in_front`), waiting its turn for the endpoint while the
  background service or the daemon holds it, and takes the automatic backups, hourly
  (`Core.back_up_while_running`).
- **One background service keeps the device in sync and gives its reminders** (`background.py`),
  so no one needs a shell for either: `Device::keep_in_sync` holds the endpoint as the daemon
  does, waiting its turn while another process has it, and serves the command surface while it
  holds it (hence the `rpc` feature); `reminders.Giver` gives what comes due; it takes the
  backups too. It is a systemd user unit kept in `system/` and run from there (`%Y`), linked
  into the person's own units, with a drop-in naming the profile (`service.py`), only from
  Settings: the app's `background` setting (`BackgroundKind::Service`), carried out by
  `Session._keep_background`. Nothing goes under `/BTSpeak`, which each release replaces, and
  nothing needs an administrator. The links are the one record of whether it is on. The device
  logs `pi` in at every start, so it starts with the device; a user unit cannot be ordered after
  `btspeak-ready.service`, so the unit waits for it in `ExecStartPre`. The tests link into a
  temporary `XDG_CONFIG_HOME` and record the `systemctl` commands rather than run them.
- **The day's own actions lead the Day menu**: `live_menu`'s `app` may be a function, read
  after `build`, so the menu offers what the plan shown offers (`day.day_commands`).
- **A choice's reason is its row's hint.** `request_choice` has no description, so a pick
  whose choices carry a `reason` is a `dynamic_menu` instead (`session.choose(hints=…)`),
  the device saying each hint after the row; a pick without reasons stays a choice dialog.
- **Replan from now and Plan the day are one menu** (`proposals.py`) over the app's proposal:
  rows toggled by Enter with callable titles, so checking rewords a row in place, the title the
  whole proposal (said as it opens, not again after an apply, whose announcement already says
  the next). It is drawn again only for a new proposal or a refusal (`shown`).
  `proposals.NOW` fixes the moment for tests (`PlanFrom`, sent before every action).
- **Carry Forward at a block's end and Replan on an overrun are the Reminders screen's**
  (`f`, `p`, in `reminder_lists.py`): the background service has no keys, and only says the
  text, which already ends in the question or says the overrun.
- **The tests seed and check the store over `lum rpc`** (`tests/client.py`, `harness.py`), as
  another process would, and `tick` the app before each script. They run in UTC: the core
  reads the zone once per process, so a test cannot set its own.
