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
import dev.emassey0135.lumenna.app.DueEvent
import dev.emassey0135.lumenna.app.ScreenAction
import dev.emassey0135.lumenna.app.ScreenView
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Shown
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.brailleLetter
import dev.emassey0135.lumenna.app.brailleMenuBar
import dev.emassey0135.lumenna.app.brailleMenuBarOnRow
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
import dev.emassey0135.orbit.BrailleInput
import dev.emassey0135.orbit.CellBuffer
import dev.emassey0135.orbit.EditBox
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.KeyDecoder
import dev.emassey0135.orbit.KeyEvent
import dev.emassey0135.orbit.OrbitList
import dev.emassey0135.orbit.OrbitMenu
import dev.emassey0135.orbit.OrbitReader
import dev.emassey0135.orbit.OrbitWords
import dev.emassey0135.orbit.ReaderEditOutput
import dev.emassey0135.orbit.T9Input

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
    /** Its name alone (its line's `title`), which the menu of its actions is named for. */
    val title: String? = null,
)

/** An entry of a context menu: a row's action or the screen's own, and the letter that runs it. */
class Command(val text: String, val letter: String? = null, val run: () -> Unit)

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

    /** Whether the core's screen actions are this screen's (a place, Home): their letters run here. */
    open val showsScreenActions: Boolean get() = false

    /** Which of the app's lists this screen shows, if one: what a letter and a landing are. */
    open val lands: Lands get() = Lands.NONE
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
    val braille: BrailleInput,
    private val textView: TextView,
    private val finish: () -> Unit,
) : T9Input.Later {
    private val stack = ArrayDeque<Screen>()
    private val main = Handler(Looper.getMainLooper())
    private val decoder = KeyDecoder()
    /** What the device says in its own words: its fields' echo, T9's modes. */
    val orbitWords = OrbitWords()
    private val editOutput = ReaderEditOutput(reader, braille, textView.context, orbitWords)
    private val finder = FirstLetters(this)
    /** Whether the keys are the Player's phone keypad, as the last key said; at first, a device with no translator. */
    var phoneKeypad = !braille.available
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

    /**
     * A menu closed to run what was chosen in it: the screen under it, said before the menu opened,
     * is not said again, and what was chosen says what it did.
     */
    fun closeMenu(menu: Screen, run: () -> Unit) {
        if (!stack.contains(menu)) return
        while (top !== menu) stack.removeLast().leaving()
        stack.removeLast().leaving()
        val under = top
        saidScreen = under
        // What it showed before the menu: a change the choice makes to it is said, nothing else.
        lastItem = under?.let { screen -> screen.rows().getOrNull(screen.index)?.text }.orEmpty()
        run()
        if (top === under) {
            follow()
            redraw(Why.Turn)
        }
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

    /** Whether Home was shown as of the last turn, and the place the app was on. */
    private var wasHome = false
    private var lastPlace: dev.emassey0135.lumenna.app.Place? = null

    /**
     * Where the app went: Home left for a place (Enter, a letter, What Should I Work On?) shows that
     * place over Home, and the app moving from the place shown (a project renamed or deleted, today
     * from anywhere) shows where it went in its stead.
     */
    private fun followPlace(v: ViewModel) {
        val home = v.home != null
        val place = v.place
        val left = wasHome && !home
        val moved = lastPlace != null && place != lastPlace
        wasHome = home
        lastPlace = place
        if (home || !v.ready || place == null) return
        val at = stack.indexOfLast { it is ShowsPlace || it is HomeScreen }
        if (at < 0) return
        val showing = stack[at]
        when {
            showing is HomeScreen && left -> insertAbove(at, screenFor(place))
            showing is ShowsPlace && moved && showing.place != place -> {
                showing.leaving()
                val next = screenFor(place)
                next.ui = this
                stack[at] = next
                next.resumed()
                saidScreen = null
            }
        }
    }

    private fun screenFor(place: dev.emassey0135.lumenna.app.Place): Screen =
        if (place is dev.emassey0135.lumenna.app.Place.Today) DayScreen()
        else PlaceScreen(place, view.sidebar.firstOrNull { (it.kind as? dev.emassey0135.lumenna.app.SidebarKind.Place)?.field0 == place }?.title.orEmpty())

    private fun insertAbove(at: Int, screen: Screen) {
        screen.ui = this
        stack.add(at + 1, screen)
        screen.resumed()
        saidScreen = null
    }

    /** The screens the app has open: a question, the block form, a proposal, a reminder list. */
    private fun follow() {
        val v = view
        followPlace(v)
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
        // Only when none is open: a field being typed sits above the form. One just closed here is
        // still open in the view until the app has the close, and is not opened again meanwhile.
        if (v.line == null) lineClosed = false
        if (v.settings == null) settingsClosed = false
        // Settings the core opened (Home's Settings, its letter): shown here too.
        if (v.settings != null && !settingsClosed && stack.none { it is SettingsScreen }) pushApp(SettingsScreen())
        if (v.line != null && !lineClosed && stack.none { it is QuickAddScreen }) pushApp(QuickAddScreen())
        if (v.blockForm != null && stack.none { it is BlockFormScreen }) pushApp(BlockFormScreen())
        if (v.proposal != null && stack.none { it is ProposalScreen }) pushApp(ProposalScreen())
        if (v.reminders != null && stack.none { it is RemindersScreen }) pushApp(RemindersScreen())
    }

    /** Quick add was left here, and the app has not yet said its line is closed. */
    var lineClosed = false

    /** Looks back over the last day for reminders due, so Home offers them: as Home opens, and as the app comes to the front. */
    fun lookBack() = send(ShellEvent.Due(DueEvent.LookBack))

    /** Settings were left here, and the app has not yet said they are closed. */
    var settingsClosed = false

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
        pushApp(QuestionScreen(asking))
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
        // Up, Down, the ends, Tab and Shift-Tab: as Orbit's lists move.
        OrbitList.target(key, screen.index, rows.size)?.let { return moveTo(screen, rows, it) }
        when (key) {
            Key.Up, Key.Down, Key.First, Key.Last, Key.Tab, Key.BackTab -> {}
            // Left folds a row open, else goes back out of a list, never a dialog; Right opens a
            // folded row, else opens it.
            Key.Left -> if (!foldRow(screen, rows, false) && screen !is DialogScreen) back()
            Key.Right -> if (!foldRow(screen, rows, true)) open(screen, rows)
            Key.Select, Key.Dot8 -> open(screen, rows)
            Key.Dot7, Key.Back, Key.Escape -> back()
            Key.Menu -> if (screen.showsScreenActions) menuBar(screen, rows) else menu(screen, rows)
            is Key.Chord -> spaceLetter(key.dots, screen, rows)
            is Key.Enter -> brailleLetter(key.dots.toUByte())?.let { runLetter(it, screen, rows) } ?: refuse()
            else -> {}
        }
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
                    val letter = actionLetter(action.kind)
                    if (on != null) commands += Command(withLetter(action.name, letter), letter) { send(ShellEvent.ActOn(on, action)) }
                }
        }
        commands += screen.commands()
        if (commands.isEmpty()) {
            refuse()
            return
        }
        push(ContextMenuScreen(appWords.braille.actionsMenu, commands))
    }

    /**
     * Space with M, the Player's Menu, on a place, the day or Home: the core's menu bar
     * (`braille_menu_bar_on_row`), the row in hand's actions its first menu, as Orbit's own apps
     * have one menu key for both ([MenuBarScreen]).
     */
    fun menuBar(screen: Screen, rows: List<Row>) {
        val row = selected(screen, rows)
        val home = screen is HomeScreen
        val place = if (home) null else view.place
        val on = row?.on
        val offered = row?.offered.orEmpty().takeIf { on != null }.orEmpty()
        val bar = if (row != null && offered.isNotEmpty()) brailleMenuBarOnRow(place, home, true, row.title ?: row.text, offered)
        else brailleMenuBar(place, home, true)
        val hot = mutableMapOf<String, BarChoice>()
        val items = menuBarItems(bar, view, on, ::withLetter, hot)
        if (items.isEmpty()) {
            refuse()
            return
        }
        push(MenuBarScreen(OrbitMenu(items), hot))
    }

    /** Back to Home from whatever is shown over it, which says itself again. */
    fun goHome() {
        while (stack.size > 1 && top !is HomeScreen) stack.removeLast().leaving()
        top?.resumed()
        saidScreen = null
        follow()
        redraw(Why.Entered)
    }

    /** Leaving Lumenna, as Back from Home does. */
    fun quit() = finish()

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
        // This app's own lists first (a menu's commands, the settings pages), then the screen's.
        rows.firstOrNull { it.letter == letter }?.let { row ->
            row.open?.invoke()
            return
        }
        if (screen.letter(letter)) return
        // The screen's own actions that are this app's to run (the filter, Key Help, About).
        if (screen.showsScreenActions) {
            view.screenActions.firstOrNull { it.shell && it.enabled && it.letter == letter }?.let {
                runScreenAction(it)
                return
            }
        }
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

    /** A screen action: the core's `Key(id)`, but one that is this app's own to run. */
    fun runScreenAction(action: ScreenAction) {
        if (!action.shell) {
            send(ShellEvent.Key(action.id))
            return
        }
        when (action.id) {
            "filter" -> view.place?.let { push(FilterScreen(it)) } ?: refuse()
            "keyboard-help" -> push(KeyHelpScreen())
            "reminders-due" -> push(DueScreen())
            "about" -> appWords.metadata.let { say("${it.name} ${it.version}. ${it.description}") }
            else -> refuse()
        }
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
        if (screen is MenuBarScreen) {
            val newlyShown = saidScreen !== screen
            saidScreen = screen
            screen.draw(newlyShown, why, textView)
            return
        }
        if (screen is DialogScreen && screen.lineFills && phoneKeypad && screen.rows().isEmpty()) {
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
        // The person's own move wins until the app has it: its row in hand is the one before.
        val held = if (why == Why.Moved) null else screen.inHand()
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
        // The app landing on the row just said (Home's row in hand as it comes back) says nothing more.
        val landedAgain = why == Why.Landed && !newlyShown && item == lastItem && before == screen.index
        saidScreen = screen
        lastItem = item
        if (screen is DialogScreen && screen.inPlace) {
            // A value it changes is said as it changes: nothing to repeat when the app has it.
            screen.describeControl(newlyShown, why, before != screen.index)
            return
        }
        val shown = Drawn(screen.title, rows.size, screen.index, item, screen.openedAs(), screen.empty)
        describe(newlyShown, if (landedAgain) Why.Turn else why, before != screen.index || reworded, shown).forEach(reader::perform)
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
        multiline: Boolean = false,
    ): EditBox = EditBox(
        name, initial, phoneKeypad, editOutput, multiline, CellBuffer.Translator { cells, mode -> braille.translate(cells, mode) },
        this,
        timeoutMs = { reader.characterCommitTimeout.toLong() },
        commitFeedback = { reader.characterCommitFeedback },
        changed = { text, caret ->
            changed(text, caret)
            top?.let { if (it is DialogScreen) textView.text = "${it.title}\n\n${if (it.lineFills && phoneKeypad) it.display() else text}" }
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

/** [text], with the chord that also runs it said after it on the Speak. */
fun Ui.withLetter(text: String, letter: String?): String {
    val press = letter?.takeUnless { phoneKeypad }?.let(::speakPress) ?: return text
    return "$text, $press"
}

/** A menu command's name in sentence case, by its id in the core's table. */
fun titleOf(id: String): String = appWords.commands.firstOrNull { it.id == id }?.title ?: commandTitle(id)
