package com.extra.han.lumenna.device

/**
 * What the person pressed, as the screens take it: the same whether it came from the braille
 * keys (`han_brailleinput`'s chords) or the snap-on QWERTY keyboard (ordinary key events), so
 * every screen handles one vocabulary. Named after what the device's manual says the key does
 * (chapter 2.4), not after the key.
 */
sealed interface Key {
    data object Up : Key
    data object Down : Key
    data object First : Key
    data object Last : Key
    data object PageUp : Key
    data object PageDown : Key
    data object LineStart : Key
    data object LineEnd : Key
    data object CharLeft : Key
    data object CharRight : Key
    data object WordLeft : Key
    data object WordRight : Key
    data object Enter : Key
    data object Escape : Key
    data object Exit : Key
    data object Tab : Key
    data object ShiftTab : Key
    /** Opens the program's menus: Space-M or F2, Alt on the QWERTY keyboard. */
    data object Menu : Key
    /** Opens the row's own menu of actions: Enter-I, the device's "actions menu", or the Menu key. */
    data object Context : Key
    data object Help : Key
    /** Says and shows the current item again: Space-R. */
    data object Repeat : Key
    data object Find : Key
    data object Options : Key
    data object Delete : Key
    /** Stops speech: Space-7-8. */
    data object Silence : Key
    /** Space by itself: a check box or combo box's change, a space when typing. */
    data object Space : Key
    data object Backspace : Key
    /** Pans the display back, or moves to the previous item at its start. */
    data object ScrollBack : Key
    /** Pans the display on, or moves to the next item at its end. */
    data object ScrollOn : Key
    /** A routing key over [cell], from the left of the display. */
    data class Route(val cell: Int) : Key
    /** Space with these dots (bit 0 is dot 1): the letter's command, where it is not one above. */
    data class SpaceDots(val dots: Int) : Key
    /** Enter with these dots: the device's hotkey form. */
    data class EnterDots(val dots: Int) : Key
    /** Backspace with these dots. */
    data class BackspaceDots(val dots: Int) : Key
    /** A cell by itself: a first letter in a list, typed in a line (bit 0 is dot 1). */
    data class Cell(val dots: Int) : Key
    /** A character from the QWERTY keyboard, typed in a line or a first letter in a list. */
    data class Typed(val text: String) : Key
    /** Ctrl-Shift with a letter on the QWERTY keyboard: the same as Space with it. */
    data class SpaceLetter(val letter: Char) : Key
    /** Control with a letter on the QWERTY keyboard: the same as Enter with it. */
    data class EnterLetter(val letter: Char) : Key
}
