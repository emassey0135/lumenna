package dev.emassey0135.braillesense.ui

import dev.emassey0135.braillesense.input.Chords
import dev.emassey0135.braillesense.input.Key

/**
 * Letters as the frames need them: what a key stands for, finding a row by its first letter, and
 * the letter each item of a menu shows. [DeviceLetters] is the device's own way; an app with rules
 * of its own (Lumenna's come from its core) gives its own.
 */
interface Letters {
    /** A cell or a QWERTY letter as the letter it stands for, by its dots, never the person's table. */
    fun letter(key: Key): Char?

    /** Where the next of [lines] starting with [letter] is, after [from], wrapping round; null for none. */
    fun find(lines: List<String>, from: Int, letter: Char): Int?

    /** The letter each of a menu's items shows, given its title and the letter it asks for; null for one left with none. */
    fun shortcuts(items: List<Pair<String, Char?>>): List<Char?>
}

/**
 * The device's own letters: a cell is the letter a to z of its six dots (`Chords.letter`), a row is
 * found by the first letter of its text, and a menu item keeps the letter it asks for if no item
 * before it took it, else takes the first letter of its title not yet taken, as the device's menus
 * mark theirs.
 */
object DeviceLetters : Letters {
    override fun letter(key: Key): Char? = when (key) {
        is Key.Cell -> Chords.letter(key.dots and 0x3F).takeIf { key.dots and 0xC0 == 0 }
        is Key.Typed -> key.text.lowercase().singleOrNull()?.takeIf { it.isLetterOrDigit() }
        else -> null
    }

    override fun find(lines: List<String>, from: Int, letter: Char): Int? {
        if (lines.isEmpty()) return null
        return (1..lines.size).map { Math.floorMod(from + it, lines.size) }
            .firstOrNull { lines[it].trimStart().startsWith(letter, ignoreCase = true) }
    }

    override fun shortcuts(items: List<Pair<String, Char?>>): List<Char?> {
        val taken = HashSet<Char>()
        val asked = items.map { (_, letter) -> letter?.lowercaseChar()?.takeIf(taken::add) }
        return items.zip(asked).map { (item, letter) ->
            letter ?: item.first.lowercase().firstOrNull { it.isLetterOrDigit() && it !in taken }?.also { taken += it }
        }
    }
}
