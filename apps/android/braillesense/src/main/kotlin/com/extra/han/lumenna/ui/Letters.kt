package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.ui.Letters
import dev.emassey0135.lumenna.app.brailleLetter
import dev.emassey0135.lumenna.app.firstLetter
import dev.emassey0135.lumenna.app.KeyDevice
import dev.emassey0135.lumenna.app.MenuShortcut
import dev.emassey0135.lumenna.app.menuShortcuts
import dev.emassey0135.lumenna.app.pressName

/** The core's letters, as the SDK's frames ask for them: the JVM tests give their own. */
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
     * How a letter is pressed here, for a menu to show, as the device's manual names keys: the
     * core's (`press_name`), Space with it ("Space-U"), or Enter ("Enter-C") where Space with those
     * dots is already the device's on every screen.
     */
    fun press(letter: Char): String? = pressName(KeyDevice.BRAILLE_SENSE, letter.toString())
}
