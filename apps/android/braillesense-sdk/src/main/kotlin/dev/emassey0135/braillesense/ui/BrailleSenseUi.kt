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
 * call), and `announce` says what the app did.
 *
 * What is common to every screen is here, as the device's own apps have it: Space-R repeats the
 * line, the scroll keys pan (and move on at the end), a routing key routes, an edit box's keys are
 * the device's box's (`BoxEditing`, its "Delete selected text?" asked as a prompt), Space or Enter
 * with a letter runs what has it, Space-M or F2 opens the screen's menu, Escape goes back (the
 * screen's own [Screen.back], else closing it; the last closed leaves the app), and Exit leaves.
 *
 * What is said follows what is shown, so nothing talks over the person unasked. A frame newly in
 * front is said with its heading; a screen returned to, by its line. After the app's data changes,
 * [refresh] shows the line again and says it only where the app moved what is in hand (`landed`) or
 * the line changed after the person asked it to (a check box, folding). What the app [announce]s
 * is said with the line that follows it, as one thing, and kept on the cells for the Message
 * Display Time. A key's line is said at once; one the app shows of itself waits [speechDelay] for
 * what it says to come with it. Everything runs on the main thread.
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
    private val handler: Handler = Handler(Looper.getMainLooper()),
    /** How long a line the app shows of itself waits for what it says; 0 says at once (a test). */
    private val speechDelay: Long = SPEECH_DELAY,
) {
    private val editing = BoxEditing(translator, clipboard, echo, editWords)
    private val stack = ArrayList<Screen>()

    /** The frame each screen shows, kept until the screen is refreshed. */
    private val frames = HashMap<Screen, Frame>()
    private val messages = Messages(display, messageMillis, under = { frame?.rendered }, handler = handler)

    /** Counts what is shown, so a key that opened or closed a screen does not say the old one after. */
    private var shown = 0

    /** The frame last shown and what it said, which tell a new frame and a reworded line. */
    private var lastId = ""
    private var lastSpeech = ""

    /** A key asked the app to change the line: said once it has. */
    private var awaitingChange = false

    /** Inside a key: what it shows is said when it is done, at once. */
    private var inKey = false

    /** What the app said, to go with the next line said. */
    private val announcements = ArrayList<String>()

    /** The line to say, and the heading of a screen just opened, until they are said. */
    private var pendingLine: String? = null
    private var pendingHeading: String? = null
    private val saying = Runnable { flush() }

    /** The last screen closed: leave the app (`finish`). */
    var onEmpty: () -> Unit = {}

    /** Exit (Space-Z): leave the app, as the device's own apps do. */
    var onExit: () -> Unit = { onEmpty() }

    /** A key no screen took and the stack has nothing for (Help, Options, Find…): the app's. */
    var onUnhandled: (Key) -> Unit = {}

    /** The line in front, for the device's LCD and an HDMI screen: what a sighted helper reads. */
    var drawn: (String) -> Unit = {}

    /** The screen in front. */
    val top: Screen? get() = stack.lastOrNull()

    /** The frame in front. */
    val frame: Frame? get() = top?.let { frames[it] }

    /** The screens, bottom first. */
    val screens: List<Screen> get() = stack.toList()

    /** Puts [screen] in front, and says it with its heading. */
    fun push(screen: Screen) {
        stack += screen
        show(moved = true)
    }

    /** Takes [screen] off the stack wherever it is; the line of the one now in front is said again. */
    fun close(screen: Screen) = close(screen, said = true, prefix = "")

    private fun close(screen: Screen, said: Boolean, prefix: String) {
        val wasTop = top === screen
        if (!stack.remove(screen)) return
        frames.remove(screen)
        screen.closed()
        when {
            stack.isEmpty() -> onEmpty()
            wasTop -> {
                if (prefix.isNotBlank()) announcements += prefix
                // Back where the person was: its line, not its heading, as the device returns from a dialog.
                show(moved = said, returning = true, silent = !said)
            }
        }
    }

    /**
     * The app's data changed: what no longer [Screen.stands] closes, and the screen in front is
     * built again and shown. Its line is said only if [landed] (the app moved what is in hand),
     * if it is a new frame, or if a key asked for the change that it now shows.
     */
    fun refresh(landed: Boolean = false) {
        val gone = stack.filter { !it.stands() }
        val wasTop = top
        gone.forEach { screen ->
            stack.remove(screen)
            frames.remove(screen)
            screen.closed()
        }
        if (stack.isEmpty()) {
            if (gone.isNotEmpty()) onEmpty()
            return
        }
        top?.let { frames.remove(it) }
        show(moved = false, landed = landed, returning = top !== wasTop)
    }

    /** Back in front (the session's `onTaken`): says where the person is, heading and all. */
    fun front() {
        lastId = ""
        show(moved = true)
    }

    /** The person's settings changed while in front: the line shown as they now say, unsaid. */
    fun redraw() = show(moved = false)

    /** Before the settings are read again: every open box's text read in the table in force. */
    fun settle() = frames.values.forEach { frame -> frame.editBoxes().forEach { it.settle() } }

    /** The braille table changed: every open box written again in the new one, its text kept. */
    fun retable() {
        frames.values.forEach { frame -> frame.editBoxes().forEach { it.recell() } }
        frame?.let { display.show(it.rendered) }
    }

    /** Says [text] at once, cutting off what was being said. */
    fun say(text: String) = display.say(text)

    /**
     * What the app did, said with the line that follows it as one thing (or by itself if nothing
     * follows) and kept on the cells for the Message Display Time: someone reading braille alone
     * hears none of what is said.
     */
    fun announce(text: String) {
        announcements += text
        if (inKey) return
        schedule()
    }

    /** [announce]. */
    fun message(text: String) = announce(text)

    private fun schedule() {
        handler.removeCallbacks(saying)
        if (speechDelay <= 0) flush() else handler.postDelayed(saying, speechDelay)
    }

    /** Says what the app said and then the line it left, as one thing. */
    private fun flush() {
        handler.removeCallbacks(saying)
        val said = announcements.filter { it.isNotBlank() }.map { if (it.last() in ".?!") it else "$it." }
        // A line that is only what was just said is not said twice.
        val line = said.fold(pendingLine.orEmpty()) { left, sentence -> left.removeSuffix(sentence.trimEnd('.', ' ')).trimEnd() }
        val parts = said + listOfNotNull(line.takeIf { it.isNotBlank() })
        announcements.clear()
        pendingLine = null
        pendingHeading = null
        if (parts.isNotEmpty()) display.say(parts.joinToString(" "))
        if (said.isNotEmpty()) messages.show(said.joinToString(" "))
    }

    /**
     * Shows the screen in front. Its line is said if [moved], if it is a frame newly in front (with
     * its heading, unless [returning] to it), if [landed], if it is the first line with words, or
     * if it changed as a key asked; never if [silent].
     */
    private fun show(moved: Boolean, landed: Boolean = false, returning: Boolean = false, silent: Boolean = false) {
        val screen = top ?: return
        val shownFrame = frames.getOrPut(screen) { screen.frame() }
        shown++
        val rendered = shownFrame.rendered
        if (inKey) messages.clear()
        if (!messages.up) display.show(rendered)
        display.selecting(shownFrame.editBox()?.selecting == true)
        drawn(rendered.speech)
        val fresh = shownFrame.id != lastId
        lastId = shownFrame.id
        if (fresh) screen.shown(shownFrame)
        val firstWords = lastSpeech.isEmpty() && rendered.speech.isNotEmpty()
        val reworded = awaitingChange && rendered.speech != lastSpeech
        if (reworded) awaitingChange = false
        if (!silent && (moved || fresh || landed || firstWords || reworded)) {
            val heading = if (fresh && !returning) screen.heading(shownFrame) else ""
            // A screen opened inside the key that chose it may be drawn again before it is said: its heading waits.
            if (heading.isNotBlank()) pendingHeading = heading
            pendingLine = sentences(heading.ifBlank { pendingHeading.orEmpty() }, rendered.speech)
        }
        lastSpeech = rendered.speech
        if (!inKey && pendingLine != null) schedule()
    }

    /** [next] after [said], as sentences. */
    private fun sentences(said: String, next: String): String = when {
        next.isBlank() -> said
        said.isBlank() -> next
        said.last() in ".?!" -> "$said $next"
        else -> "$said. $next"
    }

    // ── Keys ─────────────────────────────────────────────────────────────────────────────

    /** A key from either keyboard: the session's `onKey`. What it shows is said as it is done. */
    fun press(key: Key) {
        inKey = true
        try {
            take(key)
        } finally {
            inKey = false
        }
        if (pendingLine != null || announcements.isNotEmpty()) flush()
    }

    private fun take(key: Key) {
        val current = frame ?: return
        when (key) {
            Key.Silence -> return display.silence()
            // Outside a box the device's read word and read character are what Space with their dots is.
            Key.SayCharacter, Key.SayWord -> if (current.editBox() == null) {
                return take(Key.SpaceDots(if (key == Key.SayWord) SAY_WORD_DOTS else SAY_CHARACTER_DOTS))
            }
            // A multi-line box reads the cursor's line itself; anything else is read whole.
            Key.Repeat -> if (current.editBox()?.multiLine != true) {
                messages.clear()
                val rendered = current.rendered
                pendingLine = rendered.speech
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
                    if (shown == before) show(moved = current.editBox() == null)
                }
                return
            }
            else -> {}
        }
        display.silence()
        current.editBox()?.let { box -> if (edit(current, box, key)) return }
        val before = shown
        val changing = current.changes(key)
        if (current.key(key)) {
            messages.clear()
            if (changing) {
                // Said once the app has made the change, when the line reads otherwise.
                awaitingChange = true
                if (shown == before) show(moved = false)
            } else if (shown == before) {
                show(moved = true)
            }
            return
        }
        when (key) {
            Key.Menu -> openMenu()
            Key.Escape -> back()
            Key.Exit -> onExit()
            is Key.SpaceDots -> letter(letters.letter(Key.Cell(key.dots)))
            is Key.EnterDots -> letter(letters.letter(Key.Cell(key.dots)))
            is Key.SpaceLetter -> letter(key.letter.lowercaseChar())
            is Key.EnterLetter -> letter(key.letter.lowercaseChar())
            else -> onUnhandled(key)
        }
    }

    private fun back() {
        val screen = top ?: return
        if (!screen.back()) close(screen)
    }

    /** Space or Enter with a letter: the frame's, else the screen's, else nothing is on it. */
    private fun letter(letter: Char?) {
        if (letter == null) return
        if (frame?.pressLetter(letter) == true || top?.letter(letter) == true) return
        announce(words.nothingOnLetter(letter))
    }

    private fun step(key: Key) {
        frame?.key(key)
        show(moved = true)
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
        // The box's line on the cells, said only as the box says it.
        show(moved = false, silent = true)
        edited.said?.let { if (inKey) pendingLine = it else display.say(it) }
    }

    private fun openMenu() {
        val items = top?.menu().orEmpty()
        if (items.isEmpty()) return announce(words.nothingHere)
        menu(items) { run -> run?.invoke() }
    }

    // ── Asks ─────────────────────────────────────────────────────────────────────────────

    /** A screen of rows, read again from [items] at each [refresh]; Space-M is [actions]. */
    fun list(
        title: String,
        items: () -> List<Item>,
        empty: String = words.empty,
        actions: () -> List<Action> = { emptyList() },
        /** The person moved to a row. */
        moved: (Item) -> Unit = {},
        /** A first letter, for a list whose rows the app finds itself; null finds here by [Item.finding]. */
        find: ((Char) -> Unit)? = null,
    ): ListScreen = ListScreen(title, items, empty, actions, moved, find).also(::push)

    /** The device's menu of [items]: [chosen] gets the value of the one chosen, or null if it closed unchosen. */
    fun <T> menu(items: List<MenuItem<T>>, chosen: (T?) -> Unit) {
        val screen = MenuScreen(chosen)
        screen.entries = items.map { screen.entry(it) }
        push(screen)
    }

    suspend fun <T> menu(items: List<MenuItem<T>>): T? = suspendCoroutine { done -> menu(items) { done.resume(it) } }

    /** The device's menu of [entries] as they are, each running its own: a row's actions. */
    fun menu(entries: List<MenuEntry>) {
        val screen = MenuScreen<Unit> {}
        screen.entries = entries
        push(screen)
    }

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
        private val moved: (Item) -> Unit,
        private val find: ((Char) -> Unit)?,
    ) : Screen() {
        private var selected: String? = null

        override fun frame(): Frame {
            val shownItems = items()
            val byKey = shownItems.associateBy { it.key }
            val rows = shownItems.map { item ->
                Row(
                    item.key, item.text, braille = item.braille, finding = item.finding, open = item.open, space = item.space,
                    actions = item.actions.map { MenuEntry(it.name, letter = it.letter, asks = it.asks, run = it.run) },
                    command = item.command, delete = item.delete, role = item.role, detail = item.detail, letter = item.letter,
                    collapse = item.collapse, expand = item.expand, asks = item.asks,
                )
            }
            return ListFrame(
                "list:${System.identityHashCode(this)}", title, rows, selected, empty, letters,
                moved = { row ->
                    selected = row.key
                    byKey[row.key]?.let(moved)
                },
                openActions = { row -> menu(row.actions) },
                findInApp = find,
                notFound = { announce(words.notFound(it)) },
                readings = readings,
                byLetter = { letter -> shownItems.firstOrNull { it.letter == letter }?.open?.let { it(); true } ?: false },
                opensNothing = { announce(words.opensNothing) },
                noActions = { announce(words.noActions) },
            ).also { frame -> frame.current?.let { selected = it.key } }
        }

        override fun menu(): List<MenuItem<() -> Unit>> = actions().map { MenuItem(it.name, it.letter, value = it.run, asks = it.asks) }

        override fun heading(frame: Frame): String = title
    }

    /**
     * A menu, as the device's own. The frame closes it before running what was chosen, so what that
     * opens is in front: closed by a choice it says nothing more, and the choice's [entry] answers;
     * left unchosen it says Cancel ahead of the line under it, and answers null.
     */
    private inner class MenuScreen<T>(private val chosen: (T?) -> Unit) : Screen() {
        var entries: List<MenuEntry> = emptyList()
        override val title: String get() = ""
        private var menuFrame: MenuFrame? = null
        private var answered = false
        private var choosing = false

        private fun answer(value: T?) {
            if (answered) return
            answered = true
            chosen(value)
        }

        fun entry(item: MenuItem<T>): MenuEntry = MenuEntry(
            item.name, children = item.children.map(::entry), hotkey = item.hotKey, letter = item.letter, asks = item.asks,
            run = { answer(item.value) },
        )

        override fun frame(): Frame = menuFrame ?: MenuFrame(entries, letters, readings, opensOnLetter, notFound = { announce(words.notFound(it)) }, closed = { cancelled ->
            if (cancelled) {
                close(this, said = true, prefix = words.cancel)
                answer(null)
            } else {
                choosing = true
                close(this, said = false, prefix = "")
            }
        }).also { menuFrame = it }

        override fun closed() {
            if (!choosing) answer(null)
        }
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

        override fun back(): Boolean = true.also { finish(false, said = !quiet) }
    }

    /** One of [options], picked with Enter. */
    private inner class ChoiceScreen(override val title: String, private val options: List<String>, first: Int, chosen: (Int?) -> Unit) : Asking<Int>(chosen) {
        private var selected: String? = first.toString()

        override fun frame(): Frame {
            val rows = options.mapIndexed { at, option -> Row(at.toString(), option, open = { finish(at) }) }
            return ListFrame(
                "choice:${System.identityHashCode(this)}", title, rows, selected, words.empty, letters,
                moved = { selected = it.key }, notFound = { announce(words.notFound(it)) }, readings = readings,
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
        /** How long a line the app shows of itself waits for what it says to arrive with it. */
        const val SPEECH_DELAY = 60L

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
