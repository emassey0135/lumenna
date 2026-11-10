package dev.emassey0135.lumenna.orbit

import android.os.Handler
import android.os.Looper
import android.widget.TextView
import dev.emassey0135.lumenna.AppModel
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.Reminded
import dev.emassey0135.lumenna.replan
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Lines
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.ListShown
import dev.emassey0135.lumenna.app.QuestionShown
import dev.emassey0135.lumenna.app.ScreenChange
import dev.emassey0135.lumenna.app.ScreenView
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Shown
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.brailleLetter
import dev.emassey0135.lumenna.app.firstLetter
import dev.emassey0135.lumenna.app.nothingOnLetter
import dev.emassey0135.lumenna.app.SpeakLetter
import dev.emassey0135.lumenna.app.speakLetter
import dev.emassey0135.lumenna.app.speakPress
import dev.emassey0135.lumenna.app.speakPressInstead
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.commandTitle
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.name

/** One thing on a screen a person can stop on: what is said, what its Select and Menu do. */
class Row(
    /** What names it across a change: the app's key for it, or its own. */
    val key: String,
    val text: String,
    /** Select: what it does. */
    val open: (() -> Unit)? = null,
    /** Its actions in short, as the core offers them, for its context menu. */
    val offered: List<Offered> = emptyList(),
    /** What its actions run on. */
    val on: ActingOn? = null,
    /** The letter that opens it: this app's lists' own, the core's (`page_letter`…). */
    val letter: String? = null,
    /** What Left and Right fold: the row's own Expand or Collapse among [offered]. */
    val expanded: Boolean? = null,
)

/** An entry of a screen's context menu that is the screen's own: New task, Undo. */
class Command(val text: String, val run: () -> Unit)

/**
 * One screen on the stack. The rows are read fresh on each [Ui.redraw] from the view the app last
 * left, so a change a sync brings in is drawn without the screen doing anything.
 */
abstract class Screen {
    lateinit var ui: Ui
    /** The row selected on this screen; kept across redraws by [Row.key]. */
    var index = 0
    var selectedKey: String? = null

    abstract val title: String
    abstract fun rows(): List<Row>

    /** What an empty list says, in the core's words. */
    open val empty: String get() = appWords.nothingHere

    /** The row the app has in hand here, by [Row.key]; null keeps the place by key or position. */
    open fun inHand(): String? = null

    /** Back was pressed: true if the screen keeps it for now (a save that may be refused). */
    open fun beforeBack(): Boolean = false

    /** The person moved to [row]: the app is told, so its details and commands follow. */
    open fun moved(row: Row) {}

    /** Said once as the screen opens, when it is a question rather than a list. */
    open fun openedAs(): Opened = Opened.List

    /** The screen came to the top, first or coming back: send what shows it. */
    open fun resumed() {}

    /** Leaving the screen: send what puts its state away. */
    open fun leaving() {}

    /** The screen's own entries in the context menu, after the row's. */
    open fun commands(): List<Command> = emptyList()

    /** Whether what the screen shows has arrived: the place it opened, once the app has opened it. */
    open fun loaded(view: ViewModel): Boolean = true

    /** Whether this screen still stands: a question the app has answered, for one. */
    open fun stands(view: ViewModel): Boolean = true

    /** A key only this screen knows; true if it took it. */
    open fun key(key: Key): Boolean = false

    /** A letter only this screen knows (Space with it); true if it took it. */
    open fun letter(letter: String): Boolean = false

    /** Which of the app's lists this screen shows, if one: what a letter and a landing are. */
    open val lands: Lands get() = Lands.NONE

    open val isEdit: Boolean get() = false
}

/** Whose list a screen shows: the app's main one, one beside it, or none of the app's. */
enum class Lands { MAIN, SIDE, NONE }

/** A row list screen built from the app's rows. */
abstract class ListScreen : Screen()

/**
 * What runs the screens: the stack, the keys, drawing and saying.
 *
 * Which row is in hand, what a key runs, and what a question asks are the app's; this keeps only
 * the place on the stack and which screen a turn of the app calls for. [redraw] describes the
 * screen to Orbit's screen reader; every description goes through [OrbitReader], so it is in
 * order, and the core's sentences come after it.
 */
class Ui(
    val app: AppModel,
    val core: Core,
    val reader: OrbitReader,
    val liblouis: Liblouis,
    private val textView: TextView,
    private val finish: () -> Unit,
) : T9Input.Later {
    private val stack = ArrayDeque<Screen>()
    private val main = Handler(Looper.getMainLooper())
    private val decoder = KeyDecoder()
    private val finder = FirstLetters(this)
    /** Whether the keys are the Player's phone keypad, as the last key said. */
    var phoneKeypad = false
        private set
    private var saidScreen: Screen? = null
    private var introduced = false
    private var landing = 0u
    private var pending: Runnable? = null

    val view: ViewModel get() = checkNotNull(app.view.value)
    val lines: Lines? get() = app.lines.value

    /** The core's description of the screen, as of the last turn (`Turn::described`). */
    var screenView: ScreenView = app.screenView()
        private set

    /** The question the core says is open, if one is. */
    val questionShown: QuestionShown? get() = (screenView.shown as? Shown.Question)?.field0

    /** The list the core says is showing, if one is. */
    val listShown: ListShown? get() = (screenView.shown as? Shown.List)?.field0

    val top: Screen? get() = stack.lastOrNull()

    fun push(screen: Screen) {
        screen.ui = this
        stack.addLast(screen)
        screen.resumed()
        follow()
        redraw(Why.Entered)
    }

    /** Takes [screen] off if it is on the stack, what was above it too. */
    fun remove(screen: Screen) {
        if (!stack.contains(screen)) return
        while (top !== screen) stack.removeLast().leaving()
        stack.removeLast().leaving()
        top?.resumed()
        follow()
        redraw(Why.Entered)
    }

    fun pop() {
        top?.let { remove(it) }
    }

    /** A reminder's notification was tapped: where it leads. */
    fun reminded(reminded: Reminded) {
        when (reminded) {
            is Reminded.Task -> push(DetailsScreen(reminded.id))
            is Reminded.Block -> {
                reminded.on?.let { send(ShellEvent.ShowBlock(it.series, it.date)) }
                if (stack.none { it is DayScreen }) push(DayScreen())
            }
            is Reminded.Replan -> app.replan(reminded)
        }
    }

    /** Back, from the keys: a screen with the app's state puts it away as it leaves. */
    fun back() {
        finder.reset()
        val screen = top ?: return
        if (screen.beforeBack()) return
        if (stack.size > 1) pop() else finish()
    }

    fun send(event: ShellEvent) {
        expectingChange = true
        app.send(event)
    }

    /** What the person just did may reword the row they are on: the next turn says it if it did. */
    private var expectingChange = false
    private var lastItem = ""

    fun say(text: String) {
        if (text.isNotBlank()) reader.speakQueued(text)
    }

    fun refuse() = reader.sound(OrbitReader.SOUND_ERROR)

    fun later(milliseconds: Long, run: () -> Unit) {
        main.postDelayed(run, milliseconds)
    }

    /** Orbit is ready, for the first time or after it came back: the screen is described anew. */
    fun readerReady() {
        introduced = true
        saidScreen = null
        redraw(Why.Entered)
    }

    /** A turn of the app has been taken: screens it opened or closed, then what it shows now. */
    fun turned(described: ScreenView) {
        screenView = described
        val moved = described.changes.any { it is ScreenChange.Moved }
        val landedMain = view.landing > landing
        val landedSide = sideLanded()
        follow()
        landing = view.landing
        // A list behind the screen on top landing says nothing: it isn't what is shown.
        val landed = when (top?.lands) {
            Lands.MAIN -> moved
            Lands.SIDE -> landedSide
            else -> false
        }
        redraw(if (landed) Why.Landed else Why.Turn)
        if (landedMain || landedSide) core.release()
        // Words already coming through what the turn said are not said twice.
        described.changes.filterIsInstance<ScreenChange.Refused>().forEach {
            refuse()
            if (!it.said) say(it.reason)
        }
    }

    private var sidebarLanding = 0u
    private var remindersLanding = 0u
    private var devicesLanding = 0u
    private var dueLanding = 0u

    private fun sideLanded(): Boolean {
        val v = view
        val landed = v.sidebarHeld.landing > sidebarLanding || v.remindersHeld.landing > remindersLanding ||
            v.devicesHeld.landing > devicesLanding || v.dueHeld.landing > dueLanding
        sidebarLanding = v.sidebarHeld.landing
        remindersLanding = v.remindersHeld.landing
        devicesLanding = v.devicesHeld.landing
        dueLanding = v.dueHeld.landing
        return landed
    }

    /** The screens the app has open: a question, the block form, a proposal, a reminder list. */
    private fun follow() {
        val v = view
        // What the app has put away goes first, from the top.
        while (true) {
            val screen = top ?: break
            if (screen.stands(v)) break
            stack.removeLast().leaving()
            saidScreen = null
        }
        v.asking?.let { asking ->
            // Open already, under something it opened (its completions).
            if (stack.any { (it as? AskingScreen)?.number == asking.number } && top !is AskingScreen) return
            val open = top as? AskingScreen
            if (open == null || open.number != asking.number) {
                if (asking.question is Question.Form) {
                    formAsked(asking)
                } else {
                    if (open != null) stack.removeLast().leaving()
                    pushQuestion(asking)
                }
            }
            return
        }
        // Only when none is open: a field being typed sits above the form.
        if (v.line != null && stack.none { it is QuickAddScreen }) pushApp(QuickAddScreen())
        if (v.blockForm != null && stack.none { it is BlockFormScreen }) pushApp(BlockFormScreen())
        if (v.proposal != null && stack.none { it is ProposalScreen }) pushApp(ProposalScreen())
        if (v.reminders != null && stack.none { it is RemindersScreen }) pushApp(RemindersScreen())
    }

    private var formAskedFor = 0u

    private fun formAsked(asking: dev.emassey0135.lumenna.app.Asking) {
        // A question is asked once; a turn arriving before the Cancel lands must not ask again.
        if (asking.number == formAskedFor) return
        formAskedFor = asking.number
        val action = asking.action
        send(ShellEvent.Cancel)
        action?.target?.let { target ->
            // The form a task asks for is its details, the row it was asked on.
            pushApp(DetailsScreen(target))
        }
    }

    private fun pushQuestion(asking: dev.emassey0135.lumenna.app.Asking) {
        pushApp(if (asking.question is Question.Text) TextQuestionScreen(asking) else QuestionScreen(asking))
    }

    private fun pushApp(screen: Screen) {
        screen.ui = this
        stack.addLast(screen)
        screen.resumed()
        saidScreen = null
    }

    // ---- keys ----------------------------------------------------------------------------

    /** A key from Orbit, on the main thread, in order. */
    fun key(event: KeyEvent, phoneKeypad: Boolean) {
        this.phoneKeypad = phoneKeypad
        val key = decoder.decode(event)
        if (key == Key.None) return
        val screen = top ?: return
        if (screen.key(key)) return
        if (phoneKeypad && finder.phoneKey(key, screen)) return
        if (!phoneKeypad && key is Key.Cell) {
            finder.letter(brailleLetter(key.dots.toUByte()))
            return
        }
        finder.reset()
        move(key, screen, phoneKeypad)
    }

    private fun move(key: Key, screen: Screen, phoneKeypad: Boolean) {
        val rows = screen.rows()
        when (key) {
            Key.Up -> moveBy(screen, rows, -1)
            Key.Down -> moveBy(screen, rows, 1)
            Key.First -> moveTo(screen, rows, 0)
            Key.Last -> moveTo(screen, rows, rows.size - 1)
            // Left folds a row open, else goes back out of a list, never a dialog; Right opens a
            // folded row, else opens it.
            Key.Left -> if (!foldRow(screen, rows, false) && screen !is DialogScreen) back()
            Key.Right -> if (!foldRow(screen, rows, true)) open(screen, rows)
            Key.Select, Key.Dot8 -> open(screen, rows)
            Key.Dot7, Key.Back, Key.Escape -> back()
            Key.Tab -> moveRound(screen, rows, 1)
            Key.BackTab -> moveRound(screen, rows, -1)
            Key.Menu -> menu(screen, rows)
            is Key.Chord -> spaceLetter(key.dots, screen, rows)
            is Key.Enter -> brailleLetter(key.dots.toUByte())?.let { runLetter(it, screen, rows) } ?: refuse()
            else -> {}
        }
    }

    private fun moveBy(screen: Screen, rows: List<Row>, delta: Int) {
        if (rows.isEmpty()) return
        moveTo(screen, rows, (screen.index + delta).coerceIn(0, rows.size - 1))
    }

    /** Tab in a list: on to the next row, round from the last to the first. */
    private fun moveRound(screen: Screen, rows: List<Row>, delta: Int) {
        if (rows.isEmpty()) return
        moveTo(screen, rows, (screen.index + delta + rows.size) % rows.size)
    }

    private fun moveTo(screen: Screen, rows: List<Row>, to: Int) {
        if (rows.isEmpty() || to == screen.index && selected(screen, rows)?.key == screen.selectedKey) return
        screen.index = to.coerceIn(0, rows.size - 1)
        screen.selectedKey = rows[screen.index].key
        screen.moved(rows[screen.index])
        redraw(Why.Moved)
    }

    private fun selected(screen: Screen, rows: List<Row>): Row? = rows.getOrNull(screen.index)

    private fun open(screen: Screen, rows: List<Row>) {
        val row = selected(screen, rows) ?: return
        row.open?.invoke()
    }

    private fun foldRow(screen: Screen, rows: List<Row>, opening: Boolean): Boolean {
        val row = selected(screen, rows) ?: return false
        val expanded = row.expanded ?: return false
        if (opening == expanded) return false
        val wanted = if (opening) dev.emassey0135.lumenna.app.ActionKind.EXPAND else dev.emassey0135.lumenna.app.ActionKind.COLLAPSE
        val action = row.offered.firstOrNull { it.kind == wanted } ?: return false
        send(ShellEvent.ActOn(row.on ?: return false, action))
        return true
    }

    /** The row's context menu: its actions, then the screen's own. */
    fun menu(screen: Screen, rows: List<Row>) {
        val row = selected(screen, rows)
        val commands = mutableListOf<Command>()
        if (row != null) {
            val on = row.on
            row.offered.filter { it.kind != dev.emassey0135.lumenna.app.ActionKind.EXPAND && it.kind != dev.emassey0135.lumenna.app.ActionKind.COLLAPSE }
                .forEach { action ->
                    if (on != null) commands += Command(withLetter(action.name, actionLetter(action.kind))) { send(ShellEvent.ActOn(on, action)) }
                }
        }
        commands += screen.commands()
        if (commands.isEmpty()) {
            refuse()
            return
        }
        push(ContextMenuScreen(appWords.braille.actionsMenu, commands))
    }

    /** Space with a letter: its command, or which key the Speak presses it with instead. */
    private fun spaceLetter(dots: Int, screen: Screen, rows: List<Row>) {
        val letter = brailleLetter(dots.toUByte())
        when {
            letter == null -> refuse()
            speakLetter(letter) == SpeakLetter.SPACE -> runLetter(letter, screen, rows)
            else -> {
                refuse()
                say(speakPressInstead(letter))
            }
        }
    }

    private fun runLetter(letter: String, screen: Screen, rows: List<Row>) {
        // This app's own lists first (the main menu's places, the settings pages), then the screen's.
        rows.firstOrNull { it.letter == letter }?.let { row ->
            row.open?.invoke()
            return
        }
        if (screen.letter(letter)) return
        if (screen.lands == Lands.NONE) {
            refuse()
            say(nothingOnLetter(letter))
            return
        }
        // The app acts on the row in hand: a list beside the main one's is held first, so it
        // answers there. D is the delete key, which no letter of the core's table is.
        rows.getOrNull(screen.index)?.let(screen::moved)
        send(if (letter == "d") ShellEvent.Key("delete") else ShellEvent.Letter(letter))
    }

    // ---- describing ----------------------------------------------------------------------

    /** Draws the top screen and tells Orbit's screen reader what changed. */
    fun redraw(why: Why) {
        val screen = top ?: return
        // Nothing to describe until the store has arrived and the screen's place has opened: an
        // empty list said on the way there would be wrong. Orbit is told only once it is ready.
        if (!view.ready || !screen.loaded(view)) {
            textView.text = screen.title
            return
        }
        if (!introduced) return
        if (screen is EditScreen && phoneKeypad) {
            textView.text = "${screen.title}\n\n${screen.display()}"
            if (saidScreen !== screen) {
                saidScreen = screen
                screen.describe()
            }
            return
        }
        val rows = screen.rows()
        if (BuildConfigDebug.on) android.util.Log.i("LumennaOrbit", "redraw $why ${screen.javaClass.simpleName} ${rows.map { it.text }}")
        val before = screen.index
        // The row in hand is the app's where it has one; else the same row by key, else the place.
        val held = screen.inHand()
        val heldIndex = held?.let { key -> rows.indexOfFirst { it.key == key }.takeIf { it >= 0 } }
        if (heldIndex != null) {
            screen.index = heldIndex
        } else {
            val byKey = screen.selectedKey?.let { key -> rows.indexOfFirst { it.key == key }.takeIf { it >= 0 } }
            screen.index = (byKey ?: screen.index).coerceIn(0, (rows.size - 1).coerceAtLeast(0))
        }
        screen.selectedKey = rows.getOrNull(screen.index)?.key
        val item = rows.getOrNull(screen.index)?.text.orEmpty()
        textView.text = buildString {
            append(screen.title).append("\n\n")
            append(if (rows.isEmpty()) screen.empty else item)
            if (rows.isNotEmpty()) append("\n\n").append(screen.index + 1).append(" of ").append(rows.size)
        }
        val newlyShown = saidScreen !== screen
        // The row in hand reworded by what the person just did (a toggle, a field saved).
        val reworded = why == Why.Turn && !newlyShown && expectingChange && item != lastItem
        if (why == Why.Turn) expectingChange = false
        saidScreen = screen
        lastItem = item
        if (screen is DialogScreen && screen.inPlace) {
            // A value it changes is said as it changes: nothing to repeat when the app has it.
            screen.describeControl(newlyShown, why, before != screen.index)
            return
        }
        val shown = Drawn(screen.title, rows.size, screen.index, item, screen.openedAs(), screen.empty)
        describe(newlyShown, why, before != screen.index || reworded, shown).forEach(reader::perform)
    }

    // ---- what a key opens ---------------------------------------------------------------

    fun editor(
        name: String,
        initial: String,
        changed: (String, Int) -> Unit,
        confirmed: (String) -> Unit,
        cancelled: () -> Unit,
        completing: (() -> Boolean)? = null,
        complete: () -> Unit = {},
    ): LineEditor = LineEditor(
        name, initial, phoneKeypad, EditOutput(reader), CellBuffer.Translator { cells, mode -> liblouis.translate(cells, mode) },
        this,
        timeoutMs = { reader.characterCommitTimeout.toLong() },
        commitFeedback = { reader.characterCommitFeedback },
        changed = { text, caret ->
            changed(text, caret)
            top?.let { if (it is EditScreen) textView.text = "${it.title}\n\n${it.display()}" else if (it is DialogScreen) textView.text = "${it.title}\n\n$text" }
        },
        confirmed, cancelled, completing, complete,
    )

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
        pending?.let(main::removeCallbacks)
        pending = null
    }

    /** Choosing a file to import or restore: Android's own picker. */
    fun pickImport() = chooseFile()

    /** Set by the activity: opens Android's file picker, then the file goes to the app. */
    var chooseFile: () -> Unit = {}

    /** Where the next of [lines] starting with the cell's letter is, after [from]; a letter alone in a combo box. */
    fun findIn(lines: List<String>, from: Int, dots: Int): Int? {
        val letter = brailleLetter(dots.toUByte()) ?: return null
        return firstLetter(lines, BooleanArray(lines.size), from.coerceAtLeast(0).toUInt(), letter)?.toInt()
    }
}

/** Verbose logging for debug builds. */
object BuildConfigDebug {
    @Volatile var on = false
}

/** Tells Orbit's reader what the line editor did, as its own edit fields report. */
private class EditOutput(private val reader: OrbitReader) : LineEditor.Output {
    override fun focusEdit(name: String, content: String, cursor: Int) = reader.focusEdit(name, content, cursor)
    override fun syncEdit(name: String, content: String, cursor: Int) = reader.editChanged(name, content, cursor, "field_opened")
    override fun typed(character: Char, resolvedWord: String?) =
        reader.characterTyped(character.toString(), "", character.isUpperCase(), resolvedWord ?: "")
    override fun typedDots(cell: Int) = reader.speak(CellBuffer.dotNumbers(cell))
    override fun candidate(character: Char, field: String) = reader.characterTyped(character.toString(), field, false, null)
    override fun edited(name: String, content: String, cursor: Int, change: String) = reader.editChanged(name, content, cursor, change)
    override fun playError() = reader.sound(OrbitReader.SOUND_ERROR)
    override fun commitFeedback(character: Char, mode: Int) {
        if (mode == 1 || mode == 4) reader.sound(OrbitReader.SOUND_COMMIT)
        if (mode == 3 || mode == 5) reader.vibrate(50)
        if (mode == 2 || mode == 4 || mode == 5) reader.speakQueued(character.toString())
    }
    override fun modeChanged(name: String, content: String, cursor: Int, mode: String) =
        reader.editChanged(name, content, cursor, "t9_mode_changed:$mode")
}

/** [text], with the chord that also runs it said after it on the Speak. */
fun Ui.withLetter(text: String, letter: String?): String {
    val press = letter?.takeUnless { phoneKeypad }?.let(::speakPress) ?: return text
    return "$text, $press"
}

/** A menu command's name in sentence case, by its id in the core's table. */
fun titleOf(id: String): String = appWords.commands.firstOrNull { it.id == id }?.title ?: commandTitle(id)
