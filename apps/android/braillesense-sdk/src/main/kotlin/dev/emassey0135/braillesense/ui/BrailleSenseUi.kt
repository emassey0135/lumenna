package dev.emassey0135.braillesense.ui

import android.os.Handler
import android.os.Looper
import dev.emassey0135.braillesense.braille.Echo
import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.braillesense.edit.BoxEditing
import dev.emassey0135.braillesense.edit.Clipboard
import dev.emassey0135.braillesense.edit.EditDisplay
import dev.emassey0135.braillesense.edit.EditWords
import dev.emassey0135.braillesense.edit.Edited
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.output.BrailleDisplay
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.dialogTitle
import dev.emassey0135.braillesense.session.BrailleSenseSession
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine

/**
 * A stack of screens that takes the keys and does the saying and showing itself, so an app is
 * written as a BTSpeak app is with BTSpeak's dialogs: `list` a screen of rows, `menu`, `askLine`,
 * `confirm`, `choose` and `form` ask and give the answer back (to a callback, or from a `suspend`
 * call), and `message` says something on the voice and the cells.
 *
 * What is common to every screen is here, as the device's own apps have it: Space-R repeats the
 * line, the scroll keys pan (and move on at the end), a routing key routes, an edit box's keys are
 * the device's box's (`BoxEditing`, its "Delete selected text?" asked as a prompt), Space-M or F2
 * opens the screen's menu, Escape closes the screen in front (an ask closed so answers nothing), and
 * Exit leaves the app. A screen newly in front is said with its heading; a [refresh] after the app's
 * data changed shows the line again and says nothing, the row in hand kept by its key, so a change
 * underneath never talks over the person. Everything runs on the main thread.
 */
class BrailleSenseUi(
    private val display: BrailleDisplay,
    private val translator: Translator,
    clipboard: Clipboard,
    private val readings: Readings,
    /** The person's Keyboard Echo: `BrailleSettings.echo`. */
    echo: () -> Echo = { Echo.of(1) },
    /** The person's Message Display Time: `BrailleSettings.messageMillis`. */
    messageMillis: () -> Long = { 3_000L },
    /** Automatically Open Menus with Shortcut: `BrailleSettings.opensOnLetter`. */
    private val opensOnLetter: () -> Boolean = { true },
    private val letters: Letters = DeviceLetters,
    private val words: UiWords = UiWords(),
    editWords: EditWords = EditWords.ENGLISH,
    handler: Handler = Handler(Looper.getMainLooper()),
) {
    private val editing = BoxEditing(translator, clipboard, echo, editWords)
    private val stack = ArrayList<Screen>()

    /** The frame each screen shows, kept until the screen is refreshed. */
    private val frames = HashMap<Screen, Frame>()
    private val messages = Messages(display, messageMillis, under = { frame?.rendered }, handler = handler)

    /** Counts what is shown, so a key that opened or closed a screen does not say the old one after. */
    private var shown = 0

    /** The last screen closed: leave the app (`finish`). */
    var onEmpty: () -> Unit = {}

    /** Exit (Space-Z): leave the app, as the device's own apps do. */
    var onExit: () -> Unit = { onEmpty() }

    /** A key no screen took and the layer has nothing for (Help, Options, Find…): the app's. */
    var onUnhandled: (Key) -> Unit = {}

    /** The line in front, for a sighted helper's screen. */
    var drawn: (String) -> Unit = {}

    /** The screen in front. */
    val top: Screen? get() = stack.lastOrNull()

    private val frame: Frame? get() = top?.let { frames[it] }

    /** Puts [screen] in front, and says it with its heading. */
    fun push(screen: Screen) {
        stack += screen
        show(newly = true)
    }

    /** Takes [screen] off the stack wherever it is; the row in hand of the one now in front is said again. */
    fun close(screen: Screen) = close(screen, said = true, prefix = "")

    private fun close(screen: Screen, said: Boolean, prefix: String) {
        val wasTop = top === screen
        if (!stack.remove(screen)) return
        frames.remove(screen)
        screen.closed()
        when {
            stack.isEmpty() -> onEmpty()
            // Back where the person was: its row, not its heading, as the device returns from a dialog.
            wasTop -> show(newly = false, said = said, prefix = prefix)
        }
    }

    /** The app's data changed: the screen in front is built again and shown, nothing said. */
    fun refresh() {
        top?.let { frames.remove(it) }
        show(newly = false, said = false)
    }

    /** Back in front (the session's `onTaken`): says where the person is. */
    fun front() = show(newly = true)

    /** The person's settings changed while in front: the line shown as they now say, unsaid. */
    fun redraw() = show(newly = false, said = false)

    /** Before the settings are read again: every open box's text read in the table in force. */
    fun settle() = frames.values.forEach { frame -> frame.editBoxes().forEach { it.settle() } }

    /** The braille table changed: every open box written again in the new one, its text kept. */
    fun retable() {
        frames.values.forEach { frame -> frame.editBoxes().forEach { it.recell() } }
        frame?.let { display.show(it.rendered) }
    }

    /** Says [text]. */
    fun say(text: String) = display.say(text)

    /** Says [text] and keeps it on the cells for the Message Display Time, then the line again. */
    fun message(text: String) {
        display.say(text)
        messages.show(text)
    }

    /** Shows the screen in front; [said] says its line, after its heading if [newly] in front and after [prefix]. */
    private fun show(newly: Boolean, said: Boolean = true, prefix: String = "") {
        val screen = top ?: return
        val shownFrame = frames.getOrPut(screen) { screen.frame() }
        shown++
        val rendered = shownFrame.rendered
        if (!messages.up) display.show(rendered)
        display.selecting(shownFrame.editBox()?.selecting == true)
        drawn(rendered.speech)
        if (!said) return
        display.say(listOf(prefix, if (newly) screen.heading else "", rendered.speech).fold("", ::sentences))
    }

    /** [next] after [said], as sentences. */
    private fun sentences(said: String, next: String): String = when {
        next.isBlank() -> said
        said.isBlank() -> next
        said.last() in ".?!" -> "$said $next"
        else -> "$said. $next"
    }

    // ── Keys ─────────────────────────────────────────────────────────────────────────────

    /** A key from either keyboard: the session's `onKey`. */
    fun press(key: Key) {
        val current = frame ?: return
        when (key) {
            Key.Silence -> return display.silence()
            // Outside a box the device's read word and read character are what Space with their dots is.
            Key.SayCharacter, Key.SayWord -> if (current.editBox() == null) {
                return press(Key.SpaceDots(if (key == Key.SayWord) SAY_WORD_DOTS else SAY_CHARACTER_DOTS))
            }
            // A multi-line box reads the cursor's line itself; anything else is read whole.
            Key.Repeat -> if (current.editBox()?.multiLine != true) {
                messages.clear()
                val rendered = current.rendered
                display.say(rendered.speech)
                display.show(rendered)
                return
            }
            Key.ScrollOn -> {
                messages.clear()
                if (!display.scrollOn() && current !is DialogFrame) step(Key.Down)
                return
            }
            Key.ScrollBack -> {
                messages.clear()
                if (!display.scrollBack() && current !is DialogFrame) step(Key.Up)
                return
            }
            is Key.Route -> {
                messages.clear()
                val at = display.routed(key.cell)
                if (at >= 0) {
                    val before = shown
                    current.routed(at)
                    // In an edit box the cursor only moves, as in the device's own: nothing is said.
                    if (shown == before) show(newly = false, said = current.editBox() == null)
                }
                return
            }
            else -> {}
        }
        display.silence()
        current.editBox()?.let { box -> if (edit(current, box, key)) return }
        val before = shown
        if (current.key(key)) {
            messages.clear()
            if (shown == before) show(newly = false)
            return
        }
        when (key) {
            Key.Menu -> openMenu()
            Key.Escape -> top?.let { close(it) }
            Key.Exit -> onExit()
            else -> onUnhandled(key)
        }
    }

    private fun step(key: Key) {
        frame?.key(key)
        show(newly = false)
    }

    private fun edit(current: Frame, box: TextBox, key: Key): Boolean {
        val shows = object : EditDisplay {
            override val width: Int get() = display.cellsWide()

            override fun cursorLine(): IntRange? {
                // The line as the display shows it at the cursor, not a message over it.
                messages.clear()
                display.show(current.rendered)
                return display.cursorLine()
            }
        }
        when (val edited = editing.press(box, key, shows)) {
            Edited.NotTaken -> return false
            is Edited.Taken -> told(current, edited)
            is Edited.Ask -> push(PromptScreen(edited.question, default = false, quiet = true) { yes -> told(current, edited.answer(yes == true)) })
        }
        return true
    }

    private fun told(current: Frame, edited: Edited.Taken) {
        if (edited.changed) current.edited()
        messages.clear()
        show(newly = false, said = false)
        edited.said?.let(display::say)
    }

    private fun openMenu() {
        val items = top?.menu().orEmpty()
        if (items.isEmpty()) return say(words.nothingHere)
        menu(items) { run -> run?.invoke() }
    }

    // ── Asks ─────────────────────────────────────────────────────────────────────────────

    /** A screen of rows, read again from [items] at each [refresh]; Space-M is [actions]. */
    fun list(
        title: String,
        items: () -> List<Item>,
        empty: String = words.empty,
        actions: () -> List<Action> = { emptyList() },
    ): ListScreen = ListScreen(title, items, empty, actions).also(::push)

    /** The device's menu of [items]: [chosen] gets the value of the one chosen, or null if it closed unchosen. */
    fun <T> menu(items: List<MenuItem<T>>, chosen: (T?) -> Unit) {
        val screen = MenuScreen(chosen)
        screen.entries = items.map { screen.entry(it) }
        push(screen)
    }

    suspend fun <T> menu(items: List<MenuItem<T>>): T? = suspendCoroutine { done -> menu(items) { done.resume(it) } }

    /** A line to type: [answered] gets its text at Enter, or null at Escape. Several lines are a form of one field. */
    fun askLine(prompt: String, initial: String = "", hint: String = "", multiLine: Boolean = false, answered: (String?) -> Unit) {
        if (multiLine) return form(prompt, listOf(Field.Line(LINE, prompt, initial, hint, true))) { answered(it?.text(LINE)) }
        push(LineScreen(prompt, initial, hint, answered))
    }

    suspend fun askLine(prompt: String, initial: String = "", hint: String = "", multiLine: Boolean = false): String? =
        suspendCoroutine { done -> askLine(prompt, initial, hint, multiLine) { done.resume(it) } }

    /** A question of Yes and No as the device's prompt asks one: Escape is No. */
    fun confirm(question: String, default: Boolean = false, answered: (Boolean) -> Unit) =
        push(PromptScreen(question, default, quiet = false) { answered(it == true) })

    suspend fun confirm(question: String, default: Boolean = false): Boolean =
        suspendCoroutine { done -> confirm(question, default) { done.resume(it) } }

    /** A list of [options] to pick one of with Enter: [chosen] gets its place, or null at Escape. */
    fun choose(title: String, options: List<String>, selected: Int = 0, chosen: (Int?) -> Unit) =
        push(ChoiceScreen(title, options, selected, chosen))

    suspend fun choose(title: String, options: List<String>, selected: Int = 0): Int? =
        suspendCoroutine { done -> choose(title, options, selected) { done.resume(it) } }

    /** A form of [fields] with OK and Cancel: [done] gets what it holds at OK, or null at Cancel or Escape. */
    fun form(title: String, fields: List<Field>, done: (FormValues?) -> Unit) = push(FormScreen(title, fields, done))

    suspend fun form(title: String, fields: List<Field>): FormValues? = suspendCoroutine { done -> form(title, fields) { done.resume(it) } }

    // ── The screens ──────────────────────────────────────────────────────────────────────

    /**
     * A screen that asks: its answer is given once, after it has closed, so what the app does with
     * it (another screen, a message) is said after the screen under it; closed unanswered, null.
     */
    private abstract inner class Asking<T>(private val answered: (T?) -> Unit) : Screen() {
        private var finished = false

        /** Closes with [value]; [said] says the screen now in front, with [prefix] ahead of it. */
        fun finish(value: T?, said: Boolean = true, prefix: String = "") {
            if (finished) return
            finished = true
            close(this, said, prefix)
            answered(value)
        }

        override fun closed() {
            if (finished) return
            finished = true
            answered(null)
        }
    }

    /** A list of rows, read again from [items] at each refresh. */
    inner class ListScreen internal constructor(
        override val title: String,
        private val items: () -> List<Item>,
        private val empty: String,
        private val actions: () -> List<Action>,
    ) : Screen() {
        private var selected: String? = null

        override fun frame(): Frame {
            val shownItems = items()
            val rows = shownItems.map { item ->
                Row(
                    item.key, item.text, finding = item.text, open = item.open, letter = item.letter, detail = item.detail,
                    actions = item.actions.map { MenuEntry(it.name, letter = it.letter, asks = it.asks, run = it.run) },
                )
            }
            return ListFrame(
                "list:${System.identityHashCode(this)}", title, rows, selected, empty, letters,
                moved = { selected = it.key },
                openActions = { row -> push(MenuScreen<Unit> {}.also { it.entries = row.actions }) },
                notFound = { say(words.notFound) },
                readings = readings,
                byLetter = { letter -> shownItems.firstOrNull { it.letter == letter }?.open?.let { it(); true } ?: false },
                opensNothing = { say(words.nothingHere) },
                noActions = { say(words.nothingHere) },
            ).also { frame -> frame.current?.let { selected = it.key } }
        }

        override fun menu(): List<MenuItem<() -> Unit>> = actions().map { MenuItem(it.name, it.letter, value = it.run, asks = it.asks) }
    }

    /** A menu, as the device's own: closing it unchosen says Cancel. */
    private inner class MenuScreen<T>(chosen: (T?) -> Unit) : Asking<T>(chosen) {
        var entries: List<MenuEntry> = emptyList()
        override val title: String get() = ""
        private var menuFrame: MenuFrame? = null

        fun entry(item: MenuItem<T>): MenuEntry = MenuEntry(
            item.name, children = item.children.map(::entry), hotkey = item.hotKey, letter = item.letter, asks = item.asks,
            run = { finish(item.value, said = false) },
        )

        override fun frame(): Frame = menuFrame ?: MenuFrame(entries, letters, readings, opensOnLetter, notFound = { say(words.notFound) }, closed = { cancelled ->
            // Chosen, the item's run finishes it; left unchosen, the device says Cancel ahead of the line.
            if (cancelled) finish(null, prefix = words.cancel)
        }).also { menuFrame = it }
    }

    /** A line to type, as the device's edit box, taken at Enter. */
    private inner class LineScreen(prompt: String, initial: String, hint: String, answered: (String?) -> Unit) : Asking<String>(answered) {
        override val title: String = prompt

        // The box reads its prompt as its label already.
        override val heading: String get() = ""

        private val lineFrame = LineFrame(
            "line:${System.identityHashCode(this)}", prompt, TextBox(initial, translator), hint, readings,
            done = { text -> finish(text) },
        )

        override fun frame(): Frame = lineFrame
    }

    /**
     * Yes and No, as the device's prompt buttons: Space and Backspace step between them. [quiet]
     * goes back to what it was asked over without saying it again.
     */
    private inner class PromptScreen(
        override val title: String,
        default: Boolean,
        private val quiet: Boolean,
        answered: (Boolean?) -> Unit,
    ) : Asking<Boolean>(answered) {
        private var selected: String? = if (default) YES else NO

        override fun frame(): Frame {
            val rows = listOf(YES to words.yes, NO to words.no).map { (key, word) ->
                Row(key, word, role = ControlRole.PROMPT_BUTTON, open = { finish(key == YES, said = !quiet) })
            }
            return ListFrame("prompt:${System.identityHashCode(this)}", title, rows, selected, "", letters, moved = { selected = it.key }, readings = readings, prompt = true)
        }
    }

    /** One of [options], picked with Enter. */
    private inner class ChoiceScreen(override val title: String, private val options: List<String>, first: Int, chosen: (Int?) -> Unit) : Asking<Int>(chosen) {
        private var selected: String? = first.toString()

        override fun frame(): Frame {
            val rows = options.mapIndexed { at, option -> Row(at.toString(), option, open = { finish(at) }) }
            return ListFrame(
                "choice:${System.identityHashCode(this)}", title, rows, selected, words.empty, letters,
                moved = { selected = it.key }, notFound = { say(words.notFound) }, readings = readings,
            )
        }
    }

    /** A form, as the device's own dialog: said with "dialog" as it opens. */
    private inner class FormScreen(override val title: String, fields: List<Field>, done: (FormValues?) -> Unit) : Asking<FormValues>(done) {
        override val heading: String get() = dialogTitle(title)

        private val dialog = DialogFrame("form:${System.identityHashCode(this)}", title, fields, translator, words, letters, readings) { values ->
            finish(values)
        }

        override fun frame(): Frame = dialog
    }

    companion object {
        /** Space-2-5 and Space-3-6, the device's read word and read character, as dots. */
        private const val SAY_WORD_DOTS = 0x12
        private const val SAY_CHARACTER_DOTS = 0x24
        private const val LINE = "line"
        private const val YES = "yes"
        private const val NO = "no"

        /**
         * The layer over [session], which it is wired into: the session's keys come here, and coming
         * to the front, the settings changing and the table changing are handled.
         */
        fun on(session: BrailleSenseSession, clipboard: Clipboard, words: UiWords = UiWords(), letters: Letters = DeviceLetters): BrailleSenseUi {
            val settings = session.settings
            val ui = BrailleSenseUi(
                session.display, session.translator, clipboard, SettingsReadings(settings),
                echo = { settings.echo }, messageMillis = { settings.messageMillis }, opensOnLetter = { settings.opensOnLetter },
                letters = letters, words = words,
            )
            session.onKey = ui::press
            session.onTaken = ui::front
            session.beforeSettings = ui::settle
            session.onTableChanged = ui::retable
            session.onSettingsChanged = ui::redraw
            return ui
        }
    }
}
