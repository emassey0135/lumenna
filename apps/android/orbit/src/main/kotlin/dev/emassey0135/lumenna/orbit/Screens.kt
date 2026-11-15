package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.commandLetter
import dev.emassey0135.lumenna.app.dueLetter
import dev.emassey0135.lumenna.app.SpeakLetter
import dev.emassey0135.lumenna.app.speakLetter
import dev.emassey0135.lumenna.app.speakPress
import dev.emassey0135.lumenna.app.ListShown
import dev.emassey0135.lumenna.app.QuestionShown
import dev.emassey0135.lumenna.app.Answer
import dev.emassey0135.lumenna.app.Asking
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.Candidate
import dev.emassey0135.lumenna.app.DayMove
import dev.emassey0135.lumenna.app.DueEvent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ProposalEvent
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.Syntax
import dev.emassey0135.lumenna.app.Typing
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.app.foldState
import dev.emassey0135.lumenna.levelsOf
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.offeredOf
import dev.emassey0135.orbit.EditBox
import dev.emassey0135.orbit.Key

/** The row a list shows: the core's line, then whether its children are folded, and its level. */
fun rowText(text: String, expanded: Boolean?, level: String?): String =
    listOfNotNull(text.ifEmpty { null }, expanded?.let(::foldState), level).joinToString(", ")

/**
 * Home, the core's (`ViewModel.home`): the places as the sidebar has them, folded as it is, each
 * with its letter, then what can be done from anywhere. Its row in hand is the sidebar's; Enter
 * opens a place or folds a heading, and the app follows where the core goes ([Ui.follow]).
 */
class HomeScreen : Screen() {
    override val title: String get() = ui.view.home?.title?.ifEmpty { null } ?: appWords.metadata.name

    override val lands: Lands get() = Lands.SIDE

    override val showsScreenActions: Boolean get() = true

    override fun loaded(view: ViewModel): Boolean = view.home != null

    // Coming back from a place: Home again. Already shown, it is left as it is. Each time it is
    // shown it looks back for reminders due, which Android's alarms show but the app's list does
    // not hold until something looks, so Home offers them while any are due.
    override fun resumed() {
        if (ui.view.home == null) ui.send(ShellEvent.Home)
        ui.send(ShellEvent.Due(DueEvent.LookBack))
    }

    /** The sidebar's row in hand, unless the person stands on one of Home's actions. */
    override fun inHand(): String? = if (selectedKey?.startsWith(ACTION) == true) null else ui.view.sidebarHeld.focus

    override fun moved(row: Row) {
        if (row.on != null) ui.send(ShellEvent.Hold(Side.SIDEBAR, row.key))
    }

    override fun rows(): List<Row> {
        val home = ui.view.home ?: return emptyList()
        val shown = home.places.filter { !it.entry.hidden }
        val levels = levelsOf(shown.map { it.entry.depth })
        val places = shown.mapIndexed { index, place ->
            val entry = place.entry
            Row(
                entry.key,
                rowText(ui.withLetter(entry.text, place.letter.ifEmpty { null }), entry.expanded, levels.getOrNull(index)),
                open = {
                    ui.send(ShellEvent.Hold(Side.SIDEBAR, entry.key))
                    ui.send(ShellEvent.Key("open"))
                },
                offered = entry.offered,
                on = ActingOn.Held(Side.SIDEBAR, entry.key),
                expanded = entry.expanded,
            )
        }
        val actions = home.actions.filter { it.enabled }.map { action ->
            Row(ACTION + action.id, ui.withLetter(action.title, action.letter.ifEmpty { null }), { ui.runScreenAction(action) })
        }
        return places + actions
    }

    override fun commands(): List<Command> = ui.screenActionCommands()

    private companion object {
        /** Home's own actions' keys, apart from the sidebar's. */
        const val ACTION = "action:"
    }
}

/** One of the app's places as a list of tasks or block series: Select shows or opens a row. */
class PlaceScreen(override val place: Place, private val name: String) : ListScreen(), ShowsPlace {
    override fun loaded(view: ViewModel): Boolean = view.place == place && view.list != null

    override val title: String get() = heading(ui.listShown, name)

    override val lands: Lands get() = Lands.MAIN

    override fun resumed() {
        if (ui.view.place != place) ui.send(ShellEvent.Open(place))
    }

    override fun inHand(): String? = ui.view.focus

    override val empty: String get() = ui.listShown?.empty?.ifEmpty { null } ?: appWords.nothingHere

    override fun moved(row: Row) = ui.send(ShellEvent.Focus(row.key))

    override fun rows(): List<Row> {
        val view = ui.view
        val list = view.list ?: return emptyList()
        val lines = ui.lines?.list.orEmpty()
        return list.items.mapIndexedNotNull { index, item ->
            if (item.hidden) return@mapIndexedNotNull null
            val line = lines.getOrNull(index)
            val key = item.row.id
            Row(
                key,
                rowText(line?.text ?: item.row.title, item.row.expanded, item.level),
                open = {
                    ui.send(ShellEvent.Focus(key))
                    ui.send(ShellEvent.Key("open"))
                },
                offered = list.offeredOf(item),
                on = ActingOn.Row(key),
                expanded = item.row.expanded,
            )
        }
    }

    override val showsScreenActions: Boolean get() = true

    override fun commands(): List<Command> = ui.screenActionCommands()
}

/** Typing a filter over the list, which the app narrows as typed. */
class FilterScreen(private val place: Place) : EditScreen() {
    private val words get() = appWords.quickAdd
    override val title: String get() = words.filter
    override fun initial(): String = ui.view.query
    override fun completes(): Boolean = true
    override fun syntax(): Syntax = Syntax.FILTER
    override fun confirmed(text: String) {
        ui.send(ShellEvent.Filter(text))
        ui.pop()
    }
    override fun cancelled() = ui.pop()
    override val hint: String get() = words.filterHint
    override val confirmWord: String get() = words.filter
}

/** The day: blocks, sittings under them, free time, now; the day's own actions in its menu. */
class DayScreen : ListScreen(), ShowsPlace {
    override val place: Place get() = Place.Today

    override fun loaded(view: ViewModel): Boolean = view.place is Place.Today && view.day != null

    override val title: String get() = heading(ui.listShown, titleOf("go-today"))

    override val lands: Lands get() = Lands.MAIN

    override fun resumed() {
        if (ui.view.place !is Place.Today) ui.send(ShellEvent.Open(Place.Today))
    }

    override val empty: String get() = appWords.nothingPlanned

    override fun inHand(): String? = ui.view.focus

    override fun moved(row: Row) = ui.send(ShellEvent.Focus(row.key))

    override fun rows(): List<Row> {
        val view = ui.view
        val day = view.day ?: return emptyList()
        val lines = ui.lines?.day.orEmpty()
        return day.items.mapIndexedNotNull { index, item ->
            if (item.hidden) return@mapIndexedNotNull null
            val key = item.key
            Row(
                key,
                rowText(lines.getOrNull(index)?.text.orEmpty(), item.expanded, item.level),
                open = {
                    ui.send(ShellEvent.Focus(key))
                    ui.send(ShellEvent.Key("open"))
                },
                offered = item.offered,
                on = ActingOn.Row(key),
                expanded = item.expanded,
            )
        }
    }

    override val showsScreenActions: Boolean get() = true

    override fun commands(): List<Command> = buildList {
        ui.view.day?.offered?.forEach { action ->
            val letter = actionLetter(action.kind)
            add(Command(ui.withLetter(action.name, letter), letter) { ui.send(ShellEvent.ActOn(ActingOn.Day, action)) })
        }
        addAll(ui.screenActionCommands())
    }
}

/** A screen showing one of the app's places, which the app's own moves between places replace. */
interface ShowsPlace {
    val place: Place
}

/** Undo and Redo on a list beside the place (a task's reminders), by the core's commands. */
fun undoCommands(ui: Ui): List<Command> = listOf("undo", "redo").map { id ->
    val letter = commandLetter(id)
    Command(ui.withLetter(titleOf(id), letter), letter) { ui.send(ShellEvent.Key(id)) }
}

/** The heading of the list the core says is showing: its name, then what it says of itself. */
fun heading(list: ListShown?, name: String): String {
    list ?: return name
    return when {
        list.readback.isEmpty() -> list.name.ifEmpty { name }
        // The day's readback is its heading, the day's name first.
        list.readback.startsWith(list.name) -> list.readback
        else -> "${list.name.ifEmpty { name }}, ${list.readback}"
    }
}

/** A row's actions, then the screen's. */
class ContextMenuScreen(private val name: String, private val commands: List<Command>) : Screen() {
    override val title: String get() = name

    override fun openedAs(): Opened = Opened.ContextMenu

    // Each command's letter runs it here too, as it does from the screen under the menu.
    override fun rows(): List<Row> = commands.mapIndexed { index, command ->
        Row(index.toString(), command.text, { ui.closeMenu(this, command.run) }, letter = command.letter)
    }

    override fun leaving() {
        ui.reader.contextMenuClosed()
    }
}

/** A confirmation, a pick, or a choice of answers: the core's question, its buttons and choices. */
open class QuestionScreen(private val asking: Asking) : DialogScreen(), AskingScreen {
    override val number: UInt get() = asking.number

    /** What the core says is asked, while this question is the one open. */
    private val shown: QuestionShown? get() = ui.questionShown?.takeIf { it.number == number }

    override fun loaded(view: ViewModel): Boolean = shown != null

    override val title: String
        get() = shown?.prompt?.ifEmpty { null } ?: asking.action?.name.orEmpty()

    override fun stands(view: ViewModel): Boolean = view.asking?.number == number

    override fun leaving() {
        if (ui.view.asking?.number == number) ui.send(ShellEvent.Cancel)
    }

    override fun cancelDialog() = ui.send(ShellEvent.Cancel)

    /** The pick the list stands on, by id: picked when the dialog is confirmed. */
    private var picked: String? = null

    private fun pick(id: String) = ui.send(ShellEvent.Answer(Answer.Picked(id, null)))

    override fun controls(): List<Control> {
        val shown = shown ?: return emptyList()
        val current = ui.view.asking ?: asking
        if (current.question is Question.Pick) {
            val choices = current.choices?.choices.orEmpty()
            val options = choices.mapIndexed { index, choice ->
                val text = shown.choices.getOrNull(index)?.text ?: choice.title
                Option(choice.id, listOfNotNull(text, choice.reason?.ifEmpty { null }).joinToString(". "))
            }
            if (!inPlace) return options.map { option -> Control.Button(option.id, option.title) { pick(option.id) } }
            val value = picked?.takeIf { id -> options.any { it.id == id } } ?: options.firstOrNull()?.id.orEmpty()
            return buildList {
                add(Control.Choosing("choices", shown.prompt, options, value, { picked = it }, list = true))
                addAll(answers(shown, confirm = { pick(value) }))
            }
        }
        return answers(shown)
    }

    private fun answers(shown: QuestionShown, confirm: (() -> Unit)? = null): List<Control> = shown.buttons.map { button ->
        Control.Button(button.id, button.title, default = button.id == "yes") {
            when (button.id) {
                "yes" -> if (confirm != null) confirm() else ui.send(ShellEvent.Answer(Answer.Yes))
                "cancel" -> ui.send(ShellEvent.Cancel)
                else -> pick(button.id)
            }
        }
    }
}

interface AskingScreen {
    val number: UInt
}

/** A line typed in answer to the app's question; a refusal is said from the core's changes. */
class TextQuestionScreen(private val asking: Asking) : EditScreen(), AskingScreen {
    override val number: UInt get() = asking.number
    private val shown: QuestionShown? get() = ui.questionShown?.takeIf { it.number == number }

    /** Kept from its first reading: the field's name is how Orbit knows it. */
    private val prompt by lazy { shown?.prompt.orEmpty() }

    override val title: String get() = prompt
    override fun loaded(view: ViewModel): Boolean = shown != null
    override fun initial(): String = shown?.typed.orEmpty()
    override fun completes(): Boolean = asking.syntax != null
    override fun syntax(): Syntax = asking.syntax ?: Syntax.QUICK_ADD
    override val confirmWord: String get() = shown?.buttons?.firstOrNull { it.id == "yes" }?.title ?: appWords.save
    override val cancelWord: String get() = shown?.buttons?.firstOrNull { it.id == "cancel" }?.title ?: appWords.cancel
    override fun confirmed(text: String) {
        ui.send(ShellEvent.Answer(Answer.Text(text)))
    }
    override fun cancelled() {
        ui.send(ShellEvent.Cancel)
    }
    override fun stands(view: ViewModel): Boolean = view.asking?.number == number
    override fun leaving() {
        super.leaving()
        if (ui.view.asking?.number == number) ui.send(ShellEvent.Cancel)
    }
}

/**
 * What a line being typed has that a list does not: its completions, and on the Speak the core's
 * Confirm and Cancel beside it, reached by Tab.
 */
abstract class EditScreen : DialogScreen() {
    protected abstract fun initial(): String
    protected abstract fun confirmed(text: String)
    protected abstract fun cancelled()
    protected open val hint: String get() = ""
    protected open fun completes(): Boolean = false
    protected open fun syntax(): Syntax = Syntax.QUICK_ADD
    protected open fun changed(text: String, caret: Int) {}

    /** What the Confirm button is called, on the Speak; null for a line with only its Select. */
    protected abstract val confirmWord: String?

    protected open val cancelWord: String get() = appWords.cancel

    override val isEdit: Boolean get() = true

    /** What it starts from, read once it has arrived: quick add's line, a question's prompt. */
    private var seed: String? = null

    private fun line(): Control.Line = Control.Line(
        "line", title, seed ?: initial().also { seed = it }, hint,
        changed = { text, caret ->
            current = text
            cursor = caret
            changed(text, caret)
        },
        completing = { completes() && candidates().isNotEmpty() },
        complete = { openCompletions() },
    )

    override fun controls(): List<Control> = buildList {
        add(line())
        if (inPlace) {
            confirmWord?.let { word -> add(Control.Button("confirm", word) { editorFor(line()).finish() }) }
            add(Control.Button("cancel", cancelWord) { cancelled() })
        }
    }

    override fun rows(): List<Row> = if (inPlace) super.rows() else emptyList()

    override fun cancelDialog() = cancelled()

    override fun lineFinished(control: Control.Line, text: String) = confirmed(text)

    protected var current = ""
    protected var cursor = 0

    private val editor: EditBox
        get() {
            val editor = editorFor(line())
            if (current.isEmpty()) {
                current = editor.display()
                cursor = current.length
            }
            return editor
        }

    fun display(): String = editor.display()

    /** On the Player, where the line is the whole screen. */
    fun describe() {
        ui.braille.follow()
        editor.focus()
        // After the field's name: what Orbit is told goes through its event queue, speech does not.
        if (hint.isNotEmpty()) ui.later(HINT_AFTER_MS) { if (ui.top === this) ui.reader.speakQueued(hint) }
    }

    override fun key(key: Key): Boolean {
        if (ui.top === this && !loaded(ui.view)) return true
        if (inPlace) return super.key(key)
        // The Player's Menu is the line's menu (its Confirm and Cancel); every other key is the line's.
        if (key == Key.Menu) return false
        editor.key(key)
        return true
    }

    /** On the Player, where Tab does not reach them, the line's Confirm and Cancel are its menu. */
    override fun commands(): List<Command> = if (inPlace) emptyList() else buildList {
        confirmWord?.let { word -> add(Command(word) { editorFor(line()).finish() }) }
        add(Command(cancelWord) { cancelled() })
    }

    /** What could go at the caret in the field's own syntax. */
    protected open fun candidates(): List<Candidate> {
        if (!completes()) return emptyList()
        return runCatching { ui.app.complete(current, TextOffsets.bytes(cursor, current), syntax()).candidates }.getOrDefault(emptyList())
    }

    private fun openCompletions() {
        val completions = runCatching { ui.app.complete(current, TextOffsets.bytes(cursor, current), syntax()) }.getOrNull() ?: return
        val start = TextOffsets.utf16(completions.start, current)
        val end = TextOffsets.utf16(completions.end, current)
        ui.push(CompletionScreen(completions.candidates.map { candidate ->
            dev.emassey0135.lumenna.app.candidateLine(candidate) to {
                val text = current.substring(0, start) + candidate.text + current.substring(end)
                current = text
                cursor = start + candidate.text.length
                editor.replace(text, cursor)
                ui.pop()
            }
        }))
    }
}

private const val HINT_AFTER_MS = 400L

/** The candidates of a completion. */
class CompletionScreen(private val options: List<Pair<String, () -> Unit>>) : Screen() {
    override val title: String get() = appWords.completions
    override fun rows(): List<Row> = options.mapIndexed { index, (text, run) -> Row(index.toString(), text, { run() }) }
}

/** Quick add: the app's line, with completion and its refusals. */
class QuickAddScreen : EditScreen() {
    private var seen = false
    override val title: String get() = appWords.quickAdd.label
    override val hint: String get() = appWords.quickAdd.hint
    override val confirmWord: String get() = appWords.quickAdd.add
    override val cancelWord: String get() = appWords.quickAdd.cancel
    override fun initial(): String = ui.view.line?.text.orEmpty()
    override fun completes(): Boolean = true

    // Starts as the place shown starts it: known only once the app has opened its line.
    override fun loaded(view: ViewModel): Boolean = view.line != null

    override fun resumed() {
        if (ui.view.line == null) ui.send(ShellEvent.Typing(Typing.Open))
    }

    override fun stands(view: ViewModel): Boolean {
        if (view.line != null) seen = true
        return !seen || view.line != null
    }

    override fun changed(text: String, caret: Int) {
        ui.send(ShellEvent.Typing(Typing.Text(text, TextOffsets.bytes(caret, text))))
    }

    override fun confirmed(text: String) {
        ui.send(ShellEvent.QuickAdd(text))
    }

    override fun cancelled() {
        ui.pop()
    }

    override fun leaving() {
        super.leaving()
        ui.lineClosed = true
        ui.send(ShellEvent.Typing(Typing.Close))
    }
}

/** A task's or block's reminders. */
class RemindersScreen : ListScreen() {
    override val lands: Lands get() = Lands.SIDE

    override val title: String
        get() = listOf(ui.view.remindersTitle.orEmpty(), ui.view.reminders?.announcement.orEmpty()).filter { it.isNotEmpty() }.joinToString(". ")

    override fun stands(view: ViewModel): Boolean = view.reminders != null

    override fun leaving() {
        if (ui.view.reminders != null) ui.send(ShellEvent.CloseReminders)
    }

    override val empty: String get() = appWords.nothingHere

    override fun inHand(): String? = ui.view.remindersHeld.focus

    override fun moved(row: Row) = ui.send(ShellEvent.Hold(Side.REMINDERS, row.key))

    override fun rows(): List<Row> {
        val list = ui.view.reminders ?: return emptyList()
        val lines = ui.lines?.reminders.orEmpty()
        return list.reminders.mapIndexed { index, reminder ->
            Row(reminder.id, lines.getOrNull(index)?.text ?: reminder.words, null, reminder.offered, ActingOn.Held(Side.REMINDERS, reminder.id))
        }
    }

    override fun commands(): List<Command> = buildList {
        ui.view.reminders?.offered?.firstOrNull { it.kind == ActionKind.NEW }?.let { action ->
            val letter = actionLetter(action.kind)
            add(Command(ui.withLetter(action.name, letter), letter) { ui.send(ShellEvent.ActOn(ActingOn.Reminders, action)) })
        }
        addAll(undoCommands(ui))
    }
}

/** What came due over the last day and was not dealt with, each with its buttons. */
class DueScreen : ListScreen() {
    override val lands: Lands get() = Lands.SIDE

    override val title: String get() = appWords.remindersDue

    override fun resumed() = ui.send(ShellEvent.Due(DueEvent.LookBack))

    override val empty: String get() = appWords.nothingDue

    override fun inHand(): String? = ui.view.dueHeld.focus

    override fun moved(row: Row) = ui.send(ShellEvent.Hold(Side.DUE, row.key))

    override fun rows(): List<Row> = ui.view.due.map { due ->
        Row(due.id, due.text, open = {
            ui.push(ContextMenuScreen(due.text, buttonCommands(due.id)))
        })
    }

    /** A button's letter presses it on the reminder in hand, as in its menu. */
    override fun letter(letter: String): Boolean {
        val row = rows().getOrNull(index) ?: return false
        val command = buttonCommands(row.key).firstOrNull { it.letter == letter } ?: return false
        command.run()
        return true
    }

    /** Each button handed back by its id: the core says what it does. */
    private fun buttonCommands(id: String): List<Command> {
        val due = ui.view.due.firstOrNull { it.id == id } ?: return emptyList()
        return due.buttons.map { button ->
            val letter = dueLetter(button.id)
            Command(ui.withLetter(button.title, letter), letter) { ui.send(ShellEvent.Due(DueEvent.Press(id, button.id))) }
        }
    }

    override fun commands(): List<Command> {
        val rows = rows()
        val row = rows.getOrNull(index) ?: return emptyList()
        return buttonCommands(row.key)
    }
}

/** The keys this device has and what they do: the core's sentences, and on the Speak its letters. */
class KeyHelpScreen : Screen() {
    override val title: String get() = appWords.braille.keyHelp

    override fun rows(): List<Row> = lines().mapIndexed { index, text -> Row(index.toString(), text) }

    private fun lines(): List<String> = if (ui.phoneKeypad) appWords.orbit.playerKeys else buildList {
        val words = appWords.braille
        add(words.moving)
        addAll(appWords.orbit.speakKeys)
        // A letter's actions, under the key that presses it.
        val actions = appWords.actionTitles
            .mapNotNull { title -> actionLetter(title.kind)?.let { it to title.title } }
            .groupBy({ it.first }, { it.second })
            .toSortedMap()
        fun letters(key: SpeakLetter) = actions.filterKeys { speakLetter(it) == key }.map { (letter, titles) ->
            "$letter, ${titles.distinct().joinToString(", ")}"
        }
        add(words.withSpace)
        addAll(letters(SpeakLetter.SPACE))
        add(appWords.orbit.withDot8)
        addAll(letters(SpeakLetter.ENTER))
        // Home's places and actions, under its name.
        ui.view.home?.let { home ->
            add(home.title)
            home.places.filter { it.letter.isNotEmpty() }.forEach { add("${speakPress(it.letter) ?: it.letter}, ${it.entry.title}") }
            home.actions.filter { it.letter.isNotEmpty() }.forEach { add("${speakPress(it.letter) ?: it.letter}, ${it.title}") }
        }
        add(words.letterAlone)
        add(words.findsARow)
    }
}

/** Replan from Now and Plan the Day: the app proposes, and says so; the person checks what to take. */
class ProposalScreen : ListScreen() {
    override val title: String get() = ui.view.proposal?.words?.title.orEmpty()

    override fun openedAs(): Opened = Opened.Dialog

    override fun stands(view: ViewModel): Boolean = view.proposal != null

    override fun leaving() {
        if (ui.view.proposal != null) ui.send(ShellEvent.Proposal(ProposalEvent.Close))
    }

    override fun rows(): List<Row> {
        val proposal = ui.view.proposal ?: return emptyList()
        val words = proposal.words
        return buildList {
            proposal.refusal?.let { add(Row("refusal", it)) }
            proposal.changes.forEachIndexed { index, change ->
                add(Row("change-$index", listOfNotNull(change.sentence, change.state, change.reason).joinToString(", "), {
                    ui.send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked)))
                }))
            }
            if (proposal.changes.isEmpty()) add(Row("nothing", words.nothing))
            if (proposal.unfit.isNotEmpty()) {
                add(Row("unfit", words.unfit))
                proposal.unfit.forEachIndexed { index, line -> add(Row("unfit-$index", line)) }
            }
            if (proposal.changes.isNotEmpty()) add(Row("apply", listOf(words.apply, words.hint).joinToString(". "), { ui.send(ShellEvent.Proposal(ProposalEvent.Apply)) }))
            if (proposal.others.isNotEmpty()) {
                add(Row("others", words.others))
                proposal.others.forEachIndexed { index, other ->
                    add(Row("other-$index", other, { ui.send(ShellEvent.Proposal(ProposalEvent.Take(index.toUInt()))) }))
                }
            }
            add(Row("cancel", words.cancel, { ui.send(ShellEvent.Proposal(ProposalEvent.Close)) }))
        }
    }
}

