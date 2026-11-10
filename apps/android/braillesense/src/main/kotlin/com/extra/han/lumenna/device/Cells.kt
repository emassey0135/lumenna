package com.extra.han.lumenna.device

/**
 * A line of braille on a display narrower than it: which window of cells is shown, and where the
 * cursor falls. Windows break at spaces, as the device's own Word Wrap does; a word longer than
 * the display is cut at its width. Nothing here touches the device, so the panning is tested on
 * the JVM.
 */
class Cells(private val width: Int) {
    private var line = ByteArray(0)
    private var cursor = -1
    private var offsets = listOf(0)
    private var window = 0

    /** Shows [cells] with the cursor at [at] (-1 for none), on the window that holds it. */
    fun set(cells: ByteArray, at: Int) {
        line = cells
        cursor = at
        offsets = wrap(cells)
        window = if (at >= 0) windowOf(at) else 0
    }

    private fun windowOf(at: Int): Int = (offsets.indexOfLast { it <= at }).coerceAtLeast(0)

    /** The window at [from], ending where the next begins or the line does. */
    private fun end(index: Int): Int = offsets.getOrElse(index + 1) { line.size }

    private fun wrap(cells: ByteArray): List<Int> = starts(cells, width)

    companion object {
        /** The offsets each window of [width] starts at: broken at the last space before the width. */
        fun starts(cells: ByteArray, width: Int): List<Int> {
            val starts = mutableListOf(0)
            var start = 0
            while (cells.size - start > width) {
                var breakAt = start + width
                var scan = breakAt - 1
                while (scan > start && cells[scan].toInt() != 0) scan--
                if (scan > start) breakAt = scan + 1
                starts += breakAt
                start = breakAt
            }
            return starts
        }

        /** Where the window of [width] that holds cell [at] of [cells] ends. */
        fun lineEnd(cells: ByteArray, at: Int, width: Int): Int =
            starts(cells, width).firstOrNull { it > at } ?: cells.size
    }

    /** Moves on a window; false at the end. */
    fun forward(): Boolean {
        if (window >= offsets.lastIndex) return false
        window++
        return true
    }

    /** Moves back a window; false at the start. */
    fun back(): Boolean {
        if (window <= 0) return false
        window--
        return true
    }

    /** Shows the last window. */
    fun toEnd() {
        window = offsets.lastIndex
    }

    /** The window's cells, as `displayRawBraille` takes them. */
    fun shown(): ByteArray {
        val start = offsets[window]
        return line.copyOfRange(start, end(window).coerceAtMost(start + width))
    }

    /** Where the cursor is in the window shown, or -1. */
    fun cursorShown(): Int {
        if (cursor < 0) return -1
        val start = offsets[window]
        return if (cursor in start until start + width) cursor - start else -1
    }

    /**
     * The cells of the window that holds the cursor, from its first to past its last (a space it
     * broke at included): the line the device's edit box calls the cursor's. Null with no cursor.
     */
    fun cursorWindow(): IntRange? {
        if (cursor < 0) return null
        val index = windowOf(cursor)
        return offsets[index] until end(index)
    }

    /** Which cell of the whole line a routing key over [cell] of the window is: -1 past the end. */
    fun routed(cell: Int): Int {
        val at = offsets[window] + cell
        return if (at <= line.size && at < offsets[window] + width) at else -1
    }
}
