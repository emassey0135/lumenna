package dev.emassey0135.lumenna

import android.service.quicksettings.Tile
import dev.emassey0135.lumenna.app.TimerControl
import org.junit.Assert.assertEquals
import org.junit.Test

class TimerTileTest {
    private val running = TimerControl(true, "Essay", "Pause timer", "Pause the timer", true)

    // The core's `timer_described`, its parts shown, without the native library.
    private val described = { name: String, action: String, detail: String, locked: Boolean -> "$name|$action|$detail|$locked" }

    @Test
    fun aRunningTimerIsOnAndSaysItsTaskAndWhatAPressDoes() {
        val shown = TimerTile.shown(running, locked = false, name = "Timer", described = described)
        assertEquals(Tile.STATE_ACTIVE, shown.state)
        assertEquals("Essay", shown.subtitle)
        assertEquals("Timer|Pause the timer|Essay|false", shown.description)
    }

    @Test
    fun behindALockTheTaskIsNeverShownOrSaid() {
        val shown = TimerTile.shown(running, locked = true, said = "Paused Essay", name = "Timer", described = described)
        assertEquals("Pause the timer", shown.subtitle)
        assertEquals("Timer|Pause the timer|Pause the timer|true", shown.description)
    }

    @Test
    fun afterAPressTheCoresAnnouncementIsTheSubtitle() {
        assertEquals("Paused Essay", TimerTile.shown(running, locked = false, said = "Paused Essay", name = "Timer", described = described).subtitle)
    }

    @Test
    fun aTimerThatCannotStartIsUnavailableAndOneNotRunningIsOff() {
        val nothing = TimerControl(false, "Nothing to time", "Start timer", "Start the timer", false)
        assertEquals(Tile.STATE_UNAVAILABLE, TimerTile.shown(nothing, locked = false, name = "Timer", described = described).state)
        assertEquals(Tile.STATE_INACTIVE, TimerTile.shown(nothing.copy(enabled = true), locked = false, name = "Timer", described = described).state)
    }
}
