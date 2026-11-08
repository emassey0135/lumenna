package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Translator

/** A line as it reads on the cells. */
class ShownLine(val cells: ByteArray, val cursor: Int, val before: Int, val pending: Int)

/**
 * A single line being typed, the device's input method being out of the path: the braille keys
 * come as chords a word at a time, in the person's own table, and are read back whole when the
 * word ends, so a contracted word becomes its text. The QWERTY keyboard inserts directly.
 */
class LineEditor(initial: String, private val translator: Translator) {
    var text: String = initial
        private set

    /** Where the caret is in [text], in UTF-16 units. */
    var caret: Int = initial.length
        private set

    private val pending = ArrayList<Byte>()

    private fun set(value: String, at: Int) {
        text = value
        caret = at.coerceIn(0, value.length)
    }

    /** Ends the word being typed, reading it back into the text. */
    fun commit() {
        if (pending.isEmpty()) return
        val cells = pending.toByteArray()
        pending.clear()
        val word = translator.back(cells) ?: return
        insert(word)
    }

    private fun insert(word: String) {
        set(text.substring(0, caret) + word + text.substring(caret), caret + word.length)
    }

    /** A cell of the word being typed. */
    fun cell(dots: Int) {
        pending += dots.toByte()
    }

    /** A character from the QWERTY keyboard. */
    fun type(typed: String) {
        commit()
        insert(typed)
    }

    fun space() {
        finished = pending.isNotEmpty()
        val before = text.length
        commit()
        lastWord = if (finished) text.substring(before.coerceAtMost(text.length)) else null
        insert(" ")
    }

    private var finished = false
    private var lastWord: String? = null

    /** The word Space just ended, once. */
    fun takeFinished(): String? = lastWord.also { lastWord = null }

    fun backspace() {
        if (pending.isNotEmpty()) {
            pending.removeAt(pending.lastIndex)
            return
        }
        if (caret == 0) return
        var start = caret - 1
        if (start > 0 && text[start].isLowSurrogate() && text[start - 1].isHighSurrogate()) start--
        set(text.removeRange(start, caret), start)
    }

    fun delete() {
        commit()
        if (caret >= text.length) return
        var end = caret + 1
        if (end < text.length && text[end - 1].isHighSurrogate() && text[end].isLowSurrogate()) end++
        set(text.removeRange(caret, end), caret)
    }

    fun left() {
        commit()
        if (caret > 0) caret--
    }

    fun right() {
        commit()
        if (caret < text.length) caret++
    }

    fun start() {
        commit()
        caret = 0
    }

    fun end() {
        commit()
        caret = text.length
    }

    fun wordLeft() {
        commit()
        var at = caret
        while (at > 0 && text[at - 1] == ' ') at--
        while (at > 0 && text[at - 1] != ' ') at--
        caret = at
    }

    fun wordRight() {
        commit()
        var at = caret
        while (at < text.length && text[at] != ' ') at++
        while (at < text.length && text[at] == ' ') at++
        caret = at
    }

    /** Moves the caret to the character over cell [cell] of the whole line. */
    fun route(cell: Int) {
        commit()
        val shown = shown()
        val before = translator.translate(text.substring(0, caret))
        if (cell < before.cells.size) {
            caret = before.textAt.getOrElse(cell) { caret }.coerceIn(0, caret)
            return
        }
        val after = translator.translate(text.substring(caret))
        val intoAfter = cell - shown.before - shown.pending
        if (intoAfter in after.textAt.indices) caret += after.textAt[intoAfter]
    }

    /** The line as cells, the word being typed raw at the caret. */
    fun shown(): ShownLine {
        val before = translator.translate(text.substring(0, caret))
        val after = translator.translate(text.substring(caret))
        val typed = pending.toByteArray()
        return ShownLine(before.cells + typed + after.cells, before.cells.size + typed.size, before.cells.size, typed.size)
    }
}
