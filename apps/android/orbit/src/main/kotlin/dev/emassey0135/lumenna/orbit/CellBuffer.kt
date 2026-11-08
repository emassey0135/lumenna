package dev.emassey0135.lumenna.orbit

/**
 * What the text being typed on the Speak holds: committed text with the word being typed as
 * braille cells. Ported from the Satellite launcher's text field (Apache-2.0, the same author's).
 *
 * Each word is a buffer of cells translated as one word by Orbit's liblouis service in the
 * person's own table, contracted included: a cell at a time mode 388 for its echo, and the word
 * again in mode 132 at each Space, when it joins the text. Dot 7 takes a cell off the word, then
 * a character off the text.
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

    /** Adds [cell], typed at the caret; what it echoes: its last character, else its dots. */
    fun type(cell: Int): Echo {
        if (word.isEmpty()) wordStart = caret
        word.add(cell)
        val live = translate.translate(word, MODE_LIVE).orEmpty()
        return if (live.isEmpty()) Echo.Dots(cell) else Echo.Character(live.last())
    }

    /** Inserts [character] at [position], the caret after it: T9's commit. */
    fun insert(character: Char, position: Int) {
        val at = position.coerceIn(0, text.length)
        text.insert(at, character)
        caret = at + 1
    }

    /** Commits the word being typed, if any. */
    fun flush() {
        if (word.isEmpty()) return
        val done = translate.translate(word, MODE_COMMIT).orEmpty()
        text.insert(wordStart, done)
        caret = wordStart + done.length
        word.clear()
    }

    /** Space: the word committed, then a space after it. */
    fun space() {
        flush()
        text.insert(caret, ' ')
        caret++
    }

    /** What deleting took: a cell off the word, a character off the text, or nothing at the start. */
    fun backspace(): Deleted {
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
        var position = caret
        if (delta < 0) {
            while (position > 0 && text[position - 1] == ' ') position--
            while (position > 0 && text[position - 1] != ' ') position--
        } else {
            while (position < text.length && text[position] != ' ') position++
            while (position < text.length && text[position] == ' ') position++
        }
        if (position == caret) return null
        caret = position
        return wordAt(caret)
    }

    /** The caret to the start or the end; the word there, or null at the end of the text. */
    fun moveEdge(start: Boolean): String? {
        flush()
        caret = if (start) 0 else text.length
        return if (caret < text.length) wordAt(caret) else null
    }

    private fun wordAt(position: Int): String {
        var end = position
        while (end < text.length && text[end] != ' ') end++
        return text.substring(position, end)
    }

    sealed interface Echo {
        data class Character(val character: Char) : Echo
        data class Dots(val cell: Int) : Echo
    }

    sealed interface Deleted {
        data object Nothing : Deleted
        data class Cell(val cell: Int, val translated: String) : Deleted
        data class Character(val character: Char) : Deleted
    }

    companion object {
        const val MODE_LIVE = 388
        const val MODE_COMMIT = 132

        /** "1 2 5", the dots of [cell] as read aloud. */
        fun dotNumbers(cell: Int): String =
            (1..8).filter { cell and (1 shl (it - 1)) != 0 }.joinToString(" ")
    }
}
