package dev.emassey0135.braillesense.input

/**
 * What the person pressed, as the screens take it: the same whether it came from the braille
 * keys (`han_brailleinput`'s chords) or the snap-on QWERTY keyboard (ordinary key events), so
 * every screen handles one vocabulary. Named after what the device's manual says the key does
 * (chapter 2.4), not after the key.
 */
public sealed interface Key {
    /** Space-1, or the up arrow: the row before. */
    public data object Up : Key
    /** Space-4, or the down arrow: the next row. */
    public data object Down : Key
    /** Space-1-2-3, Control-Home: the first row. */
    public data object First : Key
    /** Space-4-5-6, Control-End: the last row. */
    public data object Last : Key
    /** Space-1-2-6, F1 with F2, Page Up: a page back. */
    public data object PageUp : Key
    /** Space-3-4-5, F3 with F4, Page Down: a page on. */
    public data object PageDown : Key
    /** Space-1-3, Home: the line's start. */
    public data object LineStart : Key
    /** Space-4-6, End: the line's end. */
    public data object LineEnd : Key
    /** Space-3, the left arrow: a character back, or folding a row. */
    public data object CharLeft : Key
    /** Space-6, the right arrow: a character on, or unfolding a row. */
    public data object CharRight : Key
    /** Space-2, Control-Left: a word back. */
    public data object WordLeft : Key
    /** Space-5, Control-Right: a word on. */
    public data object WordRight : Key
    /** Enter: opens, runs, confirms. */
    public data object Enter : Key
    /** Space-E, F4, Escape: back. */
    public data object Escape : Key
    /** Space-Z: leaves the program. */
    public data object Exit : Key
    /** Space-4-5, F3, Tab: the next control of a dialog. */
    public data object Tab : Key
    /** Space-1-2, Space-F3, Shift-Tab: the control before. */
    public data object ShiftTab : Key
    /** Opens the program's menus: Space-M or F2, Alt on the QWERTY keyboard. */
    public data object Menu : Key
    /** Opens the row's own menu of actions: Enter-I, the device's "actions menu", or the Menu key. */
    public data object Context : Key
    /** Space-H: the program's help. */
    public data object Help : Key
    /** Says and shows the current item again: Space-R or Space-C. */
    public data object Repeat : Key
    /** Says the character at the cursor in a line: Space-3-6, as the device's edit boxes have it. */
    public data object SayCharacter : Key
    /** Says the word at the cursor in a line: Space-2-5. */
    public data object SayWord : Key
    /** Space-F, Control-F: find. */
    public data object Find : Key
    /** Space-O: the program's options. */
    public data object Options : Key
    /** Space-D, Delete: the character or row at the cursor. */
    public data object Delete : Key
    /** Deletes the word at the cursor in a line: Backspace-2-5, Alt-Backspace. */
    public data object DeleteWord : Key
    /** Deletes the line shown at the cursor: Backspace-1-4 (C), Control-Backspace. */
    public data object DeleteLine : Key
    /** Deletes the paragraph at the cursor: Backspace-1-3-4-6 or Backspace-2-3-5-6, Alt-Delete. */
    public data object DeleteParagraph : Key
    /** Stops speech: Space-7-8. */
    public data object Silence : Key
    /** Space by itself: a check box or combo box's change, a space when typing. */
    public data object Space : Key
    /** Backspace by itself: the cell before the cursor. */
    public data object Backspace : Key
    /** Pans the display back, or moves to the previous item at its start. */
    public data object ScrollBack : Key
    /** Pans the display on, or moves to the next item at its end. */
    public data object ScrollOn : Key
    /** A routing key, or a touched cell and a front Cursor button, over [cell] from the left of the display. */
    public data class Route(val cell: Int) : Key
    /** Space with these dots (bit 0 is dot 1): the letter's command, where it is not one above. */
    public data class SpaceDots(val dots: Int) : Key
    /** Enter with these dots: the device's hotkey form. */
    public data class EnterDots(val dots: Int) : Key
    /** Backspace with these dots. */
    public data class BackspaceDots(val dots: Int) : Key
    /** A cell by itself: a first letter in a list, typed in a line (bit 0 is dot 1). */
    public data class Cell(val dots: Int) : Key
    /** A character from the QWERTY keyboard, typed in a line or a first letter in a list. */
    public data class Typed(val text: String) : Key
    /** Ctrl-Shift with a letter on the QWERTY keyboard: the same as Space with it. */
    public data class SpaceLetter(val letter: Char) : Key
    /** Control with a letter on the QWERTY keyboard: the same as Enter with it. */
    public data class EnterLetter(val letter: Char) : Key
}
