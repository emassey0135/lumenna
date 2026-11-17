# The Orbit devices, for an app developer

What the Orbit Speak and the Orbit Player do that an app speaking for itself has to fit, for
someone writing one without the device in hand. The SDK already does what is described here; this
is why it does it, and what to keep in mind when you go below its screen stack.

## The devices

- **The Orbit Speak** (LAVA LZG403_25I) has braille keys: dots 1 to 8, Space, arrows and OK.
  **The Orbit Player** (LAVA LZG403_25) has a phone keypad: digits, star, pound, arrows, OK, Back
  and Menu. Both run Android 11 on arm64, and neither has a screen or an HDMI output: what an app
  draws is never seen, only what Orbit is told is heard.
- **Orbit's screen reader voices everything.** An app does not speak for itself with a voice of its
  own: it describes its screen to Orbit's reader (a list reached, its row in hand moving, a dialog
  opened, a field focused), and Orbit says it in the person's own voice, rate and verbosity, as it
  says its own apps. `OrbitReader` is that description.
- **Nothing told before the reader is ready is said.** Orbit calls `onScreenReaderReady` at once
  when it is up already. The SDK holds everything until then, and tells the screen on top again
  when the reader comes back after a restart.

## Getting the keypad

- **Satellite routes the keypad.** Orbit gives the keys to an app only when the app speaks for
  itself; Satellite's hook marks an app so when its manifest has the `orbit.self_voicing` meta-data,
  and Satellite's launcher lists it with Orbit's own apps. Without Satellite on the device, an app
  gets no keys.
- **Package names Orbit misreads**: one starting with `com.orbit.` or containing `inputmethod` is
  routed as one of Orbit's own, and the keys go elsewhere. Pick another.
- **The keypad is the app's only while it is in front**: resumed, its window focused, and the
  reader up. Something drawn over the app (a system dialog) takes the window's focus without pausing
  it, and the keys must go with it, or the person is left with a keypad that does nothing. A key that
  arrives at any other moment is answered "not taken", so it reaches whatever has the keypad. The
  SDK's `Claim` does this; giving the keypad back always comes first as the app leaves, on the
  calling thread.

## Keys Orbit keeps

- **The Speak's screen reader keeps some Space chords on every screen**, even from an app that
  speaks for itself: Space with C and T (the reader's own commands), Space with G (the braille
  table), and Space with dots 3 and 6 and dots 2 and 5 outside an edit box (reviewing the item's text
  by character and word). Leave them free.
- **Some Space chords are the SDK's moves on every screen**: Space with dots 1 or 4 (Up, Down),
  dots 4 5 and 1 2 (Tab, Shift-Tab), L and dots 4 5 6 (first and last), E and Z (Escape), and M (the
  menu). A command an app puts on a letter should not take Space with A, B, C, E, G, L, M, T or Z;
  Dot 8 with the letter is the way to press those, as Orbit's own apps press theirs. The SDK's
  `Letters` lets an app refuse Space with them and say so.
- **One menu key.** Space with M on the Speak, Menu on the Player, opens the focused item's menu,
  else the screen's: Orbit has no separate key for a row's actions. Its menus are one list of items
  and submenus, not a bar across the screen.
- **On the Player** there is no Tab and no letter: a digit opens that row of a list (1 then 2
  quickly is the twelfth; 0 alone the tenth), holding star switches to finding by letter with
  multi-tap T9, Back goes back, and Menu is the menu key.

## How the reader takes what it is told

- **Speech jumps the queue.** What an app is told to say (`speakText`) goes ahead of Orbit's queue
  of descriptions, so a sentence sent just after a list is reached would be said before the list.
  The SDK's `announce` waits a moment (about 400 ms) behind the last description, keeping
  announcements in order.
- **Orbit says its own words for its own roles**: a menu opened unnamed is "Context menu", a
  submenu is "Sub-menu", a deleted selection its own sentence. An app does not say these.
- **A word typed is echoed from the characters the app told the reader**, not from a word the app
  gives it (the reader ignores that). In contracted braille those characters are not the word's, so
  the SDK tells each cell's echo by name and says the word itself when the person's typing echo
  says words.

## Settings and root

- Orbit's settings provider answers only apps signed by Orbit, and the screen reader keeps its own
  settings in its private data, so the SDK reads them through `su`, as Orbit's own code does.
  Magisk ships on every Speak and Player. `su` from an app keeps the app's mount namespace, so
  another app's data is read at `/proc/1/root/data/…`.
- A refusal is reported once (`OrbitSession.onRootFailed`), never a default taken in silence.
- **The G chord changes the braille table and tells no app.** The liblouis service is handed the
  new table, though, so the SDK asks the service which table it holds as a line is reached and as
  the app comes back, and reads the settings again when it changed.
- The Player has no liblouis service (`BrailleInput.available` is false).

## Trying an app without the device in hand

- Nothing can press Orbit's keypad over adb. Give `OrbitSession` a `debugKeys` broadcast action,
  and a debuggable build takes keys from `am broadcast -a <action> --es name DOWN --ei action 0
  --ez long false --ez phone false` (`name` as the keypad sends it: `DOWN`, `OK`, `SPACE_DOT1_DOT3_DOT4`…;
  `action` 0 down, 1 up, 3 a chord; `phone` true for the Player's keys).
- Wake a sleeping device first (`adb shell input keyevent KEYCODE_WAKEUP`), or the window has no
  focus and the keypad is not the app's.
- What Orbit said is in its log: `adb shell setprop log.tag D`, then `olympus_SpeechManager`'s
  `sequence items` in `adb logcat`.
