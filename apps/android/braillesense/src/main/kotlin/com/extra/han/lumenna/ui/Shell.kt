package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.edit.BoxEditing
import dev.emassey0135.braillesense.edit.Clipboard
import dev.emassey0135.braillesense.edit.EditDisplay
import dev.emassey0135.braillesense.edit.Edited
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.output.BrailleDisplay
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.dialogTitle
import dev.emassey0135.braillesense.braille.Echo
import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.lumenna.AppModel
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.commandTitle
import dev.emassey0135.lumenna.flag
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.offeredOf
import dev.emassey0135.lumenna.on
import dev.emassey0135.lumenna.shownRows
import dev.emassey0135.lumenna.text
import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Answer
import dev.emassey0135.lumenna.app.Asks
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.DayKind
import dev.emassey0135.lumenna.app.DetailsEvent
import dev.emassey0135.lumenna.app.DevicesEvent
import dev.emassey0135.lumenna.app.FieldKind
import dev.emassey0135.lumenna.app.Lines
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ProposalEvent
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.ScreenChange
import dev.emassey0135.lumenna.app.ScreenView
import dev.emassey0135.lumenna.app.Shown
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.brailleMenuBar
import dev.emassey0135.lumenna.app.sidebarKey
import dev.emassey0135.lumenna.app.buttonLetter
import dev.emassey0135.lumenna.app.commandLetter
import dev.emassey0135.lumenna.app.dueLetter
import dev.emassey0135.lumenna.app.nothingOnLetter
import dev.emassey0135.lumenna.app.noRowStartsWith
import dev.emassey0135.lumenna.app.pageLetter
import dev.emassey0135.lumenna.app.SettingKind
import dev.emassey0135.lumenna.app.SettingView
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.Typing
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.foldState

/** What only the activity can do. */
interface Host {
    fun exit()

    fun importFile()
    fun requestNotifications()
    fun openNotificationSettings()
    fun deviceName(): String
}

/** A screen the person went to, on the way back from which Escape walks. */
private sealed interface Page {
    /** The places, then what can be done from anywhere: the core's Home, where the app opens. */
    data object Home : Page
    data object Listing : Page
    data class Task(val id: String) : Page
    data object Settings : Page
    data class SettingsPage(val id: String) : Page
    data object Devices : Page
    data object Reminders : Page
    data object Due : Page
    data object Help : Page
}

/**
 * The screens, over the Crux app's view model: what is shown is the app's, this decides only how
 * it is laid out on a line at a time, and what the keys do to it. All on the main thread.
 *
 * Each turn the view is laid out afresh as frames; the frame in front is said and shown if it is
 * new or the app moved what is in hand (`landing`), and only put on the cells otherwise, so a
 * sync or a new minute never cuts off what is being read.
 */
class Shell(
    private val core: Core,
    private val out: BrailleDisplay,
    private val translator: Translator,
    private val host: Host,
    private val readings: Readings,
    /** What an edit box copies to and pastes from. */
    clipboard: Clipboard,
    private val letters: Letters = BrailleLetters,
    /** How long a message stays on the cells: the device's Message Display Time. */
    private val messageMillis: () -> Long = { 3000L },
    /** Automatically Open Menus with Shortcut: whether a letter opens what it names or only goes to it. */
    private val opensOnLetter: () -> Boolean = { true },
    private val echoSetting: () -> Echo = { Echo.of(1) },
) {
    private val app: AppModel get() = core.app

    /** An edit box's own keys and what it says for them, as the device's own box. */
    private val editing = BoxEditing(translator, clipboard, echoSetting)
    private val pages = ArrayDeque<Page>(listOf(Page.Home))

    /** Home asked for and not shown yet: until it is, the app's last place is no move of its own. */
    private var awaitingHome = true
    private var menu: MenuFrame? = null
    private var frame: Frame? = null

    /** Which side list each frame is, for those that are one. */
    private val sides = HashMap<String, Side>()

    /** The row in hand in each list, by the frame it is in. */
    private val selected = HashMap<String, String>()

    /** Text being typed, kept across redraws, by what it is for. */
    private val editors = HashMap<String, TextBox>()

    private var handledForm = 0u
    private var lastLanding = 0u
    private var landedNow = false
    private var lastId = ""
    private var lastSpeech = ""
    private val announcements = ArrayList<String>()
    private var pendingRow: String? = null
    private val handler = android.os.Handler(android.os.Looper.getMainLooper())
    private val saying = Runnable { flush() }

    private var screen: ScreenView = app.screenView()

    init {
        app.described = ::described
        send(ShellEvent.Home)
    }

    private fun send(event: ShellEvent) = app.send(event)

    /** A turn's description, before its view: what the core refused that no sentence said. */
    private fun described(described: ScreenView) {
        screen = described
        val refused = described.changes.filterIsInstance<ScreenChange.Refused>().filter { !it.said }
        refused.forEach { announcements += it.reason }
        // The last of a turn to arrive, with the view and lines in place: a list drawn sooner
        // would pair this view with the last turn's lines.
        refresh(false)
        if (refused.isNotEmpty()) schedule()
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
        frame?.let { out.show(it.rendered) }
    }

    /** Something the app said: said with the next change, or at once if nothing moves. */
    fun announce(text: String) {
        announcements += text
        schedule()
    }

    private fun schedule() {
        handler.removeCallbacks(saying)
        handler.postDelayed(saying, SPEECH_DELAY)
    }

    /** Says what the app said and then the line it left, as one thing. */
    private fun flush() {
        handler.removeCallbacks(saying)
        val said = announcements.filter { it.isNotBlank() }.map { if (it.last() in ".?!") it else "$it." }
        // A list that is only what was just said is not said twice.
        val row = said.fold(pendingRow.orEmpty()) { left, sentence -> left.removeSuffix(sentence.trimEnd('.', ' ')).trimEnd() }
        val parts = said + listOfNotNull(row.takeIf { it.isNotBlank() })
        announcements.clear()
        pendingRow = null
        if (parts.isNotEmpty()) out.say(parts.joinToString(" "))
        // What changes say is for the cells too: someone reading braille alone hears none of it.
        if (said.isNotEmpty()) showMessage(said.joinToString(" "))
    }

    private val restoring = Runnable {
        messageUp = false
        frame?.rendered?.let(out::show)
    }

    private var messageUp = false

    /** A message on the cells for the device's own time, then the line under it again. */
    private fun showMessage(text: String) {
        out.show(Rendered(text))
        messageUp = true
        handler.removeCallbacks(restoring)
        handler.postDelayed(restoring, messageMillis())
    }

    private fun clearMessage() {
        messageUp = false
        handler.removeCallbacks(restoring)
    }

    /** What is on the screen for a sighted helper: the line in front. */
    fun currentText(): String = frame?.rendered?.speech.orEmpty()

    /** A reminder's notification opened on a task. */
    fun openTask(id: String) = openDetails(id)

    /** Comes to the front: says where the person is. */
    fun front() {
        lastId = ""
        refresh(true)
    }

    /** The device's settings changed while in front: the line reads as they now say, unsaid. */
    fun redraw() = refresh(false)

    private fun build(): Frame {
        menu?.let { return it }
        confirming?.let { return confirmation(it) }
        val model = view ?: return Blank
        model.asking?.let { asked -> question(model, asked)?.let { return it } }
        model.line?.let { return quickAdd(model) }
        model.blockForm?.let { return blockForm(model) }
        model.proposal?.let { return proposal(model) }
        model.reminders?.let { return reminders(model) }
        return when (val page = pages.last()) {
            Page.Home -> home(model)
            Page.Listing -> listing(model)
            is Page.Task -> task(model, page.id)
            Page.Settings -> settings(model)
            is Page.SettingsPage -> settingsPage(model, page.id)
            Page.Devices -> devices(model)
            Page.Reminders -> reminders(model)
            Page.Due -> due(model)
            Page.Help -> help()
        }
    }

    /** What a form held goes with it: the next one opened starts from its own values. */
    private fun forgetClosed(model: ViewModel?) {
        if (model?.blockForm == null) forget("block:", "blockform")
        if (model?.line == null) forget("quickadd", "quickadd")
    }

    private fun forget(editorPrefix: String, frameId: String) {
        editors.keys.removeAll { it.startsWith(editorPrefix) }
        selected.remove(frameId)
    }

    private object Blank : Frame("blank") {
        override fun key(key: Key) = false
        override val rendered = Rendered("")
    }

    private fun refresh(moved: Boolean, fromKey: Boolean = false) {
        val model = view
        val landed = model != null && model.landing != lastLanding
        landedNow = landed
        if (model != null) lastLanding = model.landing
        forgetClosed(model)
        if (model != null) follow(model)
        // The last reminder dealt with leaves nothing on its page.
        if (pages.last() == Page.Due && dueShown && model?.due.isNullOrEmpty()) pages.removeLast()
        val built = build()
        frame = built
        val rendered = built.rendered
        if (fromKey) clearMessage()
        if (!messageUp) out.show(rendered)
        out.selecting(boxOf(built)?.selecting == true)
        drawn(rendered.speech)
        val fresh = built.id != lastId
        lastId = built.id
        // A side list shown is what the app acts on from now: until it is told, a letter pressed
        // here would run on the row in hand in the main list, which the person cannot see.
        if (fresh && built is ListFrame) sides[built.id]?.let { side -> built.current?.let { held(it.key, side) } }
        val firstContent = lastSpeech.isEmpty() && rendered.speech.isNotEmpty()
        val reworded = awaitingChange && rendered.speech != lastSpeech
        if (reworded) awaitingChange = false
        if (moved || fresh || landed || firstContent || reworded) {
            val title = when {
                !fresh -> ""
                built is ListFrame -> built.title
                built is FormFrame -> built.title?.let(::dialogTitle).orEmpty()
                else -> ""
            }
            val row = rendered.speech
            // A menu's command opens its screen inside the key that chose it, and that key then
            // redraws the same screen: the screen's name, already due to be said, stays.
            pendingRow = when {
                title.isBlank() && pendingRow?.endsWith(row) == true -> pendingRow
                title.isBlank() -> row
                title.last() in ".?!" -> "$title $row"
                else -> "$title. $row"
            }
        }
        lastSpeech = rendered.speech
        if (fromKey) flush() else if (pendingRow != null) schedule()
    }

    /** A key, from the braille keys or the QWERTY keyboard. */
    fun press(key: Key) {
        // The answer says what it did and nothing more: the box it returns to is not read again.
        if (confirming != null && (key == Key.Escape || key == Key.Enter)) {
            out.silence()
            clearMessage()
            return answerConfirmation(key == Key.Enter && (frame as? ListFrame)?.current?.key == "yes")
        }
        when (key) {
            Key.Silence -> return out.silence()
            // Outside a line the dots are what they were: Space with them.
            Key.SayCharacter, Key.SayWord -> if (frame?.let(::boxOf) == null) {
                return press(Key.SpaceDots(if (key == Key.SayWord) SAY_WORD_DOTS else SAY_CHARACTER_DOTS))
            }
            // A multi-line box reads the cursor's line itself; anything else is read whole.
            Key.Repeat -> if (frame?.let(::boxOf)?.multiLine != true) return run {
                clearMessage()
                frame?.rendered?.let { out.say(it.speech); out.show(it) }
            }
            Key.ScrollOn -> {
                clearMessage()
                if (!out.scrollOn() && frame !is FormFrame) step(Key.Down)
                return
            }
            Key.ScrollBack -> {
                clearMessage()
                if (!out.scrollBack() && frame !is FormFrame) step(Key.Up)
                return
            }
            is Key.Route -> {
                clearMessage()
                val at = out.routed(key.cell)
                if (at >= 0) {
                    val routed = frame
                    routed?.routed(at)
                    // In an edit box the cursor only moves, as in the device's own: nothing is said.
                    refresh(routed == null || boxOf(routed) == null)
                }
                return
            }
            else -> {}
        }
        out.silence()
        val current = frame ?: build()
        boxOf(current)?.let { box -> if (edit(current, box, key)) return }
        if (current.key(key)) {
            if (changes(current, key)) {
                clearMessage()
                awaitingChange = true
                refresh(false)
            } else {
                refresh(true, fromKey = true)
            }
            return
        }
        // What a frame leaves is the program's.
        when (key) {
            Key.Menu -> openMenu()
            // Escape on the filter's line leaves the filter, cleared, and the list stays.
            Key.Escape -> if (filtering) clearFilter() else back()
            Key.Tab, Key.ShiftTab -> if (current is ListFrame) announce(appWords.braille.tabInAList)
            Key.Exit -> host.exit()
            Key.Help -> pushPage(Page.Help)
            Key.Options -> pushPage(Page.Settings)
            Key.Find -> startFiltering()
            Key.Context -> announce(appWords.braille.noActionsHere)
            is Key.SpaceDots -> commandLetter(BrailleLetters.of(key.dots))
            is Key.EnterDots -> commandLetter(BrailleLetters.of(key.dots))
            is Key.SpaceLetter -> commandLetter(key.letter)
            is Key.EnterLetter -> commandLetter(key.letter)
            else -> {}
        }
    }

    /**
     * The app's own moves between Home and a place: a place Home opened (Enter, a letter, a
     * command that goes to one) is shown, as a shell follows the app.
     */
    private fun follow(model: ViewModel) {
        if (model.home != null) awaitingHome = false
        if (pages.last() == Page.Home && model.home == null && !awaitingHome && model.place != null) pages.addLast(Page.Listing)
    }

    /** Back to Home, which the app shows again. */
    private fun goHome() {
        pages.clear()
        pages.addLast(Page.Home)
        awaitingHome = true
        send(ShellEvent.Home)
        refresh(true)
    }

    private fun step(key: Key) {
        out.silence()
        frame?.key(key)
        refresh(true, fromKey = true)
    }

    /** A key that asks the app to change what the row says: said once the app has answered. */
    private fun changes(frame: Frame, key: Key): Boolean {
        val control = (frame as? FormFrame)?.current
        return when {
            control is Control.Choice -> key == Key.Up || key == Key.Down || key == Key.Space || key == Key.Backspace || key == Key.First || key == Key.Last
            control is Control.Toggle -> key == Key.Space || key == Key.Backspace || key == Key.Enter
            frame is ListFrame -> (key == Key.Space && frame.current?.space != null) ||
                (key == Key.CharLeft && frame.current?.collapse != null) || (key == Key.CharRight && frame.current?.expand != null)
            else -> false
        }
    }

    private var awaitingChange = false

    private fun boxOf(frame: Frame): TextBox? = (frame as? LineFrame)?.box ?: ((frame as? FormFrame)?.current as? Control.Edit)?.box

    /**
     * A key in an edit box, as the device's own box takes it (`BoxEditing`): what it changed is told
     * to the form or line, the cells drawn again, and what the device says for it said. A key that
     * is not the box's goes on to the frame.
     */
    private fun edit(frame: Frame, box: TextBox, key: Key): Boolean {
        val display = object : EditDisplay {
            override val width: Int get() = out.cellsWide()

            override fun cursorLine(): IntRange? {
                // The line as the display shows it at the cursor, not a message over it.
                clearMessage()
                out.show(frame.rendered)
                return out.cursorLine()
            }
        }
        when (val edited = editing.press(box, key, display)) {
            Edited.NotTaken -> return false
            is Edited.Taken -> told(frame, box, edited)
            is Edited.Ask -> confirm(edited.question, yes = { told(frame, box, edited.answer(true)) }, no = { told(frame, box, edited.answer(false)) })
        }
        return true
    }

    private fun told(frame: Frame, box: TextBox, edited: Edited.Taken) {
        if (edited.changed) {
            ((frame as? FormFrame)?.current as? Control.Edit)?.changed?.invoke(box.text)
            (frame as? LineFrame)?.touched()
        }
        clearMessage()
        refresh(false)
        edited.said?.let(out::say)
    }

    /** A yes-or-no question of the shell's own over an edit box, as the device's prompt asks it. */
    private class Confirming(val prompt: String, val returnTo: String, val yes: () -> Unit, val no: () -> Unit)

    private var confirming: Confirming? = null

    /** Asks [prompt], Yes first, and runs [yes] or [no] with the box it was asked over back in front. */
    private fun confirm(prompt: String, yes: () -> Unit, no: () -> Unit) {
        confirming = Confirming(prompt, lastId, yes, no)
        clearMessage()
        refresh(true, fromKey = true)
    }

    private fun confirmation(asked: Confirming): Frame {
        val rows = appWords.yesNo.map { choice ->
            Row(choice.id, choice.title, role = ControlRole.PROMPT_BUTTON, open = { answerConfirmation(choice.id == "yes") })
        }
        return list(CONFIRMING, asked.prompt, rows, null, "", hold = false, prompt = true)
    }

    private fun answerConfirmation(yes: Boolean) {
        val asked = confirming ?: return
        confirming = null
        selected.remove(CONFIRMING)
        // Back to the box as it was, not a new screen to announce.
        lastId = asked.returnTo
        if (yes) asked.yes() else asked.no()
    }

    private fun pushPage(page: Page) {
        pages.addLast(page)
        when (page) {
            Page.Settings, is Page.SettingsPage -> send(ShellEvent.Key("settings"))
            Page.Devices -> {
                // What pairing's sentences call this device, which finds others on its network.
                send(ShellEvent.PairingAs(appWords.thisDevice, true))
                send(ShellEvent.Devices(DevicesEvent.Show))
                send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Open(host.deviceName()))))
            }
            else -> {}
        }
        refresh(true)
    }

    private fun back() {
        val model = view
        when {
            model?.asking != null -> send(ShellEvent.Cancel)
            model?.line != null -> send(ShellEvent.Typing(Typing.Close))
            model?.blockForm != null -> send(ShellEvent.Block(BlockEvent.Cancel))
            model?.proposal != null -> send(ShellEvent.Proposal(ProposalEvent.Close))
            model?.reminders != null -> send(ShellEvent.CloseReminders)
            pages.size > 1 -> {
                val left = pages.removeLast()
                when (left) {
                    // Leaving the task's form throws away what was not saved, as its Cancel does.
                    is Page.Task -> {
                        send(ShellEvent.Details(DetailsEvent.Discard))
                        forget("task:${left.id}", "task:${left.id}")
                    }
                    is Page.SettingsPage -> forget("setting:", "settings:${left.id}")
                    Page.Devices -> {
                        send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Cancel)))
                        forget("pairing-code", "devices")
                    }
                    else -> {}
                }
                if (left is Page.Settings || left is Page.SettingsPage) {
                    if (pages.last() !is Page.Settings && pages.last() !is Page.SettingsPage) send(ShellEvent.Settings(SettingsEvent.Close))
                }
                if (pages.last() == Page.Home) {
                    awaitingHome = true
                    send(ShellEvent.Home)
                }
                refresh(true)
            }
            else -> announce(appWords.braille.atHome)
        }
    }

    /** A letter pressed as a command: the core's on what it holds, else this screen's own. */
    private fun commandLetter(letter: Char?) {
        if (letter == null) return
        val current = frame
        val took = when {
            current is FormFrame -> current.press(letter)
            current is ListFrame && current.current?.command?.invoke(letter) == true -> true
            current is ListFrame && current.appHeld -> return send(ShellEvent.Letter(letter.toString()))
            else -> false
        }
        if (!took) announce(nothingOnLetter(letter.toString()))
    }

    private fun startFiltering() {
        val model = view ?: return
        if (filtering) return
        // Only a list shown is filtered: not Home, a day, a form or a list of the shell's own.
        if (pages.last() != Page.Listing || model.list == null || model.day != null || frame !is ListFrame) {
            return announce(appWords.braille.noListToFilter)
        }
        val key = "filter"
        val line = editor(key, model.query)
        frame = LineFrame("filter", appWords.quickAdd.filter, line, appWords.quickAdd.filterHint, readings, done = {
            editors.remove(key)
            filtering = false
            refresh(true)
        }, changed = { send(ShellEvent.Filter(it)) })
        filtering = true
        out.show(frame!!.rendered)
        out.say(frame!!.rendered.speech)
    }

    private var filtering = false

    /** Leaves the filter's line, the filter cleared and the list as it was. */
    private fun clearFilter() {
        editors.remove("filter")
        filtering = false
        send(ShellEvent.Filter(""))
        refresh(true)
    }

    /** Where a place chosen is going, until the app shows it: its view is the last place's till then. */
    private var goingTo: Place? = null

    /** A place the menu bar goes to (`go-today`, …), by its command, from whatever is shown. */
    private fun goTo(id: String, place: Place) {
        goingTo = place
        send(ShellEvent.Key(id))
        while (pages.size > 1) pages.removeLast()
        pages.addLast(Page.Listing)
        refresh(true)
    }

    /**
     * Space-M or F2: the shared menu bar, as every other platform's, holding every one of the
     * screen's own actions (Home's on Home) with its letter as a real hot key; each command's name
     * and whether it applies now being `ViewModel::commands` and `screen_actions`, and its letter
     * the core's. What does not apply is left out, as the device's own menus hide theirs. A row's
     * own actions are Enter-I.
     */
    private fun openMenu() {
        val model = view ?: return
        val states = model.commands.associateBy { it.id }
        val actions = model.screenActions.associateBy { it.id }
        val place = model.place.takeIf { pages.last() == Page.Listing }
        val groups = brailleMenuBar(place, pages.last() == Page.Home, true).mapNotNull { group ->
            val children = group.items.filter { states[it.id]?.enabled != false && actions[it.id]?.enabled != false }.map { item ->
                val letter = item.letter.singleOrNull()
                MenuEntry(
                    states[item.id]?.title?.ifEmpty { null } ?: actions[item.id]?.title ?: item.title,
                    hotkey = letter?.takeIf { item.hotKey }?.let(BrailleLetters::press),
                    letter = letter,
                    asks = item.asks,
                ) { runMenuCommand(item.id) }
            }
            if (children.isEmpty()) null else MenuEntry(group.title, children = children, letter = group.letter.singleOrNull())
        }
        showMenu(groups)
    }

    private fun showMenu(entries: List<MenuEntry>) {
        menu = MenuFrame(entries, letters, readings, opensOnLetter, notFound = { announce(nothingOnLetter(it.toString())) }, closed = { cancelled ->
            menu = null
            if (cancelled) announce(appWords.cancel)
            refresh(true)
        })
        refresh(true, fromKey = true)
    }

    /**
     * Runs what the menu bar names: what is the shell's own (a page of its, the filter, the key
     * help, About, the reminders due, a heading's places on Home), else the app's, as `Key(id)`.
     */
    private fun runMenuCommand(id: String) {
        when (id) {
            "projects" -> openGroupFromAnywhere(SidebarGroup.PROJECTS)
            "labels" -> openGroupFromAnywhere(SidebarGroup.LABELS)
            "filters" -> openGroupFromAnywhere(SidebarGroup.FILTERS)
            "reminders-due" -> openDue()
            "filter" -> startFiltering()
            "keyboard-help" -> pushPage(Page.Help)
            "about" -> appWords.metadata.let { announce("${it.name} ${it.version}. ${it.description}") }
            "home" -> goHome()
            "settings" -> pushPage(Page.Settings)
            "devices" -> pushPage(Page.Devices)
            "export-import" -> pushPage(Page.SettingsPage("export"))
            "restore-backup" -> pushPage(Page.SettingsPage("backups"))
            "close-window", "quit" -> host.exit()
            "go-today" -> goTo(id, Place.Today)
            "go-tasks" -> goTo(id, Place.Tasks)
            "go-blocks" -> goTo(id, Place.Blocks)
            "go-trash" -> goTo(id, Place.Trash)
            "go-history" -> goTo(id, Place.History)
            else -> send(ShellEvent.Key(id))
        }
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

    private fun actionsMenu(row: Row) = showMenu(row.actions)

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
            if (hold && !row.local) held(row.key, side)
        }, openActions = ::actionsMenu,
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

    /** What is on the display, for a sighted helper's screen. */
    var drawn: (String) -> Unit = {}

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
                detail = listOfNotNull(entry.detail.ifEmpty { null }, entry.expanded?.let(::foldState), entry.level).joinToString(", "),
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
                open = { runScreenAction(action) }, command = { homeLetter(home, it) }, local = true)
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
            refresh(true, fromKey = true)
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
        if (filtering) frame?.takeIf { it.id == "filter" }?.let { return it }
        // Until the app shows the place chosen, its view is the place left.
        goingTo?.let { if (model.place != it) return Rendering("") else goingTo = null }
        model.day?.let { return day(model, it) }
        val list = model.list ?: return Rendering(model.title)
        val worded = lines?.list.orEmpty()
        val rows = shownRows(list.items, worded).map { (item, line) ->
            val task = item.row
            val offered = list.offeredOf(item)
            val on = ActingOn.Row(task.id)
            val state = listOfNotNull(item.row.expanded?.let(::foldState), item.level, line?.detail?.ifEmpty { null })
            Row(
                task.id,
                (listOf(line?.title ?: task.title) + state).joinToString(", "),
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
            val state = listOfNotNull(item.expanded?.let(::foldState), item.level)
            val actions = entries(on, offered)
            Row(
                item.key,
                (listOf(line?.text.orEmpty()) + state).joinToString(", "),
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

    private fun task(model: ViewModel, id: String): Frame {
        val details = model.details?.takeIf { it.task.id == id } ?: return Rendering(appWords.details.gone)
        val form = details.fields
        val set = { key: String, value: String -> send(ShellEvent.Details(DetailsEvent.Field(key, value))) }
        val controls = mutableListOf<Control>()
        details.form.forEach { field ->
            when {
                field.key == "project" -> {
                    val options = details.projects.map { it.id to it.title }
                    val at = details.projects.indexOfFirst { it.title == form.project }.coerceAtLeast(0)
                    controls += Control.Choice(field.key, field.label, options, at, { set("project", it) })
                }
                form.text(field.key) != null -> {
                    val edit = editor("task:$id:${field.key}", form.text(field.key).orEmpty(), multiLine = field.kind == FieldKind.LINES)
                    controls += Control.Edit(field.key, field.label, edit, field.hint, changed = { set(field.key, it) })
                }
                else -> {
                    val options = field.options.map { it.id to it.title }
                    val at = field.options.indexOfFirst { it.id == form.priority.toString() }.coerceAtLeast(0)
                    controls += Control.Choice(field.key, field.label, options, at, { set(field.key, it) })
                }
            }
        }
        controls += Control.Button("save", appWords.details.save) { send(ShellEvent.Details(DetailsEvent.Save)) }
        details.lines.forEachIndexed { index, line -> controls += Control.Static("line$index", "${line.label}: ${line.value}") }
        details.task.offered.forEach { action ->
            controls += Control.Button("act:${action.kind}", action.name, actionLetter(action.kind)?.singleOrNull(), asks = action.asks != Asks.NOTHING) {
                send(ShellEvent.ActOn(ActingOn.Details, action))
            }
        }
        return form("task:$id", details.task.title, controls)
    }

    /**
     * A dialog. Its buttons each have a letter, pressed with Space or Enter: the core's where it
     * gives one, else the first free one of its name (`menu_shortcuts`), as a menu's items do.
     */
    private fun form(id: String, title: String?, controls: List<Control>): Frame {
        val buttons = controls.filterIsInstance<Control.Button>()
        val given = letters.shortcuts(buttons.map { it.label to it.letter }).zip(buttons).associate { (letter, button) -> button to letter }
        val lettered = controls.map { control ->
            if (control is Control.Button && control.letter == null && given[control] != null) {
                Control.Button(control.key, control.label, given[control], control.asks, control.press)
            } else {
                control
            }
        }
        return FormFrame(id, title, lettered, selected[id], letters, readings, moved = { selected[id] = it.key })
    }

    private fun blockForm(model: ViewModel): Frame {
        val form = model.blockForm!!
        val fields = form.values
        val set = { key: String, value: String -> send(ShellEvent.Block(BlockEvent.Field(key, value))) }
        val controls = mutableListOf<Control>()
        form.refusal?.let { controls += Control.Static("refusal", it) }
        form.fields.forEach { field ->
            when (field.kind) {
                FieldKind.CHOICE -> {
                    val options = field.options.map { it.id to it.title }
                    val at = field.options.indexOfFirst { it.id == fields.text(field.key) }.coerceAtLeast(0)
                    controls += Control.Choice(field.key, field.label, options, at, { set(field.key, it) })
                }
                FieldKind.TOGGLE -> {
                    val on = fields.flag(field.key)
                    controls += Control.Toggle(field.key, field.label, on) { set(field.key, it.toString()) }
                }
                else -> {
                    val initial = if (field.key == "date") form.date else fields.text(field.key)
                    val edit = editor("block:${form.heading}:${field.key}", initial, multiLine = field.kind == FieldKind.LINES)
                    controls += Control.Edit(field.key, field.label, edit, field.hint, changed = { set(field.key, it.trim()) })
                }
            }
        }
        controls += Control.Button("save", form.yes) { send(ShellEvent.Block(BlockEvent.Save)) }
        return form("blockform", form.heading, controls)
    }

    // ── Questions ──────────────────────────────────────────────────────────────────────────

    private fun question(model: ViewModel, asked: dev.emassey0135.lumenna.app.Asking): Frame? {
        val shown = (screen.shown as? Shown.Question)?.field0?.takeIf { it.number == asked.number }
            ?: return if (asked.question is Question.Immediate) null else Rendering("")
        val id = "question:${asked.number}"
        val cancelled = { send(ShellEvent.Cancel) }
        val answer = { given: Answer -> send(ShellEvent.Answer(given)) }
        if (shown.form) {
            // A task's form is its own screen here; building this frame opens it once.
            if (handledForm != asked.number) {
                handledForm = asked.number
                val action = asked.action
                if (action != null && action.kind == ActionKind.EDIT_TASK) {
                    send(ShellEvent.Details(DetailsEvent.Show(action.target)))
                    pages.addLast(Page.Task(action.target))
                }
                cancelled()
            }
            return null
        }
        if (shown.line) {
            return LineFrame(id, shown.prompt, editor(id, shown.typed), readings = readings, done = {
                answer(Answer.Text(it))
                editors.remove(id)
            })
        }
        if (asked.question is Question.Pick) {
            val offered = asked.choices ?: return Rendering(shown.prompt)
            val rows = offered.choices.mapIndexed { index, choice ->
                val line = shown.choices.getOrNull(index)
                Row(choice.id, listOfNotNull(line?.text ?: choice.title, choice.reason, line?.level).joinToString(", "),
                    braille = line?.braille?.ifEmpty { null } ?: line?.text ?: choice.title, finding = choice.title,
                    open = { answer(Answer.Picked(choice.id, null)) })
            }
            return list(id, shown.prompt, rows, null, "", hold = false)
        }
        val rows = shown.buttons.map { button ->
            Row(button.id, button.title, role = ControlRole.PROMPT_BUTTON, open = {
                when (button.id) {
                    "yes" -> answer(Answer.Yes)
                    "cancel" -> cancelled()
                    else -> answer(Answer.Picked(button.id, null))
                }
            })
        }
        return list(id, shown.prompt, rows, null, "", hold = false, prompt = true)
    }

    // ── Quick add ──────────────────────────────────────────────────────────────────────────

    /** The line as it stood when a completion was chosen, until the app's new one arrives. */
    private var completingFrom: String? = null

    private fun quickAdd(model: ViewModel): Frame {
        val line = model.line!!
        val key = "quickadd"
        if (completingFrom != null && line.text != completingFrom) {
            completingFrom = null
            editors.remove(key)
        }
        val edit = editor(key, line.text)
        val words = appWords.quickAdd
        val controls = mutableListOf<Control>()
        val typing = { text: String -> send(ShellEvent.Typing(Typing.Text(text, TextOffsets.bytes(text.length, text)))) }
        controls += Control.Edit(key, words.label, edit, words.example, typing, submit = { send(ShellEvent.QuickAdd(edit.text)) })
        val candidates = line.offer.candidates
        candidates.take(appWords.completionsShown.toInt()).forEachIndexed { index, candidate ->
            controls += Control.Button("complete$index", dev.emassey0135.lumenna.app.completeWith(candidate)) {
                completingFrom = line.text
                send(ShellEvent.Typing(Typing.Choose(index.toUInt())))
            }
        }
        line.readback.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("readback", it.replaceFirstChar { c -> c.uppercase() }) }
        model.addRefused?.let { controls += Control.Static("refused", it) }
        controls += Control.Button("add", words.add) { send(ShellEvent.QuickAdd(edit.text)) }
        return form("quickadd", commandTitle("new-task"), controls)
    }

    // ── Proposals ──────────────────────────────────────────────────────────────────────────

    private fun proposal(model: ViewModel): Frame {
        val proposal = model.proposal!!
        val words = proposal.words
        val rows = mutableListOf<Row>()
        proposal.refusal?.let { rows += Row("refusal", it) }
        if (proposal.ready && proposal.changes.isEmpty()) rows += Row("nothing", words.nothing)
        proposal.changes.forEachIndexed { index, change ->
            val text = listOfNotNull(change.state, change.sentence, change.reason).joinToString(", ")
            rows += Row("change$index", text, finding = change.sentence, open = { send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked))) },
                space = { send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked))) })
        }
        proposal.unfit.forEachIndexed { index, line -> rows += Row("unfit$index", "${words.unfit}: $line") }
        proposal.others.forEachIndexed { index, other ->
            rows += Row("other$index", other, open = { send(ShellEvent.Proposal(ProposalEvent.Take(index.toUInt()))) })
        }
        if (proposal.changes.isNotEmpty()) rows += Row("apply", words.apply, role = ControlRole.BUTTON, open = { send(ShellEvent.Proposal(ProposalEvent.Apply)) })
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

    /** Whether the reminders due page has shown any, so that the last dealt with leaves it. */
    private var dueShown = false

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
            Row(due.id, due.text, open = if (buttons.isNotEmpty()) ({ showMenu(buttons) }) else null, actions = buttons,
                delete = { send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.Dismiss(due.id))) })
        }
        return list("due", appWords.remindersDue, rows, model.dueHeld.focus, appWords.braille.nothingDue, side = Side.DUE)
    }

    // ── Settings ──────────────────────────────────────────────────────────────────────────

    private fun settings(model: ViewModel): Frame {
        val pages = model.settings?.pages.orEmpty()
        val open = { page: dev.emassey0135.lumenna.app.SettingsPage ->
            pushPage(if (page.id == "devices") Page.Devices else Page.SettingsPage(page.id))
        }
        val byLetter = { pressed: Char ->
            pages.firstOrNull { pageLetter(it.id)?.singleOrNull() == pressed }?.let { open(it); true } ?: false
        }
        val rows = pages.map { page ->
            Row(page.id, page.title, finding = page.title, detail = page.description, letter = pageLetter(page.id)?.singleOrNull(),
                open = { open(page) }, command = byLetter)
        }
        return list("settings", commandTitle("settings"), rows, null, "", hold = false)
    }

    private fun settingsPage(model: ViewModel, id: String): Frame {
        val page = model.settings?.pages?.firstOrNull { it.id == id } ?: SettingsPages.of(id)
        val controls = mutableListOf<Control>()
        val all = model.settings?.settings.orEmpty().on(page)
        all.filter { it.setting.kind != SettingKind.FOLDER }.forEach { addSetting(controls, it) }
        if (id == "reminders") reminderControls(controls)
        page?.buttons.orEmpty().forEach { button ->
            controls += Control.Button(button.id, button.title, buttonLetter(button.id)?.singleOrNull()) {
                // Syncing and backing up are the app's commands, the same from here as anywhere.
                when (button.id) {
                    "back-up", "sync-now" -> send(ShellEvent.Key(button.id))
                    else -> host.importFile()
                }
            }
        }
        if (id == "export") model.settings?.exports.orEmpty().forEach { export ->
            controls += Control.Button("export:${export.format}", export.label) { send(ShellEvent.Settings(SettingsEvent.Export(export.format))) }
        }
        page?.caption?.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("caption", it) }
        if (id == "backups" || id == "export") {
            model.settings?.offered.orEmpty().forEach { action ->
                controls += Control.Button("act:${action.kind}", action.name, asks = action.asks != Asks.NOTHING) { send(ShellEvent.ActOn(ActingOn.Settings, action)) }
            }
            model.settings?.offeredCaption?.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("offeredCaption", it) }
        }
        return form("settings:$id", page?.title, controls)
    }

    private fun addSetting(controls: MutableList<Control>, shown: SettingView) {
        val setting = shown.setting
        val set = { value: String -> send(ShellEvent.Settings(SettingsEvent.Set(setting.key, value))) }
        when (setting.kind) {
            SettingKind.TOGGLE -> {
                val on = setting.value == "true"
                controls += Control.Toggle(setting.key, setting.title, on) { set(if (it) "true" else "false") }
            }
            SettingKind.CHOICE -> {
                val options = setting.options.map { it.id to it.title }
                val at = setting.options.indexOfFirst { it.id == setting.value }.coerceAtLeast(0)
                controls += Control.Choice(setting.key, setting.title, options, at, set)
            }
            else -> {
                val edit = editor("setting:${setting.key}", shown.typed.ifEmpty { setting.value })
                controls += Control.Edit(setting.key, setting.title, edit, setting.hint, changed = {
                    send(ShellEvent.Settings(SettingsEvent.Type(setting.key, it)))
                })
            }
        }
        shown.problem?.let { controls += Control.Static("problem:${setting.key}", it) }
        if (setting.hint.isNotEmpty() && setting.kind != SettingKind.LINE && setting.kind != SettingKind.TIME && setting.kind != SettingKind.NUMBER) {
            controls += Control.Static("hint:${setting.key}", setting.hint)
        }
    }

    private fun reminderControls(controls: MutableList<Control>) {
        val words = appWords.notifications
        val access = reminderAccess()
        if (!access.notifications) {
            controls += Control.Static("notifications-off", words.off)
            controls += Control.Button("allow-notifications", words.allow) { host.requestNotifications() }
        }
        if (!access.exact) controls += Control.Static("exact-late", words.late)
    }

    /** What Android allows reminders, asked of the system each time it is shown. */
    var reminderAccess: () -> dev.emassey0135.lumenna.ReminderAccess = { dev.emassey0135.lumenna.ReminderAccess(true, true, null) }

    // ── Devices ───────────────────────────────────────────────────────────────────────────

    private fun devices(model: ViewModel): Frame {
        val devices = model.devices ?: return Rendering(SettingsPages.of("devices")?.title.orEmpty())
        val page = SettingsPages.of("devices")
        val worded = lines?.devices.orEmpty()
        val controls = mutableListOf<Control>()
        devices.status.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("status", it) }
        val syncing = page?.buttons?.firstOrNull { it.id == "sync-now" }?.title ?: commandTitle("sync-now")
        controls += Control.Button("sync", syncing) { send(ShellEvent.Key("sync-now")) }
        devices.list?.devices.orEmpty().forEachIndexed { index, device ->
            val line = worded.getOrNull(index)
            val text = line?.text ?: device.name
            val on = ActingOn.Held(Side.DEVICES, device.nodeId)
            device.offered.forEach { action ->
                controls += Control.Button("${device.nodeId}:${action.kind}", "$text: ${action.name}") { send(ShellEvent.ActOn(on, action)) }
            }
            if (device.offered.isEmpty()) controls += Control.Static(device.nodeId, text)
        }
        addPairing(controls, devices)
        return form("devices", page?.title, controls)
    }

    private fun addPairing(controls: MutableList<Control>, devices: dev.emassey0135.lumenna.app.DevicesView) {
        val words = devices.words
        val pairing = devices.pairing
        val pair = { event: PairEvent -> send(ShellEvent.Devices(DevicesEvent.Pair(event))) }
        controls += Control.Static("pairing", words.title)
        val status = pairing?.problem ?: when {
            pairing?.mine != null -> words.waiting
            pairing?.running == true -> words.connecting
            else -> words.introSentence.ifEmpty { words.intro }
        }
        controls += Control.Static("pairing-status", status)
        pairing?.mineLine?.let { controls += Control.Static("mine", it) }
        if (pairing?.running != true) controls += Control.Button("wait", words.wait) { pair(PairEvent.Wait) }
        if (pairing?.running != true || pairing.waiting) {
            val edit = editor("pairing-code", pairing?.theirs.orEmpty())
            controls += Control.Edit("code", words.theirCode, edit, changed = { pair(PairEvent.Code(it)) }, submit = {
                pair(PairEvent.Code(edit.text))
                pair(PairEvent.Join)
            })
            controls += Control.Button("join", words.join) {
                pair(PairEvent.Code(edit.text))
                pair(PairEvent.Join)
            }
        }
        pairing?.words?.let { three ->
            controls += Control.Static("match", "${words.matchTitle} ${pairing.matching ?: three.joinToString(", ")}")
            controls += Control.Button("match-yes", words.matchYes) { pair(PairEvent.Confirm(true)) }
            controls += Control.Button("match-no", words.matchNo) { pair(PairEvent.Confirm(false)) }
        }
    }

    // ── Help ─────────────────────────────────────────────────────────────────────────────

    private fun help(): Frame {
        val device = dev.emassey0135.lumenna.app.braillesenseKeys().map { row ->
            Row(row.keys, "${row.keys}: ${row.what}", finding = row.what)
        }
        // What the screen under it adds, each by the letter that runs it.
        val here = view?.screenActions.orEmpty().mapNotNull { action ->
            val press = action.letter.singleOrNull()?.let(BrailleLetters::press) ?: return@mapNotNull null
            Row("command:${action.id}", "$press: ${action.title}", finding = action.title)
        }
        return list("help", appWords.braille.keyHelp, device + here, null, "", hold = false)
    }

    private companion object {
        /** Dots 3-4-5-6, which begin a number. */
        /** Space-2-5 and Space-3-6, the device's read word and read character, as dots. */
        private const val SAY_WORD_DOTS = 0x12
        private const val SAY_CHARACTER_DOTS = 0x24

        /** The shell's own yes-or-no question. */
        private const val CONFIRMING = "confirming"


        /** How long after a turn its speech waits, for what the app said to arrive with it. */
        const val SPEECH_DELAY = 60L

    }
}
