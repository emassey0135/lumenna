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
import dev.emassey0135.lumenna.app.ShellEvent
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

    open val isEdit: Boolean get() = false
}

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
    private var described: Screen? = null
    private var introduced = false
    private var landing = 0u
    private var pending: Runnable? = null

    val view: ViewModel get() = checkNotNull(app.view.value)
    val lines: Lines? get() = app.lines.value

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
        described = null
        redraw(Why.Entered)
    }

    /** A turn of the app has been taken: screens it opened or closed, then what it shows now. */
    fun turned() {
        val landed = view.landing > landing || sideLanded()
        follow()
        landing = view.landing
        redraw(if (landed) Why.Landed else Why.Turn)
        if (landed) core.release()
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
            described = null
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
            } else if (top is EditScreen) {
                // The same question still open: its answer was refused, and the field stays.
                (top as EditScreen).refused()
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
        described = null
    }

    // ---- keys ----------------------------------------------------------------------------

    /** A key from Orbit, on the main thread, in order. */
    fun key(event: KeyEvent, phoneKeypad: Boolean) {
        this.phoneKeypad = phoneKeypad
        val key = decoder.decode(event)
        if (key == Key.None) return
        val screen = top ?: return
        if (screen is EditScreen) {
            screen.key(key)
            return
        }
        if (screen.key(key)) return
        if (phoneKeypad && finder.phoneKey(key, screen)) return
        if (!phoneKeypad && key is Key.Cell) {
            finder.letter(brailleLetter(key.dots))
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
            // Left folds a row open, else goes back; Right opens a folded row, else opens it.
            Key.Left -> if (!foldRow(screen, rows, false)) back()
            Key.Right -> if (!foldRow(screen, rows, true)) open(screen, rows)
            Key.Select, Key.Dot8 -> open(screen, rows)
            Key.Dot7, Key.Back -> back()
            Key.Menu -> menu(screen, rows)
            is Key.Chord -> chord(key.dots, screen, rows)
            else -> {}
        }
    }

    private fun moveBy(screen: Screen, rows: List<Row>, delta: Int) {
        if (rows.isEmpty()) return
        moveTo(screen, rows, (screen.index + delta).coerceIn(0, rows.size - 1))
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
                    if (on != null) commands += Command(action.name) { send(ShellEvent.ActOn(on, action)) }
                }
        }
        commands += screen.commands()
        if (commands.isEmpty()) {
            refuse()
            return
        }
        push(ContextMenuScreen(row?.text ?: screen.title, commands))
    }

    private fun chord(dots: Int, screen: Screen, rows: List<Row>) {
        if (dots == DOTS_M) {
            menu(screen, rows)
            return
        }
        val letter = brailleLetter(dots)
        if (letter == null) {
            refuse()
            return
        }
        LetterCommands.run(this, letter)
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
        if (screen is EditScreen) {
            textView.text = "${screen.title}\n\n${screen.display()}"
            if (described !== screen) {
                described = screen
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
        val newlyShown = described !== screen
        // The row in hand reworded by what the person just did (a toggle, a field saved).
        val reworded = why == Why.Turn && !newlyShown && expectingChange && item != lastItem
        if (why == Why.Turn) expectingChange = false
        described = screen
        lastItem = item
        val shown = Shown(screen.title, rows.size, screen.index, item, screen.openedAs(), screen.dialogMessage(), screen.empty)
        describe(newlyShown, why, before != screen.index || reworded, shown).forEach(reader::perform)
    }

    /** What a dialog screen says of itself; a screen that is one overrides it. */
    private fun Screen.dialogMessage(): String = (this as? DialogMessage)?.message.orEmpty()

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
            top?.let { if (it is EditScreen) textView.text = "${it.title}\n\n${it.display()}" }
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

    /** Where a key's letter runs: for [LetterCommands]. */
    fun key(id: String) = send(ShellEvent.Key(id))

    companion object {
        private const val DOTS_M = 1 or (1 shl 2) or (1 shl 3)
    }
}

/** Verbose logging for debug builds. */
object BuildConfigDebug {
    @Volatile var on = false
}

/** A question screen's sentence said as it opens. */
interface DialogMessage {
    val message: String
}

/** A letter, from the dots of a cell: the core's, once merged (`braille_letter`). */
fun brailleLetter(dots: Int): Char? {
    val index = LETTER_DOTS.indexOf(dots and 0x3f)
    return if (index < 0) null else 'a' + index
}

private val LETTER_DOTS = intArrayOf(
    0x01, 0x03, 0x09, 0x19, 0x11, 0x0b, 0x1b, 0x13, 0x0a, 0x1a, 0x05, 0x07, 0x0d,
    0x1d, 0x15, 0x0f, 0x1f, 0x17, 0x0e, 0x1e, 0x25, 0x27, 0x3a, 0x2d, 0x3d, 0x35,
)

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

/**
 * Commands from Space with a letter: the first kind the row in hand offers of a letter's kinds,
 * refused in the core's words otherwise. Until the core's own table (`ShellEvent::Letter`) is here.
 * Space with M is the row's menu (Satellite's), so it runs nothing here.
 */
object LetterCommands {
    private val kinds: Map<Char, List<ActionKind>> by lazy {
        mapOf(
            'c' to listOf(ActionKind.MARK_DONE, ActionKind.MARK_NOT_DONE, ActionKind.COLOUR),
            'e' to listOf(ActionKind.EDIT, ActionKind.EDIT_TASK),
            'b' to listOf(ActionKind.PUT_IN_BLOCK),
            's' to listOf(ActionKind.START_TIMER, ActionKind.PAUSE_TIMER, ActionKind.RESUME_TIMER, ActionKind.MAKE_SUBTASK_OF),
            't' to listOf(ActionKind.STOP_TIMER, ActionKind.MOVE_TO_TOP_LEVEL),
            'w' to listOf(ActionKind.WAIT_FOR, ActionKind.WEIGHT),
            'n' to listOf(ActionKind.STOP_WAITING, ActionKind.NEW_INSIDE),
            'r' to listOf(ActionKind.RESTORE, ActionKind.RENAME, ActionKind.REMINDERS),
            'i' to listOf(ActionKind.ASSIGN_TASK),
            'x' to listOf(ActionKind.CANCEL_DAY),
            'o' to listOf(ActionKind.RESTORE_DAY),
            'l' to listOf(ActionKind.PLANNED_LENGTH),
            'a' to listOf(ActionKind.ADD_BLOCK),
            'f' to listOf(ActionKind.CARRY_FORWARD, ActionKind.CARRY_REST),
            'q' to listOf(ActionKind.CHANGE_QUERY),
        )
    }

    fun run(ui: Ui, letter: Char) {
        when (letter) {
            'd' -> ui.key("delete")
            'u' -> ui.send(ShellEvent.Undo)
            'y' -> ui.send(ShellEvent.Redo)
            'h' -> ui.push(KeyHelpScreen())
            else -> kinds[letter]?.let { ui.send(ShellEvent.Run(it)) } ?: ui.refuse()
        }
    }
}

/** A menu command's name in sentence case, by its id in the core's table. */
fun titleOf(id: String): String = appWords.commands.firstOrNull { it.id == id }?.title ?: commandTitle(id)
