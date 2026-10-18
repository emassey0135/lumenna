package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.Action
import io.github.emassey0135.lumenna.app.Choice
import io.github.emassey0135.lumenna.app.Question
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.sentenceCase

// The words Asking.kt gives the UniFFI core's records, for the app's (crates/mobile), while
// screens move onto the app: the two are one set of records generated twice.

/** An action's name in this platform's capitals: sentence case, as Material and Wear OS write. */
val Action.name: String get() = sentence.ifEmpty { title }

/** A question's title in sentence case where it is the action's own name; one naming something keeps its capitals. */
fun Action.asks(title: String): String = if (title == this.title) name else title

/** A text question's title in sentence case: the core's, which knows which words are names. */
val Question.Text.said: String get() = sentence.ifEmpty { title }

/** A fixed button's words in sentence case. */
fun appButton(text: String): String = sentenceCase(text)

/** How a choice the app offers reads, as [option] reads the UniFFI core's. */
fun option(choice: Choice): Option {
    val date = choice.date
    val start = choice.start
    return if (date != null && start != null) {
        val span = choice.end?.let { "${Clock.time(start)} to ${Clock.time(it)}" } ?: Clock.time(start)
        Option(choice.id, "${Clock.spokenDay(date)}, $span, ${choice.title}", "", choice.depth.toInt(), choice.reason.orEmpty())
    } else {
        Option(choice.id, choice.title, choice.detail.orEmpty(), choice.depth.toInt(), choice.reason.orEmpty())
    }
}

/**
 * The app's actions as a row's: each puts its row in hand, then runs. [kind] and [subject] are
 * the UniFFI core's while lists share `RowAction` with screens still on it; the two enums name
 * the same variants.
 */
fun AppModel.rowActions(row: String, actions: List<Action>, local: (Action) -> Boolean = { false }): List<RowAction> = actions.map { action ->
    RowAction(
        action.name,
        io.github.emassey0135.lumenna.records.ActionKind.valueOf(action.kind.name),
        action.destructive,
        io.github.emassey0135.lumenna.records.Subject.valueOf(action.subject.name),
        action.primary,
    ) {
        send(ShellEvent.Focus(row))
        // An action whose screen is not yet the app's opens the shell's own (`local`).
        if (!local(action)) send(ShellEvent.Perform(action))
    }
}

/** How an app row is said, as [RowSpeech] says the UniFFI core's: the title, then everything else. */
object AppRowSpeech {
    /** When the row is due, its time in this phone's clock, then the rest of its value. */
    fun details(row: io.github.emassey0135.lumenna.app.RowView): String? {
        val due = row.due?.let { due -> row.dueTime?.let { "$due at ${Clock.time(it)}" } ?: due }
        return listOfNotNull(due, row.value).joinToString(", ").ifEmpty { null }
    }

    /**
     * Everything after the title: done or not, when it is due, notable states, whether what is
     * under it is folded away, and the level where it changes from the row shown before (the
     * app's word for it, which folding keeps right).
     */
    fun value(item: io.github.emassey0135.lumenna.app.Item): String {
        val row = item.row
        val parts = mutableListOf<String>()
        if (row.checked == true) parts += "done"
        details(row)?.let { parts += it }
        // `ready` is true of almost every task; saying it everywhere buries the states that mean something.
        parts += row.state.filter { it != "ready" }
        row.expanded?.let { parts += if (it) "expanded" else "collapsed" }
        item.level?.let { parts += it }
        return parts.joinToString(", ")
    }
}

/** A place of the UniFFI core's as the app's, for screens on the app opened from those not yet. */
fun io.github.emassey0135.lumenna.records.Place.toApp(): io.github.emassey0135.lumenna.app.Place = when (this) {
    io.github.emassey0135.lumenna.records.Place.Today -> io.github.emassey0135.lumenna.app.Place.Today
    io.github.emassey0135.lumenna.records.Place.Tasks -> io.github.emassey0135.lumenna.app.Place.Tasks
    is io.github.emassey0135.lumenna.records.Place.Project -> io.github.emassey0135.lumenna.app.Place.Project(v1)
    is io.github.emassey0135.lumenna.records.Place.Label -> io.github.emassey0135.lumenna.app.Place.Label(v1)
    is io.github.emassey0135.lumenna.records.Place.Filter -> io.github.emassey0135.lumenna.app.Place.Filter(name, query)
    io.github.emassey0135.lumenna.records.Place.Blocks -> io.github.emassey0135.lumenna.app.Place.Blocks
    io.github.emassey0135.lumenna.records.Place.Trash -> io.github.emassey0135.lumenna.app.Place.Trash
}

/**
 * What a day row says: its first line, then the rest, in every app's order (as [words] says
 * the UniFFI core's): a block "<start> to <end>, <title>, <details>"; a sitting "<title>,
 * <details>"; free time "<title>, <details>, <start> to <end>"; now "<title>, <time>"; a
 * cancelled day "<start>, <title>, <details>". The clock is this device's.
 */
fun words(item: io.github.emassey0135.lumenna.app.DayItem): Pair<String, List<String>> {
    item.sitting?.let { return it.title to it.details }
    item.free?.let { return it.title to it.details + "${Clock.time(it.start)} to ${Clock.time(it.end)}" }
    item.now?.let { return "${it.title}, ${Clock.time(it.time)}" to emptyList() }
    item.cancelled?.let { return "${Clock.time(it.start)}, ${it.title}" to it.details }
    val block = item.block ?: return "" to emptyList()
    return "${Clock.time(block.start)} to ${Clock.time(block.end)}, ${block.title}" to block.details
}

/** What a place is called before the app has shown it. */
fun placeTitle(place: io.github.emassey0135.lumenna.app.Place): String = when (place) {
    io.github.emassey0135.lumenna.app.Place.Today -> "Today"
    io.github.emassey0135.lumenna.app.Place.Tasks -> "Tasks"
    is io.github.emassey0135.lumenna.app.Place.Project -> place.field0
    is io.github.emassey0135.lumenna.app.Place.Label -> place.field0
    is io.github.emassey0135.lumenna.app.Place.Filter -> place.name
    io.github.emassey0135.lumenna.app.Place.Blocks -> "Blocks"
    io.github.emassey0135.lumenna.app.Place.Trash -> "Trash"
}

/** What identifies a place's row across a reload: a project and a label can share a name. */
fun placeKey(kind: io.github.emassey0135.lumenna.app.SidebarKind): String = when (kind) {
    is io.github.emassey0135.lumenna.app.SidebarKind.Group -> "group:${kind.field0}"
    is io.github.emassey0135.lumenna.app.SidebarKind.Place -> placeKey(kind.field0)
}

fun placeKey(place: io.github.emassey0135.lumenna.app.Place): String = when (place) {
    is io.github.emassey0135.lumenna.app.Place.Project -> "project:${place.field0}"
    is io.github.emassey0135.lumenna.app.Place.Label -> "label:${place.field0}"
    is io.github.emassey0135.lumenna.app.Place.Filter -> "filter:${place.name}"
    else -> place.toString()
}

/** One of the app's places as a row: its name, what is in it, and whether it is folded away. */
data class PlaceRow(
    val key: String,
    val entry: io.github.emassey0135.lumenna.app.SidebarEntry,
    val title: String,
    val detail: String,
    val depth: Int,
    /** Folded away under a collapsed row above it. */
    val hidden: Boolean,
    /** For a row with something under it: whether that is shown, as its Collapse says. */
    val expanded: Boolean?,
)

/** The app's sidebar [entries] as rows, folded as the app has them: a row is open while it offers Collapse. */
fun placeRows(entries: List<io.github.emassey0135.lumenna.app.SidebarEntry>): List<PlaceRow> {
    var hiddenBelow: UInt? = null
    return entries.map { entry ->
        val title = when (val kind = entry.kind) {
            is io.github.emassey0135.lumenna.app.SidebarKind.Group -> when (kind.field0) {
                io.github.emassey0135.lumenna.app.SidebarGroup.PROJECTS -> "Projects"
                io.github.emassey0135.lumenna.app.SidebarGroup.LABELS -> "Labels"
                io.github.emassey0135.lumenna.app.SidebarGroup.FILTERS -> "Saved filters"
            }
            is io.github.emassey0135.lumenna.app.SidebarKind.Place -> placeTitle(kind.field0)
        }
        // The core's line is the title, then what is in it: shown beneath it here. A heading
        // is in sentence case here, where the core's line has it in title case.
        val rest = if (entry.text.startsWith(title, ignoreCase = true)) entry.text.drop(title.length) else entry.text
        val below = hiddenBelow
        if (below != null && entry.depth <= below) hiddenBelow = null
        val hidden = hiddenBelow != null
        val expanded = entry.actions.firstNotNullOfOrNull {
            when (it.kind) {
                io.github.emassey0135.lumenna.app.ActionKind.COLLAPSE -> true
                io.github.emassey0135.lumenna.app.ActionKind.EXPAND -> false
                else -> null
            }
        }
        if (expanded == false && !hidden) hiddenBelow = entry.depth
        PlaceRow(placeKey(entry.kind), entry, title, rest.removePrefix(", "), entry.depth.toInt(), hidden, expanded)
    }
}
