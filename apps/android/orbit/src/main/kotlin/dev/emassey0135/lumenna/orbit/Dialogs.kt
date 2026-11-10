package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.appWords

/** An option of a [Control.Choosing], by the id the form takes. */
data class Option(val id: String, val title: String)

/**
 * One control of a dialog, as Orbit's own dialogs have them. On the Speak each is active as soon as
 * it is reached; on the Player, which has no Tab, each is a row that [DialogScreen.rows] opens.
 */
sealed class Control(val key: String, val offered: List<Offered>, val on: ActingOn?) {
    /** What it says as a row, and what its focus says beside its role. */
    abstract val text: String

    /** A line typed in place: [commit] takes it on leaving the control, if it changed. */
    class Line(
        key: String,
        val name: String,
        val value: String,
        val hint: String = "",
        val commit: (String) -> Unit = {},
        val changed: (String, Int) -> Unit = { _, _ -> },
        val completing: (() -> Boolean)? = null,
        val complete: () -> Unit = {},
        /** Several lines: dot 8 starts a new one, Tab leaves it. */
        val multiline: Boolean = false,
        offered: List<Offered> = emptyList(),
        on: ActingOn? = null,
    ) : Control(key, offered, on) {
        override val text: String get() = listOf(name, value.ifEmpty { appWords.noneValue }).joinToString(", ")
    }

    /** One value of [options]: a combo box, or with [list] the list inside a dialog. */
    class Choosing(
        key: String,
        val name: String,
        val options: List<Option>,
        val value: String,
        val choose: (String) -> Unit,
        val list: Boolean = false,
        offered: List<Offered> = emptyList(),
        on: ActingOn? = null,
    ) : Control(key, offered, on) {
        override val text: String
            get() = listOf(name, options.firstOrNull { it.id == value }?.title ?: value.ifEmpty { appWords.noneValue }).joinToString(", ")
    }

    /** On or off, [toggle] changing it. */
    class Check(key: String, val name: String, val checked: Boolean, val toggle: () -> Unit) : Control(key, emptyList(), null) {
        override val text: String
            get() = listOf(name, appWords.yesNo.firstOrNull { it.id == if (checked) "yes" else "no" }?.title.orEmpty()).joinToString(", ")
    }

    /** Pressed with Select or dot 8. [letter] runs it too. */
    class Button(key: String, val title: String, val letter: String? = null, val default: Boolean = false, val press: () -> Unit) :
        Control(key, emptyList(), null) {
        override val text: String get() = title
    }

    /** A line to read, between the controls. */
    class Note(key: String, override val text: String, offered: List<Offered> = emptyList(), on: ActingOn? = null) :
        Control(key, offered, on)
}

/**
 * A dialog: controls in order, moved between with Tab and Shift-Tab on the Speak, each active as
 * soon as it is reached, as in Orbit's own dialogs (the Editor's Save As). Space with E or Z cancels
 * and Select presses the focused button; Left never leaves it, on either device. On the Player the same controls are rows, a line or a
 * choice opening its own screen.
 */
abstract class DialogScreen : ListScreen() {
    /** Whether this device's keys work the controls in place. */
    val inPlace: Boolean get() = !ui.phoneKeypad

    abstract fun controls(): List<Control>

    /** Select where it presses no button: the dialog's own default. */
    protected open fun confirmDialog() {
        controls().filterIsInstance<Control.Button>().firstOrNull { it.default }?.let {
            commitFocused()
            it.press()
        }
    }

    /** Space with E or Z, or dot 7 off a line. */
    protected open fun cancelDialog() = ui.pop()

    override fun openedAs(): Opened = Opened.Dialog

    override fun rows(): List<Row> = controls().map(::rowOf)

    private fun rowOf(control: Control): Row = when (control) {
        is Control.Line -> Row(control.key, control.text, {
            ui.push(FieldEditScreen(control.name, control.value, control.hint, control.commit))
        }, control.offered, control.on)
        is Control.Choosing -> Row(control.key, control.text, {
            ui.push(ChoiceScreen(control.name, control.options, control.value, control.choose))
        }, control.offered, control.on)
        is Control.Check -> Row(control.key, control.text, control.toggle)
        is Control.Button -> Row(control.key, control.text, control.press, letter = control.letter)
        is Control.Note -> Row(control.key, control.text, null, control.offered, control.on)
    }

    // ---- the editors ----

    private class Editing(val editor: LineEditor, val from: String)

    private val editors = HashMap<String, Editing>()

    /** The editor of [control], made anew only if the app changed its line underneath. */
    protected fun editorFor(control: Control.Line): LineEditor {
        val editing = editors[control.key]
        if (editing != null && (control.value == editing.from || control.value == editing.editor.display())) return editing.editor
        val editor = ui.editor(
            control.name, control.value,
            changed = control.changed,
            confirmed = { text -> lineFinished(control, text) },
            cancelled = ::cancelDialog,
            completing = control.completing,
            complete = control.complete,
            multiline = control.multiline,
        )
        editors[control.key] = Editing(editor, control.value)
        return editor
    }

    /** Select finished the line: it is taken, then the dialog confirmed. */
    protected open fun lineFinished(control: Control.Line, text: String) {
        if (text != control.value) control.commit(text)
        confirmDialog()
    }

    /** What is typed in the focused control goes to the form; whether anything changed. */
    protected fun commitFocused(): Boolean {
        val control = controls().getOrNull(index) as? Control.Line ?: return false
        val text = editorFor(control).text()
        if (text == control.value) return false
        control.commit(text)
        return true
    }

    // ---- the keys, on the Speak ----

    override fun key(key: Key): Boolean {
        if (!inPlace) return false
        when (key) {
            Key.Tab -> return moveFocus(1)
            Key.BackTab -> return moveFocus(-1)
            Key.Escape -> {
                cancelDialog()
                return true
            }
            Key.Menu -> return false
            else -> {}
        }
        return when (val control = controls().getOrNull(index)) {
            is Control.Line -> {
                editorFor(control).key(key)
                true
            }
            is Control.Choosing -> choosingKey(control, key)
            is Control.Check -> checkKey(control, key)
            is Control.Button -> offLine(key, press = control.press)
            is Control.Note, null -> offLine(key, press = {})
        }
    }

    /** A button or a line to read: Up and Down go on to the next control, as a list does. */
    private fun offLine(key: Key, press: () -> Unit): Boolean = when (key) {
        Key.Select, Key.Dot8 -> {
            press()
            true
        }
        Key.Dot7 -> {
            cancelDialog()
            true
        }
        Key.Left, Key.Right -> true
        else -> false
    }

    private fun moveFocus(by: Int): Boolean {
        val controls = controls()
        if (controls.isEmpty()) return true
        commitFocused()
        index = (index + by + controls.size) % controls.size
        selectedKey = controls[index].key
        ui.redraw(Why.Moved)
        return true
    }

    /** What is chosen until the app has it, so a quick second press moves on from there. */
    private val choosing = HashMap<String, String>()

    private fun choosingKey(control: Control.Choosing, key: Key): Boolean {
        val options = control.options
        val value = choosing[control.key]?.takeUnless { it == control.value } ?: control.value
        choosing.remove(control.key)
        val at = options.indexOfFirst { it.id == value }
        val to = when (key) {
            Key.Up -> (if (at < 0) 0 else at - 1).coerceAtLeast(0)
            Key.Down -> (at + 1).coerceAtMost(options.size - 1)
            Key.First -> 0
            Key.Last -> options.size - 1
            is Key.Cell -> ui.findIn(options.map { it.title }, at, key.dots) ?: return ui.refuse().let { true }
            Key.Select, Key.Dot8 -> {
                confirmDialog()
                return true
            }
            Key.Dot7 -> {
                cancelDialog()
                return true
            }
            Key.Left, Key.Right -> return true
            else -> return false
        }.coerceIn(0, (options.size - 1).coerceAtLeast(0))
        val option = options.getOrNull(to) ?: return true
        choosing[control.key] = option.id
        if (to == at) ui.refuse() else ui.reader.itemChanged(control.name, options.size, to, option.title)
        if (option.id != control.value) control.choose(option.id)
        return true
    }

    private fun checkKey(control: Control.Check, key: Key): Boolean = when (key) {
        Key.Up, Key.Down, Key.Select, Key.Dot8 -> {
            control.toggle()
            ui.reader.focusCheck(control.name, !control.checked)
            true
        }
        Key.Dot7 -> {
            cancelDialog()
            true
        }
        Key.Left, Key.Right -> true
        else -> false
    }

    // ---- what Orbit is told ----

    private var told = false

    /** The dialog is told once, as it opens, then the control focused whenever it is reached. */
    fun describeControl(newlyShown: Boolean, why: Why, moved: Boolean) {
        val control = controls().getOrNull(index) ?: return
        if (!told) {
            told = true
            ui.reader.dialog(title, "", "")
        }
        if (!newlyShown && why != Why.Moved && why != Why.Entered && !moved) return
        when (control) {
            is Control.Line -> {
                ui.liblouis.follow()
                editorFor(control).focus()
                if (control.hint.isNotEmpty()) ui.later(HINT_AFTER_MS) { ui.reader.speakQueued(control.hint) }
            }
            is Control.Choosing -> {
                val at = control.options.indexOfFirst { it.id == control.value }
                ui.reader.focusChoice(
                    control.name, if (control.list) OrbitReader.ROLE_LIST_BOX else OrbitReader.ROLE_COMBO_BOX,
                    control.options.size, at.coerceAtLeast(0), control.options.getOrNull(at)?.title.orEmpty(),
                )
            }
            is Control.Check -> ui.reader.focusCheck(control.name, control.checked)
            is Control.Button -> ui.reader.focusControl(control.title, OrbitReader.ROLE_BUTTON)
            is Control.Note -> ui.reader.focusText(control.text)
        }
    }

    private companion object {
        const val HINT_AFTER_MS = 400L
    }
}
