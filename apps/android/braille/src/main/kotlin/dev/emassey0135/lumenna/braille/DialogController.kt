package dev.emassey0135.lumenna.braille

import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ControlMove
import dev.emassey0135.lumenna.app.ControlSends
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.controlEvent

/**
 * A form or dialog the core describes (`DialogShown`: a question, the task form, the block form,
 * quick add, a settings page), worked as a braille device works one: a control in hand, moved by
 * Tab and Shift-Tab round its controls, a line's value taken as the control is left, a choice
 * stepped or found by its first letter, a check toggled, the default button and Cancel. What each
 * does is sent through the core's `control_event`, so no device builds an event for a control.
 *
 * No keys and no words: a device maps its own keys onto these calls and says the control in hand
 * in its own roles. What is typed, chosen or toggled here is held until the core's next description
 * has it ([update]), so a quick second press goes on from the first; a value the core changed
 * underneath (a completion chosen, a refused setting put back) wins.
 */
class DialogController(
    dialog: DialogShown,
    /** Sends one event to the app. */
    private val send: (ShellEvent) -> Unit,
    /** A button that is the device's own to do (`ControlSends.Shell`): a file to pick, pairing. */
    private val shell: (ControlShown) -> Unit = {},
    /** The core's `control_event`; a test gives its own, the native library being absent there. */
    private val events: (ControlSends, String, ControlMove) -> List<ShellEvent> = ::controlEvent,
) {
    /** The dialog as the core last described it. */
    var dialog: DialogShown = dialog
        private set

    val controls: List<ControlShown> get() = dialog.controls

    /** Where the control in hand is among [controls]. */
    var index: Int = dialog.focus.toInt().coerceIn(0, (dialog.controls.size - 1).coerceAtLeast(0))
        private set

    /** The control in hand, if the dialog has any. */
    val control: ControlShown? get() = controls.getOrNull(index)

    /** What is held here until the core has it, by control key, with the core's value it began from. */
    private class Held(val from: String, val value: String)

    private val typed = HashMap<String, Held>()
    private val chosen = HashMap<String, Held>()
    private val checks = HashMap<String, Held>()

    /**
     * The core's newer description. The same dialog keeps the control in hand by its key (else its
     * place) and what is held here; another one (a new `id`) starts afresh on its `focus`.
     * Whether it was the same dialog.
     */
    fun update(newer: DialogShown): Boolean {
        val same = newer.id == dialog.id
        val key = control?.key
        dialog = newer
        if (!same) {
            typed.clear()
            chosen.clear()
            checks.clear()
            index = newer.focus.toInt()
        } else {
            index = controls.indexOfFirst { it.key == key }.takeIf { it >= 0 } ?: index
            settle(typed) { it.value }
            settle(chosen) { selectedId(it) }
            settle(checks) { it.checked.toString() }
        }
        index = index.coerceIn(0, (controls.size - 1).coerceAtLeast(0))
        return same
    }

    /** Drops what the core now has, or changed underneath from what it began from. */
    private fun settle(held: HashMap<String, Held>, now: (ControlShown) -> String) {
        held.entries.removeAll { (key, value) ->
            val shown = controls.firstOrNull { it.key == key } ?: return@removeAll true
            val current = now(shown)
            current == value.value || current != value.from
        }
    }

    // ---- moving --------------------------------------------------------------------------

    /** Tab: the next control, round from the last to the first, the line left taken. */
    fun next(): ControlShown? = moveTo(index + 1)

    /** Shift-Tab: the control before, round from the first to the last. */
    fun previous(): ControlShown? = moveTo(index - 1)

    /** To the control at [to] (wrapping), as a device without Tab reaches one; the line left taken. */
    fun moveTo(to: Int): ControlShown? {
        if (controls.isEmpty()) return null
        commit()
        index = Math.floorMod(to, controls.size)
        return control
    }

    // ---- what each control holds now ---------------------------------------------------

    /** A line's text: what is typed here, else the core's. */
    fun text(of: ControlShown): String = typed[of.key]?.value ?: of.value

    /** Which option a choice or list holds, by position: what is chosen here, else the core's. */
    fun selected(of: ControlShown): Int? {
        val id = chosen[of.key]?.value ?: return of.selected?.toInt()
        return of.options.indexOfFirst { it.id == id }.takeIf { it >= 0 }
    }

    /** A check's state: as toggled here, else the core's. */
    fun checked(of: ControlShown): Boolean = checks[of.key]?.value?.toBooleanStrictOrNull() ?: of.checked

    /** What [of] holds as it is sent: a line's text, an option's id, a check's `true` or `false`. */
    fun value(of: ControlShown): String = when (of.kind) {
        ControlType.LINE, ControlType.LINES -> text(of)
        ControlType.CHOICE, ControlType.LIST -> selected(of)?.let { of.options.getOrNull(it)?.id }.orEmpty()
        ControlType.CHECK -> checked(of).toString()
        ControlType.BUTTON, ControlType.TEXT -> of.value
    }

    private fun selectedId(of: ControlShown): String = of.selected?.toInt()?.let { of.options.getOrNull(it)?.id }.orEmpty()

    // ---- doing ---------------------------------------------------------------------------

    /** The line in hand now reads [text], the cursor at [cursor] (a UTF-16 offset): sent as typed. */
    fun type(text: String, cursor: Int = text.length) {
        val line = control?.takeIf { it.kind == ControlType.LINE || it.kind == ControlType.LINES } ?: return
        typed[line.key] = Held(typed[line.key]?.from ?: line.value, text)
        deliver(line.sends, text, ControlMove.Typed(TextOffsets.bytes(cursor, text)))
    }

    /** The line in hand left (Tab, a button pressed): its text taken, if it changed. Whether it did. */
    fun commit(): Boolean {
        val line = control?.takeIf { it.kind == ControlType.LINE || it.kind == ControlType.LINES } ?: return false
        val held = typed[line.key] ?: return false
        if (held.value == held.from) return false
        deliver(line.sends, held.value, ControlMove.Set)
        return true
    }

    /**
     * A choice or a list moved [by] options, stopping at its ends: a choice is changed as it moves,
     * a list only moves until its button is pressed. The option reached, or null at an end.
     */
    fun step(by: Int): Int? {
        val choice = control?.takeIf { it.kind == ControlType.CHOICE || it.kind == ControlType.LIST } ?: return null
        if (choice.options.isEmpty()) return null
        val at = selected(choice)
        val to = when {
            at == null -> if (by < 0) choice.options.size - 1 else 0
            else -> at + by
        }
        if (to !in choice.options.indices || to == at) return null
        choose(to)
        return to
    }

    /** The choice or list in hand holds option [option]: a choice sends it, a list holds it. */
    fun choose(option: Int) {
        val choice = control?.takeIf { it.kind == ControlType.CHOICE || it.kind == ControlType.LIST } ?: return
        val picked = choice.options.getOrNull(option) ?: return
        chosen[choice.key] = Held(chosen[choice.key]?.from ?: selectedId(choice), picked.id)
        if (choice.kind == ControlType.CHOICE) deliver(choice.sends, picked.id, ControlMove.Set)
    }

    /** The next option after the one held whose title starts with [letter], round the list; null for none. */
    fun find(letter: String): Int? {
        val choice = control?.takeIf { it.kind == ControlType.CHOICE || it.kind == ControlType.LIST } ?: return null
        val options = choice.options
        if (options.isEmpty() || letter.isEmpty()) return null
        val from = selected(choice) ?: -1
        for (step in 1..options.size) {
            val at = Math.floorMod(from + step, options.size)
            if (options[at].title.trimStart().startsWith(letter, ignoreCase = true)) {
                if (at != from) choose(at)
                return at
            }
        }
        return null
    }

    /** The check in hand turned over; what it is now, or null when it is no check. */
    fun toggle(): Boolean? {
        val check = control?.takeIf { it.kind == ControlType.CHECK } ?: return null
        val now = !checked(check)
        checks[check.key] = Held(checks[check.key]?.from ?: check.checked.toString(), now.toString())
        deliver(check.sends, now.toString(), ControlMove.Set)
        return now
    }

    /**
     * The button in hand pressed, or a line that submits (quick add's, a question's) pressed with
     * its text. A button sending what a line or list holds is pressed with that control's value.
     * Whether anything was pressed.
     */
    fun press(): Boolean {
        val pressed = control ?: return false
        return when (pressed.kind) {
            ControlType.BUTTON -> press(pressed)
            ControlType.LINE, ControlType.LIST -> if (submits(pressed.sends)) press(pressed) else pressDefault()
            else -> false
        }
    }

    /** The dialog's default button pressed (Enter where the control in hand presses nothing itself). */
    fun pressDefault(): Boolean = controls.firstOrNull { it.default }?.let(::press) ?: false

    /** Escape or Back: what closing the dialog unanswered sends. Whether it sent anything. */
    fun cancel(): Boolean {
        typed.clear()
        dialog.cancel.forEach(send)
        return dialog.cancel.isNotEmpty()
    }

    /** The button with [letter], pressed; whether there was one. */
    fun pressLetter(letter: String): Boolean =
        controls.firstOrNull { it.kind == ControlType.BUTTON && it.letter.isNotEmpty() && it.letter == letter }?.let(::press) ?: false

    /** [button] pressed: the line in hand taken first, then what it sends. */
    fun press(button: ControlShown): Boolean {
        val sends = button.sends
        // A button sending what a line or a list holds takes that control's value.
        val source = if (submits(sends)) controls.firstOrNull { it.kind != ControlType.BUTTON && sameSends(it.sends, sends) } else null
        // The line in hand is taken first, unless it is what is pressed with.
        if (source == null || source.key != control?.key) commit()
        if (sends is ControlSends.Shell) {
            shell(button)
            return true
        }
        val value = source?.let(::value) ?: value(button)
        deliver(sends, value, ControlMove.Pressed)
        if (source != null) typed.remove(source.key)
        return true
    }

    private fun deliver(sends: ControlSends, value: String, how: ControlMove) = events(sends, value, how).forEach(send)

    private companion object {
        fun submits(sends: ControlSends): Boolean =
            sends is ControlSends.QuickAdd || sends is ControlSends.Answer || sends is ControlSends.Pick

        fun sameSends(a: ControlSends, b: ControlSends): Boolean = a::class == b::class
    }
}
