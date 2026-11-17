package dev.emassey0135.orbit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ListsTest {
    @Test fun upAndDownStopAtTheEndsAndTabGoesRound() {
        assertEquals(0, OrbitList.target(Key.Up, 0, 3))
        assertEquals(2, OrbitList.target(Key.Down, 2, 3))
        assertEquals(0, OrbitList.target(Key.Tab, 2, 3))
        assertEquals(2, OrbitList.target(Key.BackTab, 0, 3))
        assertEquals(2, OrbitList.target(Key.Last, 0, 3))
        assertNull(OrbitList.target(Key.Select, 0, 3))
        assertNull("an empty list moves nowhere", OrbitList.target(Key.Down, 0, 0))
    }

    @Test fun aFirstLetterFindsTheNextRowWrappingRound() {
        val lines = listOf("apple", "banana", "avocado")
        assertEquals(2, ListFinder.next(lines, 0, "a"))
        assertEquals(0, ListFinder.next(lines, 2, "A"))
        assertNull(ListFinder.next(lines, 0, "z"))
    }

    private class Finding(override val count: Int) : ListFinder.Host {
        val opened = mutableListOf<Int>()
        val found = mutableListOf<String>()
        var refused = 0
        val letters = mutableListOf<Boolean>()
        override val commitMs: Long = 500
        override fun find(letter: String) { found += letter }
        override fun open(index: Int) { opened += index }
        override fun refuse() { refused++ }
        override fun speak(text: String) {}
        override fun modeChanged(mode: T9Input.Mode) {}
        override fun lettersChanged(on: Boolean) { letters += on }
    }

    private class Manual : T9Input.Later {
        var run: (() -> Unit)? = null
        override fun after(milliseconds: Long, run: () -> Unit) { this.run = run }
        override fun cancel() { run = null }
    }

    @Test fun aDigitNoLongerNumberCanFollowOpensItsRowAtOnce() {
        val host = Finding(5)
        val finder = ListFinder(host, Manual())
        assertTrue(finder.phoneKey(Key.Digit(3)))
        assertEquals(listOf(2), host.opened)
    }

    @Test fun zeroAloneIsTheTenthRowAndANumberPastTheListIsRefused() {
        val host = Finding(12)
        val finder = ListFinder(host, Manual())
        finder.phoneKey(Key.Digit(0))
        assertEquals(listOf(9), host.opened)
        val short = Finding(3)
        ListFinder(short, Manual()).phoneKey(Key.Digit(7))
        assertEquals(1, short.refused)
        assertEquals(emptyList<Int>(), short.opened)
    }

    @Test fun holdingStarTurnsLettersOnAndAHeldDigitIsThenFound() {
        val host = Finding(5)
        val finder = ListFinder(host, Manual())
        finder.phoneKey(Key.StarHeld)
        assertTrue(finder.letters)
        assertEquals(listOf(true), host.letters)
        finder.phoneKey(Key.DigitHeld(4))
        assertEquals(listOf("4"), host.found)
        assertFalse("the list's own keys go on", finder.phoneKey(Key.Up))
    }

    @Test fun aDialogsTabMovesAndItsCancelKeysCancel() {
        assertEquals(OrbitDialog.Does.Next, OrbitDialog.does(Key.Tab, OrbitDialog.Control.EDIT))
        assertEquals(OrbitDialog.Does.Previous, OrbitDialog.does(Key.BackTab, OrbitDialog.Control.BUTTON))
        assertEquals(OrbitDialog.Does.Cancel, OrbitDialog.does(Key.Escape, OrbitDialog.Control.CHECK_BOX))
        assertEquals(OrbitDialog.Does.Cancel, OrbitDialog.does(Key.Dot7, OrbitDialog.Control.BUTTON))
        assertEquals(OrbitDialog.Does.NotTaken, OrbitDialog.does(Key.Menu, OrbitDialog.Control.COMBO_BOX))
    }

    @Test fun anEditBoxTakesItsOwnKeysDotSevenAmongThem() =
        assertEquals(OrbitDialog.Does.Edit, OrbitDialog.does(Key.Dot7, OrbitDialog.Control.EDIT))

    @Test fun upAndDownChangeAChoiceOrACheckAndLeftAndRightNeverLeave() {
        assertEquals(OrbitDialog.Does.Step(-1), OrbitDialog.does(Key.Up, OrbitDialog.Control.COMBO_BOX))
        assertEquals(OrbitDialog.Does.Find(0x01), OrbitDialog.does(Key.Cell(0x01), OrbitDialog.Control.LIST_BOX))
        assertEquals(OrbitDialog.Does.Toggle, OrbitDialog.does(Key.Down, OrbitDialog.Control.CHECK_BOX))
        assertEquals(OrbitDialog.Does.Nothing, OrbitDialog.does(Key.Left, OrbitDialog.Control.BUTTON))
        assertEquals(OrbitDialog.Does.Press, OrbitDialog.does(Key.Dot8, OrbitDialog.Control.BUTTON))
        assertEquals(OrbitDialog.Does.Nothing, OrbitDialog.does(Key.Select, OrbitDialog.Control.TEXT))
    }
}
