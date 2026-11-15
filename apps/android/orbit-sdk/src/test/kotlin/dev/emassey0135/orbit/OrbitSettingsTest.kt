package dev.emassey0135.orbit

import org.junit.Assert.assertEquals
import org.junit.Test

class OrbitSettingsTest {
    @Test fun eachMarkedQueryGivesItsKeysValue() {
        val output = "@@key_selection_mode\nRow: 0 key=key_selection_mode, value=Contracted Braille, type=string\n" +
            "@@key_contracted_braille_input_table\nRow: 0 key=key_contracted_braille_input_table, value=en-ueb-g2.ctb, type=string\n" +
            "@@key_computer_braille_input_table\nNo result found.\n"
        assertEquals(
            mapOf("key_selection_mode" to "Contracted Braille", "key_contracted_braille_input_table" to "en-ueb-g2.ctb"),
            OrbitSettings.parseMarked(output),
        )
    }

    @Test fun theTypingEchoSaysWhetherWordsAndCharactersAreEchoed() {
        assertEquals(true, OrbitSettings.TypingEcho.of(3).words)
        assertEquals(true, OrbitSettings.TypingEcho.of(3).characters)
        assertEquals(false, OrbitSettings.TypingEcho.of(1).words)
        assertEquals(OrbitSettings.TypingEcho.CHARACTERS, OrbitSettings.TypingEcho.of(9))
    }
}
