package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.BrailleLetters
import dev.emassey0135.orbit.EditBox
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.ListFinder
import dev.emassey0135.orbit.OrbitDialog
import dev.emassey0135.orbit.OrbitReader

/** A control of a [DialogScreen], holding what the person made of it. */
internal sealed class Control(val key: String, val label: String) {
    class Edit(val field: Field.Line) : Control(field.key, field.label) {
        var value = field.value
        var editor: EditBox? = null
        var editorPhone = false
    }

    class Combo(val field: Field.Choice) : Control(field.key, field.label) {
        var selected = field.selected.coerceIn(0, (field.options.size - 1).coerceAtLeast(0))
    }

    class Check(field: Field.Check) : Control(field.key, field.label) {
        var checked = field.checked
    }

    class Text(val field: Field.Text) : Control(field.key, "")

    class Button(key: String, label: String, val press: () -> Unit) : Control(key, label)

    companion object {
        fun of(field: Field): Control = when (field) {
            is Field.Line -> Edit(field)
            is Field.Choice -> Combo(field)
            is Field.Check -> Check(field)
            is Field.Text -> Text(field)
        }
    }
}

/**
 * A dialog as Orbit's own are, the Editor's Save As: on the Speak, told as a dialog, then each
 * control reached in its role, Tab and Shift-Tab going round them and each key doing what
 * [OrbitDialog.does] says. The Player has no Tab, so there its controls are the rows of a list, as
 * Lumenna's dialogs are: a line or a choice opens a screen of its own, a check turns over, a button
 * presses. Which the person has is the last key's, so the same dialog serves both.
 *
 * Select in a line of one presses [default]; Escape, Dot 7 off a line and Back close it. [closing]
 * is called once as it closes, whatever closed it.
 */
internal class DialogScreen(
    title: String,
    private val controls: List<Control>,
    start: Int,
    private val default: Control.Button?,
    private val closing: () -> Unit,
) : ListScreen(title, { emptyList() }, "") {
    private var focus = start.coerceIn(0, (controls.size - 1).coerceAtLeast(0))
    private var announced = false

    init {
        controls.getOrNull(focus)?.let { hold(it.key) }
    }

    /** What the form holds now: a line as typed, its word in progress taken in. */
    fun values(): FormValues = FormValues(
        controls.filterIsInstance<Control.Edit>().associate { it.key to valueOf(it) },
        controls.filterIsInstance<Control.Combo>().associate { it.key to it.selected },
        controls.filterIsInstance<Control.Check>().associate { it.key to it.checked },
    )

    private fun valueOf(edit: Control.Edit): String = edit.editor?.text() ?: edit.value

    // ---- the Player: the controls as rows ----------------------------------------------

    override fun items(): List<Item> = controls.map { Item(it.key, rowText(it), rowOpen(it)) }

    private fun rowText(control: Control): String = when (control) {
        is Control.Edit -> joined(control.label, valueOf(control).ifEmpty { ui.orbitWords.blank })
        is Control.Combo -> joined(control.label, control.field.options.getOrNull(control.selected).orEmpty())
        is Control.Check -> joined(control.label, if (control.checked) ui.words.checked else ui.words.notChecked)
        is Control.Text -> control.field.value
        is Control.Button -> control.label
    }

    private fun joined(label: String, value: String): String = listOf(label, value).filter { it.isNotEmpty() }.joinToString(", ")

    private fun rowOpen(control: Control): () -> Unit = when (control) {
        is Control.Edit -> {
            {
                ui.askLine(control.label, valueOf(control), control.field.hint, control.field.multiLine) { typed ->
                    if (typed != null) {
                        control.value = typed
                        control.editor = null
                    }
                }
            }
        }
        is Control.Combo -> {
            { ui.choose(control.label, control.field.options, control.selected) { at -> if (at != null) control.selected = at } }
        }
        is Control.Check -> {
            {
                control.checked = !control.checked
                rebuild()
                tellMoved()
            }
        }
        is Control.Text -> {
            {}
        }
        is Control.Button -> control.press
    }

    // ---- what Orbit is told ---------------------------------------------------------------

    /** The dialog is told once, as it opens; coming back to it, only what is in hand. */
    override fun tellShown() {
        if (ui.phoneKeypad) {
            rebuild()
            if (!announced) reader.dialog(title, "", current?.text.orEmpty())
            announced = true
            tellMoved()
        } else {
            if (!announced) reader.dialog(title, "", "")
            announced = true
            tellControl()
        }
    }

    override fun refresh(tell: Boolean) {
        if (ui.phoneKeypad) super.refresh(tell)
    }

    private fun tellControl() {
        when (val control = controls.getOrNull(focus) ?: return) {
            is Control.Edit -> {
                ui.braille?.follow()
                editorOf(control).focus()
                val hint = control.field.hint
                if (hint.isNotEmpty()) ui.afterTelling { if (ui.top === this && controls.getOrNull(focus) === control) reader.speakQueued(hint) }
            }
            is Control.Combo -> reader.focusChoice(
                control.label, OrbitReader.ROLE_COMBO_BOX, control.field.options.size, control.selected,
                control.field.options.getOrNull(control.selected).orEmpty(),
            )
            is Control.Check -> reader.focusCheck(control.label, control.checked)
            is Control.Button -> reader.focusControl(control.label, OrbitReader.ROLE_BUTTON)
            is Control.Text -> reader.focusText(control.field.value)
        }
    }

    // ---- the Speak: the controls in place ------------------------------------------------

    override fun key(key: Key): Boolean {
        if (ui.phoneKeypad) return super.key(key)
        val control = controls.getOrNull(focus) ?: return false
        when (val does = OrbitDialog.does(key, kindOf(control))) {
            OrbitDialog.Does.Next -> moveFocus(1)
            OrbitDialog.Does.Previous -> moveFocus(-1)
            OrbitDialog.Does.Cancel -> ui.close(this)
            // A choice holds what it was moved to already: Select presses only a button.
            OrbitDialog.Does.Press -> if (control is Control.Button) control.press()
            OrbitDialog.Does.Toggle -> if (control is Control.Check) {
                control.checked = !control.checked
                reader.focusCheck(control.label, control.checked)
            }
            is OrbitDialog.Does.Step -> if (control is Control.Combo) choose(control, control.selected + does.by)
            OrbitDialog.Does.First -> if (control is Control.Combo) choose(control, 0)
            OrbitDialog.Does.Last -> if (control is Control.Combo) choose(control, control.field.options.size - 1)
            is OrbitDialog.Does.Find -> if (control is Control.Combo) find(control, does.dots)
            OrbitDialog.Does.Edit -> return control is Control.Edit && editorOf(control).key(key)
            OrbitDialog.Does.Nothing -> {}
            OrbitDialog.Does.NotTaken -> return false
        }
        return true
    }

    private fun kindOf(control: Control): OrbitDialog.Control = when (control) {
        is Control.Edit -> OrbitDialog.Control.EDIT
        is Control.Combo -> OrbitDialog.Control.COMBO_BOX
        is Control.Check -> OrbitDialog.Control.CHECK_BOX
        is Control.Button -> OrbitDialog.Control.BUTTON
        is Control.Text -> OrbitDialog.Control.TEXT
    }

    private fun moveFocus(by: Int) {
        if (controls.isEmpty()) return
        // The line left keeps what was typed, its word in progress taken in.
        (controls[focus] as? Control.Edit)?.let { it.value = valueOf(it) }
        focus = Math.floorMod(focus + by, controls.size)
        tellControl()
    }

    /** The combo box holds option [to], stopping at its ends with the error tone. */
    private fun choose(combo: Control.Combo, to: Int) {
        val options = combo.field.options
        if (to !in options.indices) return ui.refuse()
        combo.selected = to
        reader.itemChanged(combo.label, options.size, to, options[to])
    }

    private fun find(combo: Control.Combo, dots: Int) {
        val letter = BrailleLetters.letter(dots)?.toString() ?: return ui.refuse()
        val at = ListFinder.next(combo.field.options, combo.selected, letter) ?: return ui.refuse()
        choose(combo, at)
    }

    /** The line's edit box, made again from what it holds if the keys turned out to be the other device's. */
    private fun editorOf(edit: Control.Edit): EditBox {
        val now = edit.editor
        if (now != null && edit.editorPhone == ui.phoneKeypad) return now
        if (now != null) edit.value = now.text()
        edit.editorPhone = ui.phoneKeypad
        return ui.editBox(
            edit.label, edit.value, edit.field.multiLine,
            confirmed = { typed ->
                // Select in a line of one: the dialog's default, as Orbit's dialogs press theirs.
                edit.value = typed
                edit.editor = null
                default?.press?.invoke()
            },
            cancelled = { ui.close(this) },
        ).also { edit.editor = it }
    }

    override fun closed() {
        reader.dialogClosed()
        closing()
    }
}
