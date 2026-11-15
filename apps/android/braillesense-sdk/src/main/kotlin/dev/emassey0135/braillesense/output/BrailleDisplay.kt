package dev.emassey0135.braillesense.output

import dev.emassey0135.braillesense.output.Cells
import dev.emassey0135.braillesense.output.Output
import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.braillesense.reading.BraillePart

/**
 * A line for the display: [text] in the person's table, its cursor at [cursor] (a character of
 * it, -1 for none), or an edit box's own [cells] with the cursor at [cellCursor]; between what
 * goes [before] it (a role, a label) and [after] it (a letter, a sign, a role).
 */
class DisplayLine(
    val text: String,
    val cursor: Int = -1,
    val cells: ByteArray? = null,
    val cellCursor: Int = -1,
    val before: List<BraillePart> = emptyList(),
    val after: List<BraillePart> = emptyList(),
)

/**
 * What goes to the device: speech, and the line on the cells in the person's table, panned across
 * the display as its Word Wrap does. An app hands it what to say and show; what the service wants
 * is here.
 */
class BrailleDisplay(private val out: Output, private val translator: Translator) {
    private var cells: Cells? = null
    private var width = 0

    /** Where an edit box's own text begins on the line shown: 0 for a line that has none. */
    private var textStart = 0

    /** Where it ends: what follows (the role after it) is not its. */
    private var textEnd = 0

    private fun display(): Cells {
        val known = out.width
        if (cells == null || (known > 0 && known != width)) {
            width = if (known > 0) known else 40
            cells = Cells(width)
        }
        return cells!!
    }

    /** How many cells the display has: 40 until the service says. */
    fun cellsWide(): Int {
        display()
        return width
    }

    fun say(text: String) {
        if (text.isNotBlank()) out.say(text)
    }

    fun silence() = out.silence()

    private var selecting = false

    /** Shows a selection under way, as the device's edit box does, by its cursor. */
    fun selecting(on: Boolean) {
        if (on == selecting) return
        selecting = on
        out.selecting(on)
    }

    private fun cellsOf(parts: List<BraillePart>): ByteArray =
        parts.fold(ByteArray(0)) { all, part ->
            all + if (part.computer) translator.computer(part.text) else translator.translate(part.text).cells
        }

    /** Puts [line] on the display, no speech. */
    fun show(line: DisplayLine) {
        val display = display()
        val before = cellsOf(line.before)
        val after = cellsOf(line.after)
        val ready = line.cells
        if (ready != null) {
            display.set(before + ready + after, line.cellCursor.takeIf { it >= 0 }?.plus(before.size) ?: -1)
            textStart = before.size
            textEnd = before.size + ready.size
        } else {
            val translated = translator.translate(line.text)
            val cursor = line.cursor
                .takeIf { it >= 0 }
                ?.let { translated.cellOf.getOrElse(it.coerceAtMost(translated.cellOf.lastIndex.coerceAtLeast(0))) { -1 } }
                ?: -1
            display.set(before + translated.cells + after, cursor.takeIf { it >= 0 }?.plus(before.size) ?: -1)
            textStart = 0
            textEnd = 0
        }
        push(display)
    }

    private fun push(display: Cells) {
        out.cells(display.shown(), display.cursorShown())
    }

    /** Pans on; false when the line ended here. */
    fun scrollOn(): Boolean = display().let { d -> d.forward().also { if (it) push(d) } }

    /** Pans back; false at the start of the line. */
    fun scrollBack(): Boolean = display().let { d -> d.back().also { if (it) push(d) } }

    /**
     * The cells of an edit box's text on the display line that holds its cursor, as the device's
     * Backspace-1-4 takes them: the label and role beside it are not the text's. Null where the line
     * shown is no edit box's.
     */
    fun cursorLine(): IntRange? {
        val window = display().cursorWindow() ?: return null
        val length = textEnd - textStart
        val from = (window.first - textStart).coerceIn(0, length)
        val to = (window.last + 1 - textStart).coerceIn(0, length)
        return from until to
    }

    /** Which cell of the line's text a routing key over [cell] is: -1 past its end or over a label. */
    fun routed(cell: Int): Int = display().routed(cell).let { if (it < textStart) -1 else it - textStart }
}
