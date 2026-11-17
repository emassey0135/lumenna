package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.EditBox
import dev.emassey0135.orbit.Key

/**
 * A line asked for ([OrbitUi.askLine]): the SDK's [EditBox], told as Orbit's own edit fields are,
 * typed in cells on the Speak and with T9 on the Player. Select or Dot 8 confirms (Dot 8 starts a
 * line in one of several lines), Space with E or Z and the Player's held Back cancel, and so does
 * Back on the Speak. Its menu is OK and Cancel. [answered] is called once, with null unless confirmed.
 */
internal class LineScreen(
    private val prompt: String,
    initial: String,
    private val hint: String,
    private val multiLine: Boolean,
    private val answered: (String?) -> Unit,
) : Screen() {
    private var text = initial
    private var editor: EditBox? = null
    private var editorPhone = false
    private var answer: String? = null

    override val title: String get() = prompt

    /** The edit box, made again from what it holds if the keys turned out to be the other device's. */
    private fun editor(): EditBox {
        val now = editor
        if (now != null && editorPhone == ui.phoneKeypad) return now
        if (now != null) text = now.text()
        editorPhone = ui.phoneKeypad
        return ui.editBox(
            prompt, text, multiLine,
            confirmed = { typed ->
                answer = typed
                ui.close(this)
            },
            cancelled = { ui.close(this) },
            changed = { typed, _ -> text = typed },
        ).also { editor = it }
    }

    override fun tellShown() {
        ui.braille?.follow()
        editor().focus()
        // What Orbit is told goes through its event queue and speech does not: the hint waits for the field.
        if (hint.isNotEmpty()) ui.afterTelling { if (ui.top === this) reader.speakQueued(hint) }
    }

    override fun key(key: Key): Boolean {
        if (key == Key.Menu) return false
        // On the Player every key is the line's: Back's press is nothing, its release deletes.
        return editor().key(key) || ui.phoneKeypad
    }

    override fun actions(): List<Action> = listOf(
        Action(ui.words.ok) { editor().finish() },
        Action(ui.words.cancel) { ui.close(this) },
    )

    override fun closed() = answered(answer)
}
