# Orbit SDK (unofficial)

An unofficial Android library for apps that speak for themselves on Orbit Research's Android
devices: the **Orbit Speak**, with braille keys, and the **Orbit Player**, with a phone keypad.
Neither has a screen. With this library an app hears the keypad, tells Orbit's own screen reader
what is on its screen so that Orbit voices it in the person's own voice and verbosity, and gives
the person lists, menus, edit boxes and dialogs that work the way Orbit's own apps do.

You write an app as a stack of screens, much as a BTSpeak app is written with BTSpeak's `dialogs`:
show a list, ask for a line, a yes or no, a choice or a form, and get the answer back in a callback
or from a `suspend` call. The stack takes the keys and does all the telling.

Orbit Research did not write this library and does not support it. It was worked out from how the
devices behave, their user guides, and study of how their own apps use the device's services.
Nothing in it is copied from Orbit's apps. The T9 input and the Player's list finding are ported
from the Satellite launcher (Apache-2.0, by the same author).

The library depends on nothing but Android. The BrailleSense SDK, for the HIMS BrailleSense 7, has
the same screen stack under the same names, so an app's screens can be written once for both.

## What a device needs

- **Satellite**, its launcher and hook. Orbit sends the keypad to an app only when the app speaks
  for itself, and Satellite's hook does that for an app whose manifest says so:

  ```xml
  <application …>
      <meta-data android:name="orbit.self_voicing" android:value="true" />
  </application>
  ```

  Satellite's launcher then lists the app beside Orbit's own.
- **A package name Orbit routes keys to**: not starting with `com.orbit.`, and not containing
  `inputmethod`.
- **Root**, for reading Orbit's settings (the braille table, the selection mode, the typing echo).
  Magisk ships on every Speak and Player. If root is refused, the person is told once and typing
  goes on in the table last read.
- Android 11 (the library's minSdk is 30), arm64.

[docs/DEVICE.md](docs/DEVICE.md) has what the devices do that an app must fit: the keys each keeps
for itself, how the screen reader takes what it is told, what it says on its own.

## Adding it to your build

There is no published release yet. Add the library to your project as a module:

1. Put this repository in your project, as a git submodule for example:

   ```
   git submodule add https://github.com/emassey0135/orbit-sdk.git orbit-sdk
   ```

2. Include it in your `settings.gradle.kts`:

   ```kotlin
   include(":orbit-sdk")
   ```

3. Depend on it from your app's `build.gradle.kts`:

   ```kotlin
   dependencies {
       implementation(project(":orbit-sdk"))
   }
   ```

Its `build.gradle.kts` uses two aliases from your version catalog, `libs.plugins.android.library`
and `libs.junit`, as this repository's own `gradle/libs.versions.toml` defines them. Add them to
yours if it lacks them. Gradle reads only your project's settings file, so this repository's
`settings.gradle.kts` is ignored there.

To build the library on its own instead, run `./gradlew assembleRelease` here; the AAR is in
`build/outputs/aar/`.

## Quick start

[`sample/`](sample/src/main/kotlin/dev/emassey0135/orbit/sample/MainActivity.kt) is a complete app:
a list of notes to add, edit, pin and delete. Its heart is this:

```kotlin
class MainActivity : Activity() {
    private lateinit var session: OrbitSession
    private lateinit var ui: OrbitUi
    private val notes = mutableListOf("Buy milk", "Call the bank")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        session = OrbitSession(this, "Notes")
        ui = OrbitUi.on(session)
        ui.onEmpty = ::finish
        ui.list(
            "Notes",
            items = { notes.map { note -> Item(note, note, open = { edit(note) }, delete = { delete(note) }) } },
            empty = "No notes",
            actions = { listOf(Action("New note", 'n') { newNote() }) },
        )
        session.created()
    }

    private fun newNote() = ui.launch {
        val text = ui.askLine("New note") ?: return@launch
        notes += text
        ui.land(text)
    }

    private fun edit(note: String) = ui.launch {
        val text = ui.askLine("Note", initial = note) ?: return@launch
        notes[notes.indexOf(note)] = text
        ui.refresh()
    }

    private fun delete(note: String) = ui.launch {
        if (ui.confirm("Delete $note?")) {
            notes.remove(note)
            ui.refresh()
            ui.announce("Deleted")
        }
    }

    override fun onResume() { super.onResume(); session.resumed() }
    override fun onPause() { session.paused(); super.onPause() }   // first thing
    override fun onWindowFocusChanged(hasFocus: Boolean) { super.onWindowFocusChanged(hasFocus); session.windowFocus(hasFocus) }
    override fun onDestroy() { session.destroyed(); super.onDestroy() }
}
```

`OrbitSession` is the device for one activity: Orbit's screen reader, the Speak's braille input,
and the keypad, which is the app's only while the activity is resumed, its window has focus and
the reader is up. `OrbitUi.on(session)` builds the stack and wires it in. Nothing is said before
Orbit's reader is ready; the stack tells the screen on top then.

`ui.launch` runs the `suspend` asks in turn without a coroutine library. With one, call them from
`lifecycleScope.launch` instead; every call is on the main thread.

## The screen stack

`OrbitUi` (package `dev.emassey0135.orbit.ui`). The screen on top takes each key; the stack does
what it leaves, and tells Orbit's reader what is shown, as Orbit's own apps tell it.

### Showing and asking

Each ask has a callback form and a `suspend` twin, and answers exactly once: null (false for
`confirm`) when its screen is closed by Back or Escape. It answers after its screen has closed, so
what the app does next is told after the screen under it.

| | |
|---|---|
| `list(title, items, empty, actions, moved, find)` | A list of `Item`s, read again from `items` at each `refresh`. `empty` is said when it has no rows; `actions` is the screen's own menu, after the row's; `moved` hears the person move; `find` takes a letter alone, for an app that finds rows by a rule of its own. Returns the `ListScreen`. |
| `menu(items)` | A menu of `MenuItem`s, submenus and all. Gives the `value` of the item chosen. |
| `askLine(prompt, initial, hint, multiLine)` | An edit box as Orbit's own. Gives its text when confirmed. |
| `confirm(question, default)` | A dialog of Yes and No, `default` in hand first. |
| `choose(title, options, selected)` | A list of options. Gives the place of the one picked. |
| `form(title, fields)` | A dialog of `Field.Line`, `Field.Choice`, `Field.Check` and `Field.Text`, then OK and Cancel. Gives `FormValues` at OK (`text(key)`, `choice(key)`, `checked(key)`). |

### When the app's data changes

| | |
|---|---|
| `refresh()` | Read the screen on top again. The row in hand stays on the same row, found by its key, and nothing is said unless that row moved, or the person's own action reworded it (a check box ticked). A screen whose `stands()` is false closes. |
| `land(key)` | The app moved what is in hand itself, to `key`: a refresh that tells the row, unless it was just told. |
| `announce(text)`, `message(text)` | What the app says of a change. Orbit's speech jumps the queue of what it is told, so this waits a moment behind what was just told, in order. |
| `refused(text)` | Orbit's error tone, then why. |
| `batch { … }` | Push and close several screens; only the one left on top is told, as the block ends. |
| `push`, `close`, `top`, `screens`, `insert`, `replace` | The stack itself: a screen of your own on top, one taken off, one put above another or in another's place. |
| `onEmpty` | Called when the last screen closes: finish the activity. |

### The rows

`Item(key, text, open, actions, letter, detail, expanded, fold, toggle, delete, title)`, all but
the first two optional:

| | |
|---|---|
| `key`, `text` | What names the row across a refresh, and what Orbit says of it. |
| `open` | Select, Dot 8 and Right. |
| `actions` | Its own `Action`s, first on the menu key's menu while it is in hand. |
| `letter` | Space (or Dot 8) with it opens the row from anywhere in the list. |
| `detail` | Said after the text, with a comma: its state, its count. |
| `expanded`, `fold` | A row that folds: Right unfolds it, Left folds it. |
| `toggle` | Select on a row with no `open`: a check box's change, said once the app has made it. |
| `delete` | Space with D deletes it. |
| `title` | Its name alone, for the menu of its actions. |

`Action(name, letter, asks, run)` is something the menu key offers; `asks` (it opens a dialog) is
the BrailleSense's mark and changes nothing on Orbit. `MenuItem(name, letter, children, value)` is
an item of `menu`.

### The keys, as Orbit's own apps have them

- **Lists**: Up and Down stop at the ends; Tab and Shift-Tab go round; Space with L or with dots
  4 5 6 go to the first or last row. A letter alone moves to the next row starting with it.
- **Back**: Left (on a row that does not fold), Escape (Space with E or Z), Dot 7 and the Player's
  Back close the screen, unless the screen takes it (`Screen.back`). Back from the last screen
  calls `onEmpty`.
- **The menu key** (Space with M, the Player's Menu) opens one menu: the row in hand's actions,
  then the screen's. Orbit has one menu key for both. A screen may give a menu bar of its own
  instead (`Screen.menuBar`), with hot keys that run from anywhere in it.
- **Letters with Space**: Space with a letter runs the action of that letter, the row's first,
  else says nothing is on it. Orbit keeps Space with some letters for itself (see
  [DEVICE.md](docs/DEVICE.md)); Dot 8 with the letter does the same, and an app's own `Letters`
  can refuse Space for those letters and say to use Dot 8.
- **Dialogs** on the Speak are Orbit's: Tab and Shift-Tab between controls, each told in its role;
  Up and Down change a combo box or check box; Select presses.
- **On the Player**, which has no Tab and no letters, a digit opens that row and holding star
  finds by letter with T9, in lists and menus alike. A form's controls are the rows of a list.
  Lines are typed with multi-tap T9.

### Screens of your own

Subclass `Screen`, or `ListScreen` for a list. A screen says what it is (`tellShown`) and what moved
(`tellMoved`, `refresh`, `land`), may take keys first (`key`), Back (`back`) and Space with a letter
(`letter`), names its menu (`actions`, `inHandActions`, `menuBar`), says when what it shows has
arrived (`loaded`), and whether it still stands (`stands`). `ListScreen` adds the row the app holds
in hand (`inHand`), the person's moves (`moved`) and finding by letter (`findLetter`).

`UiWords` holds the few words the stack says itself, in English by default; pass your own for
another language. `Letters` says which key presses a letter.

### The same shape as the BrailleSense SDK

`BrailleSenseUi` in the BrailleSense SDK has the same members under the same names: `list`, `menu`,
`askLine`, `confirm`, `choose`, `form`, `refresh`, `land`, `announce`, `message`, `refused`,
`batch`, `launch`, `onEmpty`, and `Item`, `Action`, `MenuItem`, `Field`, `FormValues` and `UiWords`'
common words. An app's screen code compiles against either by changing its imports; the session
and the activity's wiring are each device's own.

## Below the stack

For an app that draws its screens its own way.

| | |
|---|---|
| `OrbitReader` | Orbit's screen reader. An app describes its screen and Orbit voices it: `appOpened`, `focusList`, `itemChanged`, `dialog`, `menuOpened`/`focusMenu`/`contextMenuClosed`, `focusEdit`, `focusControl`, `focusCheck`, `focusChoice`, `focusText`, `characterTyped`, `editChanged`. It also speaks, plays Orbit's sounds and vibrates. `Reader` is the same as an interface, so what is built over it can be tested with one that records. |
| `Claim`, `KeyDelivery` | Holding the keypad only while the activity is in front, and moving a key from the binder thread to the app's. |
| `KeyDecoder`, `Key` | The keypad callback as a key: the Speak's chords named by its user guide, the Player's digits, star, pound, Back and Menu. `BrailleLetters` gives a cell's letter. |
| `EditBox` | An edit box like Orbit's fields: cells translated a word at a time in the person's table on the Speak, multi-tap T9 on the Player, moving by character, word and line, and the Editor's selection and clipboard. It tells the reader through an `Output`; `ReaderEditOutput` is the ready-made one. `CellBuffer` and `T9Input` are underneath. |
| `OrbitMenu` | A menu like those of Orbit's Editor and File Manager: one list of items and submenus, Up and Down round a level, Right and Select into a submenu, Left back out, a letter an access key. |
| `OrbitList`, `ListFinder` | How a list moves, and the Player's finding of a row by number or by letter. |
| `OrbitDialog` | What a key does in a dialog, as in the Editor's Save As. |
| `BrailleInput`, `OrbitSettings`, `Root` | The Speak's liblouis service in the person's table and selection mode, and Orbit's settings read through root. |
| `OrbitWords`, `CellNames` | What the device says in its own words: the fields' echo, the Editor's selection, T9's modes. English by default. |
| `DebugKeys` | Orbit's keys pressed over adb, in a debuggable build. |

## Testing

The JVM tests run without a device:

```
./gradlew testDebugUnitTest
```

Build an `OrbitUi` over a `Reader` that records what it is told, and press keys with `ui.key`, as
[`src/test/kotlin/dev/emassey0135/orbit/UiTest.kt`](src/test/kotlin/dev/emassey0135/orbit/UiTest.kt)
does. `delay` and `now` can be given too, so what waits behind a description runs when the test
says.

On a device, nothing can press Orbit's keypad over adb, so name a broadcast for `OrbitSession`'s
`debugKeys` and a debuggable build takes keys from it:

```
adb shell input keyevent KEYCODE_WAKEUP
adb shell am broadcast -a <your action> --es name DOWN --ei action 0 --ez long false --ez phone false
```

Wake the device first, or the window has no focus and the keypad is not the app's. What Orbit said
is in its log: `adb shell setprop log.tag D`, then look for `olympus_SpeechManager`'s
`sequence items` in `adb logcat`.

## API reference

```
./gradlew dokkaGenerate
```

writes it to `build/dokka/html`. CI keeps it with each build.

## Licence

Apache License 2.0: see [LICENSE](LICENSE) and [NOTICE](NOTICE).
