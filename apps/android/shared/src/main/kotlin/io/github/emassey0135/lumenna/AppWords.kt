package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.ActingOn
import io.github.emassey0135.lumenna.app.Action
import io.github.emassey0135.lumenna.app.ActionKind
import io.github.emassey0135.lumenna.app.Choice
import io.github.emassey0135.lumenna.app.Item
import io.github.emassey0135.lumenna.app.Line
import io.github.emassey0135.lumenna.app.ListView
import io.github.emassey0135.lumenna.app.Offered
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.SidebarEntry
import io.github.emassey0135.lumenna.app.SidebarKind
import io.github.emassey0135.lumenna.app.Side
import io.github.emassey0135.lumenna.app.sidebarKey

// What Android adds to the lines the core words (crates/app, through the turn's `lines`):
// what a tree control would report by itself, which a Compose list does not.

/** An action's name in this platform's capitals: sentence case, as Material and Wear OS write. */
val Action.name: String get() = sentence.ifEmpty { title }

/** A choice the app offers, as its [line] reads, with why it is offered. */
fun option(choice: Choice, line: Line): Option =
    Option(choice.id, line.title, line.detail, choice.depth.toInt(), choice.reason.orEmpty(), choice.swatch)

/** A choice that is a name and what tells it apart: a project in the details' picker. */
fun option(choice: Choice): Option =
    Option(choice.id, choice.title, choice.detail.orEmpty(), choice.depth.toInt(), choice.reason.orEmpty(), choice.swatch)

/** An action as a row offers it, in short: its name, in sentence case as the app words it here. */
val Offered.name: String get() = sentence.ifEmpty { title }

/** [item]'s actions in short: its places among the list's own ([ListView.offered]). */
fun ListView.offeredOf(item: Item): List<Offered> = item.actions.mapNotNull { offered.getOrNull(it.toInt()) }

/**
 * A row's actions, in short: each runs on [row] (a row of the place shown, or held in a list
 * beside the main one, [side]: the places, a reminder list, the devices, so the change lands
 * back there). The app puts the row in hand and runs its action whole as it has it now. One
 * that is [local] (a form this app opens itself) only puts the row in hand.
 */
fun AppModel.rowActions(row: String, actions: List<Offered>, local: (Offered) -> Boolean = { false }, side: Side? = null): List<RowAction> =
    actingOn(if (side != null) ActingOn.Held(side, row) else ActingOn.Row(row), actions, local)

/** The actions offered on what [on] names, in short: a row, the day, the details' task, a reminder list. */
fun AppModel.actingOn(on: ActingOn, actions: List<Offered>, local: (Offered) -> Boolean = { false }): List<RowAction> =
    actions.map { action ->
        RowAction(action.name, action.kind, action.destructive, action.subject, action.primary) {
            if (!local(action)) {
                send(ShellEvent.ActOn(on, action))
            } else {
                when (on) {
                    is ActingOn.Row -> send(ShellEvent.Focus(on.field0))
                    is ActingOn.Held -> send(ShellEvent.Hold(on.field0, on.field1))
                    else -> {}
                }
            }
        }
    }

/**
 * A key [row] does not offer: the row put in hand (or held in [side]), then the app runs the
 * key's kind there and says why it has none, in the core's words.
 */
fun AppModel.refusal(row: String, side: Side? = null): (io.github.emassey0135.lumenna.app.ActionKind) -> Unit = { kind ->
    send(if (side != null) ShellEvent.Hold(side, row) else ShellEvent.Focus(row))
    send(ShellEvent.Run(listOf(kind)))
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

/** What identifies a place's row across a reload, as the app names its row in hand. */
fun placeKey(place: Place): String = sidebarKey(SidebarKind.Place(place))

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
        val expanded = entry.offered.firstNotNullOfOrNull {
            when (it.kind) {
                ActionKind.COLLAPSE -> true
                ActionKind.EXPAND -> false
                else -> null
            }
        }
        if (expanded == false && !hidden) hiddenBelow = entry.depth
        PlaceRow(entry.key, entry, entry.title, entry.detail, entry.depth.toInt(), hidden, expanded)
    }
}
