package dev.emassey0135.orbit

import android.os.Handler
import android.os.Looper

/**
 * Finding a row of a list from the Player's phone keypad, as Satellite's launcher does (its
 * `ListFinder`, Apache-2.0, the same author's): a digit opens that row, 2 the second, 1 and 2 quickly
 * the twelfth, and 0 alone the tenth; holding star turns finding by letter on or off, letters then
 * typed with T9 ([T9Input]) and handed to the app to find, a held digit being the digit itself. On
 * the Speak a cell is a letter already: the app finds it the same way ([next] where it has no
 * rule of its own).
 */
class ListFinder(private val host: Host, later: T9Input.Later, private val main: Handler = Handler(Looper.getMainLooper())) {
    /** The list being found in, and what is said. */
    interface Host {
        /** How many rows the list has now. */
        val count: Int

        /** How long T9 waits before a letter is taken: `OrbitReader.characterCommitTimeout`. */
        val commitMs: Long

        /** A letter (or a held digit) was typed: move to the next row starting with it. */
        fun find(letter: String)

        /** A number was typed: open the row at [index], from 0. */
        fun open(index: Int)

        /** What was typed names no row: the error tone. */
        fun refuse()

        /** Says [text]: a T9 candidate. */
        fun speak(text: String)

        /** Star changed T9's mode: say it ([OrbitWords.mode]). */
        fun modeChanged(mode: T9Input.Mode)

        /** Holding star turned finding by letter [on] or off: say so. */
        fun lettersChanged(on: Boolean)
    }

    private var number = 0
    private val openNumber = Runnable { openNumber() }

    /** Whether the Player's digits type letters (T9) rather than open rows. */
    var letters = false
        private set

    private val t9 = T9Input(object : T9Input.Host {
        override fun caret(): Int = 0
        override fun timeoutMs(): Long = host.commitMs
        override fun candidate(character: Char) = host.speak(character.toString())
        override fun commit(character: Char, position: Int, feedback: Boolean) = host.find(character.toString())
        override fun modeChanged(mode: T9Input.Mode) = host.modeChanged(mode)
    }, later)

    /** Forgets what was typed, as any key that is not finding's does. */
    fun reset() {
        number = 0
        main.removeCallbacks(openNumber)
        t9.cancel()
    }

    /** A Player key: true if it was finding's, the rest going on to the list's keys. */
    fun phoneKey(key: Key): Boolean {
        when (key) {
            Key.StarHeld -> {
                reset()
                letters = !letters
                host.lettersChanged(letters)
            }
            // Star, pound and a held digit are finding's whether or not letters are on.
            is Key.DigitHeld -> if (letters) {
                t9.commitNow()
                host.find(key.digit.toString())
            }
            Key.Star -> if (letters) t9.nextMode()
            Key.Pound -> if (letters) t9.nextLayout()
            is Key.Digit -> if (letters) t9.tap(key.digit) else digit(key.digit)
            else -> return false
        }
        return true
    }

    private fun digit(digit: Int) {
        main.removeCallbacks(openNumber)
        val size = host.count
        number = if (number == 0 && digit == 0) 10 else number * 10 + digit
        if (number < 1 || number > size) {
            number = 0
            host.refuse()
        } else if (digit == 0 && number == 10 || number * 10 > size) {
            openNumber()
        } else {
            main.postDelayed(openNumber, TYPING_MS)
        }
    }

    private fun openNumber() {
        val at = number
        number = 0
        main.removeCallbacks(openNumber)
        if (at in 1..host.count) host.open(at - 1)
    }

    companion object {
        /** How long a number waits for its next digit. */
        const val TYPING_MS = 1000L

        /** Where the next of [lines] starting with [letter] is, after [from], wrapping round; null for none. */
        fun next(lines: List<String>, from: Int, letter: String): Int? {
            if (lines.isEmpty() || letter.isEmpty()) return null
            return (1..lines.size).map { Math.floorMod(from + it, lines.size) }
                .firstOrNull { lines[it].trimStart().startsWith(letter, ignoreCase = true) }
        }
    }
}
