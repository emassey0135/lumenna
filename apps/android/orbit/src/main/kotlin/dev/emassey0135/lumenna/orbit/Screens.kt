package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.actionLetter
import dev.emassey0135.lumenna.app.commandLetter
import dev.emassey0135.lumenna.app.dueLetter
import dev.emassey0135.lumenna.app.ListShown
import dev.emassey0135.lumenna.app.Asking
import dev.emassey0135.lumenna.app.DueEvent
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.app.KeyDevice
import dev.emassey0135.lumenna.app.Line
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ProposalEvent
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.Syntax
import dev.emassey0135.lumenna.app.Typing
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.app.lineWithState
import dev.emassey0135.lumenna.levelsOf
import dev.emassey0135.lumenna.offeredOf
import dev.emassey0135.orbit.ui.MenuItem

/**
 * Home, the core's (`ViewModel.home`): the places as the sidebar has them, folded as it is, each
 * with its letter, then what can be done from anywhere. Its row in hand is the sidebar's; Enter
 * opens a place or folds a heading, and the app follows where the core goes ([Ui.turned]).
 */
class HomeScreen : AppList() {
    override val title: String get() = host.view.home?.title?.ifEmpty { null } ?: appWords.metadata.name

    override val lands: Lands get() = Lands.SIDE

    override val showsScreenActions: Boolean get() = true

    override fun loaded(view: ViewModel): Boolean = view.home != null

    // Coming back from a place: Home again. Already shown, it is left as it is. As it first opens
    // it looks back for reminders due, as the app does each time it comes to the front
    // ([Ui.lookBack]): Android's alarms show them, but the app's list holds none until something
    // looks, and Home offers them only while any are due.
    override fun resumed() {
        if (host.view.home == null) host.send(ShellEvent.Home)
        if (!looked) {
            looked = true
            host.lookBack()
        }
    }

    private var looked = false

    /** The sidebar's row in hand, unless the person stands on one of Home's actions. */
    override fun inHand(): String? = if (current?.key?.startsWith(ACTION) == true) null else host.view.sidebarHeld.focus

    override fun moved(row: Row) {
        if (row.on != null) host.send(ShellEvent.Hold(Side.SIDEBAR, row.key))
    }

    override fun rows(): List<Row> {
        val home = host.view.home ?: return emptyList()
        val shown = home.places.filter { !it.entry.hidden }
        val levels = levelsOf(shown.map { it.entry.depth })
        val places = shown.mapIndexed { index, place ->
            val entry = place.entry
            // The entry's line with its press said after it; its folding and level joined by the core.
            val line = Line(
                title = entry.title,
                detail = entry.detail,
                text = host.withLetter(entry.text, place.letter.ifEmpty { null }),
                braille = entry.text,
                level = levels.getOrNull(index),
            )
            Row(
                entry.key,
                lineWithState(line, entry.expanded),
                open = {
                    host.send(ShellEvent.Hold(Side.SIDEBAR, entry.key))
                    host.send(ShellEvent.Key("open"))
                },
                offered = entry.offered,
                on = ActingOn.Held(Side.SIDEBAR, entry.key),
                expanded = entry.expanded,
                title = entry.title,
            )
        }
        val actions = home.actions.filter { it.enabled }.map { action ->
            Row(ACTION + action.id, host.withLetter(action.title, action.letter.ifEmpty { null }), { host.runScreenAction(action) })
        }
        return places + actions
    }

    private companion object {
        /** Home's own actions' keys, apart from the sidebar's. */
        const val ACTION = "action:"
    }
}

/** One of the app's places as a list of tasks or block series: Select shows or opens a row. */
class PlaceScreen(override val place: Place, private val name: String) : AppList(), ShowsPlace {
    override fun loaded(view: ViewModel): Boolean = view.place == place && view.list != null

    override val title: String get() = heading(host.listShown, name)

    override val lands: Lands get() = Lands.MAIN

    override fun resumed() {
        if (host.view.place != place) host.send(ShellEvent.Open(place))
    }

    override fun inHand(): String? = host.view.focus

    override val empty: String get() = host.listShown?.empty?.ifEmpty { null } ?: appWords.nothingHere

    override fun moved(row: Row) = host.send(ShellEvent.Focus(row.key))

    override fun rows(): List<Row> {
        val view = host.view
        val list = view.list ?: return emptyList()
        val lines = host.lines?.list.orEmpty()
        return list.items.mapIndexedNotNull { index, item ->
            if (item.hidden) return@mapIndexedNotNull null
            val line = lines.getOrNull(index)
            val key = item.row.id
            Row(
                key,
                line?.let { lineWithState(it, item.row.expanded) } ?: item.row.title,
                open = {
                    host.send(ShellEvent.Focus(key))
                    host.send(ShellEvent.Key("open"))
                },
                offered = list.offeredOf(item),
                on = ActingOn.Row(key),
                expanded = item.row.expanded,
                title = line?.title ?: item.row.title,
            )
        }
    }

    override val showsScreenActions: Boolean get() = true
}

/** Typing a filter over the list, which the app narrows as it is confirmed: a line of this app's own. */
class FilterScreen(@Suppress("unused") private val place: Place) : CoreDialog() {
    private val own by lazy {
        val words = appWords.quickAdd
        ownLine("filter", words.filter, words.filter, host.view.query, words.filterHint, words.filter)
            .let { it.copy(controls = it.controls.map { control -> if (control.key == "line") control.copy(syntax = Syntax.FILTER) else control }) }
    }

    override fun shown(): DialogShown = own

    override val lineFills: Boolean get() = true

    override fun shell(control: ControlShown) {
        when (control.key) {
            "confirm" -> {
                val text = line()?.let { dialog?.text(it) }.orEmpty()
                host.remove(this)
                host.send(ShellEvent.Filter(text))
            }
            "cancel" -> host.remove(this)
        }
    }
}

/** The day: blocks, sittings under them, free time, now; the day's own actions in the menu bar's Day menu. */
class DayScreen : AppList(), ShowsPlace {
    override val place: Place get() = Place.Today

    override fun loaded(view: ViewModel): Boolean = view.place is Place.Today && view.day != null

    override val title: String get() = heading(host.listShown, titleOf("go-today"))

    override val lands: Lands get() = Lands.MAIN

    override fun resumed() {
        if (host.view.place !is Place.Today) host.send(ShellEvent.Open(Place.Today))
    }

    override val empty: String get() = appWords.nothingPlanned

    override fun inHand(): String? = host.view.focus

    override fun moved(row: Row) = host.send(ShellEvent.Focus(row.key))

    override fun rows(): List<Row> {
        val view = host.view
        val day = view.day ?: return emptyList()
        val lines = host.lines?.day.orEmpty()
        return day.items.mapIndexedNotNull { index, item ->
            if (item.hidden) return@mapIndexedNotNull null
            val key = item.key
            val line = lines.getOrNull(index)
            Row(
                key,
                line?.let { lineWithState(it, item.expanded) }.orEmpty(),
                open = {
                    host.send(ShellEvent.Focus(key))
                    host.send(ShellEvent.Key("open"))
                },
                offered = item.offered,
                on = ActingOn.Row(key),
                expanded = item.expanded,
                title = line?.title,
            )
        }
    }

    override val showsScreenActions: Boolean get() = true
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

/** The heading of the list the core says is showing (`ListShown::heading`), else [name]. */
fun heading(list: ListShown?, name: String): String = list?.heading?.ifEmpty { null } ?: name

/**
 * The core's question, as its dialog (`ScreenView::dialog`): a confirmation, a line, a pick or a
 * choice of answers, with its buttons in the core's words, or the task form a task's Edit asks for
 * (`Question::Form`), the app showing that task's details as it asks, whose Save and Cancel close
 * the question too. On the Player a question that is one line is the whole screen.
 */
class QuestionScreen(private val asking: Asking) : CoreDialog(), AskingScreen {
    override val number: UInt get() = asking.number

    override fun shown(): DialogShown? = host.screenView.dialog?.takeIf { it.id == "question:$number" }

    // A form has several lines, each a row of its own; a question asking for one line is that line.
    override val lineFills: Boolean
        get() = shown()?.controls?.count { it.kind == ControlType.LINE || it.kind == ControlType.LINES } == 1

    // The task form asked for keeps the task's other actions a menu away, as the details do.
    override val offered: List<Offered>
        get() = if (asking.question is Question.Form) host.view.details?.task?.offered.orEmpty() else emptyList()
    override val on: ActingOn? get() = if (asking.question is Question.Form) ActingOn.Details else null

    override fun stands(view: ViewModel): Boolean = view.asking?.number == number

    /** Back is the dialog's cancel (the task form's Discard among it): what was typed is thrown away. */
    override fun beforeBack(): Boolean = dialog?.cancel() ?: false

    override fun leaving() {
        if (host.view.asking?.number == number) host.send(ShellEvent.Cancel)
    }

    /** Escape: the question's Cancel; the app closing it takes the screen away. */
    override fun cancelDialog() {
        dialog?.cancel()
    }
}

interface AskingScreen {
    val number: UInt
}

/** The candidates of a completion. */
class CompletionScreen(private val options: List<Pair<String, () -> Unit>>) : AppList() {
    override val title: String get() = appWords.completions
    override fun rows(): List<Row> = options.mapIndexed { index, (text, run) -> Row(index.toString(), text, { run() }) }
}

/** Quick add: the core's dialog while its line is open, with completion and its refusals. */
class QuickAddScreen : CoreDialog() {
    private var seen = false

    override fun shown(): DialogShown? = host.screenView.dialog?.takeIf { it.id == "quick-add" }

    override val lineFills: Boolean get() = true

    // Starts as the place shown starts it: known only once the app has opened its line.
    override fun loaded(view: ViewModel): Boolean = view.line != null && shown() != null

    override fun resumed() {
        if (host.view.line == null) host.send(ShellEvent.Typing(Typing.Open))
    }

    override fun stands(view: ViewModel): Boolean {
        if (view.line != null) seen = true
        return !seen || view.line != null
    }

    override fun cancelDialog() = host.remove(this)

    override fun leaving() {
        host.lineClosed = true
        host.send(ShellEvent.Typing(Typing.Close))
    }
}

/** A task's or block's reminders. */
class RemindersScreen : AppList() {
    override val lands: Lands get() = Lands.SIDE

    override val title: String
        get() = host.view.remindersHeading.orEmpty()

    override fun stands(view: ViewModel): Boolean = view.reminders != null

    override fun leaving() {
        if (host.view.reminders != null) host.send(ShellEvent.CloseReminders)
    }

    override fun inHand(): String? = host.view.remindersHeld.focus

    override fun moved(row: Row) = host.send(ShellEvent.Hold(Side.REMINDERS, row.key))

    override fun rows(): List<Row> {
        val list = host.view.reminders ?: return emptyList()
        val lines = host.lines?.reminders.orEmpty()
        return list.reminders.mapIndexed { index, reminder ->
            Row(reminder.id, lines.getOrNull(index)?.text ?: reminder.words, null, reminder.offered, ActingOn.Held(Side.REMINDERS, reminder.id))
        }
    }

    override fun commands(): List<Command> = buildList {
        host.view.reminders?.offered?.firstOrNull { it.kind == ActionKind.NEW }?.let { action ->
            val letter = actionLetter(action.kind)
            add(Command(host.withLetter(action.name, letter), letter) { host.send(ShellEvent.ActOn(ActingOn.Reminders, action)) })
        }
        addAll(undoCommands(host))
    }
}

/** What came due over the last day and was not dealt with, each with its buttons. */
class DueScreen : AppList() {
    override val lands: Lands get() = Lands.SIDE

    override val title: String get() = appWords.remindersDue

    override fun resumed() = host.send(ShellEvent.Due(DueEvent.LookBack))

    override val empty: String get() = appWords.nothingDue

    override fun inHand(): String? = host.view.dueHeld.focus

    override fun moved(row: Row) = host.send(ShellEvent.Hold(Side.DUE, row.key))

    // A reminder's buttons are its menu, Orbit's own kind, each handed back by its id.
    override fun rows(): List<Row> = host.view.due.map { due ->
        Row(due.id, due.text, open = {
            host.stack.menu(buttonCommands(due.id).map { MenuItem(it.text, it.letter?.singleOrNull(), value = it.run) }) { run -> run?.invoke() }
        })
    }

    /** A button's letter presses it on the reminder in hand, as in its menu. */
    override fun letter(letter: String): Boolean {
        val row = currentRow ?: return false
        val command = buttonCommands(row.key).firstOrNull { it.letter == letter } ?: return false
        command.run()
        return true
    }

    /** Each button handed back by its id: the core says what it does. */
    private fun buttonCommands(id: String): List<Command> {
        val due = host.view.due.firstOrNull { it.id == id } ?: return emptyList()
        return due.buttons.map { button ->
            val letter = dueLetter(button.id)
            Command(host.withLetter(button.title, letter), letter) { host.send(ShellEvent.Due(DueEvent.Press(id, button.id))) }
        }
    }

    override fun commands(): List<Command> {
        val row = currentRow ?: return emptyList()
        return buttonCommands(row.key)
    }
}

/**
 * The keys this device has and what they do: the core's key help for the Speak or the Player
 * (`App::key_help_on`), over what the screen beneath it shows, its headings and keys a line each.
 */
class KeyHelpScreen : AppList() {
    override val title: String get() = help().title

    override fun rows(): List<Row> = help().lines.mapIndexed { index, text -> Row(index.toString(), text) }

    private fun help() = host.app.keyHelp(if (host.phoneKeypad) KeyDevice.ORBIT_PLAYER else KeyDevice.ORBIT_SPEAK, true)
}

/** About: the core's lines (`ShellWords::about_lines`), a line each, as Key Help is a list. */
class AboutScreen : AppList() {
    override val title: String get() = titleOf("about")

    override fun rows(): List<Row> = appWords.aboutLines.mapIndexed { index, text -> Row(index.toString(), text) }
}

/** Replan from Now and Plan the Day: the app proposes, and says so; the person checks what to take. */
class ProposalScreen : AppList() {
    override val title: String get() = host.view.proposal?.words?.title.orEmpty()

    override fun openedAs(): Opened = Opened.Dialog

    override fun stands(view: ViewModel): Boolean = view.proposal != null

    override fun leaving() {
        if (host.view.proposal != null) host.send(ShellEvent.Proposal(ProposalEvent.Close))
    }

    override fun rows(): List<Row> {
        val proposal = host.view.proposal ?: return emptyList()
        val words = proposal.words
        return buildList {
            proposal.refusal?.let { add(Row("refusal", it)) }
            proposal.changes.forEachIndexed { index, change ->
                add(Row("change-$index", change.read, {
                    host.send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked)))
                }))
            }
            if (proposal.changes.isEmpty()) add(Row("nothing", words.nothing))
            proposal.unfitLines.forEachIndexed { index, line -> add(Row("unfit-$index", line)) }
            if (proposal.changes.isNotEmpty()) add(Row("apply", proposal.applyLine, { host.send(ShellEvent.Proposal(ProposalEvent.Apply)) }))
            if (proposal.others.isNotEmpty()) {
                add(Row("others", words.others))
                proposal.others.forEachIndexed { index, other ->
                    add(Row("other-$index", other, { host.send(ShellEvent.Proposal(ProposalEvent.Take(index.toUInt()))) }))
                }
            }
            add(Row("cancel", words.cancel, { host.send(ShellEvent.Proposal(ProposalEvent.Close)) }))
        }
    }
}
