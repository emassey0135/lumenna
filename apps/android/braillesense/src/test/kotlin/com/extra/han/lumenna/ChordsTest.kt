package com.extra.han.lumenna

import com.extra.han.lumenna.device.Chords
import com.extra.han.lumenna.device.Chords.DOT1
import com.extra.han.lumenna.device.Chords.DOT2
import com.extra.han.lumenna.device.Chords.DOT3
import com.extra.han.lumenna.device.Chords.DOT4
import com.extra.han.lumenna.device.Chords.DOT5
import com.extra.han.lumenna.device.Chords.DOT6
import com.extra.han.lumenna.device.Chords.DOT7
import com.extra.han.lumenna.device.Chords.DOT8
import com.extra.han.lumenna.device.Chords.ENTER
import com.extra.han.lumenna.device.Chords.SPACE
import com.extra.han.lumenna.device.Key
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ChordsTest {
    @Test fun aRealKeyPressCarriesItsChordInTheThirdField() =
        assertEquals(Key.Down, Chords.decode(-1, 20, "20 0 ${SPACE or DOT4}", false))

    @Test fun anInjectedChordIsTheValueItself() =
        assertEquals(Key.Menu, Chords.decode(-1, 29696, "29696 0 0", false))

    @Test fun onlyKeysAreKeys() = assertNull(Chords.decode(2037, 0, null, false))

    @Test fun spaceWithADotPatternIsItsCommand() {
        assertEquals(Key.Up, Chords.fromChord(SPACE or DOT1))
        assertEquals(Key.Down, Chords.fromChord(SPACE or DOT4))
        assertEquals(Key.First, Chords.fromChord(SPACE or DOT1 or DOT2 or DOT3))
        assertEquals(Key.Last, Chords.fromChord(SPACE or DOT4 or DOT5 or DOT6))
        assertEquals(Key.Escape, Chords.fromChord(SPACE or DOT1 or DOT5))
        assertEquals(Key.Exit, Chords.fromChord(SPACE or DOT1 or DOT3 or DOT5 or DOT6))
        assertEquals(Key.Help, Chords.fromChord(SPACE or DOT1 or DOT2 or DOT5))
        assertEquals(Key.Tab, Chords.fromChord(SPACE or DOT4 or DOT5))
        assertEquals(Key.ShiftTab, Chords.fromChord(SPACE or DOT1 or DOT2))
    }

    @Test fun anythingElseWithSpaceIsALetterCommand() =
        assertEquals(Key.SpaceDots(0x2d), Chords.fromChord(SPACE or DOT1 or DOT3 or DOT4 or DOT6)) // x

    @Test fun spaceAloneAndEnterAloneAreThemselves() {
        assertEquals(Key.Space, Chords.fromChord(SPACE))
        assertEquals(Key.Enter, Chords.fromChord(ENTER))
        assertEquals(Key.Backspace, Chords.fromChord(DOT7))
    }

    @Test fun enterWithILettersTheActionsMenu() =
        assertEquals(Key.Context, Chords.fromChord(ENTER or DOT2 or DOT4))

    @Test fun aCellAloneIsACell() =
        assertEquals(Key.Cell(0x11), Chords.fromChord(DOT1 or DOT5)) // e

    @Test fun eightDotModeMakesDotsSevenAndEightDots() {
        assertEquals(Key.Cell(0x41), Chords.fromChord(DOT1 or DOT7, eightDot = true))
        assertEquals(Key.Cell(0x81), Chords.fromChord(DOT1 or DOT8, eightDot = true))
    }

    @Test fun spaceWithBothLowerDotsSilences() =
        assertEquals(Key.Silence, Chords.fromChord(SPACE or DOT7 or DOT8))

    @Test fun theScrollKeysPan() {
        assertEquals(Key.ScrollBack, Chords.fromChord(Chords.SCROLL_LEFT_UP))
        assertEquals(Key.ScrollOn, Chords.fromChord(Chords.SCROLL_RIGHT_DOWN))
    }

    @Test fun routingKeysNameTheirCell() =
        assertEquals(Key.Route(3), Chords.fromChord(Chords.ROUTE_FIRST + 3))
}
