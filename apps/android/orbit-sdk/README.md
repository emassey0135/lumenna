# Orbit SDK (unofficial)

An unofficial Android library for apps that speak for themselves on Orbit Research's Android
devices, the **Orbit Speak** (braille keys) and the **Orbit Player** (a phone keypad). With it an
app hears the keypad, tells Orbit's own screen reader what is on its screen so that Orbit voices it
in the person's voice and verbosity, and gives the person edit boxes, lists, menus and dialogs that
work the way Orbit's own apps do.

It was worked out by trying the devices and reading their user guides. Orbit Research did not write
it and does not support it. Nothing in it is copied from Orbit's APKs. The T9 input and the
Player's list finding are ported from the Satellite launcher (Apache-2.0, by the same author).

It lives in Lumenna's repository for now, as the Gradle module `:orbit-sdk` (package
`dev.emassey0135.orbit`). It depends on nothing but Android, so it can move to a repository of its
own. It has no published release yet: use it as a module of your own build.

## What a device needs

- **Satellite** (its launcher and hook). Orbit sends the keypad to an app only when the app says it
  speaks for itself. Satellite's hook does that for any app whose manifest has:

  ```xml
  <application …>
      <meta-data android:name="orbit.self_voicing" android:value="true" />
  </application>
  ```

- **Root**, for reading Orbit's settings (the braille table, the selection mode, the typing echo).
  Orbit's settings provider only answers apps signed by Orbit, and the screen reader keeps its
  settings in its own data, so `OrbitSettings` reads them through `su`, as Orbit's own SDK does.
  Magisk ships on every Speak and Player. If root is refused, `BrailleInput.failed` says so once,
  and typing goes on in the table last read.
- Android 11 (minSdk 30), arm64. Your package should not start with `com.orbit.` or contain
  `inputmethod`, which Orbit's key routing reads otherwise.

## Getting started

```kotlin
class MainActivity : ComponentActivity(), OrbitReader.Listener {
    private val main = Handler(Looper.getMainLooper())
    private lateinit var reader: OrbitReader
    private lateinit var braille: BrailleInput
    private val decoder = KeyDecoder()
    private val keys by lazy { KeyDelivery<Pair<KeyEvent, Boolean>>(reader.claim, { main.post(it) }) { (event, phone) -> pressed(event, phone) } }

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        reader = OrbitReader(this, this)
        reader.claim.guardCrashes()   // a crash gives the keypad back
        braille = BrailleInput(this)  // the Speak's liblouis service; unavailable on the Player
        reader.bind()
        braille.bind()
    }

    override fun onResume() { super.onResume(); braille.follow(); reader.claim.resumed() }
    override fun onPause() { reader.claim.paused(); super.onPause() }
    override fun onWindowFocusChanged(hasFocus: Boolean) { super.onWindowFocusChanged(hasFocus); reader.claim.windowFocus(hasFocus) }
    override fun onDestroy() { reader.unbind(); braille.unbind(); super.onDestroy() }

    // From Orbit, on binder threads.
    override fun readerReady() {
        main.post { reader.appOpened("My App", ""); describe() }
    }
    override fun key(event: KeyEvent, phoneKeypad: Boolean): Boolean = keys.deliver(event to phoneKeypad)
    override fun settingsChanged() = braille.refresh()

    private fun pressed(event: KeyEvent, phoneKeypad: Boolean) {
        when (val key = decoder.decode(event)) { /* … */ }
    }
}
```

Nothing reaches Orbit before `readerReady`. Orbit calls it at once if it is already up, and
speech sent before it is dropped. The keypad is the app's only while the activity is resumed, its
window has focus and the reader is up (`reader.claim`). A key that arrives at any other moment is
answered "not taken", so it goes on to whatever has the keypad.

## The API

### Talking to Orbit

| | |
|---|---|
| `OrbitReader` | Orbit's screen reader. The app describes its screen and Orbit voices it: `appOpened`, `focusList`, `itemChanged`, `dialog`/`dialogClosed`, `contextMenu`/`menuOpened`/`focusMenu`/`contextMenuClosed`, `focusEdit`, `focusControl`, `focusCheck`, `focusChoice`, `focusText`, `characterTyped`, `editChanged`. It can also `speak`, `speakQueued`, `stop`, play Orbit's `sound`s (`SOUND_ERROR`…) and `vibrate`. `Listener` hears that the reader is ready, the keys, and settings changing. `claim` holds the keypad only while the app is in front. |
| `Claim`, `KeyDelivery` | Holding the keypad while resumed and focused, and moving a key from the binder thread to the app's. |
| `ServiceLink` | One bound service, each call made in order on one worker thread, and bound again when the service dies. |
| `DebugKeys` | Keys pressed over `adb shell am broadcast`, in a debuggable build. Nothing else can press the keypad remotely. |

### Keys

`KeyDecoder.decode(KeyEvent)` turns the keypad callback into a `Key`. The Speak's keys are named
from its user guide:

- **Moving:** Space with dots 1 or 4 is `Up` or `Down`. Space with dots 4 5 or 1 2 is `Tab` or
  `BackTab`. Space with L or dots 4 5 6 is `First` or `Last`.
- **Escape and the menu:** Space with E or Z is `Escape`. Space with M is `Menu`.
- **Dots:** `Dot7` and `Dot8` on their own. Dots alone are a `Cell`, Space with dots is a `Chord`,
  and Dot 8 with dots is `Enter`.

The Player gives `Digit`, `Star`, `Pound`, `Back` and `Menu`, each with a held form. Pressing and
releasing a held key counts once.

### Widgets, as Orbit's own apps have them

| | |
|---|---|
| `EditBox` | An edit box like Orbit's fields: Speak cells translated a word at a time in the person's table, the Player's multi-tap T9, moving by character, word and line, and the Editor's selection and clipboard. It reports to the reader through an `Output`; `ReaderEditOutput` is the ready-made one, using the system clipboard. `CellBuffer` and `T9Input` sit underneath it. |
| `OrbitMenu` | A menu like those in Orbit's Editor and File Manager: one list of items and submenus. Up and Down wrap round, Right or Select open a submenu, Left goes back, a letter is an access key, and choosing closes the menu. `OrbitReader.focusMenu(menu)` tells Orbit about the item in hand. Orbit has one menu key, so a row's actions and the screen's commands share one menu. |
| `OrbitList` | How a list moves: Up and Down stop at the ends, Tab and Shift-Tab wrap round, and Space with L or dots 4 5 6 go to the ends. `target(key, index, size)` gives the row to move to. Tell the reader with `focusList` and `itemChanged`. |
| `ListFinder` | The Player's way of finding a row. A digit opens that row; 1 then 2 typed quickly is the twelfth, and 0 alone the tenth. Holding star switches to finding by letter with T9. `ListFinder.next` finds the next row by first letter, for the Speak's cells or T9's letters. |
| `OrbitDialog` | What a key does in a dialog, as in the Editor's Save As. Tab and Shift-Tab move between controls. Up and Down change a combo box or a list box, or toggle a check box. Select presses. Space with E or Z, or Dot 7, cancels. Left and Right never leave. Tell the reader about each control in its role (see the class's documentation). |

### Braille input and settings

| | |
|---|---|
| `BrailleInput` | The Speak's liblouis service, in the person's table and selection mode (contracted, uncontracted or computer braille), with their typing echo. Call `follow` as a line is reached and as the app comes back. Orbit's G chord changes the table without telling other apps, so the service is asked which table it holds. `changed` and `failed` report what happened. `available` is false on the Player. |
| `OrbitSettings`, `Root` | Orbit's settings read through root, and the root helpers. A refusal is a `RootFailure`, never a default taken in silence. |

### Words

`OrbitWords` holds what the device says: a field's echo of cells and deletions, the Editor's
selection and clipboard, T9's modes, and what a root failure says. `CellNames` holds the names of
cells with no letters (the capital sign, the number sign…). Both default to the stock English of
Orbit's own build, so an app sounds like the device. Pass your own for another language.

## Tests

```
./gradlew :orbit-sdk:testDebugUnitTest
```

The JVM tests cover the key decoder, the claim, the cell buffer, the edit box, T9, the menu, the
list moves, the finder, the dialog keys, the cell names and reading the settings.

## Licence

AGPL-3.0-or-later, as the rest of Lumenna, while it lives here.
