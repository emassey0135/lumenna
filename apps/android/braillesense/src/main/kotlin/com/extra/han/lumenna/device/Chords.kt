package com.extra.han.lumenna.device

/**
 * The composed chord `han_brailleinput` delivers, a key per bit, and what it means. The device's
 * own manual (chapter 2.4, "General Commands and Navigation") names every command here.
 */
object Chords {
    const val DOT1 = 0x1000
    const val DOT2 = 0x0800
    const val DOT3 = 0x0400
    const val DOT4 = 0x4000
    const val DOT5 = 0x8000
    const val DOT6 = 0x10000
    /** Shares its bit with Backspace. */
    const val DOT7 = 0x0200
    /** Shares its bit with Enter. */
    const val DOT8 = 0x20000
    const val SPACE = 0x2000
    const val BACKSPACE = DOT7
    const val ENTER = DOT8
    /** The upper advance keys, which double as Control and Alt beside the Space bar. */
    const val CONTROL = 0x04000000
    const val ALT = 0x08000000
    /** The four function keys. */
    const val F1 = 0x00040000
    const val F2 = 0x00080000
    const val F3 = 0x00100000
    const val F4 = 0x00200000
    private const val FUNCTION_KEYS = F1 or F2 or F3 or F4

    /** The scroll keys beside the display, and the codes of holding one down. */
    const val SCROLL_UP = 0x00400000
    const val SCROLL_DOWN = 0x00800000
    const val SCROLL_UP_LONG = 0x01000000
    const val SCROLL_DOWN_LONG = 0x02000000

    /** Routing keys, as key codes: the first over cell 0. The second range is other models'. */
    const val ROUTE_FIRST = 272
    private const val ROUTE_LAST = 303
    private const val ROUTE_FIRST_LARGE = 352
    private const val ROUTE_LAST_LARGE = 391

    private const val SIX = DOT1 or DOT2 or DOT3 or DOT4 or DOT5 or DOT6

    /** The dots of a chord as a cell's bits, bit 0 dot 1. Dots 7 and 8 only when [eight]. */
    fun dots(chord: Int, eight: Boolean = false): Int {
        var dots = 0
        if (chord and DOT1 != 0) dots = dots or 0x01
        if (chord and DOT2 != 0) dots = dots or 0x02
        if (chord and DOT3 != 0) dots = dots or 0x04
        if (chord and DOT4 != 0) dots = dots or 0x08
        if (chord and DOT5 != 0) dots = dots or 0x10
        if (chord and DOT6 != 0) dots = dots or 0x20
        if (eight) {
            if (chord and DOT7 != 0) dots = dots or 0x40
            if (chord and DOT8 != 0) dots = dots or 0x80
        }
        return dots
    }

    /** Cell bits (bit 0 dot 1) from a dot list, for naming the patterns below. */
    private fun pattern(vararg dot: Int): Int = dot.fold(0) { bits, d -> bits or (1 shl (d - 1)) }

    /** Space with these dots has one meaning on every screen of the device. */
    private val SPACED: Map<Int, Key> = mapOf(
        pattern(1) to Key.Up,
        pattern(4) to Key.Down,
        pattern(1, 2, 3) to Key.First,
        pattern(4, 5, 6) to Key.Last,
        pattern(1, 2, 6) to Key.PageUp,
        pattern(3, 4, 5) to Key.PageDown,
        pattern(1, 3) to Key.LineStart,
        pattern(4, 6) to Key.LineEnd,
        pattern(3) to Key.CharLeft,
        pattern(6) to Key.CharRight,
        pattern(2) to Key.WordLeft,
        pattern(5) to Key.WordRight,
        pattern(4, 5) to Key.Tab,
        pattern(1, 2) to Key.ShiftTab,
        pattern(1, 3, 4) to Key.Menu, // M
        pattern(1, 5) to Key.Escape, // E
        pattern(1, 3, 5, 6) to Key.Exit, // Z
        pattern(1, 2, 5) to Key.Help, // H
        pattern(1, 2, 3, 5) to Key.Repeat, // R
        pattern(1, 4) to Key.Repeat, // C: the device's lists and edit boxes take both alike
        pattern(3, 6) to Key.SayCharacter, // the device's edit boxes: the character at the cursor
        pattern(2, 5) to Key.SayWord, // and the word
        pattern(1, 2, 4) to Key.Find, // F
        pattern(1, 3, 5) to Key.Options, // O
        pattern(1, 4, 5) to Key.Delete, // D
    )

    /**
     * What arrives on `onResponse`, or null for what is not a key of ours.
     *
     * A real press comes as the key code the system mapped it to, with the chord in the third
     * field of [data] ("67 0 512"); injected for testing, the chord is the first field and
     * [value] ("7424 0 0"). A code of -1 is a key; others are the service's own news.
     */
    fun decode(code: Int, value: Int, data: String?, eightDot: Boolean, doubleTapRoutes: Boolean = false): Key? {
        touched(code, value, data, doubleTapRoutes)?.let { return it }
        if (code != -1) return null
        val parts = data.orEmpty().trim().split(' ').filter { it.isNotEmpty() }
        val chord = parts.getOrNull(2)?.toIntOrNull()?.takeIf { it != 0 } ?: value
        val keyCode = parts.getOrNull(0)?.toIntOrNull() ?: value
        if (chord and FUNCTION_KEYS == 0) fromFunctionKey(keyCode, chord)?.let { return it }
        return fromChord(chord, eightDot)
    }

    /**
     * The touch display's cursor routing, which comes on the same callback as the keys but as a
     * gesture: code -7 when a cell is touched and a front Cursor button pressed (again for each
     * cell as the finger slides with the button held), the cell in [value]; and code -5 for a tap,
     * the kind in [value] (2 a one-finger double tap, 17 one held) and the cell in [data], which
     * routes only where the person turned on Double-Tap to Route Cursor. Cells count from 1.
     */
    private fun touched(code: Int, value: Int, data: String?, doubleTapRoutes: Boolean): Key? = when {
        code == SMART_CURSOR && value >= 1 -> Key.Route(value - 1)
        code == TAP && doubleTapRoutes && (value == DOUBLE_TAP || value == DOUBLE_TAP_HOLD) ->
            data?.trim()?.toIntOrNull()?.takeIf { it >= 1 }?.let { Key.Route(it - 1) }
        else -> null
    }

    private const val SMART_CURSOR = -7
    private const val TAP = -5
    private const val DOUBLE_TAP = 2
    private const val DOUBLE_TAP_HOLD = 17

    /** F1 to F4 as Android's own codes (131 on), from a keyboard that sends those. */
    private fun fromFunctionKey(keyCode: Int, chord: Int): Key? = when (keyCode) {
        // F1 alone is the system's Quick Search, here as everywhere; it is not ours to take.
        KEYCODE_F2 -> Key.Menu
        KEYCODE_F3 -> if (chord and SPACE != 0) Key.ShiftTab else Key.Tab
        KEYCODE_F4 -> Key.Escape
        else -> null
    }

    /** The chord alone, without the key code that may come with it. */
    fun fromChord(chord: Int, eightDot: Boolean = false): Key? {
        if (chord in ROUTE_FIRST..ROUTE_LAST) return Key.Route(chord - ROUTE_FIRST)
        if (chord in ROUTE_FIRST_LARGE..ROUTE_LAST_LARGE) return Key.Route(chord - ROUTE_FIRST_LARGE)
        if (chord and FUNCTION_KEYS != 0) return functionKeys(chord)
        if (chord == SCROLL_UP || chord == SCROLL_UP_LONG) return Key.ScrollBack
        if (chord == SCROLL_DOWN || chord == SCROLL_DOWN_LONG) return Key.ScrollOn
        val spaced = chord and SPACE != 0
        val enter = chord and ENTER != 0
        val backspace = chord and BACKSPACE != 0
        val bits = dots(chord)
        return when {
            spaced && (chord and DOT7 != 0) && (chord and DOT8 != 0) -> Key.Silence
            spaced && bits == 0 && !enter && !backspace -> Key.Space
            spaced && !enter && !backspace -> SPACED[bits] ?: Key.SpaceDots(bits)
            // In eight-dot mode these two are dots, typed with the rest of a character.
            eightDot && (enter || backspace) && bits != 0 -> Key.Cell(dots(chord, eight = true))
            enter && backspace -> null
            enter && bits == 0 -> Key.Enter
            enter && bits == pattern(2, 4) -> Key.Context // I: the device's actions menu
            enter -> Key.EnterDots(bits)
            backspace && bits == 0 -> Key.Backspace
            backspace -> Key.BackspaceDots(bits)
            bits != 0 -> Key.Cell(bits)
            else -> null
        }
    }

    /**
     * The function keys, by the manual's chapter 2.4.3. F1 alone, F1 or F3 with a letter, F1-F4 (the
     * task list), F2-F3 (the next program) and anything held with the scroll keys are the system's:
     * they are not taken, or they would stop working here.
     */
    private fun functionKeys(chord: Int): Key? = when (chord) {
        F2 -> Key.Menu
        F3 -> Key.Tab
        F3 or SPACE -> Key.ShiftTab
        F4 -> Key.Escape
        F1 or F2 -> Key.PageUp
        F3 or F4 -> Key.PageDown
        else -> null
    }

    private const val KEYCODE_F2 = 132
    private const val KEYCODE_F3 = 133
    private const val KEYCODE_F4 = 134
}
