package dev.emassey0135.orbit

/**
 * A menu as Orbit's own apps have one: the Editor's and the File Manager's, opened by the one menu
 * key (the Speak's Space with M, the Player's Menu), its items and submenus in one list (File,
 * Edit, Read…) rather than a bar across the screen. By its user guide and its SDK's `Menu`:
 *
 * - Up and Down (and Tab and Shift-Tab) move round a level, from the last to the first, as Orbit's
 *   context menus do; the top and bottom go to its ends.
 * - Right opens a submenu on its first item, and does nothing on an item; Select opens a submenu
 *   or chooses an item, which closes the menu.
 * - Left goes back up a level, onto the submenu it came from, and closes the menu at the top.
 * - A letter alone is an item's access key in the level shown: the only item of that letter is
 *   selected as Select would (a submenu opened, an item chosen); several are moved round, the next
 *   after the one in hand first.
 *
 * Pure, so a JVM test holds it; how it is told to the screen reader is [OrbitReader.focusMenu].
 */
public class OrbitMenu<T>(root: List<Item<T>>) {
    /** One entry: a submenu when it has [children], else an item whose [value] is chosen. */
    public class Item<T>(
        /** What Orbit says of it. */
        public val name: String,
        /** The letter that selects it in its level, lowercase; none for one no letter reaches. */
        public val accessKey: Char? = null,
        /** Its items, for a submenu. */
        public val children: List<Item<T>> = emptyList(),
        /** What choosing it gives. */
        public val value: T? = null,
    ) {
        /** Whether it opens a level of its own. */
        public val isSubMenu: Boolean get() = children.isNotEmpty()
    }

    /** What a key did to the menu. */
    public sealed interface Outcome<out T> {
        /** The item in hand moved within its level, or stayed where a key moves nothing. */
        public data object Moved : Outcome<Nothing>
        /** A submenu opened, on its first item. */
        public data object Opened : Outcome<Nothing>
        /** Back up a level, on the submenu left. */
        public data object Back : Outcome<Nothing>
        /** Left at the top: the menu closes with nothing chosen. */
        public data object Closed : Outcome<Nothing>
        /** An item chosen: the menu closes and it runs. */
        public data class Chosen<T>(val value: T) : Outcome<T>
        /** A key that reaches nothing here (a letter no item has): Orbit's error tone. */
        public data object Refused : Outcome<Nothing>
    }

    private class Level<T>(val items: List<Item<T>>, var index: Int)

    private val levels = ArrayDeque(listOf(Level(root, 0)))
    private val level: Level<T> get() = levels.last()

    /** The items of the level shown. */
    public val items: List<Item<T>> get() = level.items

    /** Where the item in hand is in its level, from 0. */
    public val index: Int get() = level.index

    /** The item in hand; none in a menu with nothing in it. */
    public val current: Item<T>? get() = level.items.getOrNull(level.index)

    /** How many submenus deep the level shown is: 0 at the top. */
    public val depth: Int get() = levels.size - 1

    /** Up or Shift-Tab: the item before, round from the first to the last. */
    public fun up(): Outcome<T> = moveBy(-1)

    /** Down or Tab: the next item, round from the last to the first. */
    public fun down(): Outcome<T> = moveBy(1)

    /** The first item of the level. */
    public fun first(): Outcome<T> = moveTo(0)

    /** The last item of the level. */
    public fun last(): Outcome<T> = moveTo(level.items.size - 1)

    /** Moves to [at] in the level shown, as a list's own finding does. */
    public fun moveTo(at: Int): Outcome<T> {
        if (level.items.isNotEmpty()) level.index = at.coerceIn(0, level.items.size - 1)
        return Outcome.Moved
    }

    private fun moveBy(delta: Int): Outcome<T> {
        val size = level.items.size
        if (size > 0) level.index = (level.index + delta + size) % size
        return Outcome.Moved
    }

    /** Right: opens a submenu on its first item; on an item, nothing. */
    public fun right(): Outcome<T> {
        val item = current ?: return Outcome.Refused
        if (!item.isSubMenu) return Outcome.Moved
        levels.addLast(Level(item.children, 0))
        return Outcome.Opened
    }

    /** Left: back up a level, onto the submenu left; at the top, closes the menu. */
    public fun left(): Outcome<T> {
        if (levels.size == 1) return Outcome.Closed
        levels.removeLast()
        return Outcome.Back
    }

    /** Select: opens a submenu, or chooses an item. */
    public fun select(): Outcome<T> {
        val item = current ?: return Outcome.Refused
        if (item.isSubMenu) return right()
        val value = item.value ?: return Outcome.Refused
        return Outcome.Chosen(value)
    }

    /** A letter alone, [letter] lowercase: the item of that access key, else [Outcome.Refused]. */
    public fun accessKey(letter: Char): Outcome<T> {
        val matching = level.items.indices.filter { level.items[it].accessKey == letter }
        return when (matching.size) {
            0 -> Outcome.Refused
            1 -> {
                level.index = matching.single()
                select()
            }
            else -> {
                // Round them, from the one after the item in hand.
                level.index = matching.firstOrNull { it > level.index } ?: matching.first()
                Outcome.Moved
            }
        }
    }
}
