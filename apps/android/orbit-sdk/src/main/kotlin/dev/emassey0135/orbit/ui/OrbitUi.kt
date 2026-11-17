package dev.emassey0135.orbit.ui

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import dev.emassey0135.orbit.BrailleInput
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
import dev.emassey0135.orbit.session.OrbitSession
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
 * with E or Z) and Dot 7 go back ([Screen.back], else the screen closes), the menu key (Space with
 * M, the Player's Menu) opens the screen's [Screen.menuBar] or else one menu of the row in hand's
 * actions then the screen's own, as Orbit has one menu key for both, and Space or Dot 8 with a
 * letter goes to [Screen.letter], then runs the action of that letter. Anything else sounds Orbit's
 * error tone.
 *
 * A screen newly on top is told as what it is (a list, a dialog, a menu, an edit box) once it has
 * [Screen.loaded]; afterwards only a move is told. When the app's data changes, [refresh] reads the
 * screen on top again, the row in hand kept by its key, and says nothing unless what is in hand
 * moved; [land] is the app moving what is in hand itself, which is told. What the app says beside
 * ([announce]) comes after what was told.
 *
 * Everything runs on the main thread: hand [key] what [dev.emassey0135.orbit.KeyDelivery] delivers,
 * and [readerReady] what [OrbitReader.Listener.readerReady] says, posted there.
 */
public class OrbitUi(
    /** Orbit's screen reader: an [OrbitReader], or a test's. */
    reader: Reader,
    /** The Speak's braille input; none where nothing is typed in cells (a test). */
    public val braille: BrailleInput? = null,
    /** For the system's clipboard in edit boxes; none leaves copying and pasting doing nothing. */
    context: Context? = null,
    /** What the layer says in words of its own. */
    public val words: UiWords = UiWords(),
    /** What the device says in its own words: its fields' echo, T9's modes. */
    public val orbitWords: OrbitWords = OrbitWords(),
    /** Which letter a key presses: by default its six dots, with Space or Dot 8 alike. */
    public val letters: Letters = Letters(),
    /** The main thread's handler: for the Player's numbers. */
    private val main: Handler = Handler(Looper.getMainLooper()),
    /** When T9 takes a letter: one pending call, on [main] unless a test gives its own. */
    internal val later: T9Input.Later = HandlerLater(main),
    /** What turns cells into text: the person's table through [braille]. */
    private val translator: CellBuffer.Translator = CellBuffer.Translator { cells, mode -> braille?.translate(cells, mode) },
    /** How an edit box tells the reader of itself; by default through [reader]. */
    editOutput: EditBox.Output? = null,
    /** Runs something after a wait in milliseconds, on the main thread: a hint, what was announced. */
    private val delay: (milliseconds: Long, run: () -> Unit) -> Unit = { milliseconds, run -> main.postDelayed(Runnable { run() }, milliseconds) },
    /** The time in milliseconds, for how long ago something was told. */
    private val now: () -> Long = { SystemClock.uptimeMillis() },
) {
    /** What every screen tells Orbit through: [reader] itself, noting when a description went to it. */
    public val reader: Reader = Telling(reader)

    private val output: EditBox.Output = editOutput ?: ReaderEditOutput(this.reader, braille, context, orbitWords)

    private val stack = ArrayList<Screen>()
    private val decoder = KeyDecoder()
    private var ready = false

    /** The screen last told as newly shown; another on top has yet to be. */
    private var told: Screen? = null

    /** [onEmpty] has been called for the stack as it is: a nested close does not call it twice. */
    private var emptied = false

    /** The screen Back was pressed on, so its release does not reach the screen under it. */
    private var backPressedOn: Screen? = null

    /** Inside [batch]: nothing is told until it ends. */
    private var batching = 0

    /** When a description last went to Orbit, which what is announced waits behind. */
    private var describedAt: Long? = null
    private val announcing = ArrayDeque<String>()
    private var announceWaiting = false

    /** Called when the last screen closes (Back from the first): the app finishes. */
    public var onEmpty: () -> Unit = {}

    /**
     * Whether the keys are the Player's phone keypad, as the last key said. Before any key, a device
     * with no braille translator is taken for the Player; with no [braille] at all, the Speak.
     */
    public var phoneKeypad: Boolean = braille?.available == false

    /** The screen shown. */
    public val top: Screen? get() = stack.lastOrNull()

    /** The screens open, from the first to the one shown. */
    public val screens: List<Screen> get() = stack.toList()

    /** Puts [screen] on top and tells the reader of it. */
    public fun push(screen: Screen) {
        require(stack.none { it === screen }) { "The screen is open already" }
        screen.ui = this
        stack.add(screen)
        emptied = false
        screen.onTop()
        if (top === screen) tellTop()
    }

    /** Puts [screen] just above [above], wherever that is: told if it is then on top. */
    public fun insert(screen: Screen, above: Screen) {
        require(stack.none { it === screen }) { "The screen is open already" }
        val at = stack.indexOfFirst { it === above }
        require(at >= 0) { "The screen to go above is not open" }
        screen.ui = this
        stack.add(at + 1, screen)
        emptied = false
        if (top === screen) screen.onTop()
        settle()
    }

    /**
     * Puts [screen] in [old]'s place, as the app moves from what [old] showed to what [screen]
     * shows: [old] closes, and [screen] is told if it is on top.
     */
    public fun replace(old: Screen, screen: Screen) {
        require(stack.none { it === screen }) { "The screen is open already" }
        val at = stack.indexOfFirst { it === old }
        require(at >= 0) { "The screen to replace is not open" }
        screen.ui = this
        stack[at] = screen
        if (told === old) told = null
        old.closed()
        if (top === screen) screen.onTop()
        settle()
    }

    /**
     * Takes [screen] off wherever it is; if it was on top, the screen under it is told again as
     * newly shown. An ask closed so answers with nothing.
     */
    public fun close(screen: Screen) {
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
        val under = top
        if (quiet && wasTop && told === screen) told = under
        screen.closed()
        if (wasTop && under != null && top === under) under.onTop()
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
        if (!ready || batching > 0 || !screen.loaded) return
        told = screen
        screen.tellShown()
    }

    /** Whether [screen] is on top and the reader has been told of it. */
    internal fun isShown(screen: Screen): Boolean = ready && top === screen && told === screen

    /**
     * Does [block], which may push and close several screens, telling nothing until it ends: then
     * only the screen on top is told, if it has not been, so the person hears where they end up.
     */
    public fun batch(block: () -> Unit) {
        batching++
        try {
            block()
        } finally {
            batching--
            if (batching == 0) settle()
        }
    }

    /**
     * The app's data changed: screens that no longer [Screen.stands] close, and the screen on top
     * reads its rows again, the row in hand kept by its key, told only if it moved.
     */
    public fun refresh(): Unit = reread { screen, shown -> screen.refresh(shown) }

    /**
     * The app moved what is in hand itself, to [key] where it names one (else where the screen's
     * app holds it): a refresh that tells the screen on top what is in hand, unless that is what was
     * just told, as it was told.
     */
    public fun land(key: String? = null): Unit = reread { screen, shown ->
        if (shown) {
            screen.land(key)
        } else {
            // Not yet told: it is told whole on that row once it is.
            if (key != null) (screen as? ListScreen)?.hold(key)
            screen.refresh(false)
        }
    }

    private fun reread(read: (Screen, Boolean) -> Unit) {
        batch {
            for (screen in stack.asReversed().toList()) {
                if (!screen.stands()) remove(screen, quiet = false)
            }
        }
        val screen = top ?: return
        val shown = ready && told === screen
        read(screen, shown)
        if (!shown) tellTop()
    }

    /** Says [text] after what is being said, at once. */
    public fun say(text: String) {
        if (text.isNotBlank()) reader.speakQueued(text)
    }

    /**
     * Says what the app has to say (what a change did), after what was just told: speech goes ahead
     * of Orbit's queue of descriptions, so it waits a moment behind one, in the order announced.
     */
    public fun announce(text: String) {
        if (text.isBlank()) return
        announcing.addLast(text)
        if (announceWaiting) return
        val since = describedAt?.let { now() - it }
        if (since == null || since >= SETTLE_MS) return speakAnnounced()
        announceWaiting = true
        delay(SETTLE_MS - since) {
            announceWaiting = false
            speakAnnounced()
        }
    }

    private fun speakAnnounced() {
        while (announcing.isNotEmpty()) reader.speakQueued(announcing.removeFirst())
    }

    /** Orbit's error tone. */
    public fun refuse(): Unit = reader.sound(OrbitReader.SOUND_ERROR)

    /** Orbit's error tone, then why, where there is something to say, after what was told. */
    public fun refused(sentence: String?) {
        refuse()
        if (sentence != null) announce(sentence)
    }

    /** [announce]: what the app has to say, after what was just told. */
    public fun message(text: String): Unit = announce(text)

    /** Orbit's reader is ready, for the first time or again: the screen on top is told anew. */
    public fun readerReady() {
        ready = true
        told = null
        tellTop()
    }

    // ---- keys ------------------------------------------------------------------------------

    /** A key from Orbit's keypad, on the main thread, in order. */
    public fun key(event: KeyEvent, phoneKeypad: Boolean) {
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
            Key.Left, Key.Back, Key.Escape, Key.Dot7 -> back(screen)
            Key.Menu -> openMenu(screen)
            is Key.BackReleased -> {}
            is Key.Chord, is Key.Enter -> when (val pressed = letters.pressed(key)) {
                is Letters.Pressed.Letter -> runLetter(screen, pressed.letter)
                is Letters.Pressed.Refused -> refused(pressed.said)
                Letters.Pressed.None -> refuse()
            }
            else -> refuse()
        }
    }

    /** Back from [screen]: its own if it takes it, else it closes, and from the first the app ends. */
    public fun back(screen: Screen) {
        if (!screen.back()) close(screen)
    }

    /** The one menu key: the screen's own menu tree, else the row in hand's actions, then the screen's. */
    private fun openMenu(screen: Screen) {
        val bar = screen.menuBar()
        if (bar != null) {
            if (bar.items.isEmpty()) return refuse()
            push(MenuScreen(bar.items, bar.hotKeys) { run -> if (run != null) acted(screen, run) })
            return
        }
        val actions = screen.inHandActions() + screen.actions()
        if (actions.isEmpty()) return refuse()
        val items = actions.map { MenuItem(it.name, it.letter, value = it, asks = it.asks) }
        push(MenuScreen(items) { action -> if (action != null) acted(screen, action.run) })
    }

    /** Space or Dot 8 with a letter: the screen's, then the action of that letter, the row in hand's first. */
    private fun runLetter(screen: Screen, letter: Char) {
        if (screen.letter(letter)) return
        val action = (screen.inHandActions() + screen.actions()).firstOrNull { it.letter?.lowercaseChar() == letter }
        if (action == null) return refused(words.nothingOnLetter(letter))
        acted(screen, action.run)
    }

    /** What the person chose to do on [screen]: the row in hand is read again if it then reads otherwise. */
    private fun acted(screen: Screen, run: () -> Unit) {
        (screen as? ListScreen)?.expect()
        run()
    }

    // ---- what to ask -----------------------------------------------------------------------

    /**
     * A list of [items], read again at each [refresh], pushed. [empty] is said when it has no rows;
     * [actions] are the screen's own on the menu key, after the row in hand's. [moved] hears the
     * person move to a row themselves; [find] takes a letter alone, for an app that finds rows by
     * a rule of its own (none finds the next row whose text starts with it).
     */
    public fun list(
        title: String,
        items: () -> List<Item>,
        empty: String = words.empty,
        actions: () -> List<Action> = { emptyList() },
        moved: (Item) -> Unit = {},
        find: ((Char) -> Unit)? = null,
    ): ListScreen {
        val heard = moved
        val finding = find
        return object : ListScreen(title, items, empty, actions) {
            override fun moved(item: Item) = heard(item)

            override fun findLetter(letter: Char): Boolean {
                val found = finding ?: return false
                found(letter)
                return true
            }
        }.also(::push)
    }

    /** A menu of [items] and submenus; [chosen] gets the value of the item chosen, or null. */
    public fun <T> menu(items: List<MenuItem<T>>, chosen: (T?) -> Unit) {
        if (items.isEmpty()) {
            refuse()
            chosen(null)
            return
        }
        push(MenuScreen(items, chosen = chosen))
    }

    /** [menu], waited for. */
    public suspend fun <T> menu(items: List<MenuItem<T>>): T? = suspendCoroutine { done -> menu(items) { done.resume(it) } }

    /** A line typed; [answered] gets it as confirmed, or null. */
    public fun askLine(prompt: String, initial: String = "", hint: String = "", multiLine: Boolean = false, answered: (String?) -> Unit): Unit =
        push(LineScreen(prompt, initial, hint, multiLine, answered))

    /** [askLine], waited for. */
    public suspend fun askLine(prompt: String, initial: String = "", hint: String = "", multiLine: Boolean = false): String? =
        suspendCoroutine { done -> askLine(prompt, initial, hint, multiLine) { done.resume(it) } }

    /** Yes or No, [default] in hand first; [answered] gets false for No and for a dialog closed. */
    public fun confirm(question: String, default: Boolean = false, answered: (Boolean) -> Unit) {
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
    public suspend fun confirm(question: String, default: Boolean = false): Boolean =
        suspendCoroutine { done -> confirm(question, default) { done.resume(it) } }

    /** One of [options], [selected] in hand first; [chosen] gets its place, or null. */
    public fun choose(title: String, options: List<String>, selected: Int = 0, chosen: (Int?) -> Unit): Unit =
        push(ChoiceScreen(title, options, selected, words.empty, chosen))

    /** [choose], waited for. */
    public suspend fun choose(title: String, options: List<String>, selected: Int = 0): Int? =
        suspendCoroutine { done -> choose(title, options, selected) { done.resume(it) } }

    /** A form of [fields], then OK and Cancel; [done] gets what it holds at OK, or null. */
    public fun form(title: String, fields: List<Field>, done: (FormValues?) -> Unit) {
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
    public suspend fun form(title: String, fields: List<Field>): FormValues? =
        suspendCoroutine { done -> form(title, fields) { done.resume(it) } }

    /**
     * Runs [block] at once on this thread, for an app with no coroutine library: each ask in it
     * waits for its answer, and it goes on on the main thread as the answer comes.
     */
    public fun launch(block: suspend () -> Unit) {
        block.startCoroutine(Continuation(EmptyCoroutineContext) { it.getOrThrow() })
    }

    // ---- for screens -----------------------------------------------------------------------

    /** An edit box as Orbit's own, for this device's keys as the last key said them, told through [reader]. */
    public fun editBox(
        name: String,
        initial: String,
        multiLine: Boolean,
        confirmed: (String) -> Unit,
        cancelled: () -> Unit,
        changed: (text: String, caret: Int) -> Unit = { _, _ -> },
        completing: (() -> Boolean)? = null,
        complete: () -> Unit = {},
    ): EditBox = EditBox(
        name, initial, phoneKeypad, output, multiLine, translator, later,
        timeoutMs = { reader.characterCommitTimeout.toLong() },
        commitFeedback = { reader.characterCommitFeedback },
        changed = changed,
        confirmed = confirmed,
        cancelled = cancelled,
        completing = completing,
        complete = complete,
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

    /** [run] once what was just told has reached Orbit's queue: speech jumps it, so a hint waits a moment. */
    public fun afterTelling(run: () -> Unit): Unit = delay(SETTLE_MS, run)

    /** The reader the app gave, noting each description sent through it, which what is announced waits behind. */
    private inner class Telling(private val base: Reader) : Reader by base {
        private fun described() {
            describedAt = now()
        }

        override fun focusList(name: String, count: Int, selected: Int, item: String) {
            base.focusList(name, count, selected, item)
            described()
        }

        override fun itemChanged(name: String, count: Int, selected: Int, item: String) {
            base.itemChanged(name, count, selected, item)
            described()
        }

        override fun dialog(name: String, message: String, focused: String) {
            base.dialog(name, message, focused)
            described()
        }

        override fun menuOpened() {
            base.menuOpened()
            described()
        }

        override fun focusMenu(item: String, subMenu: Boolean, accessKey: Char?, position: Int, total: Int) {
            base.focusMenu(item, subMenu, accessKey, position, total)
            described()
        }

        override fun focusMenu(menu: OrbitMenu<*>) {
            val item = menu.current ?: return
            focusMenu(item.name, item.isSubMenu, item.accessKey, menu.index + 1, menu.items.size)
        }

        override fun focusEdit(name: String, content: String, cursor: Int, line: String, multiline: Boolean) {
            base.focusEdit(name, content, cursor, line, multiline)
            described()
        }

        override fun focusControl(name: String, role: Int) {
            base.focusControl(name, role)
            described()
        }

        override fun focusText(content: String) {
            base.focusText(content)
            described()
        }

        override fun focusCheck(name: String, checked: Boolean) {
            base.focusCheck(name, checked)
            described()
        }

        override fun focusChoice(name: String, role: Int, count: Int, selected: Int, item: String) {
            base.focusChoice(name, role, count, selected, item)
            described()
        }
    }

    /** Making the stack over a session. */
    public companion object {
        /** How long what is said waits behind a description, which goes through Orbit's queue. */
        private const val SETTLE_MS = 400L

        /** Before a dialog's own buttons' keys, so a field's key cannot be the same. */
        private const val BUTTON = "\u0000button:"

        /**
         * The stack over [session], wired into it: the session's keys come here, Orbit's reader
         * being ready tells the screen on top, a new braille table shows a line again, and Orbit's
         * settings unread (root refused) are said once in [orbitWords]. [reader] is what the stack
         * tells Orbit through: the session's, or one that wraps it (to show what is told, say).
         */
        public fun on(
            session: OrbitSession,
            words: UiWords = UiWords(),
            letters: Letters = Letters(),
            orbitWords: OrbitWords = OrbitWords(),
            reader: Reader = session.reader,
        ): OrbitUi {
            val ui = OrbitUi(reader, session.braille, session.context, words, orbitWords, letters)
            session.onKey = ui::key
            session.onReady = ui::readerReady
            session.onTableChanged = ui::refresh
            session.onRootFailed = { ui.say(orbitWords.rootFailed.replace("%s", it.reason)) }
            return ui
        }
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
