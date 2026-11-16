# The BTSpeak app

Read when working in `apps/btspeak/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

`apps/btspeak/`: Python, on the device's own `dialogs` library, drawing the Crux app, which runs
in the same process beside the store: `crates/ffi` built by BoltFFI's Python target
(`build-core.sh`, `boltffi pack python`, with the `rpc` feature, needing Python 3.10 or later
with pip) into `native/`, not committed. `README.md` gives running it, its files and the tests
(`python3 -m unittest discover -s tests`). The library is only on a BTSpeak
(`/BTSpeak/Python/BTSpeak/`); elsewhere the tests run on `tests/btspeak_stub.py`, with the core
built and `target/debug` on `PATH` for `lum`. On the device itself run them with `python3 -S`:
the device's site packages put the real `dialogs` in place of the stub, and it needs a
terminal. On the Mac the system's Python is 3.9: build and
test with a newer one (`PYTHON=…`).

- **Its version is Lumenna's** (`[workspace.metadata.lumenna]`), which `build-core.sh` writes
  into `pyproject.toml` as a plain value: BT Code reads `project.version` itself when it sends the
  app to the Code Store, so it cannot be `dynamic`. `records`' metadata test holds the copy
  committed to it.
- **A shell and nothing else.** No date parsing, no computed states, no folding of its own:
  each screen sends what the person did (`Session.send`, which waits until the app is still and
  says what it said) and draws `view()` with the core's `lines()`. Turns arrive on the app's
  thread (`core.py`) and queue; a menu wakes on them (`restless`), and what the app said that no
  action asked for is said as it settles. Seeing another process's writes is `tick`, every five
  seconds (`TICK`).
- **It opens on the core's Home** (`menus.home`, `ShellEvent::Home`, `ViewModel::home`): the
  places as the sidebar has them, headings folding the projects, labels and saved filters under
  them, each with its letter (`HomePlace::letter`, the item's `shortcut`, whose press runs Enter);
  its row in hand the sidebar's. Opening a place (Enter, `Key("open")`, or a letter) leaves Home in
  the app; the place's screen is shown (`menus.place_screen`) until left, then Home is asked for
  again. No main menu of this app's own, and no screen per heading.
- **M-Chord on a place's screen and on Home is the core's menu bar** (`braille_menu_bar(place,
  home, sentence)`, `session.menu_bar`), as the device's editor has one (`Editor/Source/src/
  menu_ring.c`, `Libraries/ui_menu.c`): it opens on the screen's own menu (`SCREEN_MENU`, "Today
  menu", "Home menu": its `screen_actions`), then File, Edit, View, Task, Day on Today, and Help,
  each a menu of its own said by name as it opens (`_RingMenu`). Tab, and M-Chord pressed again,
  go to the next menu and Shift-Tab to the previous, round from either end; a letter runs its item
  within a menu; Z-Chord leaves. `session._menu`'s `ScreenMenu` answers M-Chord with it
  (`on_AppMenu`), as the device's file manager opens a menu of its own. What is picked runs on the
  row under the cursor (`actions.run_from_bar`): as the app runs it (`Key(id)`, the row put in
  hand), a heading of Home's by its letter there, and what the core leaves to a platform here
  (`actions.bar_commands`: the screen actions it marks `shell`, Sync now's wait, Devices and
  pairing, Export and import, Restore from a backup, Close window and Quit). The bar's items carry
  no `shell` of their own yet, so that list is this app's. A command that takes the app to another
  place or Home (View's places, What should I work on?) closes every screen
  (`session.moved_on`) and Home's loop shows where the app now is; Quit closes them all
  (`session.leaving`). Unlike the editor, no chord jumps straight to one menu: the dialogs pass the
  editor's menu chords to no Python dialog, and a menu's letter runs an item in it, as the editor's
  do, so a menu is reached by Tab. Dot 7 with M-Chord stays the row's actions. Screens that are no
  place (a reminder list, a settings page, the devices) keep a single M-Chord menu of their own.
- **A screen's letters are its `screen_actions`'** (`actions.screen_commands`), pressed from the
  list as hot keys: each run as `Key(id)` and its letter pressed as `Letter`, but those the core
  marks `shell` (the filter, Key help, About, Reminders due) and Sync now's wait, which
  `tasks.own_commands` runs. Home's letters come before a row's (`live_menu(screen_first=True)`),
  as the core's do, so a row's action whose letter Home takes says none in its context menu. The
  day's own actions (`day.day_commands`) are its hot keys too. A key the app answers by opening
  something (the quick-add line, settings) is followed by `actions.follow`.
- **A row's context menu is its actions in short** (`actions.py`): the rows' actions merged into
  one list of commands that keeps each row's order, each offered where its row has it, with
  the core's letter (`action_letter`); a letter pressed runs through the app
  (`actions.letter`, `Event::Letter`), which says why if the row does not offer it.
  Running one is `ActOn`, naming the row (`acting_on`), which the app puts in hand; `follow` then
  answers whatever the app opens, one after another, with the device's dialogs: a question (a
  refusal staying in it), the block form, a proposal, a reminder list. The task form registers
  in `actions.FORMS`. Erase Permanently has no letter: only the context menu runs it.
- **Enter is `Key("open")` on every row** (`actions.press`): a place opened, a task's details, a
  block edited, a trashed task restored, a device renamed; never a command chosen per screen.
  **Every letter this app shows is the core's** (`L.action_letter`, `ScreenAction::letter`,
  `HomePlace::letter`, `page_letter`, `button_letter`, `due_letter`), none written here: what a
  letter runs on the row is the app's (`Event::Letter`), `Command.through` unset only for what
  this app runs itself (the screen actions the core marks `shell`, Sync now's wait, Settings'
  buttons). A plain letter is the command, never Space
  with one; a capital, Dot 7 with the letter, moves by first letter, which the device's dialogs
  already do, so this app needs `first_letter` nowhere. **The
  delete keys send `Key("delete")`** (a command that is only the keys', not `listed`): Control-D,
  the D chord and low-d. What it runs on the row (Delete, Delete from trash, Unassign, Unpair) or
  why it runs nothing is the app's (`keys::runs`); never a list of kinds here.
- **Settings are the core's pages** (`SettingsView.pages`), every one listed, never filtered here:
  each page's settings, caption, buttons and letter are the core's (`page_letter`,
  `button_letter`); a time or number line says the setting's own hint, else `SettingsView.words`.
  The app opens them for Settings (`Key("settings")`) as well as this screen does
  (`preferences.opened`), and leaving puts them away (`settings_open` keeps `follow` from opening
  them twice).
- **Every question and form is the core's dialog, drawn with the device's** (`forms.py`, over
  `lumenna_app::dialog`): a question, the block form and quick add are `ScreenView::dialog`, the
  task form a question's (Enter or Edit details shows the task, and the question becomes its form),
  a settings page `App::settings_page_on`. `forms.ask` picks the device's dialog by the controls (a
  list, a line, one button, several), `forms.form` asks a form's lines, choices and checks with
  `request_form`, and a settings page is a menu of its controls. What the person did goes to
  `control_event` and what comes back is sent: no event is built here, and no field is known by
  its key (they are the core's `field:…`, `setting:…`). A form sends only the fields the person
  changed, then presses its default button: a block's kind brings its own settings unless they
  were changed too. A refusal is the dialog's `refusal` line, said before it is asked again. The
  prompt is `QuestionShown::prompt`; a pairing code is sent as typed, spaces and all. Two things are
  still this app's: a project choice says its level after it (`Lines::projects`), and a check
  setting's states are its setting's own options ("On", "Off"), neither of which the controls
  carry yet.
- **The Export and import page lists the store's own actions** (`SettingsView.offered`, Erase
  everything deleted) as rows after its buttons, with no shortcut, each handed back as `ActOn` with
  `ActingOnSettings` (`preferences.store_items`), the core's `offered_caption` its hint, which
  the device says and pushes to the display after a pause; the app asks its Confirm question.
- **Speech and braille come from one string** (`DynamicMenuDialog.draw()` sets both from it),
  but where the app shows a line apart in braille (`Line::braille`) it is the menu item's
  `braille_title`, as the device's own mail list shows its statuses short. Every short form is
  the core's (`words::block_braille`, `sitting_braille`, `Plan::summary_braille`). A list row's
  kind is never shown: speech never says it. A menu's title has no braille of its own on the
  device (T-Chord says and pushes the spoken title), so the day's menu answers T-Chord itself
  (`session._menu`, a `DynamicMenuDialog` subclass, as the device's file manager does) with the
  summary short.
- **Depth is never indentation**: it says nothing in speech. The app says the level only where
  it *changes*, against the previous *visible* row, so folding keeps it right. The projects,
  labels and filters screens each list one heading's places, so their level counts from there:
  folding is the app's (`hidden`, `expanded`), the levels the core's `levels` of their depths
  under the heading, as a project pick's are of its choices'.
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
  (`Core.back_up_while_running`). It gives reminders too, a `reminders.Giver` on a thread of its
  own (`reminders.give_in_background`), waiting its turn for the core's lock while the service
  has it, so nothing is said twice.
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
  text, which already ends in the question or says the overrun. The list looks back by the app's
  own clock (`DueEvent::LookBack`) and hands each button back by its id (`DueEvent::Press`), so
  its tests place blocks by the real clock (`block_at`), as the core's flow test does.
- **The tests seed and check the store over `lum rpc`** (`tests/client.py`, `harness.py`), as
  another process would, and `tick` the app before each script. They run in UTC: the core
  reads the zone once per process, so a test cannot set its own.
- **`help/en` is this app's own prose, for now**: H-Chord's topics, and Keyboard shortcuts on
  Home's menu (`menus.key_help`, the topic of the screen in front). What in it is the device's
  own stays here: the device's keys (H-Chord, Z-Chord, M-Chord and Dot 7, Control-D and the D
  chord, left and right, a capital moving by first letter) and how the device's dialogs behave.
  What should become the core's Key Help later, since every braille device would say it: each
  screen's and Home's commands with their letters (`screen_actions`, `HomeView`), each row's
  actions and letters (`action_letter`), what Enter does per row (`keys::runs_on`), what an
  action asks (Weight's range, Delete's choice for a project's tasks, Erase Permanently's warning),
  how to write a quick-add line and a filter, pairing's steps, and what the exports and backups
  hold. Keep the help in step with Home and the screens' menus until then.
