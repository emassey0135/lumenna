package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.name
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.widget.TextView
import dev.emassey0135.lumenna.AppModel
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.Reminded
import dev.emassey0135.lumenna.replan
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Lines
import dev.emassey0135.lumenna.app.Offered
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
import dev.emassey0135.lumenna.app.noRowStartsWith
import dev.emassey0135.lumenna.app.nothingOnLetter
import dev.emassey0135.lumenna.app.KeyDevice
import dev.emassey0135.lumenna.app.LetterPress
import dev.emassey0135.lumenna.app.letterPress
import dev.emassey0135.lumenna.app.pressInstead
import dev.emassey0135.lumenna.app.withPress
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.commandTitle
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.orbit.BrailleInput
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.session.OrbitSession
import dev.emassey0135.orbit.OrbitWords
import dev.emassey0135.orbit.Reader
import dev.emassey0135.orbit.ui.Action
import dev.emassey0135.orbit.ui.HelperText
import dev.emassey0135.orbit.ui.Item
import dev.emassey0135.orbit.ui.Letters
import dev.emassey0135.orbit.ui.ListScreen
import dev.emassey0135.orbit.ui.MenuBar
import dev.emassey0135.orbit.ui.OrbitUi
import dev.emassey0135.orbit.ui.Screen
import dev.emassey0135.orbit.ui.UiWords

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

/** Whose list a screen shows: the app's main one, one beside it, or none of the app's. */
enum class Lands { MAIN, SIDE, NONE }

/** One of this app's screens, a list ([AppList]) or a dialog ([CoreDialog]), as [Ui] knows it. */
interface AppScreen {
    /** This app's side of the screens: the view, and where what the person does is sent. */
    var host: Ui

    /** Which of the app's lists this screen shows, if one: what a letter and a landing are. */
    val lands: Lands get() = Lands.NONE
}

/** [offered] as the SDK's actions, in the core's words and letters, each run on [on]; none without it. */
fun offeredActions(host: Ui, offered: List<Offered>, on: ActingOn?): List<Action> {
    if (on == null) return emptyList()
    return offered.filter { it.kind != ActionKind.EXPAND && it.kind != ActionKind.COLLAPSE }.map { action ->
        val letter = actionLetter(action.kind)
        Action(host.withLetter(action.name, letter), letter?.singleOrNull()) { host.send(ShellEvent.ActOn(on, action)) }
    }
}

/** [row] as the SDK's list has it: its actions in the core's words and letters, and its folding. */
private fun Row.item(host: Ui): Item {
    val on = on
    val actions = offeredActions(host, offered, on)
    val fold: ((Boolean) -> Unit)? = if (expanded == null || on == null) null else { open ->
        val wanted = if (open) ActionKind.EXPAND else ActionKind.COLLAPSE
        offered.firstOrNull { it.kind == wanted }?.let { host.send(ShellEvent.ActOn(on, it)) }
    }
    return Item(key, text, open, actions, letter?.singleOrNull(), expanded = expanded, fold = fold, title = title)
}

/**
 * One of this app's screens on the SDK's stack ([OrbitUi]): a list of [Row]s read from the view the
 * app last left each time it is read, so a change a sync brings in is drawn without the screen
 * doing anything. The stack decides what is told when; a screen says what it shows ([rows]), what
 * the app holds in hand there ([inHand]), and what it sends as the person moves and acts.
 */
abstract class AppList : ListScreen("", { emptyList() }, ""), AppScreen {
    override lateinit var host: Ui

    abstract override val title: String

    /** What an empty list says, in the core's words. */
    override val empty: String get() = appWords.nothingHere

    override val lands: Lands get() = Lands.NONE

    /** Whether the core's screen actions are this screen's (a place, Home): its menu is the menu bar. */
    open val showsScreenActions: Boolean get() = false

    abstract fun rows(): List<Row>

    /** The screen's own entries in the context menu, after the row's. */
    open fun commands(): List<Command> = emptyList()

    /** The person moved to [row]: the app is told, so its details and commands follow. */
    open fun moved(row: Row) {}

    /** Whether what the screen shows has arrived: the place it opened, once the app has opened it. */
    open fun loaded(view: ViewModel): Boolean = true

    /** Whether this screen still stands: a question the app has answered, for one. */
    open fun stands(view: ViewModel): Boolean = true

    /** The screen came to the top, first or coming back: send what shows it. */
    open fun resumed() {}

    /** Leaving the screen: send what puts its state away. */
    open fun leaving() {}

    /** Back was pressed: true if the screen keeps it for now (a form that discards first). */
    open fun beforeBack(): Boolean = false

    /** A letter only this screen knows (Space with it); true if it took it. */
    open fun letter(letter: String): Boolean = false

    /** Said once as the screen opens, when it is a question rather than a list. */
    open fun openedAs(): Opened = Opened.List

    private var shownRows: Map<String, Row> = emptyMap()

    /** The row in hand, as this app built it. */
    val currentRow: Row? get() = current?.let { shownRows[it.key] }

    override fun items(): List<Item> {
        val made = rows()
        shownRows = made.associateBy { it.key }
        return made.map { it.item(host) }
    }

    // Nothing is told until the store has arrived and the screen's place has opened: an empty
    // list said on the way there would be wrong.
    override val loaded: Boolean get() = host.view.ready && loaded(host.view)

    override fun stands(): Boolean = stands(host.view)

    override fun onTop() = resumed()

    override fun closed() = leaving()

    override fun back(): Boolean = beforeBack()

    override fun moved(item: Item) {
        shownRows[item.key]?.let { moved(it) }
    }

    override fun actions(): List<Action> = commands().map { command -> Action(command.text, command.letter?.singleOrNull()) { command.run() } }

    override fun menuBar(): MenuBar? = if (showsScreenActions) host.menuBar(this) else null

    override fun tellShown() {
        if (openedAs() != Opened.Dialog) return super.tellShown()
        // Orbit says the dialog; the answer in hand is then an item of its list.
        rebuild()
        reader.dialog(title, "", current?.text.orEmpty())
        tellMoved()
    }

    /**
     * Space (or Dot 8) with a letter: a row's own letter, this screen's, a screen action this app
     * runs itself; then on the app's own lists the core's `Letter` on the row in hand (D the delete
     * key, which the core's table does not have). Elsewhere the stack's actions of that letter.
     */
    override fun letter(letter: Char): Boolean {
        val text = letter.toString()
        rows().firstOrNull { it.letter == text }?.open?.let { opening ->
            opening()
            return true
        }
        if (this.letter(text)) return true
        if (showsScreenActions) {
            host.view.screenActions.firstOrNull { it.shell && it.enabled && it.letter == text }?.let {
                host.runScreenAction(it)
                return true
            }
        }
        if (lands == Lands.NONE) return false
        // The app acts on the row in hand: a list beside the main one's is held first, so it answers there.
        currentRow?.let { moved(it) }
        host.send(if (text == "d") ShellEvent.Key("delete") else ShellEvent.Letter(text))
        return true
    }

    /** A letter alone: the core finds it on its own lists (`FindLetter`), its `first_letter` on this app's. */
    override fun findLetter(letter: Char): Boolean {
        val text = letter.toString()
        if (lands == Lands.MAIN) {
            host.send(ShellEvent.FindLetter(text))
            return true
        }
        val at = firstLetter(rows.map { it.text }, booleanArrayOf(), index.toUInt(), text)?.toInt()
        when {
            at == null -> host.stack.refused(noRowStartsWith(text))
            at == index -> tellMoved()
            else -> moveTo(at, byPerson = true)
        }
        return true
    }
}

/**
 * Which key presses a letter, by the core (`letter_press` for the Speak): Space with it, or Dot 8
 * with it where the Speak keeps Space with those dots, Space with such a letter saying so
 * (`press_instead`). A letter is the core's `braille_letter`, a to z, none with dot 7 or 8.
 */
object CoreLetters : Letters() {
    override fun of(dots: Int): Char? = brailleLetter(dots.toUByte())?.singleOrNull()

    override fun pressed(key: Key): Letters.Pressed = when (key) {
        is Key.Chord -> {
            val letter = brailleLetter(key.dots.toUByte())
            when {
                letter == null -> Letters.Pressed.Refused(null)
                letterPress(KeyDevice.ORBIT_SPEAK, letter) == LetterPress.SPACE -> letter.singleOrNull()?.let { Letters.Pressed.Letter(it) } ?: Letters.Pressed.Refused(null)
                else -> Letters.Pressed.Refused(pressInstead(KeyDevice.ORBIT_SPEAK, letter))
            }
        }
        is Key.Enter -> of(key.dots)?.let { Letters.Pressed.Letter(it) } ?: Letters.Pressed.Refused(null)
        else -> Letters.Pressed.None
    }
}

/**
 * This app's side of the screens, over the SDK's stack ([stack]), which takes the keys, keeps the
 * screens and tells Orbit's reader what is shown when. What is in hand, what a key runs and what a
 * question asks are the app's; this follows the app's view each turn (the screens the app opens and
 * closes, the place it moves to) and says when the app moved what is in hand ([turned]).
 */
class Ui(
    val app: AppModel,
    val core: Core,
    session: OrbitSession,
    textView: TextView,
    private val finish: () -> Unit,
) {
    private val main = Handler(Looper.getMainLooper())

    /** The Speak's braille input, the session's. */
    val braille: BrailleInput = session.braille

    /** What the device says in its own words: its fields' echo, T9's modes. */
    val orbitWords = OrbitWords()

    /** The screens, the keys, and what Orbit is told, wired into the session. */
    val stack = OrbitUi.on(
        session,
        reader = HelperText(session.reader, orbitWords) { textView.text = it },
        orbitWords = orbitWords,
        letters = CoreLetters,
        words = UiWords(
            ok = appWords.save,
            cancel = appWords.cancel,
            empty = appWords.nothingHere,
            lettersOn = appWords.orbit.lettersOn,
            lettersOff = appWords.orbit.lettersOff,
            noRowStartsWith = { noRowStartsWith(it.toString()) },
            nothingOnLetter = { nothingOnLetter(it.toString()) },
        ),
    )

    init {
        // Back from Home, the last screen, leaves Lumenna, on both devices.
        stack.onEmpty = finish
        // A device with no translator is the Player until a key says otherwise.
        stack.phoneKeypad = !braille.available
    }

    /** What Orbit is told through. */
    val reader: Reader get() = stack.reader

    /** Whether the keys are the Player's phone keypad, as the last key said. */
    val phoneKeypad: Boolean get() = stack.phoneKeypad

    val view: ViewModel get() = checkNotNull(app.view.value)
    val lines: Lines? get() = app.lines.value

    /** The core's description of the screen, as of the last turn (`Turn::described`). */
    var screenView: ScreenView = app.screenView()
        private set

    /** The question the core says is open, if one is. */
    val questionShown: QuestionShown? get() = (screenView.shown as? Shown.Question)?.field0

    /** The list the core says is showing, if one is. */
    val listShown: ListShown? get() = (screenView.shown as? Shown.List)?.field0

    /**
     * The main list as the app last described it, for a place or the day: none while the description
     * is of a list beside it (a task's reminders), which a place shown again under it, before the app
     * has heard that list closed, must not take for its own.
     */
    val mainShown: ListShown? get() = listShown?.takeIf { it.side == null }

    val top: Screen? get() = stack.top

    /** This app's screens know it before the stack shows them. */
    private fun adopt(screen: Screen): Screen {
        (screen as? AppScreen)?.host = this
        return screen
    }

    fun push(screen: Screen) = stack.push(adopt(screen))

    /** Takes [screen] off if it is on the stack. */
    fun remove(screen: Screen) = stack.close(screen)

    fun pop() {
        stack.top?.let { stack.close(it) }
    }

    /** A reminder's notification was tapped: where it leads. */
    fun reminded(reminded: Reminded) {
        when (reminded) {
            is Reminded.Task -> push(DetailsScreen(reminded.id))
            is Reminded.Block -> {
                reminded.on?.let { send(ShellEvent.ShowBlock(it.series, it.date)) }
                if (stack.screens.none { it is DayScreen }) push(DayScreen())
            }
            is Reminded.Replan -> app.replan(reminded)
        }
    }

    /** Back, from the activity's own back: as the keys' Back. */
    fun back() {
        stack.top?.let { stack.back(it) }
    }

    /** Sends what the person did: the row they are on may then read otherwise, and is read again if it does. */
    fun send(event: ShellEvent) {
        (stack.top as? ListScreen)?.expect()
        app.send(event)
    }

    fun say(text: String) = stack.say(text)

    /** What the core says of a change, after what is told of it. */
    fun announce(text: String) = stack.announce(text)

    fun refuse() = stack.refuse()

    fun later(milliseconds: Long, run: () -> Unit) {
        main.postDelayed(Runnable { run() }, milliseconds)
    }

    // ---- following the app -------------------------------------------------------------------

    private var landing = 0u

    /**
     * A turn of the app has been taken: the screens it opened or closed, then what it shows now,
     * told only where the app moved what is in hand on the list shown (the core's `Moved` on the
     * main list, a landing on one beside it), then its refusals.
     */
    fun turned(described: ScreenView) {
        screenView = described
        val moved = described.changes.any { it is ScreenChange.Moved }
        val landedMain = view.landing > landing
        val landedSide = sideLanded()
        landing = view.landing
        stack.batch { follow() }
        // A list behind the screen on top landing says nothing: it isn't what is shown.
        val landed = when ((stack.top as? AppScreen)?.lands) {
            Lands.MAIN -> moved
            Lands.SIDE -> landedSide
            else -> false
        }
        if (landed) stack.land() else stack.refresh()
        if (landedMain || landedSide) core.release()
        // Words already coming through what the turn said are not said twice.
        described.changes.filterIsInstance<ScreenChange.Refused>().forEach {
            stack.refused(if (it.said) null else it.reason)
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
        val screens = stack.screens
        val at = screens.indexOfLast { it is ShowsPlace || it is HomeScreen }
        if (at < 0) return
        val showing = screens[at]
        when {
            showing is HomeScreen && left -> stack.insert(adopt(screenFor(place)), above = showing)
            showing is ShowsPlace && moved && showing.place != place -> stack.replace(showing, adopt(screenFor(place)))
        }
    }

    private fun screenFor(place: dev.emassey0135.lumenna.app.Place): Screen =
        if (place is dev.emassey0135.lumenna.app.Place.Today) DayScreen()
        else PlaceScreen(place, view.sidebar.firstOrNull { (it.kind as? dev.emassey0135.lumenna.app.SidebarKind.Place)?.field0 == place }?.title.orEmpty())

    /** The screens the app has open: a question, the block form, a proposal, a reminder list. */
    private fun follow() {
        val v = view
        followPlace(v)
        // What the app has put away goes first, from the top.
        while (true) {
            val screen = stack.top ?: break
            if (screen.stands()) break
            stack.close(screen)
        }
        followRinging(v)
        v.asking?.let { asking ->
            // Open already, under something it opened (its completions).
            if (stack.screens.any { (it as? AskingScreen)?.number == asking.number } && stack.top !is AskingScreen) return
            // Every question is its dialog, a task's form asked for among them: the app shows that
            // task's details as it asks.
            val open = stack.top as? AskingScreen
            if (open == null || open.number != asking.number) {
                if (open is Screen) stack.close(open)
                push(QuestionScreen(asking))
            }
            return
        }
        // Only when none is open: a field being typed sits above the form. One just closed here is
        // still open in the view until the app has the close, and is not opened again meanwhile.
        if (v.line == null) lineClosed = false
        if (v.settings == null) settingsClosed = false
        val screens = stack.screens
        // Settings the core opened (Home's Settings, its letter): shown here too.
        if (v.settings != null && !settingsClosed && screens.none { it is SettingsScreen }) push(SettingsScreen())
        if (v.line != null && !lineClosed && screens.none { it is QuickAddScreen }) push(QuickAddScreen())
        if (v.blockForm != null && screens.none { it is BlockFormScreen }) push(BlockFormScreen())
        if (v.proposal != null && screens.none { it is ProposalScreen }) push(ProposalScreen())
        if (v.reminders != null && screens.none { it is RemindersScreen }) push(RemindersScreen())

    }

    /** Quick add was left here, and the app has not yet said its line is closed. */
    var lineClosed = false

    /** Looks back over the last day for reminders due, so Home offers them: as Home opens, and as the app comes to the front. */
    fun lookBack() = send(ShellEvent.Due(DueEvent.LookBack))

    /** A reminder that went off and brought the app to the front, until the app holds it, and when it came. */
    private var ringing: Triple<dev.emassey0135.lumenna.Ringing, Long, Boolean>? = null

    /**
     * A reminder went off and brought the app to the front: the app looks over the moments the
     * alarm found it between, not by its own clock, which does not tick in the background and has
     * not reached it; once the app's reminders due hold it, it is said as its dialog
     * ([RingingScreen]), in the words its notification had. One not found within a while (dealt
     * with on another device meanwhile) is let go, the while counted on a clock that runs on as the
     * device sleeps, so a ring is never said when the device next wakes.
     */
    fun ring(firing: dev.emassey0135.lumenna.Ringing, fromBackground: Boolean = false) {
        ringing = Triple(firing, SystemClock.elapsedRealtime(), fromBackground)
        send(ShellEvent.Due(DueEvent.Look(firing.since, firing.now)))
    }

    private fun followRinging(v: ViewModel) {
        val (firing, came, fromBackground) = ringing ?: return
        if (SystemClock.elapsedRealtime() - came > RINGING_MS) {
            ringing = null
            return
        }
        val due = v.due.firstOrNull { due -> due.firings.any { it.key == firing.key && it.occurrence == firing.occurrence } } ?: return
        ringing = null
        if (stack.screens.none { it is RingingScreen && it.id == due.id }) push(RingingScreen(due.id, fromBackground))
    }

    /** Settings were left here, and the app has not yet said they are closed. */
    var settingsClosed = false

    // ---- what a key opens ---------------------------------------------------------------------

    /**
     * Space with M, the Player's Menu, on a place, the day or Home: the core's menu bar
     * (`braille_menu_bar_on_row`), the row in hand's actions its first menu, as Orbit's own apps
     * have one menu key for both.
     */
    fun menuBar(screen: AppList): MenuBar {
        val row = screen.currentRow
        val home = screen is HomeScreen
        val place = if (home) null else view.place
        val on = row?.on
        val offered = row?.offered.orEmpty().takeIf { on != null }.orEmpty()
        val bar = if (row != null && offered.isNotEmpty()) brailleMenuBarOnRow(place, home, true, row.title ?: row.text, offered)
        else brailleMenuBar(place, home, true)
        val hot = mutableMapOf<Char, () -> Unit>()
        val items = menuBarItems(bar, view, on, ::withLetter, hot) { runBarChoice(it) }
        return MenuBar(items, hot)
    }

    /** Back to Home from whatever is shown over it, which says itself again. */
    fun goHome() {
        stack.batch {
            while (stack.screens.size > 1 && stack.top !is HomeScreen) stack.top?.let { stack.close(it) }
            follow()
        }
    }

    /** Leaving Lumenna, as Back from Home does. */
    fun quit() = finish()

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
            "about" -> push(AboutScreen())
            else -> refuse()
        }
    }

    /** Choosing a file to import or restore, in Orbit's File Manager. */
    fun pickImport() = chooseFile()

    /** Set by the activity: opens Orbit's File Manager, then the file goes to the app. */
    var chooseFile: () -> Unit = {}

    /**
     * Set by the activity: the folder setting [key], now [current], chosen in Orbit's File Manager
     * and set to the folder chosen.
     */
    var chooseFolder: (key: String, current: String) -> Unit = { _, _ -> refuse() }

    /** Set by the activity: what Android allows reminders here, asked of the system each time. */
    var reminderAccess: () -> dev.emassey0135.lumenna.ReminderAccess = { dev.emassey0135.lumenna.ReminderAccess(true, true, null) }

    /** Set by the activity: opens the app's own page in Android's settings, where its background use is allowed. */
    var allowBackground: () -> Unit = { refuse() }

    /** Set by the activity: back to the app that was in front before a reminder brought this one to the front. */
    var returnToPrevious: () -> Unit = {}
}

/** Verbose logging for debug builds. */
object BuildConfigDebug {
    @Volatile var on = false
}

/** [text], with the chord that also runs it said after it on the Speak, as the core joins them (`with_press`). */
fun Ui.withLetter(text: String, letter: String?): String {
    if (letter == null || phoneKeypad) return text
    return withPress(KeyDevice.ORBIT_SPEAK, text, letter)
}

/** A menu command's name in sentence case, by its id in the core's table. */
fun titleOf(id: String): String = appWords.commands.firstOrNull { it.id == id }?.title ?: commandTitle(id)

/** How long a reminder that brought the app to the front is waited for among the reminders due. */
private const val RINGING_MS = 15_000L
