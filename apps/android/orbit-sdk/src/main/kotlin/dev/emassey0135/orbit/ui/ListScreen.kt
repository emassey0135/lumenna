package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.BrailleLetters
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.ListFinder
import dev.emassey0135.orbit.OrbitList

/**
 * A list, as Orbit's own are: told as a list box, each move as the item changing. Up and Down stop
 * at the ends, Tab and Shift-Tab go round, Space with L and dots 4 5 6 go to the ends ([OrbitList]);
 * Select, Dot 8 and Right open the row in hand. On the Speak a cell alone moves to the next row
 * starting with its letter, and Space (or Dot 8) with a row's [Item.letter] opens it; on the Player
 * a digit opens that row and holding star finds by letter with T9 ([ListFinder]).
 *
 * The rows are read from [items] each time the list is shown and at each [OrbitUi.refresh], the
 * row in hand kept by its [Item.key]. Subclass it for a list of your own; [items] and [tellShown]
 * are open.
 */
open class ListScreen(
    override val title: String,
    items: () -> List<Item>,
    /** What the list says with no rows. */
    open val empty: String,
    actions: () -> List<Action> = { emptyList() },
) : Screen() {
    private val source = items
    private val own = actions

    /** The rows as last read. */
    var rows: List<Item> = emptyList()
        private set

    /** Where the row in hand is, from 0. */
    var index: Int = 0
        private set

    private var heldKey: String? = null

    /** The text of the row in hand as last told, so a row the person reworded is read again. */
    private var said: String? = null

    /** The person just opened a row or ran an action here: the next refresh says the row if it now reads otherwise. */
    internal var expecting = false

    private var finder: ListFinder? = null

    /** The row in hand. */
    val current: Item? get() = rows.getOrNull(index)

    /** The rows as they are now. */
    protected open fun items(): List<Item> = source()

    override fun actions(): List<Action> = own()

    override fun inHandActions(): List<Action> = current?.actions.orEmpty()

    /** Puts the row of [key] in hand, told as a move while the list is shown. */
    fun hold(key: String) {
        heldKey = key
        if (!attached || !ui.isShown(this)) return
        val at = rows.indexOfFirst { it.key == key }
        if (at >= 0) moveTo(at)
    }

    /** Reads the rows again, the row in hand kept by its key, else by its place. */
    protected fun rebuild() {
        rows = items()
        val byKey = heldKey?.let { key -> rows.indexOfFirst { it.key == key }.takeIf { it >= 0 } }
        index = (byKey ?: index).coerceIn(0, (rows.size - 1).coerceAtLeast(0))
        current?.let { heldKey = it.key }
    }

    override fun tellShown() {
        rebuild()
        expecting = false
        said = current?.text
        reader.focusList(title, rows.size, index, current?.text.orEmpty())
        if (rows.isEmpty()) ui.say(empty)
    }

    override fun tellMoved() {
        said = current?.text
        reader.itemChanged(title, rows.size, index, current?.text.orEmpty())
    }

    override fun refresh(tell: Boolean) {
        val before = current?.key
        val wasEmpty = rows.isEmpty()
        rebuild()
        val acted = expecting
        expecting = false
        if (!tell) return
        when {
            rows.isEmpty() -> if (!wasEmpty) {
                said = null
                ui.say(empty)
            }
            current?.key != before -> tellMoved()
            acted && current?.text != said -> tellMoved()
        }
    }

    override fun key(key: Key): Boolean {
        if (ui.phoneKeypad && finding().phoneKey(key)) return true
        finder?.reset()
        OrbitList.target(key, index, rows.size)?.let {
            moveTo(it)
            return true
        }
        return when (key) {
            // Only an empty list leaves a move to here: there is nowhere to go.
            Key.Up, Key.Down, Key.First, Key.Last, Key.Tab, Key.BackTab -> {
                ui.refuse()
                true
            }
            Key.Select, Key.Dot8, Key.Right -> {
                open()
                true
            }
            is Key.Cell -> {
                find(BrailleLetters.letter(key.dots)?.toString())
                true
            }
            is Key.Chord -> letter(key.dots)
            is Key.Enter -> letter(key.dots)
            else -> false
        }
    }

    /** Moves the row in hand to [at], told as Orbit's lists tell a move. */
    protected fun moveTo(at: Int) {
        if (rows.isEmpty() || at == index) return
        index = at.coerceIn(0, rows.size - 1)
        heldKey = rows[index].key
        tellMoved()
    }

    private fun open() {
        val row = current ?: return ui.refuse()
        val opening = row.open ?: return ui.refuse()
        expecting = true
        opening()
    }

    /** The next row starting with [letter], said again where it is the only one. */
    private fun find(letter: String?) {
        val at = letter?.let { ListFinder.next(rows.map(Item::text), index, it) } ?: return ui.refuse()
        if (at == index) tellMoved() else moveTo(at)
    }

    /** Space or Dot 8 with a letter: the row of that letter opened; else not the list's, but the actions'. */
    private fun letter(dots: Int): Boolean {
        val letter = BrailleLetters.letter(dots) ?: return false
        val at = rows.indexOfFirst { it.letter?.lowercaseChar() == letter }
        if (at < 0) return false
        moveTo(at)
        open()
        return true
    }

    private fun finding(): ListFinder = finder ?: ui.finder(
        rows = { rows.size },
        found = ::find,
        opened = { at ->
            // Opened at once, as the Player's own lists do: the row is not said on the way.
            if (at in rows.indices) {
                index = at
                heldKey = rows[at].key
                open()
            }
        },
    ).also { finder = it }
}
