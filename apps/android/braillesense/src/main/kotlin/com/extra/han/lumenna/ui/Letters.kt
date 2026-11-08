package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Key
import dev.emassey0135.lumenna.app.ActionKind as K

/**
 * A chord's dots as a letter, by the core's dots for a to z, never the person's table: the same
 * chord is the same command whatever is on the display. The core's own table replaces this when
 * it lands on master (`lumenna_app::keys`).
 */
object BrailleLetters {
    private val DOTS: Map<Int, Char> = mapOf(
        0x01 to 'a', 0x03 to 'b', 0x09 to 'c', 0x19 to 'd', 0x11 to 'e', 0x0b to 'f', 0x1b to 'g',
        0x13 to 'h', 0x0a to 'i', 0x1a to 'j', 0x05 to 'k', 0x07 to 'l', 0x0d to 'm', 0x1d to 'n',
        0x15 to 'o', 0x0f to 'p', 0x1f to 'q', 0x17 to 'r', 0x0e to 's', 0x1e to 't', 0x25 to 'u',
        0x27 to 'v', 0x3a to 'w', 0x2d to 'x', 0x3d to 'y', 0x35 to 'z',
    )

    fun of(dots: Int): Char? = DOTS[dots]

    /** A cell or a QWERTY letter as the letter it stands for in a list or a menu. */
    val letters = Letters { key ->
        when (key) {
            is Key.Cell -> of(key.dots)
            is Key.Typed -> key.text.lowercase().singleOrNull()?.takeIf { it.isLetterOrDigit() }
            else -> null
        }
    }

    /**
     * What a letter runs on a row, by the kinds it offers (the core's table). Space with it runs
     * the letter's command, except Space's letters the device itself keeps (E, F, H, M, O, R, Z:
     * Escape, Find, Help, Menu, Options, Repeat, Exit), which only Enter with the letter reaches.
     */
    val COMMANDS: Map<Char, List<K>> = mapOf(
        'c' to listOf(K.MARK_DONE, K.MARK_NOT_DONE, K.COLOUR),
        'e' to listOf(K.EDIT, K.EDIT_TASK),
        'b' to listOf(K.PUT_IN_BLOCK),
        'm' to listOf(K.MOVE_TO_PROJECT, K.MOVE_UNDER, K.MERGE_INTO, K.LOG_MINUTES),
        's' to listOf(K.MAKE_SUBTASK_OF, K.START_TIMER, K.PAUSE_TIMER, K.RESUME_TIMER),
        't' to listOf(K.MOVE_TO_TOP_LEVEL, K.STOP_TIMER),
        'w' to listOf(K.WAIT_FOR, K.WEIGHT),
        'n' to listOf(K.STOP_WAITING, K.NEW_INSIDE),
        'r' to listOf(K.RESTORE, K.RENAME, K.REMINDERS),
        'i' to listOf(K.ASSIGN_TASK),
        'x' to listOf(K.CANCEL_DAY),
        'o' to listOf(K.RESTORE_DAY),
        'l' to listOf(K.PLANNED_LENGTH),
        'a' to listOf(K.ADD_BLOCK),
        'q' to listOf(K.CHANGE_QUERY),
        'f' to listOf(K.CARRY_FORWARD, K.CARRY_REST),
        ',' to listOf(K.MOVE_UP),
        '.' to listOf(K.MOVE_DOWN),
    )

    /** Letters Space with them means something on every screen of the device. */
    val RESERVED: Set<Char> = setOf('e', 'f', 'h', 'm', 'o', 'r', 'z', 'd')
}
