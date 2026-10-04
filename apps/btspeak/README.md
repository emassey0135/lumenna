# Lumenna on BTSpeak and BTBraille

The device *is* the screen reader, so there is no accessibility API to map onto: speech and
braille are the entire output layer, and `/BTSpeak/Python/BTSpeak/dialogs.py` has already
made every accessibility decision, consistently, device-wide. This app is a client of that
library and of `lum rpc`, and almost nothing else.

## Running it

```
python3 __main__.py
```

`lum` is found on `PATH`, or else in the checkout the app runs from (`target/release`, then
`target/debug`), so a git clone with `cargo build` done needs nothing else. The device is
aarch64 Linux, so the ordinary Linux build runs here unmodified — no cross-compilation, no
separate target.

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
silence. So the startup failure — no `lum` on `PATH`, the likeliest first-run problem — is
said in a dialog rather than printed to stderr, and the spawned server's stderr goes to
`rpc.log` in the profile directory rather than onto the screen the app is drawing.

BT Code already has an option to add an app to the user menu, so this does not write a
`~/BTSpeak/user.menu` line itself. §16.11 originally specified a `.menu` file in
`/BTSpeak/Menus/` as "the entire integration surface"; that was written for an app shipped
*with* the device, and this one is installed by the person using it.

## What it talks to

`lum rpc` — the typed command surface over JSON-RPC on stdio (§8, §12). `connect.py` looks
for the sync daemon's socket first and spawns a server if nothing answers, which is §8's
fallback rule for this class of client. `lum daemon install`, run once from a shell, sets the
daemon up as a system service; until then the app spawns its own server, which works but does
not sync in the background — *Devices and sync* says which it is.

Nothing here parses a date, computes a state, or decides what completing a task does to its
subtasks. That is all core's, and reimplementing any of it in a UI is the business-logic leak
principle 2 forbids.

## Files

| | |
| --- | --- |
| `client.py` | JSON-RPC over a reader and a writer. A thread sorts replies from notifications. |
| `connect.py` | Socket, or a spawned `lum rpc`. |
| `rows.py` | Rows to menu items: level markers, folding, and composing a line from components. |
| `session.py` | What every menu shares: the client, live rebuilding, and how a reply is said. |
| `menus.py` | The contract check and the main menu. |
| `tasks.py` | Task lists, what can be done to one task, quick add, and the trash. |
| `day.py` | The planner — blocks, sittings, free time and now — and the block series. |
| `organise.py` | Projects, labels and saved filters. |
| `preferences.py` | Settings, devices, sync and pairing, backups, export and import. |

The menus are laid out as the phone's are — the day, tasks, projects, labels, filters, the
trash, settings — so the two can be described in one breath. Enter on a row offers
everything that can be done to it; the device's delete keys delete; left and right fold.

No third-party Python. The client is one stdlib file; a dependency to build
`{"jsonrpc": "2.0", …}` would be more surface than it saves. `dialogs` comes from the device.

## Two things worth knowing before changing it

**Speech and braille cannot diverge.** `DynamicMenuDialog.draw()` sets `content_text` and
`content_braille` from the same string. §16.11 offers a compact level marker in the title or
a ~20-line subclass overriding the selection branch of `draw()`; this uses the marker, and
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

`test_rows.py` is pure. `test_client.py` drives a real `lum rpc`, and skips itself if the
binary has not been built. `test_menus.py` drives the menus themselves against a real
server: `tests/btspeak_stub.py` stands in for the device's `dialogs` and plays a script — open
this row, choose that, type this — failing at once if the app asks for something else, so
each test reads as the conversation a person would have. Pairing is among them, against a
second server.

## Not built yet

Reminders. §16.11 wants a separate resident Python service — `host.say()` plus a sound —
because delivery belongs to whatever process owns an output device and the daemon owns none
(§8). That needs §11's model first, and a systemd unit under `/BTSpeak/Systemd/` with the
implementation under `/BTSpeak/Services/`, following the twenty-odd existing examples. The
one thing to improve on: `btspeak-calendar-reminders` keeps its fired set in memory, so
restarting re-announces the day, and `ReminderAck` (§3.9) already handles that properly.
Their `# TBD: braille` also means reminders never reach the display; push to both.
