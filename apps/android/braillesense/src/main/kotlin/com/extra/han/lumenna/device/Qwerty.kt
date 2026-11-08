package com.extra.han.lumenna.device

import android.view.KeyEvent

/**
 * The snap-on QWERTY keyboard, which is an ordinary keyboard to Android: what its keys mean, by
 * the table in the manual's Common Commands (arrows and Home/End for moving, Escape, Tab, F1 for
 * help, Alt for the menus, the Application key for a row's own menu; Control is Enter's hotkeys
 * and Control-Shift Space's commands).
 */
object Qwerty {
    /** A key press as a [Key], or null where it means nothing here. [char] is the key's character. */
    fun decode(keyCode: Int, meta: Int, char: Int): Key? {
        val shift = meta and KeyEvent.META_SHIFT_ON != 0
        val ctrl = meta and KeyEvent.META_CTRL_ON != 0
        val alt = meta and KeyEvent.META_ALT_ON != 0
        return when (keyCode) {
            KeyEvent.KEYCODE_DPAD_UP -> Key.Up
            KeyEvent.KEYCODE_DPAD_DOWN -> Key.Down
            KeyEvent.KEYCODE_DPAD_LEFT -> if (ctrl) Key.WordLeft else Key.CharLeft
            KeyEvent.KEYCODE_DPAD_RIGHT -> if (ctrl) Key.WordRight else Key.CharRight
            KeyEvent.KEYCODE_PAGE_UP -> Key.PageUp
            KeyEvent.KEYCODE_PAGE_DOWN -> Key.PageDown
            KeyEvent.KEYCODE_MOVE_HOME -> if (ctrl) Key.First else Key.LineStart
            KeyEvent.KEYCODE_MOVE_END -> if (ctrl) Key.Last else Key.LineEnd
            KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> Key.Enter
            KeyEvent.KEYCODE_ESCAPE, KeyEvent.KEYCODE_BACK -> Key.Escape
            KeyEvent.KEYCODE_TAB -> if (shift) Key.ShiftTab else Key.Tab
            KeyEvent.KEYCODE_DEL -> Key.Backspace
            KeyEvent.KEYCODE_FORWARD_DEL -> Key.Delete
            KeyEvent.KEYCODE_F1 -> Key.Help
            KeyEvent.KEYCODE_F2 -> Key.Menu
            KeyEvent.KEYCODE_F3 -> if (shift) Key.ShiftTab else Key.Tab
            KeyEvent.KEYCODE_F4 -> if (alt) Key.Exit else Key.Escape
            KeyEvent.KEYCODE_F10 -> if (shift) Key.Context else Key.Options
            KeyEvent.KEYCODE_MENU -> Key.Context
            KeyEvent.KEYCODE_ALT_LEFT, KeyEvent.KEYCODE_ALT_RIGHT -> null
            else -> printed(meta, char, ctrl, shift, alt)
        }
    }

    private fun printed(meta: Int, char: Int, ctrl: Boolean, shift: Boolean, alt: Boolean): Key? {
        if (char == 0) return null
        val text = String(Character.toChars(char))
        val letter = text.lowercase().singleOrNull()?.takeIf { it in 'a'..'z' }
        return when {
            ctrl && shift && letter != null -> Key.SpaceLetter(letter)
            ctrl && letter != null -> when (letter) {
                'f' -> Key.Find
                else -> Key.EnterLetter(letter)
            }
            alt -> null
            char == ' '.code -> Key.Space
            else -> Key.Typed(text)
        }
    }
}
