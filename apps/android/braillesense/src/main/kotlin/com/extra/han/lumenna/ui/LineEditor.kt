package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Translator

/** A line as it reads on the cells. */
class ShownLine(val cells: ByteArray, val cursor: Int)

/**
 * A single line being typed, held as the device's own edit boxes hold it: in cells, as typed, the
 * caret a cell, so Backspace takes one cell, a routing key lands on the cell under it, and the line
 * shows what was typed. It is read as text only when something asks for [text], a word at a time,
 * and a word nobody touched keeps the text it came with, so an edit never rewords the rest of a
 * field by way of a translation there and back.
 *
 * The QWERTY keyboard types text, so its word is held as text and shown as its translation.
 */
class LineEditor(initial: String, private val translator: Translator) {
    private class Word(var cells: ByteArray, var text: String?)

    /** One blank cell between each and the next. */
    private val words = ArrayList<Word>()

    init {
        val texts = initial.split(' ')
        val parts = split(translator.translate(initial).cells)
        texts.forEachIndexed { index, text ->
            val cells = if (parts.size == texts.size) parts[index] else cellsOf(text)
            words += Word(cells, text)
        }
    }

    /** The caret, as a cell of the line. */
    var caret: Int = length
        private set

    private val length: Int get() = words.sumOf { it.cells.size } + words.size - 1

    private fun cellsOf(text: String): ByteArray = if (text.isEmpty()) ByteArray(0) else translator.translate(text).cells

    private fun split(cells: ByteArray): List<ByteArray> {
        val parts = ArrayList<ByteArray>()
        var from = 0
        cells.forEachIndexed { index, cell ->
            if (cell == BLANK) {
                parts += cells.copyOfRange(from, index)
                from = index + 1
            }
        }
        parts += cells.copyOfRange(from, cells.size)
        return parts
    }

    private fun textOf(word: Word): String =
        word.text ?: (if (word.cells.isEmpty()) "" else translator.back(word.cells).orEmpty()).also { word.text = it }

    /** The line as text: read back, for what has changed, in the person's table. */
    val text: String get() = words.joinToString(" ", transform = ::textOf)

    /** The word the caret is in, and how far into it. */
    private fun place(at: Int = caret): Pair<Int, Int> {
        var left = at
        words.forEachIndexed { index, word ->
            if (left <= word.cells.size) return index to left
            left -= word.cells.size + 1
        }
        return words.lastIndex to words.last().cells.size
    }

    private fun startOf(index: Int): Int = words.take(index).sumOf { it.cells.size + 1 }

    private val flat: ByteArray
        get() = words.foldIndexed(ByteArray(0)) { index, line, word ->
            if (index == 0) word.cells else line + BLANK + word.cells
        }

    /** A cell typed at the caret. */
    fun cell(dots: Int) {
        val (index, offset) = place()
        val word = words[index]
        word.cells = word.cells.copyOfRange(0, offset) + dots.toByte() + word.cells.copyOfRange(offset, word.cells.size)
        word.text = null
        caret++
    }

    /** A character from the QWERTY keyboard: a word's text is known, so its cells follow from it. */
    fun type(typed: String) {
        typed.forEach { char ->
            if (char == ' ') return@forEach space()
            val (index, offset) = place()
            val word = words[index]
            val before = textOf(word)
            val at = if (offset >= word.cells.size) before.length else translator.translate(before).textAt.getOrElse(offset) { before.length }
            val now = before.substring(0, at) + char + before.substring(at)
            word.text = now
            word.cells = cellsOf(now)
            caret = startOf(index) + cellsOf(now.substring(0, at + 1)).size
        }
    }

    /** The word Space just ended, once; null where it ended none. */
    private var finished: String? = null

    fun takeFinished(): String? = finished.also { finished = null }

    fun space() {
        val (index, offset) = place()
        val word = words[index]
        val left = Word(word.cells.copyOfRange(0, offset), if (offset == word.cells.size) word.text else if (offset == 0) "" else null)
        val right = Word(word.cells.copyOfRange(offset, word.cells.size), if (offset == 0) word.text else if (offset == word.cells.size) "" else null)
        words[index] = left
        words.add(index + 1, right)
        finished = left.takeIf { it.cells.isNotEmpty() }?.let(::textOf)
        caret++
    }

    /** What Backspace or Delete just removed, once: [BLANK] for a space, null for nothing. */
    private var removed: Byte? = null

    fun takeRemoved(): Byte? = removed.also { removed = null }

    private fun removeAt(at: Int): Boolean {
        val (index, offset) = place(at)
        val word = words[index]
        if (offset < word.cells.size) {
            removed = word.cells[offset]
            word.cells = word.cells.copyOfRange(0, offset) + word.cells.copyOfRange(offset + 1, word.cells.size)
            word.text = null
            return true
        }
        if (index == words.lastIndex) return false
        val next = words.removeAt(index + 1)
        removed = BLANK
        word.text = when {
            word.cells.isEmpty() -> next.text
            next.cells.isEmpty() -> word.text
            else -> null
        }
        word.cells += next.cells
        return true
    }

    /** Takes the cell before the caret. */
    fun backspace() {
        if (caret > 0 && removeAt(caret - 1)) caret--
    }

    /** Takes the cell at the caret. */
    fun delete() {
        removeAt(caret)
    }

    fun left() {
        if (caret > 0) caret--
    }

    fun right() {
        if (caret < length) caret++
    }

    fun start() {
        caret = 0
    }

    fun end() {
        caret = length
    }

    fun wordLeft() {
        val line = flat
        var at = caret
        while (at > 0 && line[at - 1] == BLANK) at--
        while (at > 0 && line[at - 1] != BLANK) at--
        caret = at
    }

    fun wordRight() {
        val line = flat
        var at = caret
        while (at < line.size && line[at] != BLANK) at++
        while (at < line.size && line[at] == BLANK) at++
        caret = at
    }

    /** Puts the caret on the cell of the line under a routing key. */
    fun route(cell: Int) {
        caret = cell.coerceIn(0, length)
    }

    /** The cell at the caret, and what comes before it in its word: for saying it. Null at the end. */
    fun cellAtCaret(): Byte? = flat.getOrNull(caret)

    /** The cells of the word the caret is in, ahead of the caret. */
    fun wordBeforeCaret(): ByteArray {
        val (index, offset) = place()
        return words[index].cells.copyOfRange(0, offset)
    }

    /** The text of the word the caret is in. */
    fun wordAtCaret(): String = textOf(words[place().first])

    /** The line as cells, exactly as typed. */
    fun shown(): ShownLine = ShownLine(flat, caret)

    companion object {
        /** A space. */
        const val BLANK: Byte = 0
    }
}
