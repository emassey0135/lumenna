package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.BlockFields
import dev.emassey0135.lumenna.app.TaskFields
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/** The forms' fields by the core's keys, which the phone's and the watch's forms draw. */
class FormFieldsTest {
    private val task = TaskFields("Essay", "", "", 4u, "", "Inbox", "", "", "", "", "", "🎯")

    private val block = BlockFields("Deep work", "09:00", "60", "work", true, true, false, "", "", "", "", "teal", "🧠", "")

    @Test
    fun theEmojiIsALineOfTheTaskFormHeldAndChangedByItsKey() {
        // A key the mapping lacks is drawn as the priority's choice, so the Emoji must be text.
        assertEquals("🎯", task.text("emoji"))
        assertEquals("📞", task.with("emoji", "📞").emoji)
        assertEquals("", task.with("emoji", "").text("emoji"))
        assertFalse("an emoji is not corrected as it is typed", isProse("emoji"))
    }

    @Test
    fun theEmojiIsALineOfTheBlockFormBesideItsColour() {
        assertEquals("🧠", block.text("emoji"))
        assertEquals("teal", block.text("colour"))
    }

    @Test
    fun anEmojiIsDrawnBeforeATitleThatHasOneAndNothingIsAddedToOneThatHasNone() {
        assertEquals("🧠 Deep work", withEmoji("🧠", "Deep work"))
        assertEquals("Deep work", withEmoji(null, "Deep work"))
        assertEquals("Deep work", withEmoji("", "Deep work"))
    }
}
