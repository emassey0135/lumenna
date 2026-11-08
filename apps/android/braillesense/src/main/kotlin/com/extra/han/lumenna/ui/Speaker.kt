package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Cells
import com.extra.han.lumenna.device.Output
import com.extra.han.lumenna.device.Translator

/**
 * What goes to the device: speech, and the line on the cells in the person's table, panned across
 * the display. The screens hand it what to say and show; what the service wants is here.
 */
class Speaker(private val out: Output, private val translator: Translator) {
    private var cells: Cells? = null
    private var width = 0

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

    /** Puts [rendered] on the display, no speech. */
    fun show(rendered: Rendered) {
        val display = display()
        val ready = rendered.cells
        if (ready != null) {
            display.set(ready, rendered.cellCursor)
        } else {
            val translated = translator.translate(rendered.braille)
            val cursor = rendered.cursor
                .takeIf { it >= 0 }
                ?.let { translated.cellOf.getOrElse(it.coerceAtMost(translated.cellOf.lastIndex.coerceAtLeast(0))) { -1 } }
                ?: -1
            display.set(translated.cells, cursor)
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

    /** Which cell of the whole line a routing key over [cell] is: -1 past its end. */
    fun routed(cell: Int): Int = display().routed(cell)
}
