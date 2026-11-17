package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.ListFinder
import dev.emassey0135.orbit.OrbitList

/**
 * A list, as Orbit's own are: told as a list box, each move as the item changing. Up and Down stop
 * at the ends, Tab and Shift-Tab go round, Space with L and dots 4 5 6 go to the ends ([OrbitList]);
 * Select and Dot 8 open the row in hand. Right unfolds a row that folds, else opens it; Left folds
 * it, else goes back. On the Speak a cell alone moves to the next row starting with its letter
 * ([findLetter] where the app finds rows itself), and Space (or Dot 8) with a row's [Item.letter]
 * opens it; on the Player a digit opens that row and holding star finds by letter with T9
 * ([ListFinder]).
 *
 * The rows are read from [items] each time the list is shown and at each [OrbitUi.refresh], the
 * row in hand kept by its [Item.key], or put where the app holds it ([inHand]). Subclass it for a
 * list of your own; [items], [tellShown], [inHand], [moved] and [findLetter] are open.
 */
public open class ListScreen(
    override val title: String,
    items: () -> List<Item>,
    /** What the list says with no rows. */
    public open val empty: String,
    actions: () -> List<Action> = { emptyList() },
) : Screen() {
    private val source = items
    private val own = actions

    /** The rows as last read. */
    public var rows: List<Item> = emptyList()
        private set

    /** Where the row in hand is, from 0. */
    public var index: Int = 0
        private set

    private var heldKey: String? = null

    /** The row in hand as last told, so a row the person reworded is read again and one landed on twice is not. */
    private var said: String? = null
    private var saidKey: String? = null

    /** The person just acted here: the next refresh says the row in hand if it now reads otherwise. */
    private var expecting = false

    private var finder: ListFinder? = null

    /** The row in hand. */
    public val current: Item? get() = rows.getOrNull(index)

    /** The rows as they are now. */
    protected open fun items(): List<Item> = source()

    /**
     * The row the app holds in hand, for an app that keeps one of its own: followed each time the
     * rows are read, ahead of the row the person last stood on. None leaves the person's.
     */
    public open fun inHand(): String? = null

    /** The person moved to [item] themselves (a key, a letter, a number): for an app that follows the row in hand. */
    public open fun moved(item: Item) {}

    /** A letter alone, for an app that finds a row by it itself: true if it took it. */
    public open fun findLetter(letter: Char): Boolean = false

    override fun actions(): List<Action> = own()

    override fun inHandActions(): List<Action> = current?.actions.orEmpty()

    /**
     * What the person does next may reword the row in hand (a check, a fold, a save): the next
     * refresh reads it again if it does. Opening a row, running an action and folding say so already.
     */
    public fun expect() {
        expecting = true
    }

    /** Puts the row of [key] in hand, told as a move while the list is shown. */
    public fun hold(key: String) {
        heldKey = key
        if (!attached || !ui.isShown(this)) return
        val at = rows.indexOfFirst { it.key == key }
        if (at >= 0) moveTo(at)
    }

    /** Reads the rows again: the row in hand [landOn], else the app's, else the person's by its key, else by its place. */
    protected fun rebuild(landOn: String? = null) {
        // Asked first, so an app's answer can look at the row in hand as it was.
        val held = inHand()
        rows = items()
        val at = listOfNotNull(landOn, held, heldKey).firstNotNullOfOrNull { key -> rows.indexOfFirst { it.key == key }.takeIf { it >= 0 } }
        index = (at ?: index).coerceIn(0, (rows.size - 1).coerceAtLeast(0))
        current?.let { heldKey = it.key }
    }

    override fun tellShown() {
        rebuild()
        expecting = false
        remember()
        reader.focusList(title, rows.size, index, current?.line.orEmpty())
        // Said once: a heading that says the list is empty already needs no repeat.
        if (rows.isEmpty() && empty !in title) ui.say(empty)
    }

    override fun tellMoved() {
        remember()
        reader.itemChanged(title, rows.size, index, current?.line.orEmpty())
    }

    private fun remember() {
        said = current?.line
        saidKey = current?.key
    }

    override fun refresh(tell: Boolean) {
        val before = current?.key
        val wasEmpty = rows.isEmpty()
        rebuild()
        val acted = expecting
        expecting = false
        if (!tell) return
        when {
            rows.isEmpty() -> if (!wasEmpty) emptied()
            current?.key != before -> tellMoved()
            acted && current?.line != said -> tellMoved()
        }
    }

    override fun land(key: String?) {
        val wasEmpty = rows.isEmpty()
        rebuild(key)
        expecting = false
        when {
            rows.isEmpty() -> if (!wasEmpty) emptied()
            // Landing on the row just said, as it was said, says nothing more.
            current?.key == saidKey && current?.line == said -> {}
            else -> tellMoved()
        }
    }

    /** A list emptied under the person: what it says with nothing in it, not a blank row. */
    private fun emptied() {
        said = null
        saidKey = null
        ui.say(empty)
    }

    override fun key(key: Key): Boolean {
        if (ui.phoneKeypad && finding().phoneKey(key)) return true
        finder?.reset()
        OrbitList.target(key, index, rows.size)?.let {
            moveTo(it, byPerson = true)
            return true
        }
        return when (key) {
            // Only an empty list leaves a move to here: there is nowhere to go.
            Key.Up, Key.Down, Key.First, Key.Last, Key.Tab, Key.BackTab -> {
                ui.refuse()
                true
            }
            Key.Select, Key.Dot8 -> {
                open()
                true
            }
            Key.Right -> {
                if (!fold(true)) open()
                true
            }
            Key.Left -> fold(false)
            is Key.Cell -> {
                val letter = ui.letters.of(key.dots)
                if (letter == null) ui.refuse() else if (!findLetter(letter)) find(letter.toString())
                true
            }
            is Key.Chord, is Key.Enter -> when (val pressed = ui.letters.pressed(key)) {
                is Letters.Pressed.Letter -> openLettered(pressed.letter) || deleted(pressed.letter)
                is Letters.Pressed.Refused -> {
                    ui.refused(pressed.said)
                    true
                }
                Letters.Pressed.None -> false
            }
            else -> false
        }
    }

    /** Moves the row in hand to [at], told as Orbit's lists tell a move; [byPerson] tells the app too. */
    protected fun moveTo(at: Int, byPerson: Boolean = false) {
        if (rows.isEmpty() || at == index) return
        index = at.coerceIn(0, rows.size - 1)
        heldKey = rows[index].key
        if (byPerson) moved(rows[index])
        tellMoved()
    }

    /** Select: the row's open, else its toggle (said once the app has changed it), else the error tone. */
    private fun open() {
        val row = current ?: return ui.refuse()
        val opening = row.open ?: row.toggle ?: return ui.refuse()
        expecting = true
        opening()
    }

    /** Space (or Dot 8) with D on a row that deletes: deleted, and the list read again once the app has it. */
    private fun deleted(letter: Char): Boolean {
        if (letter != DELETE) return false
        val deleting = current?.delete ?: return false
        expecting = true
        deleting()
        return true
    }

    /** Right or Left on a row that folds the other way: folded, and read again once the app has it. */
    private fun fold(open: Boolean): Boolean {
        val row = current ?: return false
        val folding = row.fold ?: return false
        val expanded = row.expanded ?: return false
        if (expanded == open) return false
        expecting = true
        folding(open)
        return true
    }

    /** The next row starting with [letter], said again where it is the only one, else why not. */
    private fun find(letter: String) {
        val at = ListFinder.next(rows.map(Item::text), index, letter)
        if (at == null) {
            letter.firstOrNull()?.let { ui.refused(ui.words.noRowStartsWith(it.lowercaseChar())) } ?: ui.refuse()
            return
        }
        if (at == index) tellMoved() else moveTo(at, byPerson = true)
    }

    /** Space or Dot 8 with a letter: the row of that letter opened where it stays; else not the list's. */
    private fun openLettered(letter: Char): Boolean {
        val row = rows.firstOrNull { it.letter?.lowercaseChar() == letter } ?: return false
        val opening = row.open ?: return false
        expecting = true
        opening()
        return true
    }

    private fun finding(): ListFinder = finder ?: ui.finder(
        rows = { rows.size },
        found = { text ->
            val letter = text.singleOrNull()?.lowercaseChar()
            if (letter == null || !findLetter(letter)) find(text)
        },
        opened = { at ->
            // Opened at once, as the Player's own lists do: the row is not said on the way.
            if (at in rows.indices) {
                index = at
                heldKey = rows[at].key
                moved(rows[at])
                open()
            }
        },
    ).also { finder = it }

    private companion object {
        /**
         * The letter whose press (Space with it, or Dot 8 where the app's letters keep Space) deletes
         * the row in hand. Orbit's lists have no delete key of their own, so it is D, for Delete.
         */
        const val DELETE = 'd'
    }
}
