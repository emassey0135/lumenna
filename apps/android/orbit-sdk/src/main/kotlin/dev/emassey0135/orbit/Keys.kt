package dev.emassey0135.orbit

/** A key decoded from Orbit's keypad callback, whichever device it came from. */
public sealed interface Key {
    /** Up: the arrow, or Space with dot 1 on the Speak. */
    public data object Up : Key
    /** Down: the arrow, or Space with dot 4 on the Speak. */
    public data object Down : Key
    /** Up held. */
    public data object First : Key
    /** Down held. */
    public data object Last : Key
    /** The left arrow. */
    public data object Left : Key
    /** The right arrow. */
    public data object Right : Key
    /** OK or Select pressed. */
    public data object Select : Key
    /** OK held. */
    public data object SelectHeld : Key

    /** The Speak's dot 7 alone. */
    public data object Dot7 : Key
    /** The Speak's dot 8 alone. */
    public data object Dot8 : Key
    /** The Speak's Space alone. */
    public data object Space : Key
    /** Dot keys without Space: a braille cell, as a bit per dot (1 is dot 1). */
    public data class Cell(val dots: Int) : Key
    /** Space with dots: a command chord, those the decoder has not named. */
    public data class Chord(val dots: Int) : Key
    /** Dot 8 (Enter) with dots, the rest of them: Orbit's apps' commands. */
    public data class Enter(val dots: Int) : Key
    /** Space with dots 4 and 5: the next control of a dialog. */
    public data object Tab : Key
    /** Space with dots 1 and 2: the control before. */
    public data object BackTab : Key
    /** Space with E or Z: back, whatever the row. */
    public data object Escape : Key

    /** The Player's digit, tapped: the release of a press that was not held. */
    public data class Digit(val digit: Int) : Key
    /** The Player's digit, held. */
    public data class DigitHeld(val digit: Int) : Key
    /** The Player's star, tapped. */
    public data object Star : Key
    /** The Player's star, held: finding by letter, or the mode in a line. */
    public data object StarHeld : Key
    /** The Player's pound, tapped. */
    public data object Pound : Key
    /** The Player's pound, held. */
    public data object PoundHeld : Key
    /** The Player's Back pressed. */
    public data object Back : Key
    /** The Player's Back let go: [held] after a long press. */
    public data class BackReleased(val held: Boolean) : Key
    /** The Player's Menu. */
    public data object Menu : Key

    /** A key the decoder does not know, or the release of one that already acted: nothing. */
    public data object None : Key
}

/** What the keypad callback gives: Bundle's `keyName`, `action` and `isLongPress`. */
public data class KeyEvent(val name: String, val action: Int, val longPress: Boolean) {
    /** The keypad callback's actions. */
    public companion object {
        /** A key went down. */
        public const val PRESS: Int = 0
        /** A key came up: a tap, where it was not held. */
        public const val RELEASE: Int = 1
        /** A key held, repeating. */
        public const val REPEAT: Int = 2
        /** A chord of the Speak's keys, pressed together. */
        public const val CHORD: Int = 3
    }
}

/**
 * Turns the callback's raw events into [Key]s.
 *
 * Arrows, OK and the Player's keys send a press (action 0, with `isLongPress` true on a held
 * press's repeat), then a release. Dots arrive as releases, chords whole (`SPACE_DOT1_DOT5`).
 * Space with dots names what the Speak's user guide says it is: Up and Down, Tab and Shift-Tab,
 * back, the top and bottom, the menu. Orbit keeps some chords even from a self-voicing app (the
 * screen reader's C, G, T, and Space with dots 2, 3, 5 and 6, which review the item's text outside
 * an edit box), which a table of commands must leave free.
 * Keeps a held phone key's release from counting too.
 */
public class KeyDecoder {
    /** A Player key that has acted while held, its release ignored. */
    private var heldKey: String? = null

    /** What [event] means, or [Key.None]. */
    public fun decode(event: KeyEvent): Key {
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
            KeyEvent.CHORD -> chord(name)
            else -> Key.None
        }
    }

    private fun chord(name: String): Key {
        if (name.startsWith("SPACE_")) {
            val dots = dots(name.removePrefix("SPACE_")) ?: return Key.None
            return when (dots) {
                bits(1) -> Key.Up
                bits(4) -> Key.Down
                bits(1, 2, 3) -> Key.First
                bits(4, 5, 6) -> Key.Last
                bits(1, 5), bits(1, 3, 5, 6) -> Key.Escape
                bits(4, 5) -> Key.Tab
                bits(1, 2) -> Key.BackTab
                bits(1, 3, 4) -> Key.Menu
                else -> Key.Chord(dots)
            }
        }
        val dots = dots(name) ?: return Key.None
        val enter = bits(8)
        // Dot 7 with it too is no command: it is typing, an 8-dot cell.
        return if (dots and enter != 0 && dots and bits(7) == 0 && dots != enter) Key.Enter(dots and enter.inv()) else Key.Cell(dots)
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

    /** The chords the decoder names. */
    public companion object {
        private fun bits(vararg dots: Int): Int = dots.fold(0) { all, dot -> all or (1 shl (dot - 1)) }

        private fun isPhoneKey(name: String): Boolean =
            name.length == 1 && (name[0].isDigit() || name == "*" || name == "#")

        /** The dots of a name like `DOT1_DOT5`, a bit per dot; null for anything else. */
        public fun dots(name: String): Int? {
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
