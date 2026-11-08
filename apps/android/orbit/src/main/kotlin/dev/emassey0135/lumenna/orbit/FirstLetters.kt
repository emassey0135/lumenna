package dev.emassey0135.lumenna.orbit

import android.os.Handler
import android.os.Looper
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.firstLetter
import dev.emassey0135.lumenna.app.noRowStartsWith
import dev.emassey0135.lumenna.appWords
import android.os.SystemClock
import java.util.Locale

/**
 * Finding a row by typing. A letter moves to the next row starting with it: the app's own lists
 * by the core (`ShellEvent::FindLetter`, which goes by a row's name), this app's by the core's
 * `first_letter`. On the Player a digit opens that row, 2 the second, 1 and 2 quickly the twelfth,
 * and 0 alone the tenth; holding star turns finding by letter on or off, letters then typed with
 * T9. The digits and T9 are ported from the Satellite launcher (`ListFinder`, Apache-2.0, the same
 * author's).
 */
class FirstLetters(private val ui: Ui) {
    private val main = Handler(Looper.getMainLooper())
    private var number = 0
    private val openNumber = Runnable { openNumber() }

    /** Whether the Player's digits type letters (T9) rather than open rows. */
    private var letters = false

    private val t9 = T9Input(object : T9Input.Host {
        override fun caret(): Int = 0
        override fun timeoutMs(): Long = ui.reader.characterCommitTimeout.toLong()
        override fun candidate(character: Char) = ui.reader.speak(character.toString())
        override fun commit(character: Char, position: Int, feedback: Boolean) =
            letter(character.toString())
        override fun modeChanged(name: String) = ui.reader.speak(name)
    }, ui)

    /** Forgets what was typed, as any other key does. */
    fun reset() {
        number = 0
        main.removeCallbacks(openNumber)
        t9.cancel()
    }

    /** A letter from the keys, or null for a cell that is none. */
    fun letter(letter: String?) {
        val screen = ui.top ?: return
        if (letter == null) {
            ui.refuse()
            return
        }
        // The app's own lists: it says if nothing starts with it, as a refusal.
        if (screen.lands == Lands.MAIN) {
            ui.send(ShellEvent.FindLetter(letter))
            return
        }
        val rows = screen.rows()
        val at = firstLetter(rows.map { it.text }, booleanArrayOf(), screen.index.toUInt(), letter)?.toInt()
        when {
            at == null -> {
                ui.refuse()
                ui.say(noRowStartsWith(letter))
            }
            at == screen.index -> ui.say(rows[at].text)
            else -> {
                screen.index = at
                screen.selectedKey = rows[at].key
                screen.moved(rows[at])
                ui.redraw(Why.Moved)
            }
        }
    }

    /** A Player key: true if it was finding's, the rest going on to the list's keys. */
    fun phoneKey(key: Key, screen: Screen): Boolean {
        when (key) {
            Key.StarHeld -> {
                reset()
                letters = !letters
                ui.say(if (letters) appWords.orbit.lettersOn else appWords.orbit.lettersOff)
            }
            is Key.DigitHeld -> if (letters) {
                t9.commitNow()
                letter(key.digit.toString())
            }
            Key.Star -> if (letters) t9.nextMode()
            Key.Pound -> if (letters) t9.nextLayout()
            is Key.Digit -> if (letters) t9.tap(key.digit) else digit(key.digit, screen)
            else -> return false
        }
        return true
    }

    private fun digit(digit: Int, screen: Screen) {
        main.removeCallbacks(openNumber)
        val size = screen.rows().size
        number = if (number == 0 && digit == 0) 10 else number * 10 + digit
        if (number < 1 || number > size) {
            number = 0
            ui.refuse()
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
        val screen = ui.top ?: return
        val rows = screen.rows()
        if (at < 1 || at > rows.size) return
        screen.index = at - 1
        screen.selectedKey = rows[at - 1].key
        screen.moved(rows[at - 1])
        rows[at - 1].open?.invoke()
    }

    private companion object {
        const val TYPING_MS = 1000L
    }
}
