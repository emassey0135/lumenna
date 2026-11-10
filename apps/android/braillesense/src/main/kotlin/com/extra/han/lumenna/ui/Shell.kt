package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Echo
import com.extra.han.lumenna.device.Key
import com.extra.han.lumenna.device.Translator
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
import dev.emassey0135.lumenna.app.ControlRole
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.DayKind
import dev.emassey0135.lumenna.app.DayMove
import dev.emassey0135.lumenna.app.DetailsEvent
import dev.emassey0135.lumenna.app.DevicesEvent
import dev.emassey0135.lumenna.app.FieldKind
import dev.emassey0135.lumenna.app.Lines
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ProposalEvent
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.QuestionShown
import dev.emassey0135.lumenna.app.ScreenChange
import dev.emassey0135.lumenna.app.ScreenView
import dev.emassey0135.lumenna.app.Shown
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.brailleMainMenu
import dev.emassey0135.lumenna.app.brailleMenuBar
import dev.emassey0135.lumenna.app.groupEmpty
import dev.emassey0135.lumenna.app.levels
import dev.emassey0135.lumenna.app.pressToAdd
import dev.emassey0135.lumenna.app.sidebarKey
import dev.emassey0135.lumenna.app.buttonLetter
import dev.emassey0135.lumenna.app.braillesenseCellEcho
import dev.emassey0135.lumenna.app.braillesenseCharacterEcho
import dev.emassey0135.lumenna.app.commandLetter
import dev.emassey0135.lumenna.app.dialogTitle
import dev.emassey0135.lumenna.app.dueLetter
import dev.emassey0135.lumenna.app.nothingOnLetter
import dev.emassey0135.lumenna.app.noRowStartsWith
import dev.emassey0135.lumenna.app.pageLetter
import dev.emassey0135.lumenna.app.placeCommands
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

    /** What the system clipboard holds as text, null for nothing. */
    fun clipboard(): String?

    /** Puts [text] on the system clipboard, or empties it with null. */
    fun setClipboard(text: String?)

    fun importFile()
    fun requestNotifications()
    fun openNotificationSettings()
    fun deviceName(): String
}

/** A screen the person went to, on the way back from which Escape walks. */
private sealed interface Page {
    data object Places : Page
    /** One heading's places: the projects, labels or saved filters. */
    data class Group(val group: SidebarGroup) : Page
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
    private val out: Speaker,
    private val translator: Translator,
    private val host: Host,
    private val readings: Readings,
    private val letters: Letters = BrailleLetters,
    /** How long a message stays on the cells: the device's Message Display Time. */
    private val messageMillis: () -> Long = { 3000L },
    /** Automatically Open Menus with Shortcut: whether a letter opens what it names or only goes to it. */
    private val opensOnLetter: () -> Boolean = { true },
    private val echoSetting: () -> Echo = { Echo.of(1) },
) {
    private val app: AppModel get() = core.app
    private val pages = ArrayDeque<Page>(listOf(Page.Places))
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
        val model = view ?: return Blank
        model.asking?.let { asked -> question(model, asked)?.let { return it } }
        model.line?.let { return quickAdd(model) }
        model.blockForm?.let { return blockForm(model) }
        model.proposal?.let { return proposal(model) }
        model.reminders?.let { return reminders(model) }
        return when (val page = pages.last()) {
            Page.Places -> places(model)
            is Page.Group -> groupPage(model, page.group)
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
        // The last reminder dealt with leaves nothing on its page.
        if (pages.last() == Page.Due && model?.due.isNullOrEmpty()) pages.removeLast()
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
        when (key) {
            Key.Silence -> return out.silence()
            Key.SayCharacter, Key.SayWord -> {
                val editor = frame?.let { editorOf(it) }
                if (editor != null) {
                    clearMessage()
                    out.silence()
                    // At the end of the line the cursor is past the last character: that one is said.
                    if (key == Key.SayWord) sayWord(editor) else (editor.cellAtCaret() ?: editor.cellBeforeCaret())?.let(::sayCell)
                    return
                }
                // Outside a line the dots are what they were: Space with them.
                return press(Key.SpaceDots(if (key == Key.SayWord) SAY_WORD_DOTS else SAY_CHARACTER_DOTS))
            }
            Key.Repeat -> return run {
                clearMessage()
                // In a multi-line box, the line the cursor is on, as the device's own reads it.
                frame?.let(::boxOf)?.takeIf { it.multiLine }?.let { box ->
                    out.silence()
                    sayLine(box)
                    return
                }
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
                    refresh(routed == null || editorOf(routed) == null)
                }
                return
            }
            else -> {}
        }
        out.silence()
        val current = frame ?: build()
        boxOf(current)?.let { box -> if (editCommand(current, box, key)) return }
        if (current.key(key)) {
            if (typing(current, key)) {
                clearMessage()
                refresh(false)
                echo(current, key)
            } else if (caretMoves(current, key)) {
                clearMessage()
                refresh(false)
                sayCaret(current, key)
            } else if (changes(current, key)) {
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
            Key.Escape -> back()
            Key.Exit -> host.exit()
            Key.Help -> pushPage(Page.Help)
            Key.Options -> pushPage(Page.Settings)
            Key.Find -> startFiltering()
            Key.Context -> {}
            is Key.SpaceDots -> commandLetter(BrailleLetters.of(key.dots))
            is Key.EnterDots -> commandLetter(BrailleLetters.of(key.dots))
            is Key.SpaceLetter -> commandLetter(key.letter)
            is Key.EnterLetter -> commandLetter(key.letter)
            else -> {}
        }
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

    private fun editorOf(frame: Frame): LineEditor? = boxOf(frame)?.line

    private fun typing(frame: Frame, key: Key): Boolean {
        val box = boxOf(frame) ?: return false
        return key is Key.Cell || key is Key.Typed || key == Key.Backspace || key == Key.Space || (key == Key.Enter && box.multiLine)
    }

    private fun caretMoves(frame: Frame, key: Key): Boolean {
        val box = boxOf(frame) ?: return false
        return key == Key.CharLeft || key == Key.CharRight || key == Key.WordLeft || key == Key.WordRight ||
            (box.multiLine && (key == Key.Up || key == Key.Down || key == Key.First || key == Key.Last))
    }

    /**
     * The device's edit-box commands, before the box's own keys: a selection begun with Enter-B or
     * made of everything with Enter-A, copied with Enter-C, cut with Enter-X, added to the clipboard
     * with Enter-P and read with Backspace-1-2; Enter-V pastes and Enter-D clears the clipboard, all
     * through the system's clipboard. With a selection, Backspace takes it and Escape leaves it, and
     * typing ends it. Control with the letter on the QWERTY keyboard is Enter with it.
     */
    private fun editCommand(frame: Frame, box: TextBox, key: Key): Boolean {
        val words = appWords.braille.editBox
        val letter = when (key) {
            is Key.EnterDots -> BrailleLetters.of(key.dots)
            is Key.EnterLetter -> key.letter.lowercaseChar()
            else -> null
        }
        val changed = {
            (frame as? FormFrame)?.current?.let { (it as? Control.Edit)?.changed?.invoke(box.text) }
            (frame as? LineFrame)?.touched()
            refresh(false)
        }
        fun done(said: String) {
            clearMessage()
            refresh(false)
            out.say(said)
        }
        when {
            letter == 'b' -> box.startSelection().also { done(words.startSelection) }
            letter == 'a' -> box.selectAll().also { done(words.selectAll) }
            letter == 'c' || letter == 'x' || letter == 'p' -> {
                val selected = box.selected() ?: return true.also { done(words.nothingSelected) }
                when (letter) {
                    'p' -> host.setClipboard(listOfNotNull(host.clipboard()?.takeIf { it.isNotEmpty() }, selected).joinToString("\n"))
                    else -> host.setClipboard(selected)
                }
                if (letter == 'x') box.deleteSelected() else box.cancelSelection()
                if (letter == 'x') changed()
                done(when (letter) { 'c' -> words.copied; 'x' -> words.cut; else -> words.appended })
            }
            letter == 'v' -> {
                val pasted = host.clipboard()?.takeIf { it.isNotEmpty() } ?: return true.also { done(words.clipboardEmpty) }
                box.insert(pasted)
                changed()
                done(words.pasted)
            }
            letter == 'd' -> host.setClipboard(null).also { done(words.clipboardCleared) }
            key is Key.BackspaceDots && key.dots == SELECTION_READ -> box.selected()?.let { out.say(it) } ?: done(words.nothingSelected)
            !box.selecting -> return false
            key == Key.Backspace -> {
                box.deleteSelected()
                changed()
                done(words.deleted)
            }
            key == Key.Escape -> box.cancelSelection().also { done(words.selectionCanceled) }
            key is Key.Cell || key is Key.Typed || key == Key.Space || key == Key.Enter -> {
                // Typing ends a selection, and goes on to be typed.
                box.cancelSelection()
                return false
            }
            else -> return false
        }
        return true
    }

    /** What typing says, as the device's Keyboard Echo has it; what Backspace takes is always said. */
    private fun echo(frame: Frame, key: Key) {
        val editor = editorOf(frame) ?: return
        val setting = echoSetting()
        when (key) {
            is Key.Cell -> if (setting.characters) editor.cellBeforeCaret()?.let(::sayCell)
            is Key.Typed -> if (setting.characters) out.say(key.text)
            Key.Space -> {
                val word = editor.takeFinished()
                when {
                    word != null && setting.words -> out.say(word)
                    setting.space -> out.say(appWords.braille.space)
                }
            }
            Key.Backspace -> {
                val box = boxOf(frame)
                if (box?.takeJoined() == true) out.say(appWords.braille.editBox.previousLine) else editor.takeRemoved()?.let(::sayCell)
            }
            // A new line, as the device's multi-line box says it: the word it finished where the
            // echo says words, else "Enter".
            Key.Enter -> {
                val word = boxOf(frame)?.takeFinished()
                when {
                    word != null && setting.words -> out.say(word)
                    setting.space -> out.say(appWords.braille.editBox.enter)
                }
            }
            else -> {}
        }
    }

    /** The line the cursor is on, or "blank". */
    private fun sayLine(box: TextBox) {
        out.say(box.line.text.ifBlank { appWords.braille.editBox.blank })
    }

    /**
     * A cell by itself, as the device's own edit boxes say it: in computer braille the character
     * it is, else the core's name for it (its letters, never the whole word it can stand for, and
     * a cell with no letters by its dots).
     */
    private fun sayCell(cell: CellInWord) {
        if (cell.cell == LineEditor.BLANK) return out.say(appWords.braille.space)
        val grade = echoSetting().grade
        val said = if (grade == 0) {
            translator.back(byteArrayOf(cell.cell))?.let(::braillesenseCharacterEcho)
        } else {
            braillesenseCellEcho(cell.word, cell.at.toUInt(), grade == 2)
        }
        said?.takeIf { it.isNotBlank() }?.let(out::say)
    }

    /** The caret moved in a line: what is at it, not the whole line. */
    private fun sayCaret(frame: Frame, key: Key) {
        val editor = editorOf(frame) ?: return
        if (key == Key.Up || key == Key.Down || key == Key.First || key == Key.Last) return boxOf(frame)?.let(::sayLine) ?: Unit
        if (key == Key.WordLeft || key == Key.WordRight) return sayWord(editor)
        editor.cellAtCaret()?.let(::sayCell)
    }

    /** The word at the caret, as Space-2-5 says it. */
    private fun sayWord(editor: LineEditor) {
        editor.wordAtCaret().takeIf { it.isNotBlank() }?.let(out::say)
    }

    private fun pushPage(page: Page) {
        pages.addLast(page)
        when (page) {
            Page.Settings, is Page.SettingsPage -> send(ShellEvent.Settings(SettingsEvent.Open))
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
                    is Page.Task -> forget("task:${left.id}", "task:${left.id}")
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
                refresh(true)
            }
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
        if (model.list == null) return
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

    /** Where a place chosen is going, until the app shows it: its view is the last place's till then. */
    private var goingTo: Place? = null

    private fun goTo(place: Place, fromGroup: Boolean = false) {
        goingTo = place
        send(ShellEvent.Open(place))
        // From a heading's list, Escape comes back to that list.
        if (!fromGroup) while (pages.size > 1) pages.removeLast()
        pages.addLast(Page.Listing)
        refresh(true)
    }

    /**
     * Space-M or F2: the shared menu bar, as every other platform's, each command's name and
     * whether it applies now being `ViewModel::commands` and its letter the core's. What does not
     * apply is left out, as the device's own menus hide theirs. A row's own actions are Enter-I.
     */
    private fun openMenu() {
        val model = view ?: return
        val states = model.commands.associateBy { it.id }
        val place = model.place.takeIf { pages.last() == Page.Listing }
        val groups = brailleMenuBar(place, pages.last() == Page.Places, true).mapNotNull { group ->
            val children = group.items.filter { states[it.id]?.enabled != false }.map { item ->
                val letter = item.letter.singleOrNull()
                MenuEntry(
                    states[item.id]?.title?.ifEmpty { null } ?: item.title,
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

    /** Runs what the menu bar, the main menu or a screen's own commands name. */
    private fun runMenuCommand(id: String) {
        when (id) {
            "projects" -> openGroupFromAnywhere(SidebarGroup.PROJECTS)
            "labels" -> openGroupFromAnywhere(SidebarGroup.LABELS)
            "filters" -> openGroupFromAnywhere(SidebarGroup.FILTERS)
            "reminders-due" -> if (view?.due.isNullOrEmpty()) announce(appWords.braille.nothingDue) else pushPage(Page.Due)
            "new-project-inside" -> commandLetter(id)?.let { send(ShellEvent.Letter(it)) }
            else -> runCommand(id)
        }
    }

    private fun openGroupFromAnywhere(group: SidebarGroup) {
        while (pages.size > 1) pages.removeLast()
        openGroup(group)
    }

    /** One heading's places as their own list; a letter there runs what that row, or the heading, offers. */
    private fun openGroup(group: SidebarGroup) {
        val first = view?.let { under(it, group).firstOrNull() }
        held(first?.key ?: sidebarKey(SidebarKind.Group(group)), Side.SIDEBAR)
        pushPage(Page.Group(group))
    }

    private fun runCommand(id: String) {
        when (id) {
            "settings" -> pushPage(Page.Settings)
            "devices" -> pushPage(Page.Devices)
            "export-import" -> pushPage(Page.SettingsPage("export"))
            "restore-backup" -> pushPage(Page.SettingsPage("backups"))
            "back-up" -> core.backUpNow()
            "close-window", "quit" -> host.exit()
            "go-today" -> goTo(Place.Today)
            "go-tasks" -> goTo(Place.Tasks)
            "go-blocks" -> goTo(Place.Blocks)
            "go-trash" -> goTo(Place.Trash)
            "go-history" -> goTo(Place.History)
            "filter" -> startFiltering()
            "keyboard-help" -> pushPage(Page.Help)
            "about" -> appWords.metadata.let { announce("${it.name} ${it.version}. ${it.description}") }
            else -> send(ShellEvent.Key(id))
        }
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
            if (hold) held(row.key, side)
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

    /** A row of the sidebar, in the list of places or one heading's own. */
    private fun placeRow(
        entry: dev.emassey0135.lumenna.app.SidebarEntry,
        level: String?,
        fromGroup: Boolean,
    ): Row {
        val on = ActingOn.Held(Side.SIDEBAR, entry.key)
        val folding = { kind: ActionKind -> entry.offered.firstOrNull { it.kind == kind }?.let { action -> { send(ShellEvent.ActOn(on, action)) } } }
        return Row(
            entry.key,
            entry.title,
            finding = entry.title,
            detail = listOfNotNull(entry.detail.ifEmpty { null }, entry.expanded?.let(::foldState), level).joinToString(", "),
            letter = if (fromGroup) null else mainCommandOf(entry.kind)?.let(::commandLetter)?.singleOrNull(),
            open = when (val kind = entry.kind) {
                is SidebarKind.Place -> ({ goTo(kind.field0, fromGroup) })
                is SidebarKind.Group -> ({ openGroup(kind.field0) })
            },
            actions = entries(on, entry.offered),
            // A row's own letter runs first, through the app; else the main menu's.
            command = if (fromGroup) null else ({ pressed ->
                entry.offered.none { actionLetter(it.kind)?.singleOrNull() == pressed } && runMain(pressed)
            }),
            collapse = folding(ActionKind.COLLAPSE),
            expand = folding(ActionKind.EXPAND),
        )
    }

    private fun under(model: ViewModel, group: SidebarGroup): List<dev.emassey0135.lumenna.app.SidebarEntry> {
        var inside = false
        return model.sidebar.filter { entry ->
            if (entry.depth == 0u) {
                inside = (entry.kind as? SidebarKind.Group)?.field0 == group
                false
            } else {
                inside && !entry.hidden
            }
        }
    }

    private fun groupPage(model: ViewModel, group: SidebarGroup): Frame {
        val entries = under(model, group)
        val shownLevels = levels(entries.map { it.depth }.toUIntArray())
        val rows = entries.mapIndexed { index, entry -> placeRow(entry, shownLevels.getOrNull(index)?.ifEmpty { null }, fromGroup = true) }
        val title = model.sidebar.firstOrNull { (it.kind as? SidebarKind.Group)?.field0 == group }?.title.orEmpty()
        val add = actionLetter(ActionKind.NEW)?.singleOrNull()?.let(BrailleLetters::press)
        val empty = listOfNotNull(groupEmpty(group), add?.let(::pressToAdd)).joinToString(" ")
        return list("group:$group", title, rows, null, empty, side = Side.SIDEBAR)
    }

    private fun places(model: ViewModel): Frame {
        val rows = model.sidebar.filter { !it.hidden }.map { entry -> placeRow(entry, entry.level, fromGroup = false)
        }.let { sidebar ->
            if (model.due.isEmpty()) sidebar else sidebar + Row(
                "due",
                appWords.remindersDue,
                detail = model.due.size.toString(),
                letter = commandLetter("reminders-due")?.singleOrNull(),
                open = { pushPage(Page.Due) },
            )
        }
        return list("places", appWords.places, rows, model.sidebarHeld.focus, "", side = Side.SIDEBAR, byLetter = { letter -> onPlacesLetter(model, letter) })
    }

    /** The main menu's command that is this sidebar row, if one is. */
    private fun mainCommandOf(kind: SidebarKind): String? = when (kind) {
        is SidebarKind.Place -> when (kind.field0) {
            Place.Today -> "go-today"
            Place.Tasks -> "go-tasks"
            Place.Blocks -> "go-blocks"
            Place.History -> "go-history"
            Place.Trash -> "go-trash"
            else -> null
        }
        is SidebarKind.Group -> when (kind.field0) {
            SidebarGroup.PROJECTS -> "projects"
            SidebarGroup.LABELS -> "labels"
            SidebarGroup.FILTERS -> "filters"
        }
    }

    /** The core's main menu, each by its letter, from the list of places. */
    private fun runMain(letter: Char): Boolean {
        val command = brailleMainMenu(true).firstOrNull { it.letter.singleOrNull() == letter } ?: return false
        runMenuCommand(command.id)
        return true
    }

    /** A letter alone on the list of places: goes to its row, and opens it as the person's setting has it. */
    private fun onPlacesLetter(model: ViewModel, letter: Char): Boolean {
        if (!opensOnLetter()) {
            val row = model.sidebar.firstOrNull { entry ->
                !entry.hidden && mainCommandOf(entry.kind)?.let(::commandLetter)?.singleOrNull() == letter
            }
            if (row != null) {
                selected["places"] = row.key
                held(row.key, Side.SIDEBAR)
                refresh(true, fromKey = true)
                return true
            }
        }
        return runMain(letter)
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
                open = if (model.place == Place.Blocks) ({ held(task.id, null); send(ShellEvent.Key("open")) }) else ({ openDetails(task.id) }),
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
            val edits = offered.any { it.kind == ActionKind.EDIT || it.kind == ActionKind.EDIT_TASK || it.kind == ActionKind.RESTORE_DAY || it.kind == ActionKind.ADD_BLOCK }
            val actions = entries(on, offered)
            Row(
                item.key,
                (listOf(line?.text.orEmpty()) + state).joinToString(", "),
                braille = line?.braille?.ifEmpty { null } ?: line?.text.orEmpty(),
                finding = line?.title.orEmpty(),
                open = when {
                    edits && !heading -> ({ held(item.key, null); send(ShellEvent.Key("open")) })
                    actions.isNotEmpty() -> ({ openDayActions(item.key, actions) })
                    else -> null
                },
                actions = actions,
                delete = { held(item.key, null); send(ShellEvent.Key("delete")) },
                collapse = { held(item.key, null); send(ShellEvent.Key("collapse")) },
                expand = { held(item.key, null); send(ShellEvent.Key("expand")) },
            )
        }
        return list("day:${day.date}", lines?.dayTitle.orEmpty(), rows, model.focus, "", mainList = true)
    }

    private fun openDayActions(key: String, actions: List<MenuEntry>) = showMenu(actions)

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
            controls += Control.Button("act:${action.kind}", action.name, asks = action.asks != Asks.NOTHING) { send(ShellEvent.ActOn(ActingOn.Details, action)) }
        }
        return form("task:$id", details.task.title, controls)
    }

    private fun form(id: String, title: String?, controls: List<Control>): Frame =
        FormFrame(id, title, controls, selected[id], letters, readings, moved = { selected[id] = it.key })

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
            Row(reminder.id, words, open = if (actions.isNotEmpty()) ({ openDayActions(reminder.id, actions) }) else null, actions = actions)
        } + list.offered.map { action -> Row("add:${action.kind}", action.name, open = { send(ShellEvent.ActOn(ActingOn.Reminders, action)) }) }
        val title = list.announcement.replaceFirstChar { it.uppercase() }
        return list("reminders", title, rows, model.remindersHeld.focus, "", side = Side.REMINDERS)
    }

    private fun due(model: ViewModel): Frame {
        val rows = model.due.map { due ->
            val buttons = due.buttons.map { button ->
                val letter = dueLetter(button.id)?.singleOrNull()
                MenuEntry(button.title, hotkey = letter?.let(BrailleLetters::press), letter = letter) { pressDue(due.id, button) }
            }
            Row(due.id, due.text, open = if (buttons.isNotEmpty()) ({ openDayActions(due.id, buttons) }) else null, actions = buttons,
                delete = { send(ShellEvent.Due(dev.emassey0135.lumenna.app.DueEvent.Dismiss(due.id))) })
        }
        return list("due", appWords.remindersDue, rows, model.dueHeld.focus, "", side = Side.DUE)
    }

    private fun pressDue(id: String, button: dev.emassey0135.lumenna.app.DueButton) {
        val event = when (button.id) {
            "dismiss" -> dev.emassey0135.lumenna.app.DueEvent.Dismiss(id)
            "snooze" -> dev.emassey0135.lumenna.app.DueEvent.Snooze(id)
            "carry" -> dev.emassey0135.lumenna.app.DueEvent.Carry(id)
            "replan" -> dev.emassey0135.lumenna.app.DueEvent.Replan(id)
            else -> return
        }
        send(ShellEvent.Due(event))
    }

    // ── Settings ──────────────────────────────────────────────────────────────────────────

    private fun settings(model: ViewModel): Frame {
        val listed = model.settings?.settings.orEmpty()
        val pages = SettingsPages.shown(listed)
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
        val page = SettingsPages.of(id)
        val controls = mutableListOf<Control>()
        val all = model.settings?.settings.orEmpty().on(page)
        all.filter { it.setting.kind != SettingKind.FOLDER }.forEach { addSetting(controls, it) }
        if (id == "reminders") reminderControls(controls)
        page?.buttons.orEmpty().forEach { button ->
            controls += Control.Button(button.id, button.title, buttonLetter(button.id)?.singleOrNull()) {
                when (button.id) {
                    "back-up" -> core.backUpNow()
                    "sync-now" -> app.syncNow()
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
        controls += Control.Button("sync", syncing) { app.syncNow() }
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
        val here = view?.place?.let { placeCommands(it, true) }.orEmpty().mapNotNull { command ->
            val press = command.letter.singleOrNull()?.let(BrailleLetters::press) ?: return@mapNotNull null
            Row("command:${command.id}", "$press: ${command.title}", finding = command.title)
        }
        return list("help", appWords.braille.keyHelp, device + here, null, "", hold = false)
    }

    private companion object {
        /** Dots 3-4-5-6, which begin a number. */
        /** Space-2-5 and Space-3-6, the device's read word and read character, as dots. */
        private const val SAY_WORD_DOTS = 0x12
        private const val SAY_CHARACTER_DOTS = 0x24

        /** Backspace-1-2, the device's "read the selection". */
        private const val SELECTION_READ = 0x03

        /** How long after a turn its speech waits, for what the app said to arrive with it. */
        const val SPEECH_DELAY = 60L

    }
}
