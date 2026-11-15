package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.lumenna.app.brailleLetter
import dev.emassey0135.lumenna.app.braillesensePress
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
     * How a letter is pressed here, for a menu to show: the core's (`braillesense_press`), Space
     * with it, or Enter where Space with those dots is already the device's on every screen.
     */
    fun press(letter: Char): String? = braillesensePress(letter.toString())
}
