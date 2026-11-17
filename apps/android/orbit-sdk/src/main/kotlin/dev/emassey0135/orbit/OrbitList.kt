package dev.emassey0135.orbit

/**
 * How a list moves on Orbit's keys, for an app that tells [OrbitReader] of its list itself
 * (`focusList`, then `itemChanged` as it moves): Up and Down go a row and stop at the ends, Space
 * with L and dots 4 5 6 ([Key.First], [Key.Last]) go to them, and Tab and Shift-Tab go round from
 * the last row to the first. A letter is [ListFinder]'s, a menu [OrbitMenu]'s, which goes round
 * with Up and Down as Orbit's menus do.
 */
public object OrbitList {
    /** Where [key] takes a list of [size] rows from the row at [index]; null for a key that does not move it. */
    public fun target(key: Key, index: Int, size: Int): Int? {
        if (size <= 0) return null
        return when (key) {
            Key.Up -> (index - 1).coerceIn(0, size - 1)
            Key.Down -> (index + 1).coerceIn(0, size - 1)
            Key.First -> 0
            Key.Last -> size - 1
            Key.Tab -> Math.floorMod(index + 1, size)
            Key.BackTab -> Math.floorMod(index - 1, size)
            else -> null
        }
    }
}
