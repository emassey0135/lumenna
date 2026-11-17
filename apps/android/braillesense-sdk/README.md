# BrailleSense SDK (unofficial)

An Android library for apps that speak and write braille themselves on the **HIMS BrailleSense 7**.
With it an app takes the braille keys and the snap-on QWERTY keyboard while it is in front, speaks
in the person's own voice, and puts cells on the display in their braille table. Its lists, menus,
edit boxes and dialogs read, move and echo like the device's own, following the person's Global
Options.

It was worked out from the device, its manual and its built-in apps. HIMS did not write it and does
not support it. The device's services are reached through interfaces written here to the device's
own descriptors; none of HIMS' code is included.

There is a sibling SDK for the Orbit Speak and Orbit Player with the same screen stack, so an app's
screens can be written once for both (see [One app, two devices](#one-app-two-devices)).

## What an app needs

- **A package named `com.extra.han.*`.** The device's screen reader yields the braille keys only to
  such a package, by its name alone, and the launcher lists such apps in its Programs menu. Under
  any other name an app gets no braille keys.
- **Android 15, arm64**: every BrailleSense 7. The library's `minSdk` is 28.
- **The Android NDK**, to build liblouis. The library's `build-louis.sh` downloads liblouis' release,
  checks its hash and builds `libbraillesense-louis.so` with the NDK's clang, and gathers its tables
  into the library's assets; Gradle runs it before each build, and your APK merges both. It finds
  the newest NDK under your SDK, or the one `ANDROID_NDK_HOME` names (CI uses 30.0.16248370).

## Adding it to a build

There is no published artifact yet. Add the repository to your project as a git submodule (or copy
it), and include it as a module:

```kotlin
// settings.gradle.kts
include(":braillesense-sdk")
project(":braillesense-sdk").projectDir = file("braillesense-sdk")

// app/build.gradle.kts
dependencies {
    implementation(project(":braillesense-sdk"))
}
```

Its build file takes two plugin aliases from your version catalog, `android-library` (the Android
Gradle plugin, 9.x) and the `junit` library for its tests; add them if your catalog lacks them (see
this repository's `gradle/libs.versions.toml`). It applies Dokka by its own version.

On its own the repository is a Gradle build too: the library at the root and `sample/`.

## Quick start

`sample/` is this app, built by CI: a list of fruit, with Add on the screen's menu and Delete on
each row's.

```kotlin
class MainActivity : Activity() {
    private val session by lazy { BrailleSenseSession(this, BuildConfig.VERSION_NAME) }
    private lateinit var ui: BrailleSenseUi
    private val fruit = mutableListOf("Apple", "Banana", "Cherry")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        ui = BrailleSenseUi.on(session)
        ui.onEmpty = ::finish
        session.created()
        ui.list(
            "Fruit",
            items = {
                fruit.map { name ->
                    Item(name, name, open = { ui.message(name) }, actions = listOf(Action("Delete", 'd', asks = true) { remove(name) }))
                }
            },
            actions = { listOf(Action("Add", 'a', asks = true) { add() }) },
        )
    }

    private fun add() = ui.launch {
        val name = ui.askLine("Name") ?: return@launch
        val size = ui.choose("Size", listOf("Small", "Large")) ?: return@launch
        val added = "$name, ${if (size == 0) "small" else "large"}"
        fruit += added
        ui.land(added)                 // moved to what was made, and said
    }

    private fun remove(name: String) = ui.launch {
        if (ui.confirm("Delete $name?")) {
            fruit.remove(name)
            ui.announce("Deleted")
            ui.refresh()
        }
    }

    override fun onResume() { super.onResume(); session.resumed() }
    override fun onPause() { session.paused(); super.onPause() }   // first thing
    override fun onWindowFocusChanged(hasFocus: Boolean) { super.onWindowFocusChanged(hasFocus); session.windowFocus(hasFocus) }
    override fun onDestroy() { session.destroyed(); super.onDestroy() }
    override fun dispatchKeyEvent(event: KeyEvent): Boolean = session.keyEvent(event) || super.dispatchKeyEvent(event)
}
```

`BrailleSenseSession` is the device for one activity: it reads the person's Global Options and
follows them, reaches the output service, installs liblouis' tables, and turns both keyboards into
`Key`s on the main thread, all held only while the activity is resumed and its window has focus.
`BrailleSenseUi.on(session)` puts the screen stack over it. The SDK depends on nothing but Android:
`launch` runs the `suspend` asks without a coroutine library (use your own scope if you have one).

## The screen stack

`BrailleSenseUi` is a stack of screens that takes the keys and does all the speaking and showing,
much as BTSpeak's `dialogs` module does for a BTSpeak app: open a list, then ask with a menu, a
line, a yes-or-no question, a choice or a form. Each ask gives its answer to a callback, or returns
it from a `suspend` twin, exactly once, after its screen has closed: so what the app does next is
said after the screen under it.

| | |
|---|---|
| `list(title, items, empty, actions, moved, find)` | A screen of `Item`s, read again from `items` at each `refresh`. `actions` is the screen's menu (Space-M or F2). `moved` hears the person's moves; `find` takes a first letter for an app that finds rows itself. Returns the `ListScreen`. |
| `menu(items)` | The device's menu of `MenuItem`s (submenus, letters, hot keys, the dialog sign). Gives the chosen item's `value`, or null after saying Cancel. |
| `askLine(prompt, initial, hint, multiLine)` | An edit box with the stock box's keys. Gives its text at Enter, or null at Escape. Several lines are a form of one field with OK and Cancel. |
| `confirm(question, default)` | Yes and No as the device's prompt buttons; Space and Backspace step between them. Escape is No. |
| `choose(title, options, selected)` | A list to pick one of with Enter. Gives its place, or null. |
| `form(title, fields)` | A dialog of `Field.Line`, `Field.Choice`, `Field.Check` and `Field.Text`, then OK and Cancel, said "… dialog" as it opens. Tab and Shift-Tab move between controls, each read in its role. Gives `FormValues` at OK (`text(key)`, `choice(key)`, `checked(key)`, which throw for a key no field has), or null. |
| `refresh()` | Your data changed: screens that no longer `stand` close, and the screen in front is read again, the row in hand kept by its key. Nothing is said unless the person's own key asked for the change (a check box, folding), so a sync never cuts off what is being read. |
| `land(key)` | You moved what is in hand yourself (a row made, a row gone to): a refresh that puts `key` in hand and says the line. |
| `announce(text)`, `message(text)` | What you have to say of a change: said with the line that follows it, as one thing, and kept on the cells for the person's Message Display Time, since someone reading braille alone hears none of it. |
| `refused(text)` | Something asked for that cannot be done here, and why. |
| `batch { … }` | Pushes and closes that say nothing until the block ends; then only where the person ends up is said. |
| `push`, `close`, `top`, `screens`, `onEmpty` | The stack itself. `onEmpty` is called as the last screen closes (Escape on the first): finish the activity. Exit (Space-Z) calls `onExit`, by default the same. |

An `Item` is `Item(key, text, open, actions, letter, detail, expanded, fold, toggle, delete, title)`
and then this device's own `braille`, `finding`, `command`, `role` and `asks`:

- **Enter** or a routing key runs `open`; **Enter-I** or the Application key opens its `actions`;
  Space or Enter with its **`letter`** runs `open` from anywhere in the list.
- **Space** runs `toggle` (a check box) and **Space-D** `delete`.
- A row that folds says whether it is `expanded`: **Space-6** unfolds a folded row and **Space-3**
  folds an unfolded one, through `fold(open)`. What `toggle` and `fold` change is said once your
  `refresh` shows it.
- `detail` is read after the text, ahead of the row's number ("3/12"), which the device adds while
  Index Reporting is on.

What every screen shares is the stack's, as the device's own apps have it: Space-R repeats the line;
the scroll keys pan, and move on to the next row at a line's end; a touch with a Cursor button
routes (in an edit box it only moves the cursor); an edit box takes the stock box's keys, and asks
"Delete selected text?" as a prompt; Space-M or F2 opens the screen's menu; Escape closes the screen
in front, or answers an ask with nothing.

`UiWords` holds the few words the stack says itself (OK, Cancel, Yes, No, "No items", "Nothing on
x"…), in the device's English; pass your own for another language. A screen of your own subclasses
`Screen`: its `frame()`, and where it wants them `heading`, `menu()`, `back()` (Escape), `letter()`
(Space or Enter with a letter nothing took), `stands()` (closed at the next refresh when false) and
`closed()`.

## One app, two devices

The Orbit SDK (`dev.emassey0135.orbit.ui.OrbitUi`) has the same stack: `list`, `menu`, `askLine`,
`confirm`, `choose`, `form` and their `suspend` twins, `refresh`, `land`, `announce`, `message`,
`refused`, `batch`, `launch`, `push`, `close`, `top`, `screens` and `onEmpty`, and the same `Item`
(up to `title`), `Action`, `MenuItem` (up to `asks`), `Field`, `FormValues` and the common
`UiWords`. Screens written against those build against either by changing the imports; what
differs is the activity's wiring (a `BrailleSenseSession` here, an `OrbitSession` there) and each
device's own extras.

## Below the stack

An app can work the frames and the display itself:

| Package | |
|---|---|
| `session` | `BrailleSenseSession`, and `Claim` and `KeyDelivery`: holding the keys and display only while resumed and focused, and moving a key from the binder thread to the app's. |
| `input` | `Key`, what a press means on any screen, named by the manual's commands; `Chords.decode` (a braille-key response, the touch cursor's routing included) and `Chords.letter(dots)`; `Qwerty` and `QwertyKeyboard`, the snap-on keyboard; `HanInputClient`. |
| `output` | `HanOutputClient` (an `Output`): speech in the person's own voice settings and raw cells; `BrailleDisplay`: `say`, `silence`, `show` a `DisplayLine` in the person's table, panning with word wrap, `routed`, `selecting`, `cursorLine`. |
| `braille` | `BrailleSettings`, the person's Global Options; `BrailleTables`, `Louis` and its `Translator`. |
| `reading` | `deviceReading(DeviceLine, LineSettings, BrailleSigns)`: a control as the device reads it, its role's word and symbol (`ControlRole`), letter, hot key, sign and place, where the person's settings put them. |
| `ui` | `Frame`s that take their keys and give what to say and show (`Rendered`): `ListFrame` of `Row`s, `MenuFrame` of `MenuEntry`s, `LineFrame`, `DialogFrame`; `Letters` and `DeviceLetters`; `Readings`; `Messages`. |
| `edit` | `TextBox` and `LineEditor`, an edit box held in cells as typed; `BoxEditing`, the stock box's keys and what it says for each; `CellEcho`, `EditWords`, `Clipboard`, `SystemClipboard`. |

A list without the stack, drawn and said by the app itself:

```kotlin
val frame = ListFrame(
    "fruit", "Fruit", listOf(Row("a", "Apple"), Row("b", "Banana")), selected = null,
    empty = "Nothing here", letters = DeviceLetters, readings = SettingsReadings(session.settings),
)
session.onKey = { key ->
    if (frame.key(key)) {
        session.display.show(frame.rendered)
        session.display.say(frame.rendered.speech)
    }
}
```

## Testing

The JVM tests (`./gradlew test`) cover the chords, the claim, panning, the cell echo, reading a
line, the edit box, the lists and menus, the letters, and the screen stack and its asks. Test your
own screens the same way: build `BrailleSenseUi` over a `BrailleDisplay` of an `Output` that records
what is said, a `Translator` of your own (liblouis is native), and `speechDelay = 0`, then `press`
keys and look at what was said. `src/test/.../UiTest.kt` does exactly this.

On a device:

- **Inject a chord** from the shell: `adb shell service call han_brailleinput 1 i32 <chord> i32 0 i32 0`,
  the chord as `Chords` has its bits (Space is `0x2000`, dot 1 `0x1000`…).
- **See what was said and shown**: `BrailleSenseSession(activity, version, trace = true, tag = "MyOutput")`
  logs every line sent to the output service at info level, under the tag.

[docs/DEVICE.md](docs/DEVICE.md) holds what the device does that an app has to know: its keys, its
Global Options and how it reads a control.

## API documentation

`./gradlew dokkaGenerate` writes it to `build/dokka/html`; CI keeps it as an artifact.

## Licence

Apache License 2.0: see [LICENSE](LICENSE) and [NOTICE](NOTICE). liblouis, which the SDK builds and
loads as a separate shared library, is under the LGPL 2.1 or later, and its tables under their own
licences.
