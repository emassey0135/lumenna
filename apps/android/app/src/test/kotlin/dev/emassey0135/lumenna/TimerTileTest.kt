package dev.emassey0135.lumenna

import android.service.quicksettings.Tile
import dev.emassey0135.lumenna.app.TimerControl
import org.junit.Assert.assertEquals
import org.junit.Test

class TimerTileTest {
    private val running = TimerControl(true, "Essay", "Pause timer", "Pause the timer", true)

    @Test
    fun aRunningTimerIsOnAndSaysItsTaskAndWhatAPressDoes() {
        val shown = TimerTile.shown(running, locked = false, name = "Timer")
        assertEquals(Tile.STATE_ACTIVE, shown.state)
        assertEquals("Essay", shown.subtitle)
        assertEquals("Timer, Pause the timer, Essay", shown.description)
    }

    @Test
    fun behindALockTheTaskIsNeverShownOrSaid() {
        val shown = TimerTile.shown(running, locked = true, said = "Paused Essay", name = "Timer")
        assertEquals("Pause the timer", shown.subtitle)
        assertEquals("Timer, Pause the timer", shown.description)
    }

    @Test
    fun afterAPressTheCoresAnnouncementIsTheSubtitle() {
        assertEquals("Paused Essay", TimerTile.shown(running, locked = false, said = "Paused Essay", name = "Timer").subtitle)
    }

    @Test
    fun aTimerThatCannotStartIsUnavailableAndOneNotRunningIsOff() {
        val nothing = TimerControl(false, "Nothing to time", "Start timer", "Start the timer", false)
        assertEquals(Tile.STATE_UNAVAILABLE, TimerTile.shown(nothing, locked = false, name = "Timer").state)
        assertEquals(Tile.STATE_INACTIVE, TimerTile.shown(nothing.copy(enabled = true), locked = false, name = "Timer").state)
    }
}
