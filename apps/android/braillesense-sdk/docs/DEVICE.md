# The BrailleSense 7, for an app that speaks for itself

What the device does that an app drawing its own speech and cells has to know, found by reading
the device, its manual and its built-in apps. The SDK already does all of this; this page says why,
for a developer without a device on the desk, and for anyone working below the screen stack.

## Getting the keys and the display

- **The package name decides everything.** The device's screen reader yields the braille keys to an
  app only when its package is `com.extra.han.*`, by the name alone, and the launcher lists such
  apps in its Programs menu. Under any other name the screen reader keeps the keys.
- **Typing is the app's own.** For such a package the screen reader steps aside from typing too: a
  focused Android text field gets raw key events, not text. The stock edit box lives inside HIMS'
  own apps and no service offers one, so the SDK edits cells itself (`TextBox`).
- **The braille keys come from `han_brailleinput`'s callback** (`HanInputClient`), registered while
  the app is in front and unregistered as it leaves. Answering a key "taken" keeps it from the
  window; a key left as not taken arrives again as a key event with no device and the chord as its
  scan code, which `BrailleSenseSession.keyEvent` swallows.
- **Hold them only while in front.** Something drawn over the app without pausing it (the F1-F4
  task list, Global Options, an alert) takes only the window's focus, and the keys have to go with
  it. `Claim` holds the keys and the display only while the activity is resumed and its window has
  focus, gives them back first in `onPause` on the calling thread, and answers a key that arrives
  late "not taken": answering "taken" for a key nothing will act on leaves the person with a
  keyboard that does nothing.
- **`setActiveOutputService` is one switch for the whole device, never an app's claim.** Every HIMS
  app checks it before it speaks or writes a cell. Turning it off as you leave silences the launcher
  and every other app and leaves your last line on the display. Never send `false`; the SDK sends
  `true` as the app comes to the front, which also repairs a device someone else left off. What an
  app shows is simply overwritten by the next app's first write.

## Speech and the cells

- **Speech** is the output service's `displayAndPlayTTS` in mode 1 with an empty braille field: it
  plays in the person's own voice settings, is silent with their voice off, and a new line cuts off
  the last. Mode 2 is accepted and never sounds; a null braille field is refused.
- **The cells take dots, not text.** `displayBraille` takes Braille ASCII, letter for letter, so text
  sent as text never reaches the person's table. The SDK translates every line with liblouis in the
  person's own table and grade, read from the settings provider, and writes the dot patterns.
- **The service interfaces are ours, under the vendor's descriptors.** The device's framework has its
  own `com.jawon.han.*` classes, which win over a bundled copy of the same name and are blocked to
  apps, so the SDK's AIDL lives in its own package with the vendor's descriptors, and its parcel is
  written field for field. The transaction order is the service's and must not change.
- **Panning is the app's.** The SDK breaks lines into windows at spaces, as the device's Word Wrap
  does, and measures the display's width from the service (40 cells on the unit tried).
- **Routing is a touch.** The BrailleSense 7 has no routing keys: its display is touch-sensitive, and
  a cell touched with either front Cursor button held routes there (again for each cell the finger
  slides over). With Gestures on and Double-Tap to Route Cursor set, a double tap routes too. Both
  arrive on the keys' callback as gestures, which `Chords.decode` turns into `Key.Route`. In an edit
  box routing only moves the cursor and says nothing, as the device's own boxes do.

## The keys

- **Space with dots is a command, Enter with dots a hot key, dots alone a cell** (`Chords`): Space-1
  and Space-4 up and down, Space-1-2-3 and Space-4-5-6 the ends, Space-1-2-6 and Space-3-4-5 a page,
  Space-E Escape, Space-Z Exit, Space-M the menu, Space-R and Space-C repeat, Space-H help, Space-F
  find, Space-D delete, Space-7-8 silence speech.
- **The four keys long read as scroll keys are function keys**: F1 to F4. F2 is the menu, F3 Tab, F4
  Escape, F1 with F2 Page Up, F3 with F4 Page Down, Space-F3 Shift-Tab. F1 by itself, F1 with F4,
  F2 with F3, and either with a letter belong to the system. The real scroll keys are separate, and
  have a held form each.
- **Tab is Space-4-5 or F3, Shift-Tab Space-1-2 or Space-F3.** In a dialog they are all that move
  between controls: the dialog swallows the arrows, so Space-1 and Space-4 move only within a control
  that is a list.
- **Nothing on the device folds a row**: no built-in app has a tree. The SDK's Space-3 folds and
  Space-6 unfolds, which only an edit box otherwise uses (a character left and right).
- **Enter-I and the QWERTY keyboard's Application key are a row's actions.** The manual names only
  these, on the Android side.
- **The QWERTY keyboard sends ordinary key events**, mapped by the manual's table (`Qwerty`): Alt
  pressed and let go by itself is the menus, as F2 is; Alt with another key is that key's own.

## Global Options

They are one set of values for the whole device, in
`content://com.selvashc.settingsprovider/braille_settings` as name and value. The stock widgets in
HIMS' apps follow them for nothing; an app that draws its own cells must read and apply each one,
which `BrailleSettings` does as the app comes to the front and as the provider says they changed
(some have a hot key that works while the app is in front).

| Setting | Values | What it does |
|---|---|---|
| `BrailleCode`, `ViewInputGrade`, `EightDotMode` | | The person's braille table, grade and eight-dot braille. |
| `KeyboardEcho` | 0 off, 1 characters and words, 2 characters, 3 words (4 and 5 as 1 and 2) | What typing says. What Backspace removes is said whatever it is. |
| `ControlInformation` | 0 off, 1 before, 2 after | Where a control's role goes on the cells. Only French, Italian, Danish and Japanese models offer After; elsewhere 2 reads as Before, as the stock controls treat it. |
| `VoiceControlInformation` | 0 off, 1 before, 2 after (default) | Where the role goes in speech. |
| `AnnounceShortcutKeys` | 0 off, 1 on (default), 2 braille only | Whether a menu item's letter is said and shown. |
| `HotKeyInformation` | 0 off, 1 letters (default), 2 dot patterns, 3 braille only | How a hot key is said ("Enter-n", or "Enter-dots 1-3-4-5") and shown (`ent-n`). |
| `SupportShortCut` | 1 (default) a letter opens or runs its item, 0 it only goes to it | "Automatically Open Menus with Shortcut". |
| `IndexReporting` | 1 on (default), 0 off | Whether a row says its place, "3/12", after its text and ahead of its role. |
| Message Display Time | | How long a message stays on the cells before the line comes back. |
| `GestureDoubleTapCursor` | | Whether a double tap routes. |

## How the device reads a control

`deviceReading` composes a line as the device does:

- **Each control has a role**, a word for the voice and a symbol for the cells: menu `mn` (said
  "Menu", a submenu), menu item `mi`, list item `li`, edit box `eb`, multi-line edit box `meb`,
  computer edit box `ce`, edit combo box `ecb`, combo box `cb`, checked and unchecked check box
  `schb` and `uchb`, radio button `rb`, button `bt`, prompt button `pb`, static box `st`, link `ln`.
  Symbols are shown cell for cell, never in the person's contractions.
- **A menu item** reads its name, its letter in brackets, its hot key, then the sign of what it
  opens. Spoken, a submenu adds "pulldown" and an item that opens a dialog "dialog", ahead of the
  role: "File (f) pulldown Menu", "Rename (r) dialog menu item". On the cells a submenu ends in ` 3o`
  and a dialog in `"""`. The brackets are the code's: UEB `"<` and `">`, US `7` and `7`, the others
  `(` and `)`.
- **In a dialog** the title is said with "dialog" as it opens ("Find dialog"); a control reads its
  label, its value, and "2/5" while Index Reporting is on. Space and Backspace change a combo box, a
  check box or a prompt button.
- **A menu**: Space-1 and Space-4 move and wrap, Enter opens or runs, an item's letter jumps to it
  (and runs it, by `SupportShortCut`), Backspace or Escape goes up a level and at the top closes it
  saying Cancel, and the menu key closes it all.
- **One menu per screen, on Space-M or F2.** In File Manager, Email and the Media Player that menu is
  a menu bar of submenus; in the To-Do List, the Address Manager and the launcher a flat list of
  commands. None of them has a separate context menu: a row's actions are items of the one menu,
  acting on the row in hand, and the menu's contents change with the screen.

## The stock edit box

`TextBox` and `BoxEditing` edit as the stock box does:

- **A line is held in cells as typed**, the caret a cell: Backspace and Delete take one cell, a
  routing touch lands on the cell under it, and the line shows what was typed. It is read back as
  text only when asked, a word at a time in the person's table, so editing one word never rewords
  the rest.
- **A cell by itself is said as the stock box says it**, never back-translated alone: inside a word
  by its letters ("s t", never "still"), a cell with no letters by its dots ("dot six"), after a
  number sign a to j as digits, and a lower cell ending its word as its punctuation in contracted
  braille.
- **Moving**: Space-3 and Space-6 a character, Space-2 and Space-5 a word, Space-1-3 and Space-4-6
  the line's ends, Space-1-2-3 and Space-4-5-6 the box's; each says what it lands on. Space-3-6 says
  the character at the cursor, Space-2-5 the word, Space-C the line.
- **Several lines**: Enter starts a line, Backspace at a line's start joins it to the one before
  ("previous line"), Space-1 and Space-4 move by line. Tab and Shift-Tab always leave the box.
- **Selection and the clipboard**: Enter-B starts a selection, Enter-A selects all, Enter-C copies,
  Enter-X cuts, Enter-P appends to the clipboard, Enter-V pastes, Enter-D clears the clipboard,
  Backspace-1-2 reads the selection, Enter-U its first line. With a selection, Backspace or Space-D
  asks "Delete selected text?" as Yes and No prompt buttons. The selection is shown by a blinking
  cursor, not its extent.
- **Deleting**: Space-D the character at the cursor, Backspace-2-5 the word, Backspace-1-4 the line
  the display shows, Backspace-1-3-4-6 or Backspace-2-3-5-6 the whole line.
- **A new braille table** (the G chord, or Global Options) rewrites every open box in the new table,
  its text kept.
