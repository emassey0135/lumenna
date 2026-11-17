package dev.emassey0135.orbit

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context

/**
 * An [EditBox] told to Orbit's screen reader as its own edit fields tell it, with the system's
 * clipboard: what an app needs for an edit box that behaves as Orbit's.
 */
public class ReaderEditOutput(
    private val reader: Reader,
    /** The Speak's braille input; none on the Player, where nothing is typed in cells. */
    private val braille: BrailleInput?,
    /** For the system's clipboard; none leaves copying and pasting doing nothing, as in a test. */
    context: Context?,
    override val words: OrbitWords = OrbitWords(),
) : EditBox.Output {
    private val clipboard = context?.getSystemService(ClipboardManager::class.java)

    override fun focusEdit(name: String, content: String, cursor: Int, line: String, multiline: Boolean): Unit =
        reader.focusEdit(name, content, cursor, line, multiline)

    override fun syncEdit(name: String, content: String, cursor: Int): Unit = reader.editChanged(name, content, cursor, "field_opened")

    /**
     * Sent as its name, the character itself left empty: the reader then says it by the person's
     * character echo but keeps it out of the word it echoes, which would otherwise be made of the
     * characters a contracted word's cells echoed. A space goes as itself, which ends no word now.
     */
    override fun typed(character: Char): Unit = if (character == ' ') {
        reader.characterTyped(" ", " ", false, null)
    } else {
        reader.characterTyped("", character.toString(), character.isUpperCase(), null)
    }

    /** Said as the reader says an echo, stopping what was being said. */
    override fun word(word: String): Boolean {
        if (braille?.typingEcho?.words != true) return false
        reader.speak(word)
        return true
    }

    override val contracted: Boolean get() = braille?.contracted == true
    override val computer: Boolean get() = braille?.computer == true

    override fun copy(text: String) {
        clipboard?.setPrimaryClip(ClipData.newPlainText("text", text))
    }

    override fun paste(): String? = runCatching {
        clipboard?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.text?.toString()
    }.getOrNull()

    override fun say(text: String): Unit = reader.speak(text)
    override fun candidate(character: Char, field: String): Unit = reader.characterTyped(character.toString(), field, false, null)
    override fun edited(name: String, content: String, cursor: Int, change: String): Unit = reader.editChanged(name, content, cursor, change)
    override fun playError(): Unit = reader.sound(OrbitReader.SOUND_ERROR)

    override fun commitFeedback(character: Char, mode: Int) {
        if (mode == 1 || mode == 4) reader.sound(OrbitReader.SOUND_COMMIT)
        if (mode == 3 || mode == 5) reader.vibrate(50)
        if (mode == 2 || mode == 4 || mode == 5) reader.speakQueued(character.toString())
    }

    override fun modeChanged(name: String, content: String, cursor: Int, mode: String): Unit =
        reader.editChanged(name, content, cursor, "t9_mode_changed:$mode")
}
