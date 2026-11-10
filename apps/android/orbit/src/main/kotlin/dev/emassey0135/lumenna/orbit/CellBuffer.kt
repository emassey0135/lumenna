package dev.emassey0135.lumenna.orbit

/**
 * What the text being typed on the Speak holds: committed text with the word being typed as
 * braille cells. Ported from the Satellite launcher's text field (Apache-2.0, the same author's).
 *
 * Each word is a buffer of cells translated as one word by Orbit's liblouis service in the
 * person's own table, contracted included: a cell at a time mode 388 for its echo, and the word
 * again in mode 132 at each Space, with the blank cell after it as Orbit's own fields translate it,
 * when it joins the text. Dot 7 takes a cell off the word, then a character off the text.
 *
 * A selection is held as Orbit's Editor holds one: from where selection mode began to the
 * character at the caret as it ends, or the whole text. Typing, deleting or moving clears a
 * finished one.
 */
class CellBuffer(private val translate: Translator) {
    /** Orbit's liblouis service, in the person's table; null when it cannot say. */
    fun interface Translator {
        fun translate(cells: List<Int>, mode: Int): String?
    }

    private val text = StringBuilder()
    private val word = mutableListOf<Int>()

    /** The caret's place in the committed text; the word being typed goes in at [wordStart]. */
    var caret = 0
        private set
    private var wordStart = 0

    /** The text with the word being typed shown at its insertion point. */
    fun display(): String {
        if (word.isEmpty()) return text.toString()
        val live = translate.translate(word, MODE_LIVE).orEmpty()
        return text.substring(0, wordStart) + live + text.substring(wordStart)
    }

    /** The text as it stands, the word being typed committed: what is sent. */
    fun confirmed(): String {
        flush()
        return text.toString()
    }

    /** Starts from [initial], the caret at its end. */
    fun set(initial: String) {
        text.setLength(0)
        text.append(initial)
        word.clear()
        wordStart = initial.length
        caret = initial.length
    }

    /** Adds [cell], typed at the caret: the word's cells so far, and what they make so far. */
    fun type(cell: Int): Typed {
        clearSelection()
        if (word.isEmpty()) wordStart = caret
        word.add(cell)
        return Typed(word.toList(), word.size - 1, translate.translate(word, MODE_LIVE).orEmpty())
    }

    /** Inserts [character] at [position], the caret after it: T9's commit. */
    fun insert(character: Char, position: Int) {
        val at = position.coerceIn(0, text.length)
        text.insert(at, character)
        caret = at + 1
    }

    /** Pastes [pasted] at the caret, over the selection if there is one, the caret after it. */
    fun paste(pasted: String) {
        flush()
        extended = false
        selected()?.let { deleteSelection() }
        text.insert(caret, pasted)
        caret += pasted.length
        clearSelection()
    }

    /** Commits the word being typed, if any. */
    fun flush() {
        if (word.isEmpty()) return
        val done = translate.translate(word, MODE_COMMIT).orEmpty()
        text.insert(wordStart, done)
        caret = wordStart + done.length
        word.clear()
    }

    /**
     * Space: the word committed as Orbit's fields commit it, its cells and the blank after them
     * translated together, then a space after it. The word as it reads in print, or "" for none.
     */
    fun space(): String {
        clearSelection()
        if (word.isEmpty()) {
            text.insert(caret, ' ')
            caret++
            return ""
        }
        val done = translate.translate(word + BLANK, MODE_COMMIT).orEmpty()
        val spaced = if (done.lastOrNull()?.isWhitespace() == true) done else "$done "
        text.insert(wordStart, spaced)
        caret = wordStart + spaced.length
        word.clear()
        return done.trim()
    }

    /** A new line in a field of several: the word committed, then the line ended. */
    fun newLine() {
        clearSelection()
        flush()
        text.insert(caret, '\n')
        caret++
    }

    /** The line the caret is on, without its end. */
    fun line(): String {
        flush()
        return text.substring(lineStart(caret), lineEnd(caret))
    }

    /** The caret to the line before or after, at the same column or the line's end; the line, or null at an end. */
    fun moveLine(delta: Int): String? {
        flush()
        clearSelection()
        val start = lineStart(caret)
        val column = caret - start
        val target = if (delta < 0) {
            if (start == 0) return null
            lineStart(start - 1)
        } else {
            val end = lineEnd(caret)
            if (end >= text.length) return null
            end + 1
        }
        caret = (target + column).coerceAtMost(lineEnd(target))
        return text.substring(target, lineEnd(target))
    }

    /** The caret to the start or end of its line; the line. */
    fun moveLineEdge(start: Boolean): String {
        flush()
        clearSelection()
        caret = if (start) lineStart(caret) else lineEnd(caret)
        return text.substring(lineStart(caret), lineEnd(caret))
    }

    private fun lineStart(position: Int): Int = text.lastIndexOf("\n", position - 1).let { if (it < 0) 0 else it + 1 }

    private fun lineEnd(position: Int): Int = text.indexOf("\n", position).let { if (it < 0) text.length else it }

    /** What deleting took: a cell off the word, a character off the text, or nothing at the start. */
    fun backspace(): Deleted {
        clearSelection()
        if (word.isNotEmpty()) {
            val cell = word.removeAt(word.size - 1)
            return Deleted.Cell(cell, translate.translate(listOf(cell), MODE_LIVE).orEmpty())
        }
        if (caret <= 0) return Deleted.Nothing
        val character = text[caret - 1]
        text.deleteCharAt(caret - 1)
        caret--
        return Deleted.Character(character)
    }

    /** Moves the caret by a character; the character reached, or null at an end. */
    fun moveCharacter(delta: Int): Char? {
        flush()
        clearSelection()
        return if (delta < 0) {
            if (caret <= 0) return null
            caret--
            text[caret]
        } else {
            if (caret >= text.length) return null
            caret++
            text[caret - 1]
        }
    }

    /** Moves the caret by a word; the word reached, or null at an end. */
    fun moveWord(delta: Int): String? {
        flush()
        clearSelection()
        var position = caret
        if (delta < 0) {
            while (position > 0 && text[position - 1].isWhitespace()) position--
            while (position > 0 && !text[position - 1].isWhitespace()) position--
        } else {
            while (position < text.length && !text[position].isWhitespace()) position++
            while (position < text.length && text[position].isWhitespace()) position++
        }
        if (position == caret) return null
        caret = position
        return wordAt(caret)
    }

    /** The caret to the start or the end; the word there, or null at the end of the text. */
    fun moveEdge(start: Boolean): String? {
        flush()
        clearSelection()
        caret = if (start) 0 else text.length
        return if (caret < text.length) wordAt(caret) else null
    }

    // ---- the selection ----

    private var selectionStart = -1
    private var selectionEnd = -1

    /** Whether selection mode is on: the selection's start is held, its end not yet chosen. */
    var extended = false
        private set

    /** Everything selected. */
    fun selectAll() {
        flush()
        extended = false
        selectionStart = 0
        selectionEnd = text.length
    }

    /** Selection mode on, from the caret; or off, to the character at the caret. Whether it is on now. */
    fun toggleSelection(): Boolean {
        flush()
        if (!extended) {
            extended = true
            selectionStart = caret
            selectionEnd = -1
        } else {
            extended = false
            selectionEnd = (caret + 1).coerceAtMost(text.length)
        }
        return extended
    }

    /** What is selected, or null for nothing. */
    fun selected(): String? {
        flush()
        if (selectionStart < 0 || selectionEnd < 0 || selectionStart == selectionEnd) return null
        return text.substring(minOf(selectionStart, selectionEnd), maxOf(selectionStart, selectionEnd))
    }

    /** Takes the selection out of the text, the caret where it began. */
    fun deleteSelection() {
        if (selected() == null) return
        val from = minOf(selectionStart, selectionEnd)
        text.delete(from, maxOf(selectionStart, selectionEnd))
        caret = from
        extended = false
        selectionStart = -1
        selectionEnd = -1
    }

    private fun clearSelection() {
        if (extended) return
        selectionStart = -1
        selectionEnd = -1
    }

    private fun wordAt(position: Int): String {
        var end = position
        while (end < text.length && !text[end].isWhitespace()) end++
        return text.substring(position, end)
    }

    /** A cell typed: the [cells] of its word so far, it [at] the last, and what they make so far ([live]). */
    data class Typed(val cells: List<Int>, val at: Int, val live: String)

    sealed interface Deleted {
        data object Nothing : Deleted
        data class Cell(val cell: Int, val translated: String) : Deleted
        data class Character(val character: Char) : Deleted
    }

    companion object {
        const val MODE_LIVE = 388
        const val MODE_COMMIT = 132
        /** The blank cell Space types, ending the word. */
        const val BLANK = 0

        /** "1 2 5", the dots of [cell] as read aloud. */
        fun dotNumbers(cell: Int): String =
            (1..8).filter { cell and (1 shl (it - 1)) != 0 }.joinToString(" ")
    }
}
