package dev.emassey0135.lumenna.orbit

import org.junit.Assert.assertEquals
import org.junit.Test

class KeysTest {
    private val decoder = KeyDecoder()
    private val press = KeyEvent.PRESS
    private val release = KeyEvent.RELEASE
    private val chord = KeyEvent.CHORD

    private fun key(name: String, action: Int, long: Boolean = false) = decoder.decode(KeyEvent(name, action, long))

    @Test fun anArrowPressedMovesAndHeldGoesToTheEnd() {
        assertEquals(Key.Up, key("UP", press))
        assertEquals(Key.First, key("UP", press, long = true))
        assertEquals(Key.Last, key("DOWN", press, long = true))
    }

    @Test fun aReleaseOfAnArrowIsNothing() {
        assertEquals(Key.None, key("UP", release))
    }

    @Test fun dotSevenAndDotEightAloneAreBackAndOpen() {
        assertEquals(Key.Dot7, key("DOT7", release))
        assertEquals(Key.Dot8, key("DOT8", release))
    }

    @Test fun aChordOfSpaceAndDotsIsACommand() {
        assertEquals(Key.Chord(0b10001), key("SPACE_DOT1_DOT5", chord))
    }

    @Test fun dotsWithoutSpaceAreACell() {
        assertEquals(Key.Cell(0b10011), key("DOT1_DOT2_DOT5", chord))
        assertEquals(Key.Cell(1), key("DOT1", release))
    }

    @Test fun aDigitActsOnReleaseAndAHeldOneOnlyOnce() {
        assertEquals(Key.None, key("5", press))
        assertEquals(Key.Digit(5), key("5", release))
        assertEquals(Key.DigitHeld(7), key("7", press, long = true))
        assertEquals(Key.None, key("7", release, long = true))
    }

    @Test fun starAndPoundHeldAndTapped() {
        assertEquals(Key.StarHeld, key("*", press, long = true))
        assertEquals(Key.None, key("*", release, long = true))
        assertEquals(Key.Star, key("*", release))
        assertEquals(Key.Pound, key("#", release))
    }

    @Test fun thePlayersBackAndMenu() {
        assertEquals(Key.Back, key("BACK", press))
        assertEquals(Key.BackReleased(held = true), key("BACK", release, long = true))
        assertEquals(Key.Menu, key("MENU", release))
    }

    @Test fun somethingElseIsNothing() {
        assertEquals(Key.None, key("F4", press))
        assertEquals(Key.None, key("SPACE_F4", chord))
    }
}
