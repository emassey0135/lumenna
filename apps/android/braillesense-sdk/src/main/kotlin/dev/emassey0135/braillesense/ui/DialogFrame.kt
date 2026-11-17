package dev.emassey0135.braillesense.ui

import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.DeviceLine

/**
 * A form as the device's own dialogs work one: its [Field]s, then OK and Cancel. Tab (Space-4-5,
 * F3) and Shift-Tab are all that move between controls, going round; a line's keys are its edit
 * box's (`BoxEditing`, run ahead of the frame), Enter in a single line pressing OK; Space and
 * Backspace go round a combo box's options, Space-1-2-3 and Space-4-5-6 to its ends, and a letter
 * to the next option starting with it; Space, Backspace or Enter turn a check box over; Enter
 * presses a button, and Space or Enter with a button's letter presses it from anywhere. Each
 * control is read in the device's role, with its label, its value and, for a combo box, "2/5".
 */
public class DialogFrame(
    id: String,
    /** The dialog's name, said with "dialog" as it opens. */
    public val title: String,
    fields: List<Field>,
    translator: Translator,
    private val words: UiWords,
    private val letters: Letters,
    private val readings: Readings,
    /** OK, with what the form holds, or Cancel (Escape too), with null. */
    private val done: (FormValues?) -> Unit,
) : Frame(id) {
    private sealed interface Control {
        class Line(val field: Field.Line, val box: TextBox) : Control
        class Choice(val field: Field.Choice, var selected: Int) : Control
        class Check(val field: Field.Check, var checked: Boolean) : Control
        class Text(val field: Field.Text) : Control
        class Button(val label: String, val ok: Boolean) : Control
    }

    private val controls: List<Control> = fields.map { field ->
        when (field) {
            is Field.Line -> Control.Line(field, TextBox(field.value, translator, field.multiLine))
            is Field.Choice -> Control.Choice(field, field.selected.coerceIn(0, (field.options.size - 1).coerceAtLeast(0)))
            is Field.Check -> Control.Check(field, field.checked)
            is Field.Text -> Control.Text(field)
        }
    } + listOf(Control.Button(words.ok, true), Control.Button(words.cancel, false))

    /** The buttons' letters, as a menu's: the first free one of each name. */
    private val buttonLetters: Map<Control.Button, Char> = controls.filterIsInstance<Control.Button>().let { buttons ->
        buttons.zip(letters.shortcuts(buttons.map { it.label to null })).mapNotNull { (button, letter) -> letter?.let { button to it } }.toMap()
    }

    private var index = 0
    private var finished = false

    private val current: Control get() = controls[index]

    override fun editBox(): TextBox? = (current as? Control.Line)?.box

    override fun editBoxes(): List<TextBox> = controls.filterIsInstance<Control.Line>().map { it.box }

    override val rendered: Rendered
        get() {
            val control = current
            val reading = readings.read(lineOf(control))
            if (control is Control.Line) {
                val shown = control.box.line.shown()
                return reading.rendered(shown.cells, shown.cursor)
            }
            return reading.rendered()
        }

    private fun lineOf(control: Control): DeviceLine = when (control) {
        is Control.Line -> {
            val box = control.box
            if (box.multiLine) line(ControlRole.MULTI_LINE_EDIT_BOX, box.line.text.ifEmpty { if (box.text.isEmpty()) control.field.hint else "" }, box.line.text, label = control.field.label)
            else line(ControlRole.EDIT_BOX, box.text.ifEmpty { control.field.hint }, box.text, label = control.field.label)
        }
        is Control.Choice -> line(
            ControlRole.COMBO_BOX, control.field.options.getOrNull(control.selected).orEmpty(), label = control.field.label,
            position = control.selected + 1, count = control.field.options.size,
        )
        is Control.Check -> line(if (control.checked) ControlRole.CHECK_BOX_CHECKED else ControlRole.CHECK_BOX_UNCHECKED, control.field.label)
        is Control.Text -> line(ControlRole.STATIC_BOX, control.field.value)
        is Control.Button -> line(ControlRole.BUTTON, control.label, letter = buttonLetters[control])
    }

    /** What the form holds now. */
    public fun values(): FormValues {
        val texts = HashMap<String, String>()
        val choices = HashMap<String, Int>()
        val checks = HashMap<String, Boolean>()
        controls.forEach { control ->
            when (control) {
                is Control.Line -> texts[control.field.key] = control.box.text
                is Control.Choice -> choices[control.field.key] = control.selected
                is Control.Check -> checks[control.field.key] = control.checked
                is Control.Text, is Control.Button -> {}
            }
        }
        return FormValues(texts, choices, checks)
    }

    /** OK or Cancel, once. */
    private fun finish(ok: Boolean) {
        if (finished) return
        finished = true
        done(if (ok) values() else null)
    }

    /** Escape: as its Cancel. */
    public fun cancel(): Unit = finish(false)

    override fun routed(at: Int) {
        when (val control = current) {
            is Control.Line -> control.box.line.route(at)
            is Control.Button -> finish(control.ok)
            else -> {}
        }
    }

    override fun key(key: Key): Boolean {
        when (key) {
            Key.Tab -> return move(+1)
            Key.ShiftTab -> return move(-1)
            Key.Escape -> return true.also { cancel() }
            is Key.SpaceLetter -> return press(key.letter)
            is Key.EnterLetter -> return press(key.letter)
            is Key.SpaceDots -> return letters.letter(Key.Cell(key.dots))?.let(::press) ?: false
            is Key.EnterDots -> return letters.letter(Key.Cell(key.dots))?.let(::press) ?: false
            else -> {}
        }
        return when (val control = current) {
            is Control.Line -> !control.box.multiLine && key == Key.Enter && true.also { finish(true) }
            is Control.Choice -> choice(control, key)
            is Control.Check -> when (key) {
                Key.Space, Key.Backspace, Key.Enter -> true.also { control.checked = !control.checked }
                else -> false
            }
            is Control.Button -> key == Key.Enter && true.also { finish(control.ok) }
            is Control.Text -> false
        }
    }

    private fun move(by: Int): Boolean {
        index = Math.floorMod(index + by, controls.size)
        return true
    }

    /** The button of [letter], pressed; false where none has it. */
    private fun press(letter: Char): Boolean {
        val button = buttonLetters.entries.firstOrNull { it.value == letter.lowercaseChar() }?.key ?: return false
        finish(button.ok)
        return true
    }

    /** A combo box goes round its options, as the device's own does, and to its ends. */
    private fun choice(control: Control.Choice, key: Key): Boolean {
        val count = control.field.options.size
        if (count == 0) return false
        when (key) {
            Key.Up, Key.Backspace -> control.selected = Math.floorMod(control.selected - 1, count)
            Key.Down, Key.Space -> control.selected = Math.floorMod(control.selected + 1, count)
            Key.First -> control.selected = 0
            Key.Last -> control.selected = count - 1
            Key.Enter -> finish(true)
            is Key.Cell, is Key.Typed -> letters.letter(key)?.let { letter ->
                letters.find(control.field.options, control.selected, letter)?.let { control.selected = it }
            }
            else -> return false
        }
        return true
    }
}
