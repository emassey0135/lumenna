package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Answer
import dev.emassey0135.lumenna.app.Asking
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.Candidate
import dev.emassey0135.lumenna.app.DayKind
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

/** The root: every place, then quick add, reminders, what to work on, undo, sync, settings, keys. */
class HomeScreen : Screen() {
    override val title: String get() = appWords.metadata.name

    override fun rows(): List<Row> {
        val rows = mutableListOf<Row>()
        for (entry in ui.view.sidebar) {
            if (entry.depth != 0u) continue
            when (val kind = entry.kind) {
                is SidebarKind.Place -> rows += placeRow(entry, kind.field0)
                is SidebarKind.Group -> rows += Row(entry.key, entry.text, { ui.push(GroupScreen(kind.field0)) })
            }
        }
        rows += Row("new-task", titleOf("new-task"), { ui.push(QuickAddScreen()) })
        rows += Row("reminders-due", appWords.remindersDue, { ui.push(DueScreen()) })
        rows += Row("what-now", titleOf("what-now"), { ui.send(ShellEvent.Key("what-now")) })
        rows += Row("undo", titleOf("undo"), { ui.send(ShellEvent.Undo) })
        rows += Row("redo", titleOf("redo"), { ui.send(ShellEvent.Redo) })
        rows += Row("sync-now", titleOf("sync-now"), { ui.send(ShellEvent.Key("sync-now")) })
        rows += Row("settings", titleOf("settings"), { ui.push(SettingsScreen()) })
        rows += Row("keys", titleOf("keyboard-help"), { ui.push(KeyHelpScreen()) })
        return rows
    }

    private fun placeRow(entry: SidebarEntry, place: Place): Row = Row(entry.key, entry.text, {
        ui.push(if (place is Place.Today) DayScreen() else PlaceScreen(place, entry.title))
    })
}

/** One of the app's places as a list of tasks or block series: Select shows or opens a row. */
class PlaceScreen(private val place: Place, private val name: String) : ListScreen() {
    override fun loaded(view: ViewModel): Boolean = view.place == place && view.list != null

    override val title: String get() = listOf(name, ui.lines?.readback.orEmpty()).filter { it.isNotEmpty() }.joinToString(", ")

    override fun resumed() {
        if (ui.view.place != place) ui.send(ShellEvent.Open(place))
    }

    override fun inHand(): String? = ui.view.focus

    override val empty: String get() = ui.view.list?.empty?.ifEmpty { null } ?: appWords.nothingHere

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

    override fun commands(): List<Command> = buildList {
        add(Command(titleOf(if (place is Place.Blocks) "new-block" else "new-task")) {
            ui.pop()
            if (place is Place.Blocks) ui.send(ShellEvent.Block(BlockEvent.New(null, null))) else ui.push(QuickAddScreen())
        })
        if (place !is Place.Blocks && place !is Place.Trash && place !is Place.History) {
            add(Command(titleOf("filter")) {
                ui.pop()
                ui.push(FilterScreen(place))
            })
        }
        addAll(undoCommands(ui))
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
}

/** The day: blocks, sittings under them, free time, now; the day's own actions in its menu. */
class DayScreen : ListScreen() {
    override fun loaded(view: ViewModel): Boolean = view.place is Place.Today && view.day != null

    override val title: String
        get() {
            val view = ui.view
            val day = view.day ?: return titleOf("go-today")
            val lines = ui.lines?.day.orEmpty()
            val summary = day.items.indexOfFirst { it.kind == DayKind.SUMMARY }
            return lines.getOrNull(summary)?.text ?: ui.lines?.dayTitle.orEmpty()
        }

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
            if (item.hidden || item.kind == DayKind.SUMMARY) return@mapIndexedNotNull null
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
            add(Command(action.name) {
                ui.pop()
                ui.send(ShellEvent.ActOn(ActingOn.Day, action))
            })
        }
        fun move(id: String, step: DayMove) = add(Command(titleOf(id)) {
            ui.pop()
            ui.send(ShellEvent.Day(step))
        })
        add(Command(titleOf("new-block")) {
            ui.pop()
            ui.send(ShellEvent.Block(BlockEvent.New(null, null)))
        })
        move("previous-day", DayMove.PREVIOUS)
        move("next-day", DayMove.NEXT)
        move("go-to-day", DayMove.ASK)
        move("go-to-now", DayMove.TODAY)
        addAll(undoCommands(ui))
    }
}

fun undoCommands(ui: Ui): List<Command> = listOf(
    Command(titleOf("undo")) {
        ui.pop()
        ui.send(ShellEvent.Undo)
    },
    Command(titleOf("redo")) {
        ui.pop()
        ui.send(ShellEvent.Redo)
    },
)

/** A group's places (Projects, Labels, Filters) from the sidebar, held in its hand. */
class GroupScreen(private val group: SidebarGroup) : ListScreen() {
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

    override fun commands(): List<Command> = buildList {
        val heading = ui.view.sidebar.firstOrNull { (it.kind as? SidebarKind.Group)?.field0 == group }
        heading?.offered?.firstOrNull { it.kind == ActionKind.NEW }?.let { action ->
            add(Command(action.name) {
                ui.pop()
                ui.send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, heading.key), action))
            })
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

/** A confirmation, a pick, or a choice of answers: the app's question, as a list of answers. */
open class QuestionScreen(private val asking: Asking) : Screen(), AskingScreen, DialogMessage {
    override val number: UInt get() = asking.number

    override val title: String
        get() = when (val question = asking.question) {
            is Question.Confirm -> question.title
            is Question.Pick -> question.title.ifEmpty { asking.action?.name.orEmpty() }
            is Question.Choose -> question.title
            else -> ""
        }

    override val message: String
        get() = when (val question = asking.question) {
            is Question.Confirm -> question.message
            is Question.Choose -> question.message
            else -> ""
        }

    override fun openedAs(): Opened = Opened.Dialog

    override fun stands(view: ViewModel): Boolean = view.asking?.number == number

    override fun leaving() {
        if (ui.view.asking?.number == number) ui.send(ShellEvent.Cancel)
    }

    override fun rows(): List<Row> {
        val current = ui.view.asking ?: asking
        return when (val question = current.question) {
            is Question.Confirm -> listOf(
                Row("yes", question.yes, { ui.send(ShellEvent.Answer(Answer.Yes)) }),
                Row("cancel", appWords.cancel, { ui.send(ShellEvent.Cancel) }),
            )
            is Question.Pick -> {
                val choices = current.choices?.choices.orEmpty()
                val lines = ui.lines?.choices.orEmpty()
                choices.mapIndexed { index, choice ->
                    val text = lines.getOrNull(index)?.text ?: choice.title
                    Row(choice.id, listOfNotNull(text, choice.reason?.ifEmpty { null }).joinToString(". "), {
                        ui.send(ShellEvent.Answer(Answer.Picked(choice.id, null)))
                    })
                }
            }
            is Question.Choose -> question.answers.map { choice ->
                Row(choice.id, choice.title, { ui.send(ShellEvent.Answer(Answer.Picked(choice.id, null))) })
            }
            is Question.Immediate -> {
                ui.send(ShellEvent.Answer(Answer.Yes))
                emptyList()
            }
            else -> emptyList()
        }
    }
}

interface AskingScreen {
    val number: UInt
}

/** A line typed in answer to the app's question. */
class TextQuestionScreen(private val asking: Asking) : EditScreen(), AskingScreen {
    override val number: UInt get() = asking.number
    private val question: Question.Text get() = asking.question as Question.Text
    override val title: String
        get() = listOf(asking.refusal.orEmpty(), question.title, question.label).filter { it.isNotEmpty() }.joinToString(". ")
    override val hint: String get() = question.hint
    override fun initial(): String = asking.typed?.takeIf { asking.refusal != null } ?: question.initial
    override fun completes(): Boolean = asking.syntax != null
    override fun syntax(): Syntax = asking.syntax ?: Syntax.QUICK_ADD
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

    override fun refused() {
        val current = ui.view.asking ?: return
        if (current.number != number) return
        val why = current.refusal ?: return
        ui.refuse()
        ui.say(why)
    }
}

/** What a line being typed has that a list does not: the editor, and its completions. */
abstract class EditScreen : Screen() {
    protected lateinit var editor: LineEditor
    protected abstract fun initial(): String
    protected abstract fun confirmed(text: String)
    protected abstract fun cancelled()
    protected open val hint: String get() = ""
    protected open fun completes(): Boolean = false
    protected open fun syntax(): Syntax = Syntax.QUICK_ADD
    protected open fun changed(text: String, caret: Int) {}

    /** The app refused what was typed: the field stays, its text kept. */
    open fun refused() {}

    override val isEdit: Boolean get() = true
    override fun rows(): List<Row> = emptyList()

    override fun resumed() {
        if (!::editor.isInitialized) {
            editor = ui.editor(
                title, initial(),
                changed = { text, caret ->
                    current = text
                    cursor = caret
                    changed(text, caret)
                },
                confirmed = ::confirmed,
                cancelled = ::cancelled,
                completing = { completes() && candidates().isNotEmpty() },
                complete = { openCompletions() },
            )
            current = editor.display()
        }
    }

    protected var current = ""
    protected var cursor = 0

    fun display(): String = editor.display()

    fun describe() {
        editor.focus()
        // After the field's name: what Orbit is told goes through its event queue, speech does not.
        if (hint.isNotEmpty()) ui.later(HINT_AFTER_MS) { ui.reader.speakQueued(hint) }
    }

    override fun key(key: Key): Boolean {
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
    override fun initial(): String = ui.view.quickAdd
    override fun completes(): Boolean = true

    override fun resumed() {
        ui.send(ShellEvent.Typing(Typing.Open))
        super.resumed()
        ui.send(ShellEvent.Typing(Typing.Text(current, TextOffsets.bytes(current.length, current))))
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
            add(Command(action.name) {
                ui.pop()
                ui.send(ShellEvent.ActOn(ActingOn.Reminders, action))
            })
        }
        addAll(undoCommands(ui))
    }
}

/** What came due over the last day and was not dealt with, each with its buttons. */
class DueScreen : ListScreen() {
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

    private fun buttonCommands(id: String): List<Command> {
        val due = ui.view.due.firstOrNull { it.id == id } ?: return emptyList()
        return due.buttons.map { button ->
            Command(button.title) {
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

/** The keys this device has and what they do, each from the core's table. */
class KeyHelpScreen : Screen() {
    override val title: String get() = titleOf("keyboard-help")

    override fun rows(): List<Row> = keyHelpLines(ui.reader.isPhoneKeypad).mapIndexed { index, text -> Row(index.toString(), text) }
}

/** Replan from Now and Plan the Day: the app proposes, the person checks what to take. */
class ProposalScreen : ListScreen(), DialogMessage {
    override val title: String get() = ui.view.proposal?.words?.title.orEmpty()
    override val message: String get() = ui.view.proposal?.announcement.orEmpty()

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

