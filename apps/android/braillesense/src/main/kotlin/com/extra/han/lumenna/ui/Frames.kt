package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.braille.DialogController
import dev.emassey0135.braillesense.reading.DeviceLine
import dev.emassey0135.braillesense.ui.Frame
import dev.emassey0135.braillesense.ui.Letters
import dev.emassey0135.braillesense.ui.Readings
import dev.emassey0135.braillesense.ui.Rendered
import dev.emassey0135.braillesense.ui.line
import dev.emassey0135.braillesense.ui.rendered


/**
 * A dialog the core describes (`DialogShown`), as the device works one, over the braille apps'
 * shared [DialogController]: the controller keeps the control in hand, the tab order, what is typed
 * until the core has it, each choice and check, and presses; this maps the device's keys onto it and
 * reads the control in hand in the device's own roles (`ControlRole`). Tab (Space-4-5, F3) and
 * Shift-Tab are all that move between controls, as in the device's own dialogs; Space-1 and
 * Space-4 move only within a list, Space and Backspace change a combo box or a check box, and step
 * between a question's answers. A line's own keys are the SDK's box (`BoxEditing`, run by the shell
 * ahead of the frame), its text handed to the controller as it changes.
 */
class FormFrame(
    val controller: DialogController,
    private val letters: Letters,
    private val readings: Readings,
    /** The text box of a line, which the shell keeps while the dialog is open. */
    private val boxOf: (ControlShown) -> TextBox,
    /** A button's letter: the core's, else the first free one of its name. */
    private val letterOf: (ControlShown) -> Char?,
    /** A question of answers alone: its buttons the device's prompt buttons, said as its prompt. */
    val prompt: Boolean = false,
) : Frame(controller.dialog.id) {
    /** What it is called, said as it opens. */
    val title: String? get() = controller.dialog.title.ifEmpty { null }

    /** The control in hand. */
    val current: ControlShown? get() = controller.control

    /** The lines to read ahead of where it opens (a message, why saving was refused), said as it opens. */
    val preamble: String
        get() = controller.controls.take(controller.dialog.focus.toInt()).filter { it.kind == ControlType.TEXT }.joinToString(". ") { it.value }

    /** The box of the line in hand, if a line is in hand. */
    fun box(): TextBox? = current?.takeIf(::isLine)?.let(boxOf)

    override fun editBox(): TextBox? = box()

    override fun edited() = typed()

    /** The line in hand's text, as its box now holds it, handed to the controller where it changed. */
    fun typed() {
        val line = current?.takeIf(::isLine) ?: return
        val text = boxOf(line).text
        if (text != controller.text(line)) controller.type(text)
    }

    override val rendered: Rendered
        get() {
            val control = current ?: return Rendered("")
            val reading = readings.read(lineOf(control))
            if (isLine(control)) {
                val shown = boxOf(control).line.shown()
                return reading.rendered(shown.cells, shown.cursor)
            }
            return reading.rendered()
        }

    private fun lineOf(control: ControlShown): DeviceLine {
        val hint = control.example.ifEmpty { control.hint }
        return when (control.kind) {
            ControlType.TEXT -> line(ControlRole.STATIC_BOX, control.value)
            ControlType.LINE -> boxOf(control).let { box -> line(ControlRole.EDIT_BOX, box.text.ifEmpty { hint }, box.text, label = control.label) }
            ControlType.LINES -> boxOf(control).let { box ->
                line(ControlRole.MULTI_LINE_EDIT_BOX, box.line.text.ifEmpty { if (box.text.isEmpty()) hint else "" }, box.line.text, label = control.label)
            }
            ControlType.CHOICE -> {
                val at = controller.selected(control) ?: 0
                line(ControlRole.COMBO_BOX, control.options.getOrNull(at)?.title.orEmpty(), label = control.label, position = at + 1, count = control.options.size)
            }
            ControlType.LIST -> {
                val at = controller.selected(control) ?: 0
                line(ControlRole.LIST_ITEM, control.options.getOrNull(at)?.title.orEmpty(), label = control.label, position = at + 1, count = control.options.size)
            }
            ControlType.CHECK -> line(if (controller.checked(control)) ControlRole.CHECK_BOX_CHECKED else ControlRole.CHECK_BOX_UNCHECKED, control.label)
            ControlType.BUTTON -> line(if (prompt) ControlRole.PROMPT_BUTTON else ControlRole.BUTTON, control.label, letter = letterOf(control), asks = control.asks)
        }
    }

    /** Presses the button of [letter], the line in hand taken first; false if none has it. */
    fun press(letter: Char): Boolean {
        val button = controller.controls.firstOrNull { it.kind == ControlType.BUTTON && letterOf(it) == letter } ?: return false
        typed()
        controller.press(button)
        return true
    }

    override fun routed(at: Int) {
        val control = current ?: return
        if (isLine(control)) boxOf(control).line.route(at)
        if (control.kind == ControlType.BUTTON) controller.press()
    }

    override fun key(key: Key): Boolean {
        when (key) {
            Key.Tab -> return run { typed(); controller.next(); true }
            Key.ShiftTab -> return run { typed(); controller.previous(); true }
            else -> {}
        }
        val control = current ?: return false
        return when (control.kind) {
            // Enter in a line of one presses it (quick add's, a question's) or the dialog's default;
            // a multi-line box's Enter is its own new line.
            ControlType.LINE -> key == Key.Enter && run { typed(); controller.press(); true }
            ControlType.LINES -> false
            ControlType.CHOICE -> choice(control, key)
            ControlType.LIST -> list(control, key)
            ControlType.CHECK -> when (key) {
                Key.Space, Key.Backspace, Key.Enter -> run { controller.toggle(); true }
                else -> false
            }
            ControlType.BUTTON -> when {
                key == Key.Enter -> controller.press()
                prompt && key == Key.Space -> answer(+1)
                prompt && key == Key.Backspace -> answer(-1)
                else -> false
            }
            ControlType.TEXT -> false
        }
    }

    /** Space and Backspace among a question's answers: to the next or last of them, wrapping. */
    private fun answer(by: Int): Boolean {
        val answers = controller.controls.withIndex().filter { it.value.kind == ControlType.BUTTON }.map { it.index }
        val at = answers.indexOf(controller.index)
        if (at >= 0 && answers.size > 1) controller.moveTo(answers[Math.floorMod(at + by, answers.size)])
        return true
    }

    /** A combo box steps round its options, as the device's own does, and goes to its ends. */
    private fun choice(control: ControlShown, key: Key): Boolean {
        val options = control.options
        val step = { by: Int ->
            if (options.isNotEmpty() && controller.step(by) == null) controller.choose(if (by > 0) 0 else options.lastIndex)
            true
        }
        return when (key) {
            Key.Up, Key.Backspace -> step(-1)
            Key.Down, Key.Space -> step(+1)
            Key.First -> run { if (options.isNotEmpty()) controller.choose(0); true }
            Key.Last -> run { if (options.isNotEmpty()) controller.choose(options.lastIndex); true }
            Key.Enter -> run { if (!controller.pressDefault()) controller.next(); true }
            is Key.Cell, is Key.Typed -> run { letters.letter(key)?.let { controller.find(it.toString()) }; true }
            else -> false
        }
    }

    /** A list moves in place, stopping at its ends, and Enter picks the one in hand. */
    private fun list(control: ControlShown, key: Key): Boolean = when (key) {
        Key.Up -> run { controller.step(-1); true }
        Key.Down -> run { controller.step(+1); true }
        Key.First -> run { if (control.options.isNotEmpty()) controller.choose(0); true }
        Key.Last -> run { if (control.options.isNotEmpty()) controller.choose(control.options.lastIndex); true }
        Key.Enter -> controller.press()
        is Key.Cell, is Key.Typed -> run { letters.letter(key)?.let { controller.find(it.toString()) }; true }
        else -> false
    }

    private fun isLine(control: ControlShown) = control.kind == ControlType.LINE || control.kind == ControlType.LINES
}
