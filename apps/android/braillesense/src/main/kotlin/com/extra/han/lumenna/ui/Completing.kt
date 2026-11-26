package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.edit.LineEditor
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.Candidate
import dev.emassey0135.lumenna.app.Completions
import dev.emassey0135.lumenna.app.Syntax
import dev.emassey0135.lumenna.app.candidateLine
import dev.emassey0135.lumenna.app.putIn

/**
 * Completion inside a line of a dialog that has a syntax (quick add, the filter, a question with
 * one), asked for with Space-4, which a box of one line leaves: what fits at the cursor is the
 * core's (`complete`), offered as the device's own list, and the one chosen is put in by the core
 * (`put_in`), the cursor after it. Never by itself, even with one candidate.
 *
 * The native library is absent from the JVM tests, so each of the core's parts can be given.
 */
class Completing(
    /** What could go at a UTF-8 byte offset of a line of a syntax: the app's `complete`; null where the store cannot say. */
    private val complete: (String, UInt, Syntax) -> Completions?,
    /** Offers lines under a title as the device's list: the place chosen, or null left unchosen. */
    private val offer: (title: String, lines: List<String>, chosen: (Int?) -> Unit) -> Unit,
    /** Says what the core said where nothing is offered. */
    private val say: (String) -> Unit,
    /** What the list is called where the core's announcement is empty: `quick_add.show_completions`. */
    private val showCompletions: () -> String,
    /** What is said where nothing fits and the core's announcement is empty: `quick_add.nothing_to_complete`. */
    private val nothingToComplete: () -> String,
    private val lineOf: (Candidate) -> String = ::candidateLine,
    private val splice: (String, UInt, UInt, String) -> String = ::putIn,
) {
    /**
     * What fits at [cursor] (UTF-16) in [text] of [syntax], offered; [chosen] gets the line with the
     * one chosen put in and the cursor after it (UTF-16), and nothing at all if none is chosen.
     */
    fun ask(text: String, cursor: Int, syntax: Syntax, chosen: (text: String, cursor: Int) -> Unit) {
        val found = complete(text, TextOffsets.bytes(cursor, text), syntax)
        if (found == null || found.candidates.isEmpty()) {
            return say(found?.announcement?.ifEmpty { null } ?: nothingToComplete())
        }
        offer(found.announcement.ifEmpty { showCompletions() }, found.candidates.map(lineOf)) picked@{ at ->
            val candidate = at?.let { found.candidates.getOrNull(it) } ?: return@picked
            val now = splice(text, found.start, found.end, candidate.text)
            val after = found.start + candidate.text.encodeToByteArray().size.toUInt()
            chosen(now, TextOffsets.utf16(after, now))
        }
    }

    /** [ask] at the cursor of [box], a line of one: on a choice the box is made to read the line, its caret after the choice, then [chosen] hears it. */
    fun ask(box: TextBox, syntax: Syntax, chosen: (text: String, cursor: Int) -> Unit) {
        val text = box.text
        ask(text, textCursor(text, box.line.shown().cells, box.line.caret), syntax) { now, cursor ->
            box.fill(now, cursor)
            chosen(now, cursor)
        }
    }

    companion object {
        /**
         * Where the caret (a cell of [cells]) falls in [text], the line it holds: a line of the
         * SDK's box is its words in cells, a blank cell for each space, so the caret is counted by
         * words. At a word's start it is that word's start, anywhere else in it its end, since part
         * of a word in cells has no text of its own. The text's end where the two disagree.
         */
        fun textCursor(text: String, cells: ByteArray, caret: Int): Int {
            val words = text.split(' ')
            if (words.size != cells.count { it == LineEditor.BLANK } + 1) return text.length
            val upTo = caret.coerceIn(0, cells.size)
            val index = (0 until upTo).count { cells[it] == LineEditor.BLANK }
            val atStart = upTo == 0 || cells[upTo - 1] == LineEditor.BLANK
            val start = words.take(index).sumOf { it.length + 1 }
            return if (atStart) start else start + words[index].length
        }

        /** The cell of [cells] that [cursor] (UTF-16, in [text]) falls on, counted as [textCursor] counts. */
        fun cellCursor(text: String, cursor: Int, cells: ByteArray): Int {
            val before = text.substring(0, cursor.coerceIn(0, text.length))
            val index = before.count { it == ' ' }
            val blanks = cells.indices.filter { cells[it] == LineEditor.BLANK }
            if (index > blanks.size) return cells.size
            val start = if (index == 0) 0 else blanks[index - 1] + 1
            val end = blanks.getOrNull(index) ?: cells.size
            return if (before.isEmpty() || before.endsWith(' ')) start else end
        }

        /** [box], a line of one, read as [text], its caret on [cursor] (UTF-16): typed in as the QWERTY keyboard types. */
        fun TextBox.fill(text: String, cursor: Int) {
            selectAll()
            deleteSelected()
            insert(text)
            line.route(cellCursor(text, cursor, line.shown().cells))
        }
    }
}
