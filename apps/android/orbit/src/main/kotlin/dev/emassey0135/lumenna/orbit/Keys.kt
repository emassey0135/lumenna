package dev.emassey0135.lumenna.orbit

/** A key decoded from Orbit's keypad callback, whichever device it came from. */
sealed interface Key {
    data object Up : Key
    data object Down : Key
    /** Up held. */
    data object First : Key
    /** Down held. */
    data object Last : Key
    data object Left : Key
    data object Right : Key
    /** OK or Select pressed. */
    data object Select : Key
    /** OK held. */
    data object SelectHeld : Key

    /** The Speak's dot 7 alone. */
    data object Dot7 : Key
    /** The Speak's dot 8 alone. */
    data object Dot8 : Key
    /** The Speak's Space alone. */
    data object Space : Key
    /** Dot keys without Space: a braille cell, as a bit per dot (1 is dot 1). */
    data class Cell(val dots: Int) : Key
    /** Space with dots: a command chord. */
    data class Chord(val dots: Int) : Key

    /** The Player's digit, tapped: the release of a press that was not held. */
    data class Digit(val digit: Int) : Key
    data class DigitHeld(val digit: Int) : Key
    data object Star : Key
    data object StarHeld : Key
    data object Pound : Key
    data object PoundHeld : Key
    /** The Player's Back pressed. */
    data object Back : Key
    /** The Player's Back let go: [held] after a long press. */
    data class BackReleased(val held: Boolean) : Key
    /** The Player's Menu. */
    data object Menu : Key

    data object None : Key
}

/** What the keypad callback gives: Bundle's `keyName`, `action` and `isLongPress`. */
data class KeyEvent(val name: String, val action: Int, val longPress: Boolean) {
    companion object {
        const val PRESS = 0
        const val RELEASE = 1
        const val REPEAT = 2
        const val CHORD = 3
    }
}

/**
 * Turns the callback's raw events into [Key]s.
 *
 * Arrows, OK and the Player's keys send a press (action 0, with `isLongPress` true on a held
 * press's repeat), then a release. Dots arrive as releases, chords whole (`SPACE_DOT1_DOT5`).
 * Orbit keeps some chords even from a self-voicing app (Space with dots 2, 3, 5 or 6 outside an
 * edit box, its review chords), which a table of commands must leave free. Keeps a held phone
 * key's release from counting too.
 */
class KeyDecoder {
    /** A Player key that has acted while held, its release ignored. */
    private var heldKey: String? = null

    fun decode(event: KeyEvent): Key {
        val name = event.name
        if (isPhoneKey(name)) return phoneKey(event)
        return when (event.action) {
            KeyEvent.PRESS -> when (name) {
                "UP" -> if (event.longPress) Key.First else Key.Up
                "DOWN" -> if (event.longPress) Key.Last else Key.Down
                "LEFT" -> Key.Left
                "RIGHT" -> Key.Right
                "OK", "SELECT" -> if (event.longPress) Key.SelectHeld else Key.Select
                "BACK" -> if (event.longPress) Key.None else Key.Back
                else -> if (name == "SPACE") Key.Space else Key.None
            }
            KeyEvent.RELEASE -> when (name) {
                "DOT7" -> Key.Dot7
                "DOT8" -> Key.Dot8
                "MENU" -> Key.Menu
                "BACK" -> Key.BackReleased(event.longPress)
                "SPACE" -> Key.Space
                else -> dots(name)?.takeIf { it and 0x3f != 0 }?.let(Key::Cell) ?: Key.None
            }
            KeyEvent.CHORD -> {
                if (name.startsWith("SPACE_")) {
                    dots(name.removePrefix("SPACE_"))?.let(Key::Chord) ?: Key.None
                } else {
                    dots(name)?.let(Key::Cell) ?: Key.None
                }
            }
            else -> Key.None
        }
    }

    private fun phoneKey(event: KeyEvent): Key {
        val name = event.name
        val star = name == "*"
        val pound = name == "#"
        if (event.action == KeyEvent.PRESS && event.longPress) {
            heldKey = name
            return when {
                star -> Key.StarHeld
                pound -> Key.PoundHeld
                else -> Key.DigitHeld(name[0] - '0')
            }
        }
        if (event.action != KeyEvent.RELEASE) return Key.None
        if (heldKey == name) {
            heldKey = null
            return Key.None
        }
        return when {
            star -> Key.Star
            pound -> Key.Pound
            else -> Key.Digit(name[0] - '0')
        }
    }

    companion object {
        private fun isPhoneKey(name: String): Boolean =
            name.length == 1 && (name[0].isDigit() || name == "*" || name == "#")

        /** The dots of a name like `DOT1_DOT5`, a bit per dot; null for anything else. */
        fun dots(name: String): Int? {
            var dots = 0
            for (token in name.split('_')) {
                if (token.length != 4 || !token.startsWith("DOT")) return null
                val dot = token[3] - '0'
                if (dot !in 1..8) return null
                dots = dots or (1 shl (dot - 1))
            }
            return dots.takeIf { it != 0 }
        }
    }
}
