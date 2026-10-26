package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.Action
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Choice
import dev.emassey0135.lumenna.app.Item
import dev.emassey0135.lumenna.app.Line
import dev.emassey0135.lumenna.app.ListView
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.SidebarEntry
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.KeyCommand
import dev.emassey0135.lumenna.app.keyboardShortcuts
import dev.emassey0135.lumenna.app.sidebarKey

// What Android adds to the lines the core words (crates/app, through the turn's `lines`):
// what a tree control would report by itself, which a Compose list does not.

/** The shared keyboard table's commands by id (`keyboard_shortcuts()`), named in sentence case. */
val sharedCommands: Map<String, KeyCommand> by lazy { keyboardShortcuts().flatMap { it.commands }.associateBy { it.id } }

/**
 * A shared command's name, as every app's menus and keyboard help call it ("Undo", "Previous
 * day"): what a button doing the same is called here too.
 */
fun commandTitle(id: String): String = sharedCommands[id]?.title ?: id

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
        RowAction(action.name, action.kind, action.destructive, action.primary) {
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
 * What a key does to [row]: the row put in hand (or held in [side]) first, as the app runs a
 * key on the row in hand.
 */
class RowKeys(private val app: AppModel, private val row: String, private val side: Side?) {
    private fun inHand() = app.send(if (side != null) ShellEvent.Hold(side, row) else ShellEvent.Focus(row))

    /** A key of the core's table by its id ("delete"): what it does to the row is the core's (`keys::runs`). */
    fun key(id: String) {
        inHand()
        app.send(ShellEvent.Key(id))
    }

    /** A key the row offers no action of [kind] for: the app runs it there and says why, in the core's words. */
    fun refuse(kind: ActionKind) {
        inHand()
        app.send(ShellEvent.Run(listOf(kind)))
    }
}

/** The keys on [row], or on a row held in [side]. */
fun AppModel.rowKeys(row: String, side: Side? = null): RowKeys = RowKeys(this, row, side)

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
