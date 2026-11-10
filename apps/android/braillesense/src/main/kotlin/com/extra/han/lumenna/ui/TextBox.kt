package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Cells
import com.extra.han.lumenna.device.Translator

/** Where in a box: which line, and which cell of it. */
data class Spot(val row: Int, val cell: Int) : Comparable<Spot> {
    override fun compareTo(other: Spot): Int = compareValuesBy(this, other, Spot::row, Spot::cell)
}

/**
 * The text of an edit box as the device's own holds it: a [LineEditor] for each line, in cells as
 * typed, and the line the cursor is on. A single-line box has one line, and a new line typed or
 * pasted into it is a space; a multi-line one ([multiLine]) splits at Enter and joins again at
 * Backspace, and moves between its lines with Space-1 and Space-4.
 *
 * A selection is the device's: started at the cursor (Enter-B) and running to wherever the cursor
 * goes, or the whole box (Enter-A). Typing ends it; Backspace takes what it holds.
 */
class TextBox(initial: String, private val translator: Translator, val multiLine: Boolean = false) {
    private val lines = ArrayList<LineEditor>().apply {
        val texts = if (multiLine) initial.split('\n') else listOf(initial.replace('\n', ' '))
        texts.forEach { add(LineEditor(it, translator)) }
    }

    /** The line the cursor is on. */
    var row: Int = lines.lastIndex
        private set

    /** The line the cursor is on, whose cells the display shows. */
    val line: LineEditor get() = lines[row]

    /** How many lines it has. */
    val count: Int get() = lines.size

    /** The whole text, its lines joined by new lines. */
    val text: String get() = lines.joinToString("\n") { it.text }

    private val here: Spot get() = Spot(row, line.caret)

    /** Where a selection began, while one is under way. */
    private var anchor: Spot? = null

    /** Whether everything is selected. */
    private var all = false

    /** Whether something is selected. */
    val selecting: Boolean get() = all || anchor != null

    // ── Lines ─────────────────────────────────────────────────────────────────────────────────

    /** The word Enter just finished, once: the device's echo says it, as Space does. */
    private var finished: String? = null

    fun takeFinished(): String? = finished.also { finished = null }

    /** Enter: the line splits at the cursor, which goes to the start of the new one. */
    fun newLine() {
        val before = line.cellBeforeCaret()
        finished = if (before != null && before.cell != LineEditor.BLANK) line.wordAtCaret().takeIf { it.isNotEmpty() } else null
        lines.add(row + 1, line.split())
        row++
    }

    /** Whether Backspace just joined a line onto the one before, once: the device says "previous line". */
    private var joined = false

    fun takeJoined(): Boolean = joined.also { joined = false }

    /** Backspace: a cell, or at a line's start the line break, joining it to the line before. True when it joined. */
    fun backspace(): Boolean {
        if (line.caret == 0 && row > 0) {
            val next = lines.removeAt(row)
            row--
            line.append(next)
            joined = true
            return true
        }
        line.backspace()
        return false
    }

    /** Delete: the cell at the cursor, or at a line's end the break, joining the next line on. */
    fun delete(): Boolean {
        if (line.caret == line.size && row < lines.lastIndex) {
            line.append(lines.removeAt(row + 1))
            return true
        }
        line.delete()
        return false
    }

    /** What Backspace-2-5 took: a word or a space, or at a line's end the break to the next. */
    sealed interface WordRemoved {
        data class Word(val text: String) : WordRemoved
        data object Space : WordRemoved
        data object LineBreak : WordRemoved
    }

    /**
     * Backspace-2-5: the word at the cursor and the spaces after it, as the device's edit box takes
     * it. At the end of a line with another after it, the break, joining the next line on, which the
     * device calls "new paragraph". Null where there is nothing to take.
     */
    fun deleteWord(): WordRemoved? {
        cancelSelection()
        if (line.caret == line.size && row < lines.lastIndex) {
            line.append(lines.removeAt(row + 1))
            return WordRemoved.LineBreak
        }
        return when (val gone = line.deleteWord()) {
            is LineEditor.WordRemoved.Word -> WordRemoved.Word(gone.text)
            LineEditor.WordRemoved.Space -> WordRemoved.Space
            null -> null
        }
    }

    /**
     * Backspace-1-4: the cells [from] up to [to] of the cursor's line, the part of it shown at the
     * cursor. A line left empty in a multi-line box goes too, the next taking its place, or at the
     * end the cursor going back to the end of the line before. False where there is nothing to take.
     */
    fun deleteLine(from: Int, to: Int): Boolean {
        cancelSelection()
        if (line.size == 0 && lines.size == 1) return false
        if (line.size > 0) line.removeRange(from, to)
        if (line.size == 0 && lines.size > 1) {
            lines.removeAt(row)
            if (row > lines.lastIndex) {
                row = lines.lastIndex
                line.end()
            } else {
                line.start()
            }
        }
        return true
    }

    /** Backspace-1-3-4-6: the cursor's whole line, the cursor at the start of the one after. False where there is nothing. */
    fun deleteParagraph(): Boolean {
        cancelSelection()
        if (lines.size == 1) {
            if (line.size == 0) return false
            lines[0] = LineEditor("", translator)
            return true
        }
        lines.removeAt(row)
        row = row.coerceAtMost(lines.lastIndex)
        line.start()
        return true
    }

    /** To another line, the cursor as far along as it can be; false where there is none. */
    private fun toRow(to: Int): Boolean {
        if (to !in lines.indices || to == row) return false
        val along = line.caret
        row = to
        line.route(along)
        return true
    }

    fun up(): Boolean = toRow(row - 1)

    fun down(): Boolean = toRow(row + 1)

    /** The start of the first line. */
    fun top() {
        row = 0
        line.start()
    }

    /** The end of the last line. */
    fun bottom() {
        row = lines.lastIndex
        line.end()
    }

    // ── The selection ─────────────────────────────────────────────────────────────────────────

    /** Enter-B: a selection from the cursor, to wherever it goes. */
    fun startSelection() {
        all = false
        anchor = here
    }

    /** Enter-A. */
    fun selectAll() {
        anchor = null
        all = true
    }

    fun cancelSelection() {
        anchor = null
        all = false
    }

    private fun range(): Pair<Spot, Spot>? = when {
        all -> Spot(0, 0) to Spot(lines.lastIndex, lines.last().size)
        else -> anchor?.let { from -> if (from <= here) from to here else here to from }
    }

    /** What the selection holds, as text; null with nothing selected. */
    fun selected(): String? {
        val (from, to) = range() ?: return null
        if (from.row == to.row) return lines[from.row].textBetween(from.cell, to.cell)
        return (from.row..to.row).joinToString("\n") { index ->
            val each = lines[index]
            when (index) {
                from.row -> each.textBetween(from.cell, each.size)
                to.row -> each.textBetween(0, to.cell)
                else -> each.text
            }
        }
    }

    /**
     * Enter-U, the device's "read beginning of selected text": a selection within one line is read
     * whole, one across lines from where it begins to the end of that line as a display of [width]
     * cells breaks it. Null with nothing selected.
     */
    fun selectionStart(width: Int): String? {
        val (from, to) = range() ?: return null
        val first = lines[from.row]
        if (from.row == to.row) return first.textBetween(from.cell, to.cell)
        val end = Cells.lineEnd(first.shown().cells, from.cell, width)
        return first.textBetween(from.cell, end)
    }

    /** Takes away what is selected, the cursor where it began; false with nothing selected. */
    fun deleteSelected(): Boolean {
        val (from, to) = range() ?: return false
        cancelSelection()
        row = to.row
        line.route(to.cell)
        while (here > from) backspace()
        joined = false
        line.takeRemoved()
        return true
    }

    /** Puts [pasted] in at the cursor, a new line where it has one and the box takes them. */
    fun insert(pasted: String) {
        cancelSelection()
        val parts = if (multiLine) pasted.replace("\r\n", "\n").split('\n') else listOf(pasted.replace(Regex("[\r\n]+"), " "))
        parts.forEachIndexed { index, part ->
            if (index > 0) newLine()
            if (part.isNotEmpty()) line.type(part)
        }
        finished = null
    }

    // ── The table ─────────────────────────────────────────────────────────────────────────────

    /** Reads every line as text in the table in force, before it changes. */
    fun settle() = lines.forEach { it.settle() }

    /** Writes every line again in the table now in force: the text stays, its cells follow the table. */
    fun recell() = lines.forEach { it.recell() }
}
