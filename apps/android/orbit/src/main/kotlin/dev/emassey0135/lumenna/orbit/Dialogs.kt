package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.Candidate
import dev.emassey0135.lumenna.app.ControlSends
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.brailleLetter
import dev.emassey0135.lumenna.app.candidateLine
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.braille.DialogController
import dev.emassey0135.orbit.EditBox
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.OrbitDialog
import dev.emassey0135.orbit.OrbitReader

/**
 * A form or dialog the core describes (`DialogShown`: a question, the task form, the block form,
 * quick add, a settings page), worked by the braille apps' [DialogController], on the SDK's stack.
 *
 * On the Speak it is Orbit's own dialog: Tab and Shift-Tab move between controls, each active as
 * soon as it is reached, as in the Editor's Save As; a line types at once (the SDK's [EditBox]), Up
 * and Down change a choice or a check, Select presses a button or the default, Space with E or Z
 * cancels, and each control is told to Orbit's reader in its role. Left never leaves it. On the
 * Player, which has no Tab, the same controls are rows, a line or a choice opening a screen of its
 * own; where the dialog is a line ([lineFills]: quick add, a question's line) the line is the whole
 * screen and Menu lists the dialog's buttons.
 */
abstract class DialogScreen : AppList() {
    /** Whether this device's keys work the controls in place. */
    val inPlace: Boolean get() = !host.phoneKeypad

    /** The core's description of this dialog now; null until it has arrived. */
    abstract fun shown(): DialogShown?

    /** On the Player, whether the dialog's line is the whole screen. */
    open val lineFills: Boolean get() = false

    /** A button of the dialog that is this app's own to do: a file to pick, pairing. */
    protected open fun shell(control: ControlShown) = host.refuse()

    /** An event the dialog sends, as it goes: for a screen that follows what its buttons do. */
    protected open fun sending(event: ShellEvent) {}

    /** What a row of it offers a menu away, and what its actions run on: the task form's task. */
    protected open val offered: List<Offered> get() = emptyList()
    protected open val on: ActingOn? get() = null

    private var controller: DialogController? = null

    /** The controller over the dialog as the core describes it now. */
    protected val dialog: DialogController?
        get() {
            val now = shown() ?: return controller
            controller?.update(now) ?: run {
                controller = DialogController(now, send = { event ->
                    sending(event)
                    host.send(event)
                }, shell = ::shell)
            }
            return controller
        }

    override val title: String get() = shown()?.title.orEmpty()

    override fun loaded(view: dev.emassey0135.lumenna.app.ViewModel): Boolean = shown() != null

    override fun openedAs(): Opened = Opened.Dialog

    /** On the Speak the control in hand is the controller's, which Orbit is told of. */
    override fun inHand(): String? = if (inPlace) dialog?.control?.key else null

    override fun moved(row: Row) {
        val controller = dialog ?: return
        val at = controller.controls.indexOfFirst { it.key == row.key }
        if (at >= 0 && at != controller.index) controller.moveTo(at)
    }

    /** Escape, Back, or Dot 7 off a line: what the dialog sends unanswered, then gone. */
    protected open fun cancelDialog() {
        dialog?.cancel()
        host.remove(this)
    }

    // ---- what it shows -------------------------------------------------------------------

    /** What a control says as a row, and beside its role: its label and value. */
    protected fun textOf(control: ControlShown): String {
        val controller = dialog
        val value = when (control.kind) {
            ControlType.LINE, ControlType.LINES -> (controller?.text(control) ?: control.value).ifEmpty { appWords.noneValue }
            ControlType.CHOICE, ControlType.LIST ->
                (controller?.selected(control) ?: control.selected?.toInt())?.let { control.options.getOrNull(it)?.title } ?: control.value.ifEmpty { appWords.noneValue }
            ControlType.CHECK -> appWords.yesNo.firstOrNull { it.id == if (controller?.checked(control) ?: control.checked) "yes" else "no" }?.title.orEmpty()
            ControlType.BUTTON -> control.value
            ControlType.TEXT -> return control.value
        }
        return listOf(control.label, value).filter { it.isNotEmpty() }.joinToString(", ")
    }

    override fun rows(): List<Row> {
        val controller = dialog ?: return emptyList()
        sayRefusals(controller.controls)
        if (!inPlace && lineFills && line() != null) return emptyList()
        return controller.controls.flatMapIndexed { at, control ->
            if (inPlace) {
                listOf(Row(control.key, textOf(control), {
                    controller.moveTo(at)
                    controller.press()
                }, offered, on, control.letter.ifEmpty { null }))
            } else {
                playerRows(controller, at, control)
            }
        }
    }

    /** The Player's rows for a control: a line or a choice opens a screen, a list is its options. */
    private fun playerRows(controller: DialogController, at: Int, control: ControlShown): List<Row> = when (control.kind) {
        ControlType.LINE, ControlType.LINES -> listOf(Row(control.key, textOf(control), {
            host.push(FieldEditScreen(control.label, controller.text(control), control.hint, control.kind == ControlType.LINES) { text ->
                controller.moveTo(at)
                controller.type(text)
                controller.commit()
            })
        }, offered, on))
        ControlType.CHOICE -> listOf(Row(control.key, textOf(control), {
            host.push(ChoiceScreen(control.label, control.options.map { it.title }, controller.selected(control) ?: 0) { option ->
                controller.moveTo(at)
                controller.choose(option)
            })
        }, offered, on))
        ControlType.LIST -> control.options.mapIndexed { option, choice ->
            Row("${control.key}:${choice.id}", listOfNotNull(choice.title, choice.reason?.ifEmpty { null }).joinToString(". "), {
                controller.moveTo(at)
                controller.choose(option)
                controller.press()
            })
        }
        ControlType.CHECK -> listOf(Row(control.key, textOf(control), {
            controller.moveTo(at)
            controller.toggle()
        }, offered, on))
        ControlType.BUTTON -> listOf(Row(control.key, textOf(control), {
            controller.moveTo(at)
            controller.press()
        }, offered, on, control.letter.ifEmpty { null }))
        ControlType.TEXT -> listOf(Row(control.key, control.value, null, offered, on))
    }

    /** On the Player's whole-screen line, Menu is the dialog's buttons. */
    override fun commands(): List<Command> {
        if (inPlace || !lineFills) return emptyList()
        val controller = dialog ?: return emptyList()
        return controller.controls.filter { it.kind == ControlType.BUTTON }.map { button ->
            Command(button.label) { controller.press(button) }
        }
    }

    /** A refusal the dialog holds (a save refused, a setting's problem): said once as it comes. */
    private val refusals = HashMap<String, String>()

    private fun sayRefusals(controls: List<ControlShown>) {
        for (control in controls) {
            if (control.kind != ControlType.TEXT || !(control.key == "refusal" || control.key == "refused" || control.key.startsWith("problem:"))) continue
            if (refusals[control.key] == control.value) continue
            refusals[control.key] = control.value
            host.stack.refused(control.value)
        }
        refusals.keys.retainAll(controls.map { it.key }.toSet())
    }

    // ---- the lines ----------------------------------------------------------------------

    private class Editing(val editor: EditBox, val from: String, val phone: Boolean)

    private val editors = HashMap<String, Editing>()

    /** The dialog's line: the one in hand, else its first. */
    protected fun line(): ControlShown? {
        val controller = dialog ?: return null
        return controller.control?.takeIf { it.isLine() } ?: controller.controls.firstOrNull { it.isLine() }
    }

    private fun ControlShown.isLine() = kind == ControlType.LINE || kind == ControlType.LINES

    /** The text being typed and the caret, for completion. */
    protected var typing = ""
    protected var cursor = 0

    /**
     * The editor of [control], made anew only when the core changed its line underneath, or the
     * keys turned out to be the other device's.
     */
    protected fun editorFor(control: ControlShown): EditBox {
        val controller = dialog
        val text = controller?.text(control) ?: control.value
        val editing = editors[control.key]
        if (editing != null && editing.phone == host.phoneKeypad && (text == editing.from || text == editing.editor.display())) return editing.editor
        val editor = host.editor(
            control.label, text,
            changed = { typed, caret ->
                typing = typed
                cursor = caret
                controller?.let { c ->
                    c.controls.indexOfFirst { it.key == control.key }.takeIf { it >= 0 && it != c.index }?.let(c::moveTo)
                    c.type(typed, caret)
                }
            },
            confirmed = { typed -> lineFinished(control, typed) },
            cancelled = ::cancelDialog,
            completing = control.syntax?.let { { candidates(control).isNotEmpty() } },
            complete = { openCompletions(control) },
            multiline = control.kind == ControlType.LINES,
        )
        typing = text
        cursor = text.length
        editors[control.key] = Editing(editor, text, host.phoneKeypad)
        return editor
    }

    /** Select finished a line: a line that submits is pressed with its text, else the default. */
    protected open fun lineFinished(control: ControlShown, text: String) {
        val controller = dialog ?: return
        controller.type(text)
        if (!controller.press()) controller.commit()
    }

    /** On the Player, where the line is the whole screen: Orbit says it and its hint. */
    private fun describeLine() {
        val line = line() ?: return
        dialog?.let { c -> c.controls.indexOfFirst { it.key == line.key }.takeIf { it >= 0 && it != c.index }?.let(c::moveTo) }
        host.braille.follow()
        editorFor(line).focus()
        // After the field's name: what Orbit is told goes through its event queue, speech does not.
        if (line.hint.isNotEmpty()) host.stack.afterTelling { if (host.top === this) reader.speakQueued(line.hint) }
    }

    /** What could go at the caret in the line's own syntax. */
    private fun candidates(control: ControlShown): List<Candidate> {
        val syntax = control.syntax ?: return emptyList()
        return runCatching { host.app.complete(typing, TextOffsets.bytes(cursor, typing), syntax).candidates }.getOrDefault(emptyList())
    }

    private fun openCompletions(control: ControlShown) {
        val syntax = control.syntax ?: return
        val completions = runCatching { host.app.complete(typing, TextOffsets.bytes(cursor, typing), syntax) }.getOrNull() ?: return
        val start = TextOffsets.utf16(completions.start, typing)
        val end = TextOffsets.utf16(completions.end, typing)
        host.push(CompletionScreen(completions.candidates.map { candidate ->
            candidateLine(candidate) to {
                val text = typing.substring(0, start) + candidate.text + typing.substring(end)
                typing = text
                cursor = start + candidate.text.length
                editorFor(control).replace(text, cursor)
                host.pop()
            }
        }))
    }

    /** Whether the Player shows the dialog's line as the whole screen. */
    private val lineShown: Boolean get() = !inPlace && lineFills && line() != null

    // ---- the keys -----------------------------------------------------------------------

    override fun key(key: Key): Boolean {
        if (!loaded) return true
        val controller = dialog ?: return true
        if (!inPlace) {
            if (!lineFills || line() == null) {
                // Left never leaves a dialog, on the Player either, whose Back does.
                return key == Key.Left || super.key(key)
            }
            val line = line() ?: return false
            // The Player's Menu is the line's menu (its buttons); every other key is the line's.
            if (key == Key.Menu) return false
            editorFor(line).key(key)
            return true
        }
        // What a key does is Orbit's dialogs' (the SDK's); what it does to the core's form is ours.
        val control = controller.control
        if (control == null) {
            return when (OrbitDialog.does(key, OrbitDialog.Control.TEXT)) {
                OrbitDialog.Does.Next -> moveFocus { controller.next() }
                OrbitDialog.Does.Previous -> moveFocus { controller.previous() }
                OrbitDialog.Does.Cancel -> if (key == Key.Escape) true.also { cancelDialog() } else false
                else -> false
            }
        }
        when (val does = OrbitDialog.does(key, orbitControl(control))) {
            OrbitDialog.Does.Next -> return moveFocus { controller.next() }
            OrbitDialog.Does.Previous -> return moveFocus { controller.previous() }
            OrbitDialog.Does.Cancel -> cancelDialog()
            OrbitDialog.Does.NotTaken -> return false
            OrbitDialog.Does.Nothing -> {}
            OrbitDialog.Does.Edit -> return editorFor(control).key(key)
            OrbitDialog.Does.Press -> controller.press()
            OrbitDialog.Does.Toggle -> controller.toggle()?.let { reader.focusCheck(control.label, it) }
            is OrbitDialog.Does.Step, OrbitDialog.Does.First, OrbitDialog.Does.Last, is OrbitDialog.Does.Find -> choose(controller, control, does)
        }
        return true
    }

    /** What Orbit calls a control of the core's. */
    private fun orbitControl(control: ControlShown): OrbitDialog.Control = when (control.kind) {
        ControlType.LINE, ControlType.LINES -> OrbitDialog.Control.EDIT
        ControlType.CHOICE -> OrbitDialog.Control.COMBO_BOX
        ControlType.LIST -> OrbitDialog.Control.LIST_BOX
        ControlType.CHECK -> OrbitDialog.Control.CHECK_BOX
        ControlType.BUTTON -> OrbitDialog.Control.BUTTON
        ControlType.TEXT -> OrbitDialog.Control.TEXT
    }

    private fun moveFocus(move: () -> ControlShown?): Boolean {
        move() ?: return true
        tellControl()
        return true
    }

    /** A combo box or list box changed: Orbit is told the option it now holds. */
    private fun choose(controller: DialogController, control: ControlShown, does: OrbitDialog.Does) {
        val at = controller.selected(control) ?: 0
        val to: Int? = when (does) {
            is OrbitDialog.Does.Step -> controller.step(does.by)
            OrbitDialog.Does.First -> controller.step(-at)
            OrbitDialog.Does.Last -> controller.step(control.options.size - 1 - at)
            is OrbitDialog.Does.Find -> brailleLetter(does.dots.toUByte())?.let(controller::find)
            else -> return
        }
        if (to == null) {
            host.refuse()
            return
        }
        reader.itemChanged(control.label, control.options.size, to, control.options.getOrNull(to)?.title.orEmpty())
    }

    // ---- what Orbit is told -------------------------------------------------------------

    private var announced = false

    /** The control last told, so a turn that moved the control in hand tells it and one that did not says nothing. */
    private var toldControl: String? = null

    /**
     * Newly shown. On the Speak the dialog is told once, as it opens, then the control in hand
     * whenever it is reached. On the Player its whole-screen line is said as a field; otherwise it
     * is a list, as a question opens as a dialog first.
     */
    override fun tellShown() {
        if (inPlace) {
            dialog?.let { sayRefusals(it.controls) }
            if (!announced) {
                announced = true
                reader.dialog(title, "", "")
            }
            tellControl()
            return
        }
        if (lineShown) {
            sayRefusalsNow()
            describeLine()
            return
        }
        super.tellShown()
    }

    private fun sayRefusalsNow() {
        dialog?.let { sayRefusals(it.controls) }
    }

    override fun refresh(tell: Boolean) {
        if (inPlace) {
            sayRefusalsNow()
            if (tell && dialog?.control?.key != toldControl) tellControl()
            return
        }
        // The whole-screen line is said as it opens, its typing echoed as it goes.
        if (lineShown) return sayRefusalsNow()
        super.refresh(tell)
    }

    override fun land(key: String?) = refresh(true)

    /** The control in hand, told to Orbit in its role. */
    private fun tellControl() {
        val controller = dialog ?: return
        val control = controller.control ?: return
        toldControl = control.key
        when (control.kind) {
            ControlType.LINE, ControlType.LINES -> {
                host.braille.follow()
                editorFor(control).focus()
                if (control.hint.isNotEmpty()) host.stack.afterTelling { if (host.top === this && toldControl == control.key) reader.speakQueued(control.hint) }
            }
            ControlType.CHOICE, ControlType.LIST -> {
                val at = controller.selected(control) ?: 0
                reader.focusChoice(
                    control.label, if (control.kind == ControlType.LIST) OrbitReader.ROLE_LIST_BOX else OrbitReader.ROLE_COMBO_BOX,
                    control.options.size, at, control.options.getOrNull(at)?.title.orEmpty(),
                )
            }
            ControlType.CHECK -> reader.focusCheck(control.label, controller.checked(control))
            ControlType.BUTTON -> reader.focusControl(host.withLetter(textOf(control), control.letter.ifEmpty { null }), OrbitReader.ROLE_BUTTON)
            ControlType.TEXT -> reader.focusText(control.value)
        }
    }

    companion object {
        /** A dialog of this app's own: one line, then Confirm and Cancel, each this app's to do. */
        fun ownLine(id: String, title: String, label: String, value: String, hint: String, confirm: String, multiline: Boolean = false): DialogShown =
            DialogShown(
                id, title,
                listOf(
                    own("line", if (multiline) ControlType.LINES else ControlType.LINE, label, value, hint),
                    own("confirm", ControlType.BUTTON, confirm).copy(default = true),
                    own("cancel", ControlType.BUTTON, appWords.cancel).copy(cancel = true),
                ),
                0u, emptyList(),
            )

        private fun own(key: String, kind: ControlType, label: String, value: String = "", hint: String = "") = ControlShown(
            key = key, kind = kind, label = label, value = value, hint = hint, example = "", options = emptyList(), selected = null,
            checked = false, `field` = null, syntax = null, letter = "", asks = false, default = false, cancel = false,
            sends = ControlSends.Shell(key),
        )
    }
}

/** A line of this app's own (a field on the Player, a filter, a code to pair with): Confirm takes it. */
open class LineScreen(
    private val label: String,
    private val value: String,
    private val hint: String,
    private val multiline: Boolean,
    private val confirmWord: String,
    private val done: (String) -> Unit,
) : DialogScreen() {
    private val own by lazy { ownLine("line:$label", label, label, value, hint, confirmWord, multiline) }

    override fun shown(): DialogShown = own

    override val lineFills: Boolean get() = true

    override fun shell(control: ControlShown) {
        when (control.key) {
            "confirm" -> {
                val text = line()?.let { dialog?.text(it) }.orEmpty()
                done(text)
                host.remove(this)
            }
            "cancel" -> host.remove(this)
        }
    }

    /** Escape leaves it: nothing of the app's is open to cancel. */
    override fun cancelDialog() = host.remove(this)

    override fun lineFinished(control: ControlShown, text: String) {
        dialog?.type(text)
        dialog?.pressDefault()
    }
}

/** A field typed on the Player: the task's, the block's, a setting's. */
class FieldEditScreen(name: String, value: String, hint: String, multiline: Boolean = false, done: (String) -> Unit) :
    LineScreen(name, value, hint, multiline, appWords.save, done)

/** A choice of a control's options on the Player, the one it holds first in hand. */
class ChoiceScreen(
    private val name: String,
    private val options: List<String>,
    selected: Int,
    private val chosen: (Int) -> Unit,
) : AppList() {
    init {
        hold(selected.coerceIn(0, (options.size - 1).coerceAtLeast(0)).toString())
    }

    override val title: String get() = name

    override fun rows(): List<Row> = options.mapIndexed { at, option ->
        // Chosen first, so the form shows what was chosen as it comes back.
        Row(at.toString(), option, {
            chosen(at)
            host.remove(this)
        })
    }
}
