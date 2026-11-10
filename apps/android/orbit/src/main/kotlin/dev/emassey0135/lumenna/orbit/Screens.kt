package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.brailleMainMenu
import dev.emassey0135.lumenna.app.commandLetter
import dev.emassey0135.lumenna.app.dueLetter
import dev.emassey0135.lumenna.app.placeCommands
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
import dev.emassey0135.lumenna.app.SidebarEntry
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.Syntax
import dev.emassey0135.lumenna.app.Typing
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.app.foldState
import dev.emassey0135.lumenna.levelsOf
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.offeredOf

/** The row a list shows: the core's line, then whether its children are folded, and its level. */
fun rowText(text: String, expanded: Boolean?, level: String?): String =
    listOfNotNull(text.ifEmpty { null }, expanded?.let(::foldState), level).joinToString(", ")

/** The root: the core's main menu (the places, adding, undo, reminders, syncing, settings), then what to work on and the keys. */
class HomeScreen : Screen() {
    override val title: String get() = appWords.metadata.name

    override fun rows(): List<Row> {
        val sidebar = ui.view.sidebar
        fun place(wanted: Place) = sidebar.firstOrNull { (it.kind as? SidebarKind.Place)?.field0 == wanted }
        fun group(wanted: SidebarGroup) = sidebar.firstOrNull { (it.kind as? SidebarKind.Group)?.field0 == wanted }
        val rows = brailleMainMenu(true).mapNotNull { command ->
            val entry = when (command.id) {
                "go-today" -> place(Place.Today)
                "go-tasks" -> place(Place.Tasks)
                "go-blocks" -> place(Place.Blocks)
                "go-history" -> place(Place.History)
                "go-trash" -> place(Place.Trash)
                "projects" -> group(SidebarGroup.PROJECTS)
                "labels" -> group(SidebarGroup.LABELS)
                "filters" -> group(SidebarGroup.FILTERS)
                else -> null
            }
            val open: (() -> Unit)? = when (command.id) {
                "go-today" -> ({ ui.push(DayScreen()) })
                "go-tasks", "go-blocks", "go-history", "go-trash" -> entry?.let { found ->
                    { ui.push(PlaceScreen((found.kind as SidebarKind.Place).field0, found.title)) }
                }
                "projects" -> ({ ui.push(GroupScreen(SidebarGroup.PROJECTS)) })
                "labels" -> ({ ui.push(GroupScreen(SidebarGroup.LABELS)) })
                "filters" -> ({ ui.push(GroupScreen(SidebarGroup.FILTERS)) })
                "new-task" -> ({ ui.push(QuickAddScreen()) })
                "undo" -> ({ ui.send(ShellEvent.Undo) })
                "redo" -> ({ ui.send(ShellEvent.Redo) })
                "reminders-due" -> ({ ui.push(DueScreen()) })
                "sync-now" -> ({ ui.send(ShellEvent.Key("sync-now")) })
                "settings" -> ({ ui.push(SettingsScreen()) })
                else -> null
            }
            open?.let { Row(command.id, entry?.text ?: command.title, it, letter = command.letter) }
        }
        return rows + listOf(
            Row("what-now", titleOf("what-now"), { ui.send(ShellEvent.Key("what-now")) }),
            Row("keys", appWords.braille.keyHelp, { ui.push(KeyHelpScreen()) }),
        )
    }
}

/** One of the app's places as a list of tasks or block series: Select shows or opens a row. */
class PlaceScreen(private val place: Place, private val name: String) : ListScreen() {
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
                    when (place) {
                        Place.Trash -> ui.send(ShellEvent.Key("restore-task"))
                        Place.Blocks -> ui.send(ShellEvent.Key("open"))
                        else -> ui.push(DetailsScreen(key))
                    }
                },
                offered = list.offeredOf(item),
                on = ActingOn.Row(key),
                expanded = item.row.expanded,
            )
        }
    }

    override fun commands(): List<Command> = placeCommands(place, true).map { command ->
        Command(ui.withLetter(command.title, command.letter)) {
            // The filter is this app's field: the core's letter has no dots here.
            if (command.id == "filter") ui.push(FilterScreen(place)) else ui.send(ShellEvent.Letter(command.letter))
        }
    }
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
class DayScreen : ListScreen() {
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

    override fun commands(): List<Command> = buildList {
        ui.view.day?.offered?.forEach { action ->
            add(Command(ui.withLetter(action.name, actionLetter(action.kind))) {
                ui.send(ShellEvent.ActOn(ActingOn.Day, action))
            })
        }
        placeCommands(Place.Today, true).forEach { command ->
            add(Command(ui.withLetter(command.title, command.letter)) { ui.send(ShellEvent.Letter(command.letter)) })
        }
    }
}

fun undoCommands(ui: Ui): List<Command> = listOf(
    Command(ui.withLetter(titleOf("undo"), commandLetter("undo"))) { ui.send(ShellEvent.Undo) },
    Command(ui.withLetter(titleOf("redo"), commandLetter("redo"))) { ui.send(ShellEvent.Redo) },
)

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

/** A group's places (Projects, Labels, Filters) from the sidebar, held in its hand. */
class GroupScreen(private val group: SidebarGroup) : ListScreen() {
    override val lands: Lands get() = Lands.SIDE

    private val entries: List<SidebarEntry>
        get() {
            val under = mutableListOf<SidebarEntry>()
            var inside = false
            for (entry in ui.view.sidebar) {
                if (entry.depth == 0u) {
                    inside = (entry.kind as? SidebarKind.Group)?.field0 == group
                    continue
                }
                if (inside && !entry.hidden) under += entry
            }
            return under
        }

    override val title: String
        get() = ui.view.sidebar.firstOrNull { (it.kind as? SidebarKind.Group)?.field0 == group }?.title.orEmpty()

    override fun inHand(): String? = ui.view.sidebarHeld.focus

    override fun moved(row: Row) = ui.send(ShellEvent.Hold(Side.SIDEBAR, row.key))

    override fun rows(): List<Row> {
        val shown = entries
        val levels = levelsOf(shown.map { it.depth - 1u })
        return shown.mapIndexed { index, entry ->
            val place = (entry.kind as? SidebarKind.Place)?.field0
            Row(
                entry.key,
                rowText(entry.text, entry.expanded, levels.getOrNull(index)),
                open = { place?.let { ui.push(PlaceScreen(it, entry.title)) } },
                offered = entry.offered,
                on = ActingOn.Held(Side.SIDEBAR, entry.key),
                expanded = entry.expanded,
            )
        }
    }

    private fun headingEntry() = ui.view.sidebar.firstOrNull { (it.kind as? SidebarKind.Group)?.field0 == group }

    private fun addNew() {
        val heading = headingEntry() ?: return
        val action = heading.offered.firstOrNull { it.kind == ActionKind.NEW } ?: return
        ui.send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, heading.key), action))
    }

    /** A task in the row's own place: its tasks opened, then quick add starting as they do. */
    private fun addTaskIn() {
        val entry = entries.getOrNull(index) ?: return
        val place = (entry.kind as? SidebarKind.Place)?.field0 ?: return
        ui.push(PlaceScreen(place, entry.title))
        ui.push(QuickAddScreen())
    }

    private val addTaskWords: String
        get() = if (group == SidebarGroup.LABELS) appWords.addTaskWearingIt else appWords.addTaskToIt

    override fun letter(letter: String): Boolean = when (letter) {
        actionLetter(ActionKind.NEW) -> {
            addNew()
            true
        }
        commandLetter("add-task-in") -> {
            addTaskIn()
            true
        }
        else -> false
    }

    override fun commands(): List<Command> = buildList {
        add(Command(ui.withLetter(addTaskWords, commandLetter("add-task-in"))) { addTaskIn() })
        headingEntry()?.offered?.firstOrNull { it.kind == ActionKind.NEW }?.let { action ->
            add(Command(ui.withLetter(action.name, actionLetter(action.kind))) { addNew() })
        }
        addAll(undoCommands(ui))
    }
}

/** A row's actions, then the screen's. */
class ContextMenuScreen(private val name: String, private val commands: List<Command>) : Screen() {
    override val title: String get() = name

    override fun openedAs(): Opened = Opened.ContextMenu

    override fun rows(): List<Row> = commands.mapIndexed { index, command ->
        Row(index.toString(), command.text, {
            ui.pop()
            command.run()
        })
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

    override fun resumed() {
        // Asked once, answered at once: read in `rows`, it would answer on every redraw.
        if (asking.question is Question.Immediate) ui.send(ShellEvent.Answer(Answer.Yes))
    }

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

    private val editor: LineEditor
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
        ui.liblouis.follow()
        editor.focus()
        // After the field's name: what Orbit is told goes through its event queue, speech does not.
        if (hint.isNotEmpty()) ui.later(HINT_AFTER_MS) { ui.reader.speakQueued(hint) }
    }

    override fun key(key: Key): Boolean {
        if (ui.top === this && !loaded(ui.view)) return true
        if (inPlace) return super.key(key)
        editor.key(key)
        return true
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
            add(Command(ui.withLetter(action.name, actionLetter(action.kind))) {
                ui.send(ShellEvent.ActOn(ActingOn.Reminders, action))
            })
        }
        addAll(undoCommands(ui))
    }
}

/** What came due over the last day and was not dealt with, each with its buttons. */
class DueScreen : ListScreen() {
    override val lands: Lands get() = Lands.SIDE

    override val title: String get() = appWords.remindersDue

    override fun resumed() {
        val now = java.time.Instant.now()
        val from = now.minus(java.time.Duration.ofHours(24))
        ui.send(ShellEvent.Due(DueEvent.Look(from.toString(), now.toString())))
    }

    override val empty: String get() = appWords.nothingDue

    override fun inHand(): String? = ui.view.dueHeld.focus

    override fun moved(row: Row) = ui.send(ShellEvent.Hold(Side.DUE, row.key))

    override fun rows(): List<Row> = ui.view.due.map { due ->
        Row(due.id, due.text, open = {
            ui.push(ContextMenuScreen(due.text, buttonCommands(due.id)))
        })
    }

    /** D dismisses: the delete keys', as on BTSpeak. */
    override fun letter(letter: String): Boolean {
        if (letter != "d") return false
        val row = rows().getOrNull(index) ?: return false
        ui.send(ShellEvent.Due(DueEvent.Dismiss(row.key)))
        return true
    }

    private fun buttonCommands(id: String): List<Command> {
        val due = ui.view.due.firstOrNull { it.id == id } ?: return emptyList()
        return due.buttons.map { button ->
            Command(ui.withLetter(button.title, dueLetter(button.id))) {
                when (button.id) {
                    "carry" -> ui.send(ShellEvent.Due(DueEvent.Carry(id)))
                    "replan" -> ui.send(ShellEvent.Due(DueEvent.Replan(id)))
                    "dismiss" -> ui.send(ShellEvent.Due(DueEvent.Dismiss(id)))
                    "snooze" -> ui.send(ShellEvent.Due(DueEvent.Snooze(id)))
                }
            }
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
        add(words.mainMenu)
        brailleMainMenu(true).forEach { add("${speakPress(it.letter) ?: it.letter}, ${it.title}") }
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

