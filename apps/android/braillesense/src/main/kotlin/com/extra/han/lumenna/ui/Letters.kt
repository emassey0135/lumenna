package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Key
import dev.emassey0135.lumenna.app.brailleLetter
import dev.emassey0135.lumenna.app.firstLetter
import dev.emassey0135.lumenna.app.MenuShortcut
import dev.emassey0135.lumenna.app.menuShortcuts

/** The core's letters, as the screens ask for them: the JVM tests give their own. */
interface Letters {
    /** A cell or a QWERTY letter as the letter it stands for: by the core's dots, never the person's table. */
    fun letter(key: Key): Char?

    /** Where the next of [lines] starting with [letter] is, after [from], wrapping round. */
    fun find(lines: List<String>, from: Int, letter: Char): Int?

    /** The letter each of a menu's items shows, null for one left with none. */
    fun shortcuts(items: List<Pair<String, Char?>>): List<Char?>
}

object BrailleLetters : Letters {
    fun of(dots: Int): Char? = brailleLetter(dots.toUByte())?.singleOrNull()

    override fun letter(key: Key): Char? = when (key) {
        is Key.Cell -> of(key.dots)
        is Key.Typed -> key.text.lowercase().singleOrNull()?.takeIf { it.isLetterOrDigit() }
        else -> null
    }

    override fun find(lines: List<String>, from: Int, letter: Char): Int? =
        firstLetter(lines, BooleanArray(lines.size), from.toUInt(), letter.toString())?.toInt()

    override fun shortcuts(items: List<Pair<String, Char?>>): List<Char?> =
        menuShortcuts(items.map { (title, letter) -> MenuShortcut(title, letter?.toString()) }).map { it.singleOrNull() }

    /**
     * Space with these letters is already a command on every screen of the device (Space-1 is
     * A, Up; Space-1-2 is B, Shift-Tab; Space-1-3 is K, the line's start; Space-1-2-3 is L, the
     * first row; and C, D, E, F, H, M, O, R and Z the manual's own), so the core's letter is
     * pressed with Enter there.
     */
    private val SPACE_TAKEN = setOf('a', 'b', 'c', 'd', 'e', 'f', 'h', 'k', 'l', 'm', 'o', 'r', 'z')

    /** How a letter is pressed here, for a menu to show: a key's name is the device's. */
    fun press(letter: Char): String? = when {
        letter !in 'a'..'z' -> null
        letter in SPACE_TAKEN -> "Enter-$letter"
        else -> "Space-$letter"
    }
}
