# Lumenna on BTSpeak and BTBraille

The device *is* the screen reader, so there is no accessibility API to map onto: speech and
braille are the entire output layer, and `/BTSpeak/Python/BTSpeak/dialogs.py` has already
made every accessibility decision, consistently, device-wide. This app is a client of that
library and of `lum rpc`, and almost nothing else.

## Running it

```
python3 __main__.py
```

`lum` has to be on `PATH`. The device is aarch64 Linux, so the ordinary Linux build runs here
unmodified — no cross-compilation, no separate target.

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
- **`host.set_self_voice(True)` in `main()`**, saved and restored in a `finally`. The header
  only fires when something launches the program *through the menu*; run from a shell or from
  BT Code's project runner, nothing has read it. `read-bible` does the same, and setting it
  twice costs nothing.

Restoring it matters more than setting it: the flag lives in `/run/BTSpeak/` and is
device-wide, so leaving it on would stop brltty reading the screen for whatever runs next. A
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
fallback rule for this class of client. No daemon exists yet, so today it always spawns; the
socket half is written so that nothing above `connect.py` changes when one does.

Nothing here parses a date, computes a state, or decides what completing a task does to its
subtasks. That is all core's, and reimplementing any of it in a UI is the business-logic leak
principle 2 forbids.

## Files

| | |
| --- | --- |
| `client.py` | JSON-RPC over a reader and a writer. A thread sorts replies from notifications. |
| `connect.py` | Socket, or a spawned `lum rpc`. |
| `rows.py` | Rows to menu items: level markers, folding, and composing a line from components. |
| `menus.py` | The app. |

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
binary has not been built.

## Not built yet

Reminders. §16.11 wants a separate resident Python service — `host.say()` plus a sound —
because delivery belongs to whatever process owns an output device and the daemon owns none
(§8). That needs §11's model first, and a systemd unit under `/BTSpeak/Systemd/` with the
implementation under `/BTSpeak/Services/`, following the twenty-odd existing examples. The
one thing to improve on: `btspeak-calendar-reminders` keeps its fired set in memory, so
restarting re-announces the day, and `ReminderAck` (§3.9) already handles that properly.
Their `# TBD: braille` also means reminders never reach the display; push to both.

Sync. Blocked on the crate, not on this.
