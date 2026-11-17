package dev.emassey0135.orbit.ui

import android.content.Context
import android.os.Handler
import android.os.Looper
import dev.emassey0135.orbit.BrailleInput
import dev.emassey0135.orbit.BrailleLetters
import dev.emassey0135.orbit.CellBuffer
import dev.emassey0135.orbit.EditBox
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.KeyDecoder
import dev.emassey0135.orbit.KeyEvent
import dev.emassey0135.orbit.ListFinder
import dev.emassey0135.orbit.OrbitMenu
import dev.emassey0135.orbit.OrbitReader
import dev.emassey0135.orbit.OrbitWords
import dev.emassey0135.orbit.Reader
import dev.emassey0135.orbit.ReaderEditOutput
import dev.emassey0135.orbit.T9Input
import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.resume
import kotlin.coroutines.startCoroutine
import kotlin.coroutines.suspendCoroutine

/**
 * An app's screens on an Orbit Speak or Player, as a stack that takes the keys and tells Orbit's
 * screen reader everything itself, with calls that ask and wait: [list], [menu], [askLine],
 * [confirm], [choose], [form], each with a callback and a `suspend` twin.
 *
 * The screen on top takes each key first; what it leaves is the stack's: Left, Back, Escape (Space
 * with E or Z) and Dot 7 close it, the menu key (Space with M, the Player's Menu) opens one menu of
 * the row in hand's actions then the screen's own, as Orbit has one menu key for both, and Space or
 * Dot 8 with a letter runs the action of that letter. Anything else sounds Orbit's error tone.
 *
 * A screen newly on top is told as what it is (a list, a dialog, a menu, an edit box); afterwards
 * only a move is told. When the app's data changes, [refresh] reads the screen on top again, the
 * row in hand kept by its key, and says nothing unless what is in hand moved.
 *
 * Everything runs on the main thread: hand [key] what [dev.emassey0135.orbit.KeyDelivery] delivers,
 * and [readerReady] what [OrbitReader.Listener.readerReady] says, posted there.
 */
class OrbitUi(
    /** Orbit's screen reader: an [OrbitReader], or a test's. */
    val reader: Reader,
    /** The Speak's braille input; none where nothing is typed in cells (a test). */
    val braille: BrailleInput? = null,
    /** For the system's clipboard in edit boxes; none leaves copying and pasting doing nothing. */
    context: Context? = null,
    /** What the layer says in words of its own. */
    val words: UiWords = UiWords(),
    /** What the device says in its own words: its fields' echo, T9's modes. */
    val orbitWords: OrbitWords = OrbitWords(),
    /** The main thread's handler: for the Player's numbers and a field's hint. */
    private val main: Handler = Handler(Looper.getMainLooper()),
    /** When T9 takes a letter: one pending call, on [main] unless a test gives its own. */
    internal val later: T9Input.Later = HandlerLater(main),
    /** What turns cells into text: the person's table through [braille]. */
    private val translator: CellBuffer.Translator = CellBuffer.Translator { cells, mode -> braille?.translate(cells, mode) },
    /** How an edit box tells the reader of itself. */
    private val editOutput: EditBox.Output = ReaderEditOutput(reader, braille, context, orbitWords),
) {
    private val stack = ArrayList<Screen>()
    private val decoder = KeyDecoder()
    private var ready = false

    /** The screen last told as newly shown; another on top has yet to be. */
    private var told: Screen? = null

    /** [onEmpty] has been called for the stack as it is: a nested close does not call it twice. */
    private var emptied = false

    /** The screen Back was pressed on, so its release does not reach the screen under it. */
    private var backPressedOn: Screen? = null

    /** Called when the last screen closes (Back from the first): the app finishes. */
    var onEmpty: () -> Unit = {}

    /**
     * Whether the keys are the Player's phone keypad, as the last key said. Before any key, a device
     * with no braille translator is taken for the Player; with no [braille] at all, the Speak.
     */
    var phoneKeypad: Boolean = braille?.available == false

    /** The screen shown. */
    val top: Screen? get() = stack.lastOrNull()

    /** Puts [screen] on top and tells the reader of it. */
    fun push(screen: Screen) {
        require(stack.none { it === screen }) { "The screen is open already" }
        screen.ui = this
        stack.add(screen)
        emptied = false
        tellTop()
    }

    /**
     * Takes [screen] off wherever it is; if it was on top, the screen under it is told again as
     * newly shown. An ask closed so answers with nothing.
     */
    fun close(screen: Screen) {
        if (remove(screen, quiet = false)) settle()
    }

    /** A menu closed by a choice: the screen under it, told before it opened, is not told again. */
    internal fun closeQuietly(screen: Screen) {
        if (remove(screen, quiet = true)) settle()
    }

    private fun remove(screen: Screen, quiet: Boolean): Boolean {
        val at = stack.indexOfFirst { it === screen }
        if (at < 0) return false
        val wasTop = at == stack.size - 1
        stack.removeAt(at)
        if (quiet && wasTop && told === screen) told = top
        screen.closed()
        return true
    }

    /** After a close: the screen now on top told, if what closed did not show another itself. */
    private fun settle() {
        if (stack.isEmpty()) {
            if (!emptied) {
                emptied = true
                onEmpty()
            }
        } else if (told !== top) {
            tellTop()
        }
    }

    private fun tellTop() {
        val screen = top ?: return
        if (!ready) return
        told = screen
        screen.tellShown()
    }

    /** Whether [screen] is on top and the reader has been told of it. */
    internal fun isShown(screen: Screen): Boolean = ready && top === screen && told === screen

    /**
     * The app's data changed: screens that no longer [Screen.stands] close, and the screen on top
     * reads its rows again, the row in hand kept by its key, told only if it moved.
     */
    fun refresh() {
        for (screen in stack.asReversed().toList()) {
            if (!screen.stands()) remove(screen, quiet = false)
        }
        if (stack.isEmpty()) return settle()
        val screen = top ?: return
        val shown = ready && told === screen
        screen.refresh(shown)
        if (!shown) tellTop()
    }

    /** Says [text] after what is being said. */
    fun say(text: String) {
        if (text.isNotBlank()) reader.speakQueued(text)
    }

    /** Orbit's error tone. */
    fun refuse() = reader.sound(OrbitReader.SOUND_ERROR)

    /** A message, said: nothing waits for it. */
    fun message(text: String) = say(text)

    /** Orbit's reader is ready, for the first time or again: the screen on top is told anew. */
    fun readerReady() {
        ready = true
        told = null
        tellTop()
    }

    // ---- keys ------------------------------------------------------------------------------

    /** A key from Orbit's keypad, on the main thread, in order. */
    fun key(event: KeyEvent, phoneKeypad: Boolean) {
        this.phoneKeypad = phoneKeypad
        val key = decoder.decode(event)
        if (key == Key.None) return
        val screen = top ?: return
        if (key == Key.Back) backPressedOn = screen
        if (key is Key.BackReleased) {
            val pressedHere = backPressedOn === screen
            backPressedOn = null
            if (!pressedHere) return
        }
        if (screen.key(key)) return
        when (key) {
            Key.Left, Key.Back, Key.Escape, Key.Dot7 -> close(screen)
            Key.Menu -> openMenu(screen)
            is Key.BackReleased -> {}
            is Key.Chord -> if (!runLetter(screen, key.dots)) refuse()
            is Key.Enter -> if (!runLetter(screen, key.dots)) refuse()
            else -> refuse()
        }
    }

    /** The one menu key: the row in hand's actions, then the screen's own. */
    private fun openMenu(screen: Screen) {
        val actions = screen.inHandActions() + screen.actions()
        if (actions.isEmpty()) return refuse()
        val items = actions.map { OrbitMenu.Item<Action>(it.name, it.letter?.lowercaseChar(), value = it) }
        push(MenuScreen(items) { action -> if (action != null) runAction(screen, action) })
    }

    /** Space or Dot 8 with a letter: the action of that letter, the row in hand's first. */
    private fun runLetter(screen: Screen, dots: Int): Boolean {
        val letter = BrailleLetters.letter(dots) ?: return false
        val action = (screen.inHandActions() + screen.actions()).firstOrNull { it.letter?.lowercaseChar() == letter } ?: return false
        runAction(screen, action)
        return true
    }

    private fun runAction(screen: Screen, action: Action) {
        (screen as? ListScreen)?.expecting = true
        action.run()
    }

    // ---- what to ask -----------------------------------------------------------------------

    /** A list of [items], read again at each [refresh], pushed. */
    fun list(
        title: String,
        items: () -> List<Item>,
        empty: String = words.empty,
        actions: () -> List<Action> = { emptyList() },
    ): ListScreen = ListScreen(title, items, empty, actions).also(::push)

    /** A menu of [items] and submenus; [chosen] gets the value of the item chosen, or null. */
    fun <T> menu(items: List<MenuItem<T>>, chosen: (T?) -> Unit) {
        if (items.isEmpty()) {
            refuse()
            chosen(null)
            return
        }
        push(MenuScreen(items, chosen))
    }

    /** [menu], waited for. */
    suspend fun <T> menu(items: List<MenuItem<T>>): T? = suspendCoroutine { done -> menu(items) { done.resume(it) } }

    /** A line typed; [answered] gets it as confirmed, or null. */
    fun askLine(prompt: String, initial: String = "", hint: String = "", multiLine: Boolean = false, answered: (String?) -> Unit) =
        push(LineScreen(prompt, initial, hint, multiLine, answered))

    /** [askLine], waited for. */
    suspend fun askLine(prompt: String, initial: String = "", hint: String = "", multiLine: Boolean = false): String? =
        suspendCoroutine { done -> askLine(prompt, initial, hint, multiLine) { done.resume(it) } }

    /** Yes or No, [default] in hand first; [answered] gets false for No and for a dialog closed. */
    fun confirm(question: String, default: Boolean = false, answered: (Boolean) -> Unit) {
        var answer = false
        lateinit var screen: DialogScreen
        val yes = Control.Button(BUTTON + "yes", words.yes) {
            answer = true
            close(screen)
        }
        val no = Control.Button(BUTTON + "no", words.no) { close(screen) }
        screen = DialogScreen(question, listOf(yes, no), if (default) 0 else 1, null) { answered(answer) }
        push(screen)
    }

    /** [confirm], waited for. */
    suspend fun confirm(question: String, default: Boolean = false): Boolean =
        suspendCoroutine { done -> confirm(question, default) { done.resume(it) } }

    /** One of [options], [selected] in hand first; [chosen] gets its place, or null. */
    fun choose(title: String, options: List<String>, selected: Int = 0, chosen: (Int?) -> Unit) =
        push(ChoiceScreen(title, options, selected, words.empty, chosen))

    /** [choose], waited for. */
    suspend fun choose(title: String, options: List<String>, selected: Int = 0): Int? =
        suspendCoroutine { done -> choose(title, options, selected) { done.resume(it) } }

    /** A form of [fields], then OK and Cancel; [done] gets what it holds at OK, or null. */
    fun form(title: String, fields: List<Field>, done: (FormValues?) -> Unit) {
        var values: FormValues? = null
        lateinit var screen: DialogScreen
        val ok = Control.Button(BUTTON + "ok", words.ok) {
            values = screen.values()
            close(screen)
        }
        val cancel = Control.Button(BUTTON + "cancel", words.cancel) { close(screen) }
        val controls = fields.map { Control.of(it) } + ok + cancel
        screen = DialogScreen(title, controls, 0, ok) { done(values) }
        push(screen)
    }

    /** [form], waited for. */
    suspend fun form(title: String, fields: List<Field>): FormValues? =
        suspendCoroutine { done -> form(title, fields) { done.resume(it) } }

    /**
     * Runs [block] at once on this thread, for an app with no coroutine library: each ask in it
     * waits for its answer, and it goes on on the main thread as the answer comes.
     */
    fun launch(block: suspend () -> Unit) {
        block.startCoroutine(Continuation(EmptyCoroutineContext) { it.getOrThrow() })
    }

    // ---- for screens -----------------------------------------------------------------------

    /** An edit box as Orbit's own, for this device's keys as the last key said them, told through [reader]. */
    fun editBox(
        name: String,
        initial: String,
        multiLine: Boolean,
        confirmed: (String) -> Unit,
        cancelled: () -> Unit,
        changed: (text: String, caret: Int) -> Unit = { _, _ -> },
    ): EditBox = EditBox(
        name, initial, phoneKeypad, editOutput, multiLine, translator, later,
        timeoutMs = { reader.characterCommitTimeout.toLong() },
        commitFeedback = { reader.characterCommitFeedback },
        changed = changed,
        confirmed = confirmed,
        cancelled = cancelled,
    )

    /** The Player's finding of a row: [rows] long, [found] a letter, [opened] a row by number. */
    internal fun finder(rows: () -> Int, found: (String) -> Unit, opened: (Int) -> Unit): ListFinder = ListFinder(
        object : ListFinder.Host {
            override val count: Int get() = rows()
            override val commitMs: Long get() = reader.characterCommitTimeout.toLong()
            override fun find(letter: String) = found(letter)
            override fun open(index: Int) = opened(index)
            override fun refuse() = this@OrbitUi.refuse()
            override fun speak(text: String) = reader.speak(text)
            override fun modeChanged(mode: T9Input.Mode) = reader.speak(orbitWords.mode(mode))
            override fun lettersChanged(on: Boolean) = say(if (on) words.lettersOn else words.lettersOff)
        },
        later, main,
    )

    /** After what was told reaches Orbit's queue: speech jumps it, so a hint waits a moment. */
    internal fun afterTelling(run: () -> Unit) {
        main.postDelayed(Runnable { run() }, HINT_AFTER_MS)
    }

    private companion object {
        const val HINT_AFTER_MS = 400L

        /** Before a dialog's own buttons' keys, so a field's key cannot be the same. */
        const val BUTTON = "\u0000button:"
    }
}

/** One pending call on [main], replaced by the next: T9's commit timeout. */
internal class HandlerLater(private val main: Handler) : T9Input.Later {
    private var pending: Runnable? = null

    override fun after(milliseconds: Long, run: () -> Unit) {
        cancel()
        val runnable = Runnable {
            pending = null
            run()
        }
        pending = runnable
        main.postDelayed(runnable, milliseconds)
    }

    override fun cancel() {
        pending?.let { main.removeCallbacks(it) }
        pending = null
    }
}

/** [OrbitUi.choose]'s list: Select picks the option in hand. */
internal class ChoiceScreen(
    title: String,
    options: List<String>,
    selected: Int,
    empty: String,
    private val chosen: (Int?) -> Unit,
) : ListScreen(title, { emptyList() }, empty) {
    private var answer: Int? = null

    private val choices = options.mapIndexed { at, option ->
        Item(at.toString(), option, {
            answer = at
            ui.close(this)
        })
    }

    init {
        hold(selected.toString())
    }

    override fun items(): List<Item> = choices

    override fun closed() = chosen(answer)
}
