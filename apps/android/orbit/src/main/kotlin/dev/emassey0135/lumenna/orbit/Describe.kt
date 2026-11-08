package dev.emassey0135.lumenna.orbit

/** Why the screen is being drawn: what decides whether Orbit is told anything. */
enum class Why { Entered, Moved, Landed, Turn }

/** How a screen is said as it opens. */
enum class Opened { List, Dialog, ContextMenu, Silent }

/** One thing told to Orbit's screen reader. */
sealed interface ReaderCall {
    data class FocusList(val name: String, val count: Int, val selected: Int, val item: String) : ReaderCall
    data class ItemChanged(val name: String, val count: Int, val selected: Int, val item: String) : ReaderCall
    data class Dialog(val name: String, val message: String, val focused: String) : ReaderCall
    data class ContextMenu(val name: String) : ReaderCall
    data class Say(val text: String) : ReaderCall
}

/** A list screen as it is now. */
data class Shown(
    val title: String,
    val count: Int,
    val index: Int,
    val item: String,
    val opened: Opened,
    val message: String,
    val empty: String,
)

/**
 * What Orbit is told about a screen, so it voices it in the person's own voice and verbosity. A
 * screen newly on top is said as what it is; afterwards only a move is, one the person made or
 * one the app made ([Why.Landed]). A turn that only redrew says nothing, so a sync or a new
 * minute never talks over the person.
 */
fun describe(newlyShown: Boolean, why: Why, moved: Boolean, shown: Shown): List<ReaderCall> {
    val focus = ReaderCall.FocusList(shown.title, shown.count, shown.index, shown.item)
    val change = ReaderCall.ItemChanged(shown.title, shown.count, shown.index, shown.item)
    return when {
        newlyShown -> when (shown.opened) {
            Opened.List -> buildList {
                add(focus)
                // Said once: a heading that already says the list is empty needs no repeat.
                if (shown.count == 0 && shown.empty !in shown.title) add(ReaderCall.Say(shown.empty))
            }
            Opened.Dialog -> listOf(ReaderCall.Dialog(shown.title, shown.message, shown.item))
            Opened.ContextMenu -> listOf(ReaderCall.ContextMenu(shown.title), focus)
            Opened.Silent -> emptyList()
        }
        why == Why.Moved || why == Why.Landed -> listOf(change)
        moved && why == Why.Turn -> listOf(change)
        else -> emptyList()
    }
}
