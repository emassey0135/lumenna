package dev.emassey0135.lumenna.orbit

import dev.emassey0135.orbit.OrbitReader

/** Why the screen is being drawn: what decides whether Orbit is told anything. */
enum class Why { Entered, Moved, Landed, Turn }

/** How a screen is said as it opens. */
enum class Opened { List, Dialog, ContextMenu, Silent }

/** One thing told to Orbit's screen reader. */
sealed interface ReaderCall {
    data class FocusList(val name: String, val count: Int, val selected: Int, val item: String) : ReaderCall
    data class ItemChanged(val name: String, val count: Int, val selected: Int, val item: String) : ReaderCall
    data class Dialog(val name: String, val focused: String) : ReaderCall
    data class ContextMenu(val name: String) : ReaderCall
    data class Say(val text: String) : ReaderCall
    /** A menu of Orbit's own kind opened: Orbit says "Context menu" in its own words. */
    data object MenuOpened : ReaderCall
    /** The item in hand of a menu, as Orbit's apps tell theirs (role 8). */
    data class FocusMenu(val item: String, val subMenu: Boolean, val accessKey: Char?, val position: Int, val total: Int) : ReaderCall
}

/**
 * What Orbit is told of the menu bar ([MenuBarScreen]), as its Editor tells of its menu: the menu
 * opened as it first shows, then its item in hand, and each item moved to (a level opened or left,
 * a letter found) said as a menu's focus again. A turn that only redrew says nothing.
 */
fun describeMenu(newlyShown: Boolean, why: Why, item: ReaderCall.FocusMenu?): List<ReaderCall> = buildList {
    if (newlyShown) add(ReaderCall.MenuOpened)
    if (item != null && (newlyShown || why == Why.Moved || why == Why.Entered)) add(item)
}

/** A list screen as it is drawn now. */
data class Drawn(
    val title: String,
    val count: Int,
    val index: Int,
    val item: String,
    val opened: Opened,
    val empty: String,
)

/**
 * What Orbit is told about the screen on top, so it voices it in the person's own voice and
 * verbosity: the core's screen (`Turn::described`) and this app's own lists alike, as Orbit's
 * calls. A screen newly on top is said as what it is; afterwards only a move is, one the person
 * made or one the app made ([Why.Landed]). A turn that only redrew says nothing, so a sync or a
 * new minute never talks over the person.
 */
fun describe(newlyShown: Boolean, why: Why, moved: Boolean, shown: Drawn): List<ReaderCall> {
    val focus = ReaderCall.FocusList(shown.title, shown.count, shown.index, shown.item)
    val change = ReaderCall.ItemChanged(shown.title, shown.count, shown.index, shown.item)
    return when {
        newlyShown -> when (shown.opened) {
            Opened.List -> buildList {
                add(focus)
                // Said once: a heading that already says the list is empty needs no repeat.
                if (shown.count == 0 && shown.empty !in shown.title) add(ReaderCall.Say(shown.empty))
            }
            // Orbit says the dialog; the answer in hand is then an item of its list.
            Opened.Dialog -> listOf(ReaderCall.Dialog(shown.title, shown.item), change)
            // Orbit has said the menu is open; the first choice is then an item, not a named list.
            Opened.ContextMenu -> listOf(ReaderCall.ContextMenu(shown.title), change)
            Opened.Silent -> emptyList()
        }
        // A list emptied under the person: what it says with nothing in it, not a blank row.
        shown.count == 0 && (why != Why.Turn || moved) -> listOf(ReaderCall.Say(shown.empty))
        why == Why.Moved || why == Why.Landed -> listOf(change)
        moved && why == Why.Turn -> listOf(change)
        else -> emptyList()
    }
}

/** One description of the screen ([describe]), told to Orbit's screen reader. */
fun OrbitReader.perform(call: ReaderCall) = when (call) {
    is ReaderCall.FocusList -> focusList(call.name, call.count, call.selected, call.item)
    is ReaderCall.ItemChanged -> itemChanged(call.name, call.count, call.selected, call.item)
    is ReaderCall.Dialog -> dialog(call.name, "", call.focused)
    is ReaderCall.ContextMenu -> contextMenu(call.name, "")
    is ReaderCall.Say -> speakQueued(call.text)
    ReaderCall.MenuOpened -> menuOpened()
    is ReaderCall.FocusMenu -> focusMenu(call.item, call.subMenu, call.accessKey, call.position, call.total)
}
