package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.edit.Clipboard
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.output.BrailleDisplay
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.dialogTitle
import dev.emassey0135.braillesense.braille.Echo
import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.braillesense.ui.BrailleSenseUi
import dev.emassey0135.braillesense.ui.Frame
import dev.emassey0135.braillesense.ui.LineFrame
import dev.emassey0135.braillesense.ui.ListFrame
import dev.emassey0135.braillesense.ui.Letters
import dev.emassey0135.braillesense.ui.MenuEntry
import dev.emassey0135.braillesense.ui.MenuItem
import dev.emassey0135.braillesense.ui.Readings
import dev.emassey0135.braillesense.ui.Rendered
import dev.emassey0135.braillesense.ui.Row
import dev.emassey0135.braillesense.ui.Screen
import dev.emassey0135.braillesense.ui.UiWords
import dev.emassey0135.lumenna.AppModel
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.commandTitle
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.offeredOf
import dev.emassey0135.lumenna.on
import dev.emassey0135.lumenna.shownRows
import dev.emassey0135.lumenna.text
import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ControlSends
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.braille.DialogController
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Answer
import dev.emassey0135.lumenna.app.Asks
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.DayKind
import dev.emassey0135.lumenna.app.DetailsEvent
import dev.emassey0135.lumenna.app.DevicesEvent
import dev.emassey0135.lumenna.app.Lines
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ProposalEvent
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.ScreenChange
import dev.emassey0135.lumenna.app.ScreenView
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.brailleMenuBar
import dev.emassey0135.lumenna.app.sidebarKey
import dev.emassey0135.lumenna.app.commandLetter
import dev.emassey0135.lumenna.app.dueLetter
import dev.emassey0135.lumenna.app.nothingOnLetter
import dev.emassey0135.lumenna.app.noRowStartsWith
import dev.emassey0135.lumenna.app.pageLetter
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.Typing
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.KeyDevice
import dev.emassey0135.lumenna.app.Line
import dev.emassey0135.lumenna.app.Syntax
import dev.emassey0135.lumenna.app.detailWithState
import dev.emassey0135.lumenna.app.lineWithState
import dev.emassey0135.lumenna.app.promptOf
import dev.emassey0135.lumenna.app.settingsPageOf

/** What only the activity can do. */
interface Host {
    fun exit()

    fun importFile()

    /** The folder setting [key], now [current], chosen in the device's File Manager and set to the folder chosen. */
    fun chooseFolder(key: String, current: String)
    fun requestNotifications()
    fun openNotificationSettings()
    fun deviceName(): String

    /** Whether the app may come to the front as a reminder goes off (the SDK's `Foreground.allowed`). */
    fun comesToFront(): Boolean

    /** Android's page where the app is allowed to come to the front. */
    fun allowFront()

    /** The app's own page in Android's settings, where its background use is allowed. */
    fun allowBackground()

    /** Back to the app that was in front before a reminder brought this one to the front. */
    fun returnToPrevious()
}

/** A page the person went to, on the way back from which Escape walks. */
private sealed interface Page {
    /** The places, then what can be done from anywhere: the core's Home, where the app opens. */
    data object Home : Page
    data object Listing : Page
    data class Task(val id: String) : Page
    data object Settings : Page
    data class SettingsPage(val id: String) : Page
    data object Devices : Page
    data object Due : Page
    data object Help : Page
    data object About : Page

    /** A reminder that has just gone off and brought the app to the front, by its `Due::id`. */
    data class Ringing(val id: String, val fromBackground: Boolean = false) : Page

    /** Asked once as the app opens: what holds its reminders back, [background] use restricted, not allowed to come to the [front]. */
    data class HeldBack(val background: Boolean, val front: Boolean) : Page
}

/**
 * The screens, over the Crux app's view model, on the BrailleSense SDK's screen stack
 * ([BrailleSenseUi]): what is shown is the app's, this decides only how it is laid out on a line at
 * a time. The stack takes the keys, the menus, the edit boxes, panning and routing, and decides
 * when to speak; this follows the core's view each turn.
 *
 * Each page the person goes to is a [PageScreen], and what the app opens itself (a question, the
 * block form, quick add, a proposal, a reminder list) one [OpenedScreen] over them, which goes as the
 * app closes it. Each turn the screen in front is laid out afresh from the view; it is said when it
 * is new or the app moved what is in hand (`landing`), and only put on the cells otherwise, so a
 * sync or a new minute never cuts off what is being read. All on the main thread.
 */
class Shell(
    private val core: Core,
    out: BrailleDisplay,
    private val translator: Translator,
    private val host: Host,
    private val readings: Readings,
    /** What an edit box copies to and pastes from. */
    clipboard: Clipboard,
    private val letters: Letters = BrailleLetters,
    /** How long a message stays on the cells: the device's Message Display Time. */
    messageMillis: () -> Long = { 3000L },
    /** Automatically Open Menus with Shortcut: whether a letter opens what it names or only goes to it. */
    private val opensOnLetter: () -> Boolean = { true },
    echoSetting: () -> Echo = { Echo.of(1) },
) {
    private val app: AppModel get() = core.app

    private val ui = BrailleSenseUi(
        out, translator, clipboard, readings,
        echo = echoSetting, messageMillis = messageMillis, opensOnLetter = opensOnLetter, letters = letters,
        words = UiWords(
            cancel = appWords.cancel,
            yes = appWords.yesNo.firstOrNull { it.id == "yes" }?.title ?: "Yes",
            no = appWords.yesNo.firstOrNull { it.id == "no" }?.title ?: "No",
            opensNothing = appWords.braille.opensNothing,
            noActions = appWords.braille.noActions,
            noRowStartsWith = { noRowStartsWith(it.toString()) },
            nothingOnLetter = { nothingOnLetter(it.toString()) },
        ),
    )

    /** Home asked for and not shown yet: until it is, the app's last place is no move of its own. */
    private var awaitingHome = true

    /** Which side list each frame is, for those that are one. */
    private val sides = HashMap<String, Side>()

    /** The row in hand in each list, by the frame it is in. */
    private val selected = HashMap<String, String>()

    /** Text being typed, kept across redraws, by what it is for. */
    private val editors = HashMap<String, TextBox>()

    private var lastLanding = 0u
    private var landedNow = false

    /** Every page is being closed at once (Home from the menu, a place gone to): none of them asks for Home again. */
    private var leavingAll = false

    private var screen: ScreenView = app.screenView()

    /** What is on the display, for the LCD and an HDMI screen: what a sighted helper reads. */
    var drawn: (String) -> Unit = {}

    /** The dialog shown and its control in hand, kept across turns while it stays open (the braille apps' shared controller). */
    private var dialogController: DialogController? = null

    /** Each line's value as the core last described it, to tell when the core changed it underneath. */
    private val coreValues = HashMap<String, String>()

    /** Whether the reminders due page has shown any, so that the last dealt with leaves it. */
    private var dueShown = false

    init {
        ui.onEmpty = { host.exit() }
        ui.onExit = { host.exit() }
        ui.onUnhandled = ::unhandled
        ui.drawn = { drawn(it) }
        app.described = ::described
        ui.push(PageScreen(Page.Home))
        send(ShellEvent.Home)
    }

    private fun send(event: ShellEvent) = app.send(event)

    /** A turn's description, before its view: what the core refused that no sentence said. */
    private fun described(described: ScreenView) {
        screen = described
        described.changes.filterIsInstance<ScreenChange.Refused>().filter { !it.said }.forEach { ui.announce(it.reason) }
        // The last of a turn to arrive, with the view and lines in place: a list drawn sooner
        // would pair this view with the last turn's lines.
        turn()
        // What the app said waits for the row in hand to land, which it has.
        core.release()
    }

    private val view: ViewModel? get() = app.view.value
    private val lines: Lines? get() = app.lines.value

    private fun editor(key: String, initial: String, multiLine: Boolean = false): TextBox =
        editors.getOrPut(key) { TextBox(initial, translator, multiLine) }

    /** Before the person's settings are read again: every box's text, read in the table in force. */
    fun settle() = editors.values.forEach { it.settle() }

    /** The braille table changed: every box is written again in the new one, its text kept, as the device's own are. */
    fun retable() {
        editors.values.forEach { it.recell() }
        ui.redraw()
    }

    /** Something the app said: said with the next change, or at once if nothing moves. */
    fun announce(text: String) = ui.announce(text)

    /** What is on the screen for a sighted helper: the line in front. */
    fun currentText(): String = ui.frame?.rendered?.speech.orEmpty()

    /** A reminder's notification opened on a task. */
    fun openTask(id: String) = openDetails(id)

    /** Comes to the front: says where the person is. */
    fun front() = ui.front()

    /** The device's settings changed while in front: the line reads as they now say, unsaid. */
    fun redraw() = ui.redraw()

    /** A key, from the braille keys or the QWERTY keyboard: the stack's. */
    fun press(key: Key) = ui.press(key)

    // ── Following the view ─────────────────────────────────────────────────────────────────

    /** The page in front, under anything the app or the stack has open over it. */
    private val topPage: PageScreen? get() = ui.screens.lastOrNull { it is PageScreen } as PageScreen?

    /**
     * A turn of the app: Home left for a place it opened, what the app has open shown over the pages,
     * what it closed gone, and the screen in front laid out again, said if the app moved what is in hand.
     */
    private fun turn() {
        val model = view
        val landed = model != null && model.landing != lastLanding
        landedNow = landed
        if (model != null) {
            lastLanding = model.landing
            follow(model)
            followRinging(model)
            if (opens(model) && ui.screens.none { it is OpenedScreen }) ui.push(OpenedScreen())
        }
        if (landed) ui.land() else ui.refresh()
        // A dialog's controller and the text of its lines go with the dialog.
        val owner = ui.screens.lastOrNull { it is LumennaScreen } as LumennaScreen?
        if (owner !is FilterScreen && owner?.built !is FormFrame) closeDialog()
    }

    /**
     * The app's own moves between Home and a place: a place Home opened (Enter, a letter, a
     * command that goes to one) is shown, as a shell follows the app.
     */
    private fun follow(model: ViewModel) {
        if (model.home != null) awaitingHome = false
        if (topPage?.page == Page.Home && model.home == null && !awaitingHome && model.place != null) pushPage(Page.Listing)
    }

    /** Whether the app has something open of its own: a question, a form, quick add, a proposal, a reminder list. */
    private fun opens(model: ViewModel): Boolean =
        screen.dialog != null || model.asking?.let(::asksSomething) == true || model.proposal != null || model.reminders != null

    /** A question that shows something: not one that asks nothing, nor a form with nothing to edit. */
    private fun asksSomething(asked: dev.emassey0135.lumenna.app.Asking): Boolean = when (asked.question) {
        is Question.Immediate -> false
        is Question.Form -> asked.action != null
        else -> true
    }

    /** What the app has open, as a frame: the core's dialog first, then a form asked for, a proposal, a reminder list. */
    private fun opened(model: ViewModel): Frame? {
        screen.dialog?.let { return appDialog(it) }
        model.asking?.let { asked -> formAsked(asked)?.let { return it } }
        if (model.proposal != null) return proposal(model)
        if (model.reminders != null) return reminders(model)
        return null
    }

    /** Back to Home, which the app shows again. */
    private fun goHome() {
        closeAllPages()
        awaitingHome = true
        send(ShellEvent.Home)
    }

    /** Everything over Home closed, none of it asking for Home again. */
    private fun closeAllPages() {
        leavingAll = true
        try {
            ui.screens.asReversed().forEach { if (!(it is PageScreen && it.page == Page.Home)) ui.close(it) }
        } finally {
            leavingAll = false
        }
    }

    private fun pushPage(page: Page) {
        when (page) {
            // Settings already open stay as they are: opening them again empties the page for a turn.
            Page.Settings, is Page.SettingsPage -> if (view?.settings == null) send(ShellEvent.Key("settings"))
            Page.Devices -> {
                // What pairing's sentences call this device, which finds others on its network.
                send(ShellEvent.PairingAs(appWords.thisDevice, true))
                send(ShellEvent.Devices(DevicesEvent.Show))
                send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Open(host.deviceName()))))
            }
            else -> {}
        }
        ui.push(PageScreen(page))
    }

    /** A page left: what it held put away, settings closed when the last of them goes, Home asked for again. */
    private fun leaving(page: Page, cancelled: Boolean) {
        when (page) {
            // Leaving the task's form throws away what was not saved, as its Cancel does.
            is Page.Task -> if (!cancelled) send(ShellEvent.Details(DetailsEvent.Discard))
            // Brought to the front over another app for it, back there once it is dealt with or Escape
            // pressed on it, as the Schedule Manager leaves; not after Replan, which goes on here.
            is Page.Ringing -> {
                if (page.fromBackground && ringButton != "replan" && !leavingAll) host.returnToPrevious()
                ringButton = null
            }
            is Page.SettingsPage -> forget("setting:", "settings:${page.id}")
            Page.Devices -> {
                send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Cancel)))
                forget("pairing-code", "devices")
            }
            else -> {}
        }
        val now = topPage?.page
        if ((page is Page.Settings || page is Page.SettingsPage) && now !is Page.Settings && now !is Page.SettingsPage) {
            send(ShellEvent.Settings(SettingsEvent.Close))
        }
        if (now == Page.Home && !leavingAll) {
            awaitingHome = true
            send(ShellEvent.Home)
        }
    }

    /** No dialog shown: its controller and the text of its lines go with it. */
    private fun closeDialog() {
        dialogController = null
        editors.keys.removeAll { it.startsWith("dialog:") }
        coreValues.clear()
    }

    private fun forget(editorPrefix: String, frameId: String) {
        editors.keys.removeAll { it.startsWith(editorPrefix) }
        selected.remove(frameId)
    }

    private object Blank : Frame("blank") {
        override fun key(key: Key) = false
        override val rendered = Rendered("")
    }

    // ── The screens ────────────────────────────────────────────────────────────────────────

    /** A screen of this shell's: the menu bar is the core's, and a letter on a list the app holds is the app's. */
    private abstract inner class LumennaScreen : Screen() {
        override val title: String get() = ""

        /** The frame last laid out. */
        var built: Frame? = null

        abstract fun layout(): Frame

        override fun frame(): Frame = layout().also { built = it }

        /** A list says its name, a dialog that it is one and what it opens with, a question's answers its prompt. */
        override fun heading(frame: Frame): String = when (frame) {
            is ListFrame -> frame.title
            // A question of answers alone says the core's prompt; any other dialog its title as the device
            // names a dialog, then the lines ahead of its first control, joined as the core joins a prompt.
            is FormFrame -> if (frame.prompt) frame.said else promptOf(listOfNotNull(frame.title?.let(::dialogTitle)) + frame.leading)
            else -> ""
        }

        // A side list shown is what the app acts on from now: until it is told, a letter pressed
        // here would run on the row in hand in the main list, which the person cannot see.
        override fun shown(frame: Frame) {
            if (frame is ListFrame) sides[frame.id]?.let { side -> frame.current?.let { held(it.key, side) } }
        }

        override fun menu(): List<MenuItem<() -> Unit>> = menuBar()

        override fun letter(letter: Char): Boolean {
            if ((built as? ListFrame)?.appHeld != true) return false
            send(ShellEvent.Letter(letter.toString()))
            return true
        }
    }

    /** A page the person went to. */
    private inner class PageScreen(val page: Page) : LumennaScreen() {
        private var cancelled = false

        override fun layout(): Frame {
            val model = view ?: return Blank
            return when (page) {
                Page.Home -> home(model)
                Page.Listing -> listing(model)
                is Page.Task -> task(model, page.id)
                Page.Settings -> settings(model)
                is Page.SettingsPage -> settingsPage(model, page.id)
                Page.Devices -> devices(model)
                Page.Due -> due(model)
                Page.Help -> help()
                Page.About -> about()
                is Page.Ringing -> ringing(model, page.id)
                is Page.HeldBack -> heldBack(page)
            }
        }

        // A form closes as its Cancel does (the task form's throws away what was not saved), and the page with it.
        override fun back(): Boolean {
            cancelled = (built as? FormFrame)?.controller?.cancel() == true
            return false
        }

        // The last reminder dealt with leaves nothing on its page.
        override fun stands(): Boolean = when (page) {
            Page.Due -> !(dueShown && view?.due.isNullOrEmpty())
            // A reminder dealt with, here or anywhere, takes its dialog with it.
            is Page.Ringing -> view?.due?.any { it.id == page.id } != false
            else -> true
        }

        override fun closed() = leaving(page, cancelled)
    }

    /** What the app has open of its own, over the pages, until the app closes it. */
    private inner class OpenedScreen : LumennaScreen() {
        override fun layout(): Frame {
            val model = view ?: return Blank
            return opened(model) ?: Rendering("")
        }

        override fun stands(): Boolean = view?.let(::opens) == true

        // It goes as the app closes it: Escape asks the app to, as its Cancel would.
        override fun back(): Boolean {
            if ((built as? FormFrame)?.controller?.cancel() == true) return true
            val model = view
            when {
                model?.asking != null -> send(ShellEvent.Cancel)
                model?.line != null -> send(ShellEvent.Typing(Typing.Close))
                model?.blockForm != null -> send(ShellEvent.Block(BlockEvent.Cancel))
                model?.proposal != null -> send(ShellEvent.Proposal(ProposalEvent.Close))
                model?.reminders != null -> send(ShellEvent.CloseReminders)
                else -> return false
            }
            return true
        }
    }

    /**
     * The filter's line over the list shown (Space-F): Enter keeps the filter and goes back to the
     * list; Escape leaves it cleared, still on the list.
     */
    private inner class FilterScreen : LumennaScreen() {
        private val line = LineFrame(
            FILTER, appWords.quickAdd.filter, editor(FILTER, view?.query.orEmpty()), appWords.quickAdd.filterHint, readings,
            done = {
                editors.remove(FILTER)
                ui.close(this)
            },
            changed = { send(ShellEvent.Filter(it)) },
        )

        /** The line, with completion in the filter language on Space-4, which its box of one line leaves. */
        private val completes = object : Frame(FILTER) {
            override fun key(key: Key): Boolean {
                if (key != Key.Down) return line.key(key)
                completing.ask(line.box, Syntax.FILTER) { _, _ -> line.touched() }
                return true
            }

            override val rendered: Rendered get() = line.rendered

            override fun routed(at: Int) = line.routed(at)

            override fun editBox(): TextBox = line.box

            override fun edited() = line.edited()
        }

        override fun layout(): Frame = completes

        override fun back(): Boolean {
            editors.remove(FILTER)
            send(ShellEvent.Filter(""))
            return false
        }

        override fun letter(letter: Char): Boolean = false
    }

    // ── Keys the stack leaves ──────────────────────────────────────────────────────────────

    /** What the stack leaves: the device's Help, Options and Find, and why Tab or Enter-I does nothing. */
    private fun unhandled(key: Key) {
        when (key) {
            Key.Tab, Key.ShiftTab -> if (ui.frame is ListFrame) announce(appWords.braille.tabInAList)
            Key.Help -> pushPage(Page.Help)
            Key.Options -> pushPage(Page.Settings)
            Key.Find -> startFiltering()
            Key.Context -> announce(appWords.braille.noActionsHere)
            else -> {}
        }
    }

    private fun startFiltering() {
        val model = view ?: return
        if (ui.top is FilterScreen) return
        // Only a list shown is filtered: not Home, a day, a form or a list of the shell's own.
        if (ui.top !== topPage || topPage?.page != Page.Listing || model.list == null || model.day != null || ui.frame !is ListFrame) {
            return announce(appWords.braille.noListToFilter)
        }
        ui.push(FilterScreen())
    }

    /** Where a place chosen is going, until the app shows it: its view is the last place's till then. */
    private var goingTo: Place? = null

    /** A place the menu bar goes to (`go-today`, …), by its command, from whatever is shown. */
    private fun goTo(id: String, place: Place) {
        goingTo = place
        send(ShellEvent.Key(id))
        closeAllPages()
        pushPage(Page.Listing)
    }

    /**
     * Space-M or F2: the shared menu bar, as every other platform's, holding every one of the
     * screen's own actions (Home's on Home) with its letter as a real hot key; each command's name
     * and whether it applies now being `ViewModel::commands` and `screen_actions`, and its letter
     * the core's. What does not apply is left out, as the device's own menus hide theirs. A row's
     * own actions are Enter-I.
     */
    private fun menuBar(): List<MenuItem<() -> Unit>> {
        val model = view ?: return emptyList()
        val states = model.commands.associateBy { it.id }
        val actions = model.screenActions.associateBy { it.id }
        val page = topPage?.page
        val place = model.place.takeIf { page == Page.Listing }
        return brailleMenuBar(place, page == Page.Home, true).mapNotNull { group ->
            val children = group.items.filter { states[it.id]?.enabled != false && actions[it.id]?.enabled != false }.map { item ->
                val letter = item.letter.singleOrNull()
                MenuItem<() -> Unit>(
                    states[item.id]?.title?.ifEmpty { null } ?: actions[item.id]?.title ?: item.title,
                    letter = letter,
                    value = { runMenuCommand(item.id) },
                    hotKey = letter?.takeIf { item.hotKey }?.let(BrailleLetters::press),
                    asks = item.asks,
                )
            }
            if (children.isEmpty()) null else MenuItem(group.title, letter = group.letter.singleOrNull(), children = children)
        }
    }

    /**
     * Runs what the menu bar names: what is the shell's own (a page of its, the filter, the key
     * help, About, the reminders due, a heading's places on Home), else the app's, as `Key(id)`.
     */
    private fun runMenuCommand(id: String) {
        // A command that opens a settings page (Devices, Export and Import, Restore from a Backup) opens the core's page for it.
        settingsPageOf(id)?.let { page -> return openSettingsPage(page) }
        when (id) {
            "projects" -> openGroupFromAnywhere(SidebarGroup.PROJECTS)
            "labels" -> openGroupFromAnywhere(SidebarGroup.LABELS)
            "filters" -> openGroupFromAnywhere(SidebarGroup.FILTERS)
            "reminders-due" -> openDue()
            "filter" -> startFiltering()
            "keyboard-help" -> pushPage(Page.Help)
            "about" -> pushPage(Page.About)
            "home" -> goHome()
            "settings" -> pushPage(Page.Settings)
            "close-window", "quit" -> host.exit()
            "go-today" -> goTo(id, Place.Today)
            "go-upcoming" -> goTo(id, Place.Upcoming)
            "go-tasks" -> goTo(id, Place.Tasks)
            "go-blocks" -> goTo(id, Place.Blocks)
            "go-trash" -> goTo(id, Place.Trash)
            "go-history" -> goTo(id, Place.History)
            "go-review" -> goTo(id, Place.Review)
            "go-report" -> goTo(id, Place.Report)
            else -> send(ShellEvent.Key(id))
        }
    }

    /** A settings page by its id: the devices page as the devices screen, any other as a page of settings. */
    private fun openSettingsPage(id: String) {
        pushPage(if (SettingsPages.of(id)?.devices == true) Page.Devices else Page.SettingsPage(id))
    }

    /** One of a screen's own actions, or Home's: the shell's own run here, every other as `Key(id)`. */
    private fun runScreenAction(action: dev.emassey0135.lumenna.app.ScreenAction) {
        if (!action.enabled) return announce(nothingOnLetter(action.letter))
        if (action.shell || action.id == "settings") runMenuCommand(action.id) else send(ShellEvent.Key(action.id))
    }

    /** A heading's places, from anywhere: Home, on the heading, unfolded. */
    private fun openGroupFromAnywhere(group: SidebarGroup) {
        goHome()
        val key = sidebarKey(SidebarKind.Group(group))
        held(key, Side.SIDEBAR)
        val heading = view?.sidebar?.firstOrNull { it.key == key }
        heading?.offered?.firstOrNull { it.kind == ActionKind.EXPAND }?.let { send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, key), it)) }
    }

    private fun entries(on: ActingOn, offered: List<Offered>): List<MenuEntry> =
        offered.sortedByDescending { it.primary }.map { action ->
            val letter = actionLetter(action.kind)?.singleOrNull()
            MenuEntry(action.name, hotkey = letter?.let(BrailleLetters::press), letter = letter, asks = action.asks != Asks.NOTHING) { send(ShellEvent.ActOn(on, action)) }
        }

    private fun held(id: String, side: Side?) {
        send(if (side != null) ShellEvent.Hold(side, id) else ShellEvent.Focus(id))
    }

    private fun list(
        id: String,
        title: String,
        rows: List<Row>,
        fallback: String?,
        empty: String,
        side: Side? = null,
        hold: Boolean = true,
        mainList: Boolean = false,
        byLetter: ((Char) -> Boolean)? = null,
        prompt: Boolean = false,
    ): ListFrame {
        if (side != null) sides[id] = side
        val appChoice = fallback
        val chosen = if (landedNow && appChoice != null) appChoice else selected[id] ?: appChoice
        return ListFrame(id, title, rows, chosen, empty, letters, moved = { row ->
            selected[id] = row.key
            if (hold && row.tag !== OWN_ROW) held(row.key, side)
        }, openActions = { ui.menu(it.actions) },
            findInApp = if (mainList) ({ send(ShellEvent.FindLetter(it.toString())) }) else null,
            notFound = { announce(noRowStartsWith(it.toString())) },
            opensNothing = { announce(appWords.braille.opensNothing) },
            noActions = { announce(appWords.braille.noActions) },
            appHeld = hold,
            readings = readings,
            byLetter = byLetter,
            prompt = prompt,
        ).also { frame ->
            frame.current?.let { selected[id] = it.key }
        }
    }

    // ── Places ─────────────────────────────────────────────────────────────────────────────


    /** What is due, looked for while in front, as a desktop app looks. */
    fun look() {
        val now = java.time.Instant.now().toString()
        send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.Look(lastLook, null)))
        lastLook = now
    }

    private var lastLook = java.time.Instant.now().toString()

    /**
     * Home: the places, folded as the sidebar is, then Home's own actions, each with its letter.
     * Its row in hand is the sidebar's. Enter opens a place or folds a heading (`Key("open")`);
     * a letter, with Space or Enter or alone, opens its place or runs its action, as a device's
     * main menu does, and alone otherwise moves by first letter.
     */
    private fun home(model: ViewModel): Frame {
        val home = model.home ?: return Rendering("")
        val places = home.places.filter { !it.entry.hidden }.map { place ->
            val entry = place.entry
            val on = ActingOn.Held(Side.SIDEBAR, entry.key)
            val folding = { kind: ActionKind -> entry.offered.firstOrNull { it.kind == kind }?.let { action -> { send(ShellEvent.ActOn(on, action)) } } }
            Row(
                entry.key,
                entry.title,
                finding = entry.title,
                // What follows the name, its folding and its level after it, as the core joins them.
                detail = detailWithState(Line(entry.title, entry.detail, entry.text, entry.text, entry.level), entry.expanded),
                letter = place.letter.singleOrNull(),
                open = { held(entry.key, Side.SIDEBAR); send(ShellEvent.Key("open")) },
                actions = entries(on, entry.offered),
                // The row's own letter runs first, through the app; else Home's.
                command = { pressed -> entry.offered.none { actionLetter(it.kind)?.singleOrNull() == pressed } && homeLetter(home, pressed) },
                collapse = folding(ActionKind.COLLAPSE),
                expand = folding(ActionKind.EXPAND),
            )
        }
        val actions = home.actions.filter { it.enabled }.map { action ->
            Row("action:${action.id}", action.title, finding = action.title, letter = action.letter.singleOrNull(), asks = action.asks,
                open = { runScreenAction(action) }, command = { homeLetter(home, it) }, tag = OWN_ROW)
        }
        return list("home", home.title, places + actions, model.sidebarHeld.focus, "", side = Side.SIDEBAR, byLetter = { homeLetter(home, it) })
    }

    /** A letter of Home's: its place's or its action's, as the person's setting has it, run or only gone to. */
    private fun homeLetter(home: dev.emassey0135.lumenna.app.HomeView, letter: Char): Boolean {
        val place = home.places.firstOrNull { !it.entry.hidden && it.letter.singleOrNull() == letter }
        val action = home.actions.firstOrNull { it.enabled && it.letter.singleOrNull() == letter }
        val key = place?.entry?.key ?: action?.let { "action:${it.id}" } ?: return false
        if (!opensOnLetter()) {
            selected["home"] = key
            if (place != null) held(key, Side.SIDEBAR)
            ui.land()
            return true
        }
        when {
            place != null -> {
                held(place.entry.key, Side.SIDEBAR)
                send(ShellEvent.Key("open"))
            }
            action != null -> runScreenAction(action)
        }
        return true
    }

    private fun listing(model: ViewModel): Frame {
        // Until the app shows the place chosen, its view is the place left.
        goingTo?.let { if (model.place != it) return Rendering("") else goingTo = null }
        model.day?.let { return day(model, it) }
        val list = model.list ?: return Rendering(model.title)
        val worded = lines?.list.orEmpty()
        val rows = shownRows(list.items, worded).map { (item, line) ->
            val task = item.row
            val offered = list.offeredOf(item)
            val on = ActingOn.Row(task.id)
            Row(
                task.id,
                line?.let { lineWithState(it, item.row.expanded) } ?: task.title,
                braille = line?.braille?.ifEmpty { null } ?: line?.text ?: task.title,
                finding = task.title,
                open = { held(task.id, null); send(ShellEvent.Key("open")) },
                space = offered.firstOrNull { it.kind == ActionKind.MARK_DONE || it.kind == ActionKind.MARK_NOT_DONE }?.let { action ->
                    { send(ShellEvent.ActOn(on, action)) }
                },
                actions = entries(on, offered),
                delete = { held(task.id, null); send(ShellEvent.Key("delete")) },
                collapse = { held(task.id, null); send(ShellEvent.Key("collapse")) },
                expand = { held(task.id, null); send(ShellEvent.Key("expand")) },
            )
        }
        val empty = (list.refusal ?: list.empty)
        return list("list:${model.title}", model.title, rows, model.focus, empty, mainList = true)
    }

    private fun Rendering(text: String): Frame = object : Frame(if (text.isEmpty()) "going" else "rendering:$text") {
        override fun key(key: Key) = false
        override val rendered = Rendered(text)
    }

    private fun openDetails(id: String) {
        send(ShellEvent.Focus(id))
        send(ShellEvent.Details(DetailsEvent.Show(id)))
        pushPage(Page.Task(id))
    }

    private fun day(model: ViewModel, day: dev.emassey0135.lumenna.app.DayView): Frame {
        val worded = lines?.day.orEmpty()
        val rows = day.items.mapIndexedNotNull { index, item ->
            if (item.hidden) return@mapIndexedNotNull null
            val line = worded.getOrNull(index)
            val heading = item.kind == DayKind.SUMMARY
            val on = if (heading) ActingOn.Day else ActingOn.Row(item.key)
            val offered = if (heading) day.offered else item.offered
            val actions = entries(on, offered)
            Row(
                item.key,
                line?.let { lineWithState(it, item.expanded) }.orEmpty(),
                braille = line?.braille?.ifEmpty { null } ?: line?.text.orEmpty(),
                finding = line?.title.orEmpty(),
                open = { held(item.key, null); send(ShellEvent.Key("open")) },
                actions = actions,
                delete = { held(item.key, null); send(ShellEvent.Key("delete")) },
                collapse = { held(item.key, null); send(ShellEvent.Key("collapse")) },
                expand = { held(item.key, null); send(ShellEvent.Key("expand")) },
            )
        }
        return list("day:${day.date}", lines?.dayTitle.orEmpty(), rows, model.focus, "", mainList = true)
    }


    // ── Details ────────────────────────────────────────────────────────────────────────────

    /** A task's form, opened here (a reminder's notification): the core's, as its controls. */
    private fun task(model: ViewModel, id: String): Frame {
        val shown = app.taskForm()?.takeIf { model.details?.task?.id == id } ?: return Rendering(appWords.details.gone)
        return dialog(shown)
    }

    // ── Dialogs ────────────────────────────────────────────────────────────────────────────

    /** A dialog the app has open. */
    private fun appDialog(shown: DialogShown): Frame = dialog(shown)

    /**
     * A dialog the core describes (`DialogShown`) as a `FormFrame` over the braille apps' shared
     * `DialogController`, kept while the dialog stays open, which works its tab order, what is
     * typed or chosen until the core has it, and presses, through the core's `control_event`.
     * Each control is read in the device's role; a line or several is the SDK's edit box, its text
     * kept by the dialog. A button has the core's letter, else the first free one of its name
     * (`menu_shortcuts`). [shell] does what a button leaves to this device: a file or a folder to
     * choose in the File Manager, a backup, pairing.
     */
    private fun dialog(shown: DialogShown, shell: (String) -> Unit = {}): Frame {
        // The same dialog goes on where it was; one that held only lines to read (its controls still
        // arriving) starts again on the core's focus once it has more.
        val controller = dialogController?.takeIf { it.dialog.id == shown.id && it.controls.any { c -> c.kind != ControlType.TEXT } }?.also { it.update(shown) }
            ?: DialogController(shown, ::send, shell = { button -> (button.sends as? ControlSends.Shell)?.let { shell(it.field0) } }).also {
                closeDialog()
                dialogController = it
            }
        val taking = shown.controls.filter { it.kind != ControlType.TEXT }
        // A question of buttons alone is the device's prompt: its answers are stepped with Space.
        val prompt = taking.isNotEmpty() && taking.all { it.kind == ControlType.BUTTON } && shown.id.startsWith("question:")
        val buttons = shown.controls.filter { it.kind == ControlType.BUTTON }
        val lettered = buttons.map { it.key }.zip(letters.shortcuts(buttons.map { it.label to it.letter.singleOrNull() })).toMap()
        val boxOf = { control: ControlShown ->
            val key = "dialog:${shown.id}:${control.key}"
            // The core changed a line underneath (a completion chosen, a refused setting put back) and
            // nothing typed here is held: its box starts again from the core's.
            val seen = coreValues.put(key, control.value)
            val box = editors[key]
            if (box != null && seen != null && seen != control.value && box.text != control.value && controller.text(control) == control.value) {
                editors.remove(key)
            }
            editor(key, controller.text(control), multiLine = control.kind == ControlType.LINES)
        }
        return FormFrame(controller, letters, readings, boxOf, letterOf = { lettered[it.key] }, prompt = prompt, completing = completing)
    }

    /**
     * Completion in a line with a syntax (Space-4): the core's candidates, as the device's own list
     * headed by the core's announcement, the line said again as it reads once one is put in.
     */
    private val completing by lazy {
        Completing(
            complete = { text, cursor, syntax -> runCatching { app.complete(text, cursor, syntax) }.getOrNull() },
            offer = { title, lines, chosen ->
                ui.choose(title, lines) { at ->
                    chosen(at)
                    // The line under the list, said as it closed, is said as it now reads.
                    ui.redraw()
                }
            },
            say = ::announce,
            showCompletions = { appWords.quickAdd.showCompletions },
            nothingToComplete = { appWords.quickAdd.nothingToComplete },
        )
    }

    /** A control of this shell's own, for a page the core does not describe as a dialog. */
    private fun control(key: String, kind: ControlType, label: String = "", value: String = "", sends: ControlSends = ControlSends.Nothing, default: Boolean = false) =
        // Read here in the device's roles, label and value apart: its line as a row is never asked for.
        ControlShown(key, kind, label, value, "", "", emptyList(), null, false, null, null, "", false, default, false, sends, "", emptyList())

    private fun text(key: String, value: String) = control(key, ControlType.TEXT, value = value)

    private fun pressing(key: String, label: String, vararg events: ShellEvent) = control(key, ControlType.BUTTON, label, sends = ControlSends.Press(events.toList()))

    /**
     * A question whose dialog has not come yet: nothing to show until it does. A task's form asked
     * for (`Question.Form`) comes as the question's dialog, the app showing that task's details
     * itself as it asks. An immediate question, or a form with no row asking, is none.
     */
    private fun formAsked(asked: dev.emassey0135.lumenna.app.Asking): Frame? = when {
        asked.question is Question.Immediate -> null
        asked.question is Question.Form && asked.action == null -> null
        else -> Rendering("")
    }

    // ── Proposals ──────────────────────────────────────────────────────────────────────────

    private fun proposal(model: ViewModel): Frame {
        val proposal = model.proposal!!
        val words = proposal.words
        val rows = mutableListOf<Row>()
        proposal.refusal?.let { rows += Row("refusal", it) }
        if (proposal.ready && proposal.changes.isEmpty()) rows += Row("nothing", words.nothing)
        proposal.changes.forEachIndexed { index, change ->
            rows += Row("change$index", change.read, finding = change.sentence, open = { send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked))) },
                space = { send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked))) })
        }
        proposal.unfitLines.forEachIndexed { index, line -> rows += Row("unfit$index", line) }
        proposal.others.forEachIndexed { index, other ->
            rows += Row("other$index", other, open = { send(ShellEvent.Proposal(ProposalEvent.Take(index.toUInt()))) })
        }
        if (proposal.changes.isNotEmpty()) rows += Row("apply", proposal.applyLine, role = ControlRole.BUTTON, open = { send(ShellEvent.Proposal(ProposalEvent.Apply)) })
        rows += Row("close", words.cancel, role = ControlRole.BUTTON, open = { send(ShellEvent.Proposal(ProposalEvent.Close)) })
        return list("proposal", words.title, rows, null, "", hold = false)
    }

    // ── Reminders ─────────────────────────────────────────────────────────────────────────

    private fun reminders(model: ViewModel): Frame {
        val list = model.reminders ?: return Rendering(model.remindersTitle.orEmpty())
        val worded = lines?.reminders.orEmpty()
        val rows = list.reminders.mapIndexed { index, reminder ->
            val line = worded.getOrNull(index)
            val words = line?.text ?: reminder.words
            val on = ActingOn.Held(Side.REMINDERS, reminder.id)
            val actions = if (reminder.own) entries(on, reminder.offered) else emptyList()
            Row(reminder.id, words, open = { held(reminder.id, Side.REMINDERS); send(ShellEvent.Key("open")) }, actions = actions)
        } + list.offered.map { action -> Row("add:${action.kind}", action.name, open = { send(ShellEvent.ActOn(ActingOn.Reminders, action)) }) }
        val title = list.announcement.replaceFirstChar { it.uppercase() }
        return list("reminders", title, rows, model.remindersHeld.focus, "", side = Side.REMINDERS)
    }


    /**
     * Asked once, as the app opens, about what holds its reminders back: Android restricting it in
     * the background (DuraSpeed here), so no reminder goes off until it is opened, and not being
     * allowed to come to the front, so one that goes off only sounds and vibrates. Reminders are
     * often added by themselves, so the Reminders page alone is not enough; after this only it says so.
     */
    fun askHeldBack(background: Boolean, front: Boolean) {
        if (background || front) pushPage(Page.HeldBack(background, front))
    }

    /** What holds reminders back, in the core's words, each with its button to where it is allowed, then Not Now. */
    private fun heldBack(page: Page.HeldBack): Frame {
        val words = appWords.notifications
        val controls = mutableListOf<ControlShown>()
        if (page.background) {
            controls += text("held-back", words.heldBack)
            controls += control("allow-background", ControlType.BUTTON, words.allowBackground, sends = ControlSends.Shell("allow-background"), default = true)
        }
        if (page.front) {
            controls += text("not-in-front", words.notInFront)
            controls += control("allow-front", ControlType.BUTTON, words.allowFront, sends = ControlSends.Shell("allow-front"), default = !page.background)
        }
        controls += control("not-now", ControlType.BUTTON, words.notNow, sends = ControlSends.Shell("not-now"))
        val shown = DialogShown(id = "held-back", title = words.channel, prompt = words.channel, controls = controls, focus = 1u, cancel = emptyList())
        return dialog(shown) { what ->
            // One allowed, the other is still asked, waiting for the person back from Android's page.
            val rest = when (what) {
                "allow-background" -> page.copy(background = false).also { host.allowBackground() }
                "allow-front" -> page.copy(front = false).also { host.allowFront() }
                else -> null
            }?.takeIf { it.background || it.front }
            ui.screens.lastOrNull { it is PageScreen && it.page == page }?.let(ui::close)
            rest?.let(::pushPage)
        }
    }

    /** A reminder that went off and brought the app to the front, until the app holds it, and when it came. */
    private var ringing: Triple<dev.emassey0135.lumenna.Ringing, Long, Boolean>? = null

    /** The button last pressed on a ringing reminder's dialog. */
    private var ringButton: String? = null

    /**
     * A reminder went off and brought the app to the front, as the Schedule Manager rings: the app
     * looks over the moments the alarm found it between, not by its own clock, which does not tick
     * in the background; once the app's reminders due hold it, it is said as its dialog. One not
     * found within a while (dealt with on another device meanwhile) is let go, the while counted on
     * a clock that runs on as the device sleeps.
     */
    fun ring(firing: dev.emassey0135.lumenna.Ringing, fromBackground: Boolean = false) {
        ringing = Triple(firing, android.os.SystemClock.elapsedRealtime(), fromBackground)
        send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.Look(firing.since, firing.now)))
    }

    private fun followRinging(model: ViewModel) {
        val (firing, came, fromBackground) = ringing ?: return
        if (android.os.SystemClock.elapsedRealtime() - came > RINGING_MS) {
            ringing = null
            return
        }
        val due = model.due.firstOrNull { due -> due.firings.any { it.key == firing.key && it.occurrence == firing.occurrence } } ?: return
        ringing = null
        if (ui.screens.none { it is PageScreen && (it.page as? Page.Ringing)?.id == due.id }) pushPage(Page.Ringing(due.id, fromBackground))
    }

    /**
     * A reminder that has just gone off, said as the device says a dialog: named by the reminder, its
     * message read ahead of its buttons, which are in the core's order and words with their letters,
     * each handed back by its id. It rings no more than its notification did; Escape leaves it due.
     */
    private fun ringing(model: ViewModel, id: String): Frame {
        val due = model.due.firstOrNull { it.id == id } ?: return Rendering("")
        val buttons = due.buttons.map { button ->
            control(button.id, ControlType.BUTTON, button.title, sends = ControlSends.Shell(button.id))
                .copy(letter = dueLetter(button.id).orEmpty())
        }
        // Its message first, as a line to read, then its buttons, the first in hand.
        val shown = DialogShown(
            id = "ringing:$id", title = due.title, prompt = due.title,
            controls = listOf(text("message", due.text)) + buttons,
            focus = if (buttons.isEmpty()) 0u else 1u, cancel = emptyList(),
        )
        // Each button handed back by its id, and kept: Replan carries on in the app, so the person stays.
        return dialog(shown) { button ->
            ringButton = button
            send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.Press(due.id, button)))
        }
    }

    /** The reminders due, looked back for over the last day by the app's own clock as the list opens. */
    private fun openDue() {
        dueShown = false
        send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.LookBack))
        pushPage(Page.Due)
    }

    /** A reminder's buttons are its actions, Enter and Enter-I both, each handed back by its id. */
    private fun due(model: ViewModel): Frame {
        if (model.due.isNotEmpty()) dueShown = true
        val rows = model.due.map { due ->
            val buttons = due.buttons.map { button ->
                val letter = dueLetter(button.id)?.singleOrNull()
                MenuEntry(button.title, hotkey = letter?.let(BrailleLetters::press), letter = letter) {
                    send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.Press(due.id, button.id)))
                }
            }
            Row(due.id, due.text, open = if (buttons.isNotEmpty()) ({ ui.menu(buttons) }) else null, actions = buttons,
                delete = { send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.Dismiss(due.id))) })
        }
        return list("due", appWords.remindersDue, rows, model.dueHeld.focus, appWords.braille.nothingDue, side = Side.DUE)
    }

    // ── Settings ──────────────────────────────────────────────────────────────────────────

    private fun settings(model: ViewModel): Frame {
        val pages = model.settings?.pages.orEmpty()
        val open = { page: dev.emassey0135.lumenna.app.SettingsPage ->
            pushPage(if (page.devices) Page.Devices else Page.SettingsPage(page.id))
        }
        val byLetter = { pressed: Char ->
            pages.firstOrNull { pageLetter(it.id)?.singleOrNull() == pressed }?.let { open(it); true } ?: false
        }
        val rows = pages.map { page ->
            // Its title and what is on it, as the core reads a page as one row.
            Row(page.id, page.line, finding = page.title, letter = pageLetter(page.id)?.singleOrNull(),
                open = { open(page) }, command = byLetter)
        }
        return list("settings", commandTitle("settings"), rows, null, "", hold = false)
    }

    /** A settings page, the core's, with what Android allows reminders after the Reminders page's own. */
    private fun settingsPage(model: ViewModel, id: String): Frame {
        val core = app.settingsPage(id) ?: return Rendering(SettingsPages.of(id)?.title.orEmpty())
        val shown = if (id == "reminders") core.copy(controls = core.controls + reminderControls()) else core
        return dialog(shown) { what ->
            when (what) {
                "back-up" -> send(ShellEvent.Key("back-up"))
                "pair" -> pushPage(Page.Devices)
                "allow-notifications" -> host.requestNotifications()
                "allow-background" -> host.allowBackground()
                "allow-front" -> host.allowFront()
                "import", "restore-backup" -> host.importFile()
                // A folder setting (`backup-dir`): chosen in the File Manager, starting where it is.
                else -> core.controls.firstOrNull { it.key == "setting:$what" }?.let { host.chooseFolder(what, it.value) }
            }
        }
    }

    /** What Android allows reminders, after the Reminders page's own: Android's, so this shell's to add. */
    private fun reminderControls(): List<ControlShown> {
        val words = appWords.notifications
        val access = reminderAccess()
        val controls = mutableListOf<ControlShown>()
        if (!access.notifications) {
            controls += text("notifications-off", words.off)
            controls += control("allow-notifications", ControlType.BUTTON, words.allow, sends = ControlSends.Shell("allow-notifications"))
        }
        if (!access.exact) controls += text("exact-late", words.late)
        if (!access.background) {
            controls += text("held-back", words.heldBack)
            controls += control("allow-background", ControlType.BUTTON, words.allowBackground, sends = ControlSends.Shell("allow-background"))
        }
        if (!host.comesToFront()) {
            controls += text("not-in-front", words.notInFront)
            controls += control("allow-front", ControlType.BUTTON, words.allowFront, sends = ControlSends.Shell("allow-front"))
        }
        return controls
    }

    /** What Android allows reminders, asked of the system each time it is shown. */
    var reminderAccess: () -> dev.emassey0135.lumenna.ReminderAccess = { dev.emassey0135.lumenna.ReminderAccess(true, true, null) }

    // ── Devices ───────────────────────────────────────────────────────────────────────────

    /**
     * The devices and pairing, which the core does not describe as a dialog: built here as one, of
     * the core's records and words, so that it is worked as every other dialog is.
     */
    private fun devices(model: ViewModel): Frame {
        val page = SettingsPages.all.firstOrNull { it.devices }
        val devices = model.devices ?: return Rendering(page?.title.orEmpty())
        val worded = lines?.devices.orEmpty()
        val controls = mutableListOf<ControlShown>()
        devices.status.takeIf { it.isNotEmpty() }?.let { controls += text("status", it) }
        val syncing = page?.buttons?.firstOrNull { it.id == "sync-now" }?.title ?: commandTitle("sync-now")
        controls += pressing("sync", syncing, ShellEvent.Key("sync-now"))
        // Each device as the core words it, then its actions as buttons by their own names.
        devices.list?.devices.orEmpty().forEachIndexed { index, device ->
            val on = ActingOn.Held(Side.DEVICES, device.nodeId)
            controls += text(device.nodeId, worded.getOrNull(index)?.text ?: device.name)
            device.offered.forEach { action -> controls += pressing("${device.nodeId}:${action.kind}", action.name, ShellEvent.ActOn(on, action)) }
        }
        val words = devices.words
        val pairing = devices.pairing
        val pair = { event: PairEvent -> ShellEvent.Devices(DevicesEvent.Pair(event)) }
        controls += text("pairing", words.title)
        // Where pairing stands is the core's, what it says before one starts included.
        controls += text("pairing-status", devices.pairingStatus)
        pairing?.mineLine?.let { controls += text("mine", it) }
        if (pairing?.running != true) controls += pressing("wait", words.wait, pair(PairEvent.Wait))
        if (pairing?.running != true || pairing.waiting) {
            controls += control("code", ControlType.LINE, words.theirCode, pairing?.theirs.orEmpty(), sends = ControlSends.Shell("code"))
            controls += control("join", ControlType.BUTTON, words.join, sends = ControlSends.Shell("join"), default = true)
        }
        pairing?.question?.let { question ->
            controls += text("match", question)
            controls += pressing("match-yes", words.matchYes, pair(PairEvent.Confirm(true)))
            controls += pressing("match-no", words.matchNo, pair(PairEvent.Confirm(false)))
        }
        val focus = controls.indexOfFirst { it.kind != ControlType.TEXT }.coerceAtLeast(0)
        val title = page?.title.orEmpty()
        // Its prompt as the core makes one: the title, then the lines before the first control that takes anything.
        val prompt = promptOf(listOf(title) + controls.takeWhile { it.kind == ControlType.TEXT }.map { it.value })
        val shown = DialogShown(id = "devices", title = title, prompt = prompt, controls = controls, focus = focus.toUInt(), cancel = emptyList())
        return dialog(shown) { what ->
            // The code typed is joined with as it stands: the core takes it as typed (`PairEvent::Code`).
            if (what == "join") {
                val code = dialogController?.controls?.firstOrNull { it.key == "code" }?.let { dialogController?.text(it) }.orEmpty()
                send(pair(PairEvent.Code(code)))
                send(pair(PairEvent.Join))
            }
        }
    }

    // ── Help ─────────────────────────────────────────────────────────────────────────────

    /**
     * The core's key help for this device over what is shown (`key_help_on`): the device's own keys,
     * Home's places, the screen's commands and the rows' actions, each by the keys that press it
     * here. Each section's heading is a row, then its keys, each found by what it does.
     */
    private fun help(): Frame {
        val help = app.keyHelp(KeyDevice.BRAILLE_SENSE, true)
        val rows = help.sections.flatMapIndexed { at, section ->
            listOf(Row("section:$at", section.heading)) +
                section.keys.mapIndexed { index, key -> Row("key:$at:$index", key.line, finding = key.what) }
        }
        return list("help", help.title, rows, null, "", hold = false)
    }

    // ── About ────────────────────────────────────────────────────────────────────────────

    /**
     * About, as lines to read: the core's (`about_lines`), a row each, as Key Help is a list, under
     * the title of the core's About page.
     */
    private fun about(): Frame {
        val rows = appWords.aboutLines.mapIndexed { index, line -> Row("about:$index", line) }
        return list("about", SettingsPages.of("about")?.title.orEmpty(), rows, null, "", hold = false)
    }

    private companion object {
        /** A row's tag for one of the shell's own, which the app does not hold: Home's actions beside its places. */
        private val OWN_ROW = Any()

        /** The filter's line, and its text kept as it is typed. */
        private const val FILTER = "filter"

        /** How long a reminder that brought the app to the front is waited for among the reminders due. */
        private const val RINGING_MS = 15_000L
    }
}
