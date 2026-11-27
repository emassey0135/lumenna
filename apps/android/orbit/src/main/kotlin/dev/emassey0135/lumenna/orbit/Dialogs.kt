package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.Candidate
import dev.emassey0135.lumenna.app.ControlSends
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.app.DueEvent
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.candidateLine
import dev.emassey0135.lumenna.app.dueLetter
import dev.emassey0135.lumenna.app.firstLetter
import dev.emassey0135.lumenna.app.noRowStartsWith
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.braille.DialogController
import dev.emassey0135.orbit.ui.DialogControl
import dev.emassey0135.orbit.ui.DialogScreen

/**
 * A form or dialog the core describes (`DialogShown`: a question, the task form, the block form,
 * quick add, a settings page), worked by the braille apps' [DialogController], drawn by the SDK's
 * [DialogScreen]. How a dialog works on the Speak and on the Player is the SDK's, as Orbit's own
 * dialogs work; this hands it the core's controls as the controller holds them now, and hands the
 * controller what the person did with them.
 */
abstract class CoreDialog : DialogScreen(), AppScreen {
    override lateinit var host: Ui

    /** The core's description of this dialog now; null until it has arrived. */
    abstract fun shown(): DialogShown?

    /** A button of the dialog that is this app's own to do: a file to pick, pairing. */
    protected open fun shell(control: ControlShown) = host.refuse()

    /** An event the dialog sends, as it goes: for a screen that follows what its buttons do. */
    protected open fun sending(event: ShellEvent) {}

    /** What each control offers a menu away, and what its actions run on: the task form's task. */
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

    // Its prompt: the title, then the lines ahead of the control it opens on (a question's
    // message), which Tab passes over as it goes on from there.
    override val prompt: String get() = shown()?.prompt?.ifEmpty { null } ?: title

    // ---- the stack's hooks, as an AppList has them ---------------------------------------

    /** Whether what the dialog shows has arrived. */
    open fun loaded(view: ViewModel): Boolean = shown() != null

    override val loaded: Boolean get() = host.view.ready && loaded(host.view)

    /** Whether this dialog still stands: a question the app has answered, for one. */
    open fun stands(view: ViewModel): Boolean = true

    override fun stands(): Boolean = stands(host.view)

    /** The dialog came to the top, first or coming back: send what shows it. */
    open fun resumed() {}

    override fun onTop() = resumed()

    /** Leaving the dialog: send what puts its state away. */
    open fun leaving() {}

    override fun closed() {
        super.closed()
        leaving()
    }

    /** Back was pressed: true if the dialog keeps it for now (a form that discards first). */
    open fun beforeBack(): Boolean = false

    override fun back(): Boolean = beforeBack()

    /** Escape, or Dot 7 off a line: what the dialog sends unanswered, then gone. */
    protected open fun cancelDialog() {
        dialog?.cancel()
        host.remove(this)
    }

    override fun cancelled() = cancelDialog()

    // ---- the core's controls, as the SDK's dialog shows them -----------------------------

    override fun controls(): List<DialogControl> {
        val controller = dialog ?: return emptyList()
        val actions = offeredActions(host, offered, on)
        return controller.controls.map { control ->
            when (control.kind) {
                ControlType.LINE, ControlType.LINES ->
                    DialogControl.Line(control.key, control.label, controller.text(control), control.hint, control.kind == ControlType.LINES, actions)
                ControlType.CHOICE, ControlType.LIST -> {
                    val titles = control.options.map { it.title }
                    DialogControl.Choice(
                        control.key, control.label, titles, controller.selected(control) ?: 0, control.kind == ControlType.LIST,
                        // Each option as the core reads it in a list: its title, then why it is offered.
                        control.optionLines.ifEmpty { titles }, actions,
                    )
                }
                ControlType.CHECK -> DialogControl.Check(control.key, control.label, controller.checked(control), actions)
                ControlType.BUTTON -> {
                    val letter = control.letter.ifEmpty { null }
                    DialogControl.Button(control.key, host.withLetter(control.label, letter), letter?.singleOrNull(), actions)
                }
                ControlType.TEXT -> DialogControl.Text(control.key, control.value, actions)
            }
        }
    }

    override val focus: Int get() = dialog?.index ?: 0

    override fun focusOn(at: Int) {
        val controller = dialog ?: return
        if (at != controller.index) controller.moveTo(at)
    }

    private fun shownOf(control: DialogControl): ControlShown? = dialog?.controls?.firstOrNull { it.key == control.key }

    override fun typed(line: DialogControl.Line, text: String, caret: Int) {
        typing = text
        cursor = caret
        dialog?.type(text, caret)
    }

    override fun lineDone(line: DialogControl.Line, text: String) {
        lineFinished(shownOf(line) ?: return, text)
    }

    /** Select finished a line: a line that submits is pressed with its text, else the default. */
    protected open fun lineFinished(control: ControlShown, text: String) {
        val controller = dialog ?: return
        controller.type(text)
        if (!controller.press()) controller.commit()
    }

    override fun lineSet(line: DialogControl.Line, text: String) {
        val controller = dialog ?: return
        controller.type(text)
        controller.commit()
    }

    override fun chose(choice: DialogControl.Choice, option: Int): Boolean {
        dialog?.choose(option) ?: return false
        return true
    }

    /** A first letter among a choice's options, by the core's rule (`first_letter`). */
    override fun findOption(choice: DialogControl.Choice, letter: Char): Int? =
        firstLetter(choice.options, booleanArrayOf(), choice.selected.toUInt(), letter.toString())?.toInt()

    override fun toggled(check: DialogControl.Check): Boolean? = dialog?.toggle()

    override fun pressed(control: DialogControl) {
        val controller = dialog ?: return
        val shown = shownOf(control) ?: return
        if (shown.kind == ControlType.BUTTON) {
            controller.press(shown)
            return
        }
        controller.controls.indexOf(shown).takeIf { it >= 0 && it != controller.index }?.let(controller::moveTo)
        controller.press()
    }

    /** A letter alone on the Player's rows: the core's `first_letter`, as this app's own lists find one. */
    override fun findLetter(letter: Char): Boolean {
        val text = letter.toString()
        val at = firstLetter(rows.map { it.text }, booleanArrayOf(), index.toUInt(), text)?.toInt()
        when {
            at == null -> host.stack.refused(noRowStartsWith(text))
            at == index -> tellMoved()
            else -> moveTo(at, byPerson = true)
        }
        return true
    }

    // ---- what is said ------------------------------------------------------------------

    override fun tellShown() {
        sayRefusals()
        super.tellShown()
    }

    override fun refresh(tell: Boolean) {
        sayRefusals()
        super.refresh(tell)
    }

    /** A refusal the dialog holds (a save refused, a setting's problem): said once as it comes. */
    private val refusals = HashMap<String, String>()

    private fun sayRefusals() {
        val controls = dialog?.controls ?: return
        for (control in controls) {
            if (control.kind != ControlType.TEXT || !(control.key == "refusal" || control.key == "refused" || control.key.startsWith("problem:"))) continue
            if (refusals[control.key] == control.value) continue
            refusals[control.key] = control.value
            host.stack.refused(control.value)
        }
        refusals.keys.retainAll(controls.map { it.key }.toSet())
    }

    // ---- completion -----------------------------------------------------------------------

    /** The text being typed and the caret, for completion. */
    private var typing = ""
    private var cursor = 0

    /** The dialog's line: the one in hand, else its first. */
    protected fun line(): ControlShown? {
        val controller = dialog ?: return null
        return controller.control?.takeIf { it.isLine() } ?: controller.controls.firstOrNull { it.isLine() }
    }

    private fun ControlShown.isLine() = kind == ControlType.LINE || kind == ControlType.LINES

    override fun completing(line: DialogControl.Line): (() -> Boolean)? {
        val control = shownOf(line)?.takeIf { it.syntax != null } ?: return null
        typing = line.value
        cursor = line.value.length
        return { candidates(control).isNotEmpty() }
    }

    /** What could go at the caret in the line's own syntax. */
    private fun candidates(control: ControlShown): List<Candidate> {
        val syntax = control.syntax ?: return emptyList()
        return runCatching { host.app.complete(typing, TextOffsets.bytes(cursor, typing), syntax).candidates }.getOrDefault(emptyList())
    }

    override fun complete(line: DialogControl.Line) {
        val syntax = shownOf(line)?.syntax ?: return
        val completions = runCatching { host.app.complete(typing, TextOffsets.bytes(cursor, typing), syntax) }.getOrNull() ?: return
        val start = TextOffsets.utf16(completions.start, typing)
        val end = TextOffsets.utf16(completions.end, typing)
        host.push(CompletionScreen(completions.candidates.map { candidate ->
            candidateLine(candidate) to {
                val text = typing.substring(0, start) + candidate.text + typing.substring(end)
                putInLine(line.key, text, start + candidate.text.length)
                host.pop()
            }
        }))
    }

    companion object {
        /** A dialog of this app's own: one line, then Confirm and Cancel, each this app's to do. */
        fun ownLine(id: String, title: String, label: String, value: String, hint: String, confirm: String, multiline: Boolean = false): DialogShown =
            DialogShown(
                id = id,
                title = title,
                // Nothing to read ahead of its line: its prompt is its title.
                prompt = title,
                controls = listOf(
                    own("line", if (multiline) ControlType.LINES else ControlType.LINE, label, value, hint),
                    own("confirm", ControlType.BUTTON, confirm).copy(default = true),
                    own("cancel", ControlType.BUTTON, appWords.cancel).copy(cancel = true),
                ),
                focus = 0u,
                cancel = emptyList(),
            )

        private fun own(key: String, kind: ControlType, label: String, value: String = "", hint: String = "") = ControlShown(
            key = key, kind = kind, label = label, value = value, hint = hint, example = "", options = emptyList(), selected = null,
            checked = false, `field` = null, syntax = null, letter = "", asks = false, default = false, cancel = false,
            sends = ControlSends.Shell(key), line = "", optionLines = emptyList(),
        )
    }
}

/**
 * A reminder that has just gone off ([Ui.ring]), said as Orbit's Alarm Clock says its alarm: a dialog
 * named by the reminder, its message to read, then its buttons in the core's order and words, each
 * with its letter, handed back by its id. It rings no more than its notification did. Back leaves it
 * due, among Home's Reminders Due; dealt with here or anywhere else, it goes.
 */
class RingingScreen(val id: String) : DialogScreen(), AppScreen {
    override lateinit var host: Ui

    private val due get() = host.view.due.firstOrNull { it.id == id }

    override val title: String get() = due?.title.orEmpty()

    override val loaded: Boolean get() = host.view.ready && due != null

    override fun stands(): Boolean = due != null

    // Opening on its message, so it is read first, as the alarm's is.
    private var at = 0

    override val focus: Int get() = at

    override fun focusOn(at: Int) {
        this.at = at
    }

    override fun controls(): List<DialogControl> {
        val due = due ?: return emptyList()
        return listOf(DialogControl.Text(MESSAGE, due.text)) + due.buttons.map { button ->
            val letter = dueLetter(button.id)
            DialogControl.Button(button.id, host.withLetter(button.title, letter), letter?.singleOrNull())
        }
    }

    override fun pressed(control: DialogControl) {
        if (control is DialogControl.Button) host.send(ShellEvent.Due(DueEvent.Press(id, control.key)))
    }

    // Nothing in it is typed, chosen or checked.
    override fun lineDone(line: DialogControl.Line, text: String) {}

    override fun lineSet(line: DialogControl.Line, text: String) {}

    override fun chose(choice: DialogControl.Choice, option: Int): Boolean = false

    override fun toggled(check: DialogControl.Check): Boolean? = null

    private companion object {
        const val MESSAGE = "message"
    }
}

/**
 * Asked once, as the app opens and first finds Android holding it back in the background (on these
 * devices DuraSpeed, which restricts every app not switched on in its list): no reminder goes off
 * until the app is opened, and reminders are often added by themselves, so the Reminders page alone
 * is not enough. Its message in the core's words, then a button to where background use is allowed
 * and Not Now; after it only the Reminders page says so.
 */
class HeldBackScreen : CoreDialog() {
    private val shown: DialogShown by lazy {
        val words = appWords.notifications
        DialogShown(
            id = "held-back", title = words.channel, prompt = words.channel,
            controls = listOf(
                own(MESSAGE, ControlType.TEXT, value = words.heldBack),
                own(ALLOW, ControlType.BUTTON, label = words.allowBackground).copy(default = true),
                own(NOT_NOW, ControlType.BUTTON, label = words.notNow).copy(cancel = true),
            ),
            focus = 0u, cancel = emptyList(),
        )
    }

    override fun shown(): DialogShown = shown

    override fun shell(control: ControlShown) {
        if (control.key == ALLOW) host.allowBackground()
        host.remove(this)
    }

    private fun own(key: String, kind: ControlType, label: String = "", value: String = "") = ControlShown(
        key = key, kind = kind, label = label, value = value, hint = "", example = "", options = emptyList(), selected = null,
        checked = false, `field` = null, syntax = null, letter = "", asks = false, default = false, cancel = false,
        sends = ControlSends.Shell(key), line = "", optionLines = emptyList(),
    )

    private companion object {
        const val MESSAGE = "message"
        const val ALLOW = "allow-background"
        const val NOT_NOW = "not-now"
    }
}
