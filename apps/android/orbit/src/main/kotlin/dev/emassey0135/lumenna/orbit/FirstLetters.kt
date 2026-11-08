package dev.emassey0135.lumenna.orbit

import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import dev.emassey0135.lumenna.appWords
import java.util.Locale

/**
 * Finding a row by typing. On the Speak a letter moves to the next row starting with it; letters
 * typed quickly find the next row starting with them all, and if nothing does, the last letter
 * alone. On the Player a digit opens that row, 2 the second, 1 and 2 quickly the twelfth, and 0
 * alone the tenth; holding star turns finding by letter on or off, letters then typed with T9.
 * Ported from the Satellite launcher (`ListFinder`, Apache-2.0, the same author's).
 */
class FirstLetters(private val ui: Ui) {
    private val main = Handler(Looper.getMainLooper())
    private var prefix = ""
    private var lastLetterAt = 0L
    private var number = 0
    private val openNumber = Runnable { openNumber() }

    /** Whether the Player's digits type letters (T9) rather than open rows. */
    private var letters = false

    private val t9 = T9Input(object : T9Input.Host {
        override fun caret(): Int = 0
        override fun timeoutMs(): Long = ui.reader.characterCommitTimeout.toLong()
        override fun candidate(character: Char) = ui.reader.speak(character.toString())
        override fun commit(character: Char, position: Int, feedback: Boolean) =
            letter(character, TYPING_MS + timeoutMs())
        override fun modeChanged(name: String) = ui.reader.speak(name)
    }, ui)

    /** Forgets what was typed, as any other key does. */
    fun reset() {
        prefix = ""
        number = 0
        main.removeCallbacks(openNumber)
        t9.cancel()
    }

    /** A letter from the Speak, or null for a cell that is none. */
    fun letter(character: Char?) {
        if (character == null) {
            ui.refuse()
            return
        }
        letter(character, TYPING_MS)
    }

    private fun letter(character: Char, window: Long) {
        val screen = ui.top ?: return
        val rows = screen.rows()
        if (rows.isEmpty()) {
            ui.refuse()
            return
        }
        val labels = rows.map { it.text }
        val now = SystemClock.uptimeMillis()
        val continuing = now - lastLetterAt < window
        lastLetterAt = now
        val found = find(labels, screen.index, if (continuing) prefix else "", character)
        prefix = found.prefix
        when {
            found.index < 0 -> ui.refuse()
            found.index != screen.index -> {
                screen.index = found.index
                screen.selectedKey = rows[found.index].key
                screen.moved(rows[found.index])
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
                letter(('0' + key.digit), TYPING_MS)
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

    data class Found(val index: Int, val prefix: String)

    companion object {
        const val TYPING_MS = 1000L

        /**
         * Where typing [character] after [prefix] moves from [current]: staying put if the row
         * starts with them all, else the next row that does; else the next starting with
         * [character] alone. If the current row is the only one starting with it, nowhere: -1,
         * [character] kept for the letters after it.
         */
        fun find(labels: List<String>, current: Int, prefix: String, character: Char): Found {
            val letter = character.toString().lowercase(Locale.ROOT)
            if (prefix.isNotEmpty()) {
                val longer = prefix + letter
                if (current < labels.size && startsWith(labels[current], longer)) return Found(current, longer)
                val at = next(labels, current, longer)
                if (at >= 0) return Found(at, longer)
            }
            val at = next(labels, current, letter)
            if (at < 0) return Found(-1, "")
            return Found(if (at == current) -1 else at, letter)
        }

        private fun next(labels: List<String>, current: Int, prefix: String): Int {
            for (step in 1..labels.size) {
                val at = (current + step) % labels.size
                if (startsWith(labels[at], prefix)) return at
            }
            return -1
        }

        private fun startsWith(label: String, prefix: String): Boolean =
            label.lowercase(Locale.ROOT).startsWith(prefix)
    }
}
