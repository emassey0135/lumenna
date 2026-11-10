package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Cells
import com.extra.han.lumenna.device.Output
import com.extra.han.lumenna.device.Translator
import dev.emassey0135.lumenna.app.BraillePart

/**
 * What goes to the device: speech, and the line on the cells in the person's table, panned across
 * the display. The screens hand it what to say and show; what the service wants is here.
 */
class Speaker(private val out: Output, private val translator: Translator) {
    private var cells: Cells? = null
    private var width = 0

    /** Where an edit box's own text begins on the line shown: 0 for a line that has none. */
    private var textStart = 0

    private fun display(): Cells {
        val known = out.width
        if (cells == null || (known > 0 && known != width)) {
            width = if (known > 0) known else 40
            cells = Cells(width)
        }
        return cells!!
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

    /** Puts [rendered] on the display, no speech. */
    fun show(rendered: Rendered) {
        val display = display()
        val before = cellsOf(rendered.before)
        val after = cellsOf(rendered.after)
        val ready = rendered.cells
        if (ready != null) {
            display.set(before + ready + after, rendered.cellCursor.takeIf { it >= 0 }?.plus(before.size) ?: -1)
            textStart = before.size
        } else {
            val translated = translator.translate(rendered.braille)
            val cursor = rendered.cursor
                .takeIf { it >= 0 }
                ?.let { translated.cellOf.getOrElse(it.coerceAtMost(translated.cellOf.lastIndex.coerceAtLeast(0))) { -1 } }
                ?: -1
            display.set(before + translated.cells + after, cursor.takeIf { it >= 0 }?.plus(before.size) ?: -1)
            textStart = 0
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

    /** Which cell of the line's text a routing key over [cell] is: -1 past its end or over a label. */
    fun routed(cell: Int): Int = display().routed(cell).let { if (it < textStart) -1 else it - textStart }
}
