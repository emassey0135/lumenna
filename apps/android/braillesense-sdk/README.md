# BrailleSense SDK (unofficial)

An unofficial Android library for apps that speak and write braille themselves on the **HIMS
BrailleSense 7**. With it an app takes the braille keys and the snap-on QWERTY keyboard while it is
in front. It speaks in the person's voice and puts cells on the display in their braille table. Its
lists, menus and edit boxes read, move and echo like the device's own, following the person's
Global Options.

It was worked out from the device, its manual and its built-in apps. HIMS did not write it and does
not support it. The vendor's interfaces are reached only through our own AIDL copies; nothing of
HIMS' code ships in it.

It lives in Lumenna's repository for now, as the Gradle module `:braillesense-sdk` (package
`dev.emassey0135.braillesense`). It depends on nothing but Android, so it can move to a repository
of its own. It has no published release yet: use it as a module of your own build.

## What an app needs

- **A package named `com.extra.han.*`.** The device's screen reader yields the braille keys only to
  such a package, by its name alone, and the launcher lists such apps in its Programs menu. Under
  any other name an app gets no braille keys.
- Android 15 (every BrailleSense 7; the library's minSdk is 28), arm64.
- The NDK, to build liblouis. The module's `build-louis.sh` builds it (`libbraillesense-louis.so`)
  and gathers its tables into the module's assets. Your APK merges both.

## Getting started

`BrailleSenseSession` does an activity's wiring. It holds the keys and display only while the
activity is resumed and focused, reads the person's Global Options and follows changes to them,
installs liblouis' tables, and turns both keyboards into `Key`s.

`BrailleSenseUi` is the easiest way to write an app. It is a stack of screens that takes the keys
and does all the speaking and showing, much as BTSpeak's `dialogs` module does. You open a list,
and ask with a menu, a line, a yes-or-no question, a choice or a form. Each ask gives its answer
to a callback, or from a `suspend` call.

```kotlin
class MainActivity : ComponentActivity() {
    private val device by lazy { BrailleSenseSession(this, BuildConfig.VERSION_NAME) }
    private lateinit var ui: BrailleSenseUi
    private val fruit = mutableListOf("Apple", "Banana")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        ui = BrailleSenseUi.on(device, SystemClipboard(this, "My app"))
        ui.onEmpty = ::finish
        device.created()
        ui.list("Fruit", items = {
            fruit.mapIndexed { at, name ->
                Item(name, name, open = { ui.message(name) }, actions = listOf(Action("Delete", 'd') { remove(at) }))
            }
        }, actions = { listOf(Action("Add", 'a') { add() }) })
    }

    private fun add() = lifecycleScope.launch {
        val name = ui.askLine("Name") ?: return@launch
        val size = ui.choose("Size", listOf("Small", "Large")) ?: return@launch
        fruit += "$name, ${if (size == 0) "small" else "large"}"
        ui.refresh()
    }

    private fun remove(at: Int) = lifecycleScope.launch {
        if (ui.confirm("Delete ${fruit[at]}?")) {
            fruit.removeAt(at)
            ui.refresh()
        }
    }

    override fun onResume() { super.onResume(); device.resumed() }
    override fun onPause() { device.paused(); super.onPause() }   // first thing
    override fun onWindowFocusChanged(hasFocus: Boolean) { super.onWindowFocusChanged(hasFocus); device.windowFocus(hasFocus) }
    override fun onDestroy() { device.destroyed(); super.onDestroy() }
    override fun dispatchKeyEvent(event: KeyEvent) = device.keyEvent(event) || super.dispatchKeyEvent(event)
}
```

This is what the stack does for every screen, as the device's own apps do:

- **Coming to a screen:** a screen newly in front is said with its heading: a list's title, or a
  form's title with "dialog".
- **Refreshing:** `ui.refresh()` after your data changes shows the line again and says nothing. The
  row in hand stays on the same row, found by its key.
- **Repeating and panning:** Space-R repeats the line. The scroll keys pan, and move on to the next
  row at the end of a line.
- **Routing:** a routing key routes; in an edit box it only moves the cursor.
- **Edit boxes:** an edit box takes the stock box's keys. When there is a selection, Backspace asks
  "Delete selected text?" as a prompt.
- **Menus:** Space-M or F2 opens the screen's menu, and Enter-I a row's actions.
- **Leaving:** Escape closes the screen in front. An ask closed that way answers null, or No for a
  question. Exit (Space-Z) calls `onExit`, which by default closes the app.

The asks:

| | |
|---|---|
| `list(title, items, empty, actions)` | A screen of `Item`s, each with its key, text, Enter (`open`), its actions (`Action`s, Enter-I) and a letter that runs it. `items` is read again at each `refresh`. `actions` is the screen's Space-M menu. |
| `menu(items)` | The device's menu of `MenuItem`s: submenus, letters, hot keys, the dialog sign. Gives the chosen item's `value`. Closed unchosen, it says Cancel and gives null. |
| `askLine(prompt, initial, hint, multiLine)` | An edit box. Gives its text at Enter, or null at Escape. Several lines are a form of one field with OK and Cancel. |
| `confirm(question, default)` | Yes and No as the device's prompt buttons; Space and Backspace step between them. Escape is No. |
| `choose(title, options, selected)` | A list to pick one of with Enter. Gives its place, or null. |
| `form(title, fields)` | A dialog of `Field.Line`, `Field.Choice`, `Field.Check` and `Field.Text`, then OK and Cancel. Tab and Shift-Tab move between controls, and each control is read in its role. Gives `FormValues` at OK, or null. |
| `message(text)` | Says it and keeps it on the cells for the Message Display Time. |

`UiWords` holds the few words the stack says itself (OK, Cancel, Yes, No…), in the device's
English; pass your own for another language. A screen of your own subclasses `Screen` and gives its
`frame()`.

Below the stack, an app can work the frames and the display itself. Here is the same activity
without it:

```kotlin
class MainActivity : ComponentActivity() {
    private val device by lazy { BrailleSenseSession(this, BuildConfig.VERSION_NAME) }
    private val letters = DeviceLetters
    private val readings by lazy { SettingsReadings(device.settings) }
    private var frame: Frame? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        device.onKey = ::press
        device.onTaken = { show(said = true) }
        device.onSettingsChanged = { show(said = false) }
        device.created()
        frame = ListFrame("fruit", "Fruit", listOf(Row("a", "Apple"), Row("b", "Banana")), null, "Nothing here", letters, readings = readings)
    }

    override fun onResume() { super.onResume(); device.resumed() }
    override fun onPause() { device.paused(); super.onPause() }   // first thing
    override fun onWindowFocusChanged(hasFocus: Boolean) { super.onWindowFocusChanged(hasFocus); device.windowFocus(hasFocus) }
    override fun onDestroy() { device.destroyed(); super.onDestroy() }
    override fun dispatchKeyEvent(event: KeyEvent) = device.keyEvent(event) || super.dispatchKeyEvent(event)

    private fun press(key: Key) {
        when (key) {
            is Key.Route -> device.display.routed(key.cell).let { at -> if (at >= 0) frame?.routed(at) }
            Key.ScrollOn -> { device.display.scrollOn(); return }
            Key.ScrollBack -> { device.display.scrollBack(); return }
            else -> if (frame?.key(key) != true) return
        }
        show(said = true)
    }

    private fun show(said: Boolean) {
        val rendered = frame?.rendered ?: return
        device.display.show(rendered)
        if (said) device.display.say(rendered.speech)
    }
}
```

Something drawn over the app without pausing it (the F1-F4 task list, Global Options, an alert)
takes only the window's focus, and the keys go with it. Giving them back always comes first, on the
calling thread, so nothing of the app's lands after the next app has started writing.

## The API

### `session`: the device for one activity

| | |
|---|---|
| `BrailleSenseSession` | Covers everything below for one activity: `settings`, `output`, `translator`, `display`, `claim`, and the `onKey`, `onTaken`, `onReleased`, `beforeSettings`, `onTableChanged` and `onSettingsChanged` callbacks. The activity calls `created`, `resumed`, `paused`, `windowFocus`, `keyEvent` and `destroyed`. |
| `Claim`, `KeyDelivery` | Holding the keys and display only while resumed and focused, and moving a key from the binder thread to the app's. |

### `input`: keys

| | |
|---|---|
| `Key` | What a press means on any screen: `Up`, `Down`, `PageUp`, `LineStart`, `WordLeft`, `Enter`, `Escape`, `Tab`, `Menu`, `Context`, `Delete`, `DeleteWord`, `Route(cell)`, `Cell(dots)`, `SpaceLetter('x')`, `EnterLetter('x')`, `Typed(text)`… named by the manual's commands. |
| `Chords` | `decode` turns a braille-key response, routing gestures included, into a `Key`. `letter(dots)` gives a cell's letter. |
| `Qwerty`, `QwertyKeyboard` | The snap-on keyboard's key events, from the manual's table. Alt pressed and released on its own is the menu. |
| `HanInputClient` | Registers for the braille keys while the app is in front. |

### `output`: speech and the cells

| | |
|---|---|
| `HanOutputClient` (an `Output`) | Speech in the person's own voice settings, and raw cells, through the device's output service. |
| `BrailleDisplay` | `say`, `silence`, and `show` a `DisplayLine` in the person's table between its `BraillePart`s. Also panning (`scrollOn`, `scrollBack`, with word wrap as the device does it), `routed`, `selecting` (the selection cursor) and `cursorLine`. |
| `Cells` | The panning itself. |

### `braille`: settings and liblouis

| | |
|---|---|
| `BrailleSettings` | The person's Global Options from the device's settings provider: tables, eight-dot, Message Display Time, how a control is read (`LineSettings`), braille signs (`BrailleSigns`), Keyboard Echo (`Echo`), double-tap routing, and Automatically Open Menus with Shortcut. Nothing applies these for an app that draws its own cells, which is why the SDK reads them. |
| `BrailleTables`, `Louis`, `Translator` | liblouis in the person's table and grade. |

### `reading`: how the device reads a control

`deviceReading(DeviceLine, LineSettings, BrailleSigns)` composes a control the way the device
reads it. Each `ControlRole` has a spoken word and a symbol on the cells (`mn`, `mi`, `li`, `eb`,
`cb`…). It also adds the shortcut letter, the hot key, the sign for a submenu or a dialog, and
"3/12". Where the role goes in speech and on the cells follows the person's settings.

### `ui`: the screen stack, and the device's lists, menus, lines and dialogs

| | |
|---|---|
| `BrailleSenseUi`, `Screen` | The screen stack and its asks (above). `BrailleSenseUi.on(session, clipboard)` wires it into a session. |
| `Frame`, `Rendered` | One thing on the display that takes its keys. `rendered` is what to say and show; `BrailleDisplay.show(Rendered)` puts it on the cells. |
| `ListFrame`, `Row` | A list. Space-1 and Space-4 move and stop at the ends; Space-1-2-3 and Space-4-5-6 go to them; Space-1-2-6 and Space-3-4-5 move by 32. A letter finds the next row. Enter or a routing key opens a row, Space-3 and Space-6 fold it, Space-D deletes it, and Enter-I or the Application key opens its actions. A row's number is read with it. A question's answers become prompt buttons that Space and Backspace step through. |
| `MenuFrame`, `MenuEntry` | The device's menus, read as `mn File (f) 3o` and said "File (f) pulldown Menu". Space-1 and Space-4 wrap round. Enter opens or runs. An item's letter jumps to it and runs it, or only jumps, as the person's setting has it. Backspace or Escape goes up a level, and the menu key closes the menu. A hot key an item shows runs it from anywhere in the menu. |
| `LineFrame` | A line being typed, with its prompt, whose keys are the edit box's (`BoxEditing`). |
| `Letters`, `DeviceLetters` | A key's letter, finding by first letter, and each menu item's letter. `DeviceLetters` follows the device's own rules; pass your own `Letters` for other rules. |
| `Readings`, `SettingsReadings`, `line` | How a frame's line is read: the device's reading in the person's settings, or a test's own. |
| `DialogFrame`, `Field`, `FormValues` | A form's dialog of fields, then OK and Cancel, as `form` shows it. |
| `Messages` | A message on the cells for the person's Message Display Time, then the line under it again. Someone reading braille alone hears none of what is said, so what an app says about a change belongs on the cells too. |

### `edit`: the device's edit box

| | |
|---|---|
| `TextBox`, `LineEditor` | An edit box held in cells as they were typed, the caret on a cell, as the stock box does. A word is read back as text only when asked, so an edit never rewords the rest of a field. It can be one line or several. |
| `BoxEditing` | The stock box's keys and what it says for each. It covers moving and reading, Keyboard Echo, deleting by character, word or line, selection and the clipboard, and the "Delete selected text?" question (`Edited.Ask`, which the app asks its own way). |
| `CellEcho`, `EditWords` | How the stock box says a cell or a character on its own, and its words (`EditWords.ENGLISH`). |
| `Clipboard`, `SystemClipboard` | Where copies go. |

## Tests

```
./gradlew :braillesense-sdk:testDebugUnitTest
```

The JVM tests cover the chords, the claim, panning, the cell echo, reading a line, the edit box,
the lists and menus, the device's letters, and the screen stack and its asks.

## Licence

AGPL-3.0-or-later, as the rest of Lumenna, while it lives here.
