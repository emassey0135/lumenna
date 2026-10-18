package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.Action
import io.github.emassey0135.lumenna.app.ActionKind
import io.github.emassey0135.lumenna.app.Choice
import io.github.emassey0135.lumenna.app.Item
import io.github.emassey0135.lumenna.app.Line
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.SidebarEntry
import io.github.emassey0135.lumenna.app.SidebarKind

// What Android adds to the lines the core words (crates/desktop, through the turn's `lines`):
// what a tree control would report by itself, which a Compose list does not.

/** An action's name in this platform's capitals: sentence case, as Material and Wear OS write. */
val Action.name: String get() = sentence.ifEmpty { title }

/** A choice the app offers, as its [line] reads, with why it is offered. */
fun option(choice: Choice, line: Line): Option = Option(choice.id, line.title, line.detail, choice.depth.toInt(), choice.reason.orEmpty())

/** A choice that is a name and what tells it apart: a project in the details' picker. */
fun option(choice: Choice): Option = Option(choice.id, choice.title, choice.detail.orEmpty(), choice.depth.toInt(), choice.reason.orEmpty())

/** The app's actions as a row's: each puts its row in hand, then runs. */
fun AppModel.rowActions(row: String, actions: List<Action>, local: (Action) -> Boolean = { false }): List<RowAction> = actions.map { action ->
    RowAction(
        action.name,
        action.kind,
        action.destructive,
        action.subject,
        action.primary,
    ) {
        send(ShellEvent.Focus(row))
        if (!local(action)) send(ShellEvent.Perform(action))
    }
}

/**
 * Everything a row says after its title: the core's line, then whether what is under it is
 * folded away, and the level where it changes from the row shown before (the app's word for
 * it, which folding keeps right). A tree control would say those two itself.
 */
fun spoken(line: Line?, expanded: Boolean?, level: String?): String =
    listOfNotNull(line?.detail?.ifEmpty { null }, expanded?.let { if (it) "expanded" else "collapsed" }, level).joinToString(", ")

/** A list's rows not folded away, each with its line. */
fun shownRows(items: List<Item>, lines: List<Line>): List<Pair<Item, Line?>> =
    items.mapIndexed { index, item -> item to lines.getOrNull(index) }.filter { !it.first.hidden }

/** A list row's speech after its title. */
fun spoken(item: Item, line: Line?): String = spoken(line, item.row.expanded, item.level)

/** What identifies a place's row across a reload: a project and a label can share a name. */
fun placeKey(kind: SidebarKind): String = when (kind) {
    is SidebarKind.Group -> "group:${kind.field0}"
    is SidebarKind.Place -> placeKey(kind.field0)
}

fun placeKey(place: Place): String = when (place) {
    is Place.Project -> "project:${place.field0}"
    is Place.Label -> "label:${place.field0}"
    is Place.Filter -> "filter:${place.name}"
    else -> place.toString()
}

/** One of the app's places as a row: its name, what is in it, and whether it is folded away. */
data class PlaceRow(
    val key: String,
    val entry: SidebarEntry,
    val title: String,
    val detail: String,
    val depth: Int,
    /** Folded away under a collapsed row above it. */
    val hidden: Boolean,
    /** For a row with something under it: whether that is shown, as its Collapse says. */
    val expanded: Boolean?,
)

/** The app's sidebar [entries] as rows, folded as the app has them: a row is open while it offers Collapse. */
fun placeRows(entries: List<SidebarEntry>): List<PlaceRow> {
    var hiddenBelow: UInt? = null
    return entries.map { entry ->
        val below = hiddenBelow
        if (below != null && entry.depth <= below) hiddenBelow = null
        val hidden = hiddenBelow != null
        val expanded = entry.actions.firstNotNullOfOrNull {
            when (it.kind) {
                ActionKind.COLLAPSE -> true
                ActionKind.EXPAND -> false
                else -> null
            }
        }
        if (expanded == false && !hidden) hiddenBelow = entry.depth
        PlaceRow(placeKey(entry.kind), entry, entry.title, entry.detail, entry.depth.toInt(), hidden, expanded)
    }
}
