package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.ui.DialogControl
import dev.emassey0135.braillesense.ui.DialogFrame
import dev.emassey0135.braillesense.ui.Letters
import dev.emassey0135.braillesense.ui.Readings
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.braille.DialogController

/**
 * A dialog the core describes (`DialogShown`), worked as the device works one by the SDK's
 * [DialogFrame], over the braille apps' shared [DialogController]: the controller keeps the control
 * in hand, what is typed until the core has it, each choice and check, and presses; this gives the
 * SDK the core's controls as the controller holds them and hands it what the person did. How the
 * device's keys work a dialog, and how a control is read, are the SDK's. A line's text is the SDK's
 * box (kept by the shell while the dialog is open), handed to the controller as it changes; Space-4,
 * which a box of one line leaves, offers what could come next at its cursor where the line has a
 * syntax ([Completing]).
 */
class FormFrame(
    val controller: DialogController,
    letters: Letters,
    readings: Readings,
    /** The text box of a line, which the shell keeps while the dialog is open. */
    private val boxOf: (ControlShown) -> TextBox,
    /** A button's letter: the core's, else the first free one of its name. */
    private val letterOf: (ControlShown) -> Char?,
    /** A question of answers alone: its buttons the device's prompt buttons, said as its prompt. */
    override val prompt: Boolean = false,
    /** Completion in a line that has a syntax (Space-4); none where the shell offers none. */
    private val completing: Completing? = null,
) : DialogFrame(controller.dialog.id, letters, readings) {
    /** What it is called, said as it opens. */
    val title: String? get() = controller.dialog.title.ifEmpty { null }

    /**
     * The lines to read ahead of its first control that takes anything (a message, why saving was
     * refused), said after its title as it opens, joined as the core joins a prompt.
     */
    val leading: List<String>
        get() = controller.controls.takeWhile { it.kind == ControlType.TEXT }.map { it.value }

    /** What a question of answers alone says as it opens: its title and [leading] as one prompt, the core's (`DialogShown.prompt`). */
    val said: String get() = controller.dialog.prompt

    /** The box of the line in hand, if a line is in hand. */
    fun box(): TextBox? = (current as? DialogControl.Line)?.box

    override val controls: List<DialogControl> get() = controller.controls.map(::control)

    override val focus: Int get() = controller.index

    /** A control as the controller holds it now, in the SDK's terms. */
    private fun control(shown: ControlShown): DialogControl = when (shown.kind) {
        ControlType.TEXT -> DialogControl.Text(shown.key, shown.value)
        ControlType.LINE, ControlType.LINES -> DialogControl.Line(shown.key, shown.label, boxOf(shown), shown.example.ifEmpty { shown.hint })
        ControlType.CHOICE -> DialogControl.Choice(shown.key, shown.label, optionLines(shown), controller.selected(shown) ?: -1, shown.options.map { it.title })
        ControlType.LIST -> DialogControl.ListBox(shown.key, shown.label, optionLines(shown), controller.selected(shown) ?: -1, shown.options.map { it.title })
        ControlType.CHECK -> DialogControl.Check(shown.key, shown.label, controller.checked(shown))
        ControlType.BUTTON -> DialogControl.Button(shown.key, shown.label, letterOf(shown), shown.asks)
    }

    /** Each option as the core says it as a row: its title, then why it is offered where it says (`option_lines`). */
    private fun optionLines(shown: ControlShown): List<String> =
        shown.options.mapIndexed { at, option -> shown.optionLines.getOrNull(at) ?: option.title }

    private fun shown(control: DialogControl): ControlShown? = controller.controls.firstOrNull { it.key == control.key }

    /** Presses the button of [letter], the line in hand taken first; false if none has it. */
    fun press(letter: Char): Boolean = pressLetter(letter)

    /** The line in hand's text, as its box now holds it, handed to the controller where it changed. */
    private fun takeTyped() {
        val line = controller.control?.takeIf { it.kind == ControlType.LINE || it.kind == ControlType.LINES } ?: return
        val text = boxOf(line).text
        if (text != controller.text(line)) controller.type(text)
    }

    override fun focusOn(at: Int) {
        takeTyped()
        controller.moveTo(at)
    }

    override fun typed(line: DialogControl.Line) = takeTyped()

    override fun chose(control: DialogControl, at: Int) = controller.choose(at)

    override fun toggled(check: DialogControl.Check) {
        controller.toggle()
    }

    override fun pressed(button: DialogControl.Button) {
        takeTyped()
        shown(button)?.let(controller::press)
    }

    override fun entered(control: DialogControl): Boolean = when (control) {
        // A line of one presses what it is for (quick add's, a question's), else the dialog's default.
        is DialogControl.Line -> true.also { takeTyped(); controller.press() }
        // A combo box presses the dialog's default, or goes on to the next control.
        is DialogControl.Choice -> controller.pressDefault()
        else -> controller.press()
    }

    override fun below(line: DialogControl.Line): Boolean {
        val shown = shown(line) ?: return false
        val syntax = shown.syntax ?: return false
        val completing = completing ?: return false
        // What could come next at the cursor; the one chosen put in by the core, its box reading the
        // line with the caret after it, and sent as typed so the core has it.
        completing.ask(line.box, syntax) { text, cursor ->
            // Still the line in hand, as it is unless a turn moved it: else it is taken as it is left.
            if (controller.control?.key == line.key) controller.type(text, cursor)
        }
        return true
    }
}
