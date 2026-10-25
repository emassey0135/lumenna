package dev.emassey0135.lumenna

import android.service.quicksettings.Tile
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.emassey0135.lumenna.app.TimerControl
import dev.emassey0135.lumenna.app.controls
import java.io.File
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/** The timer's Quick Settings tile: the core's control, drawn as Android's tile states. */
@RunWith(AndroidJUnit4::class)
class TimerTileTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val directory = File(context.cacheDir, "timer-${UUID.randomUUID()}")

    @After
    fun tearDown() {
        directory.deleteRecursively()
    }

    private fun control(running: Boolean, enabled: Boolean = true, detail: String = "Essay") = TimerControl(
        running = running,
        detail = detail,
        action = if (running) "Stop Timer" else "Start Timer",
        actionSentence = if (running) "Stop timer" else "Start timer",
        enabled = enabled,
    )

    @Test
    fun theTileIsOnWhileATimerRunsOffWhenNoneDoesAndUnavailableWhenAPressWouldDoNothing() {
        assertEquals(Tile.STATE_ACTIVE, TimerTile.shown(control(running = true), locked = false).state)
        assertEquals(Tile.STATE_INACTIVE, TimerTile.shown(control(running = false), locked = false).state)
        assertEquals(Tile.STATE_UNAVAILABLE, TimerTile.shown(control(running = false, enabled = false), locked = false).state)
    }

    @Test
    fun theTileIsNamedAsTheCoreNamesItsTimerControlWithTheTaskBesideIt() {
        val core = controls().first { it.id == "timer" }
        val shown = TimerTile.shown(control(running = true), locked = false)
        assertEquals(core.sentence, shown.label)
        assertEquals("Essay", shown.subtitle)
        assertEquals("${core.sentence}, Stop timer, Essay", shown.description)
    }

    @Test
    fun behindASecureLockTheTileSaysWhatAPressDoesAndNeverTheTask() {
        val shown = TimerTile.shown(control(running = true), locked = true, said = "Started Essay")
        assertEquals("Stop timer", shown.subtitle)
        assertFalse(shown.description, shown.description.contains("Essay"))
    }

    @Test
    fun afterAPressTheTileShowsWhatTheCoreSaidOfIt() {
        val shown = TimerTile.shown(control(running = true), locked = false, said = "Timer started on Essay")
        assertEquals("Timer started on Essay", shown.subtitle)
        assertTrue(shown.description, shown.description.endsWith("Timer started on Essay"))
    }

    @Test
    fun aPressStartsOnWhatToWorkOnNowThenStopsIt() {
        val core = Core(directory)
        assertEquals(Tile.STATE_UNAVAILABLE, TimerTile.shown(core.device.timerControl(), locked = false).state)
        core.device.addTask("essay")
        val control = core.device.timerControl()
        // Free time a few minutes before the day's window or midnight ends fits no sitting.
        assumeTrue(control.detail, control.enabled)
        val before = TimerTile.shown(control, locked = false)
        assertEquals(Tile.STATE_INACTIVE, before.state)
        assertEquals("essay", before.subtitle)
        assertTrue(core.device.toggleTimer().announcement.isNotBlank())
        val running = TimerTile.shown(core.device.timerControl(), locked = false)
        assertEquals(Tile.STATE_ACTIVE, running.state)
        assertEquals("essay", running.subtitle)
        core.device.toggleTimer()
        // Stopped. What a press would start next is the core's: the block of its own the first
        // press made is still under way, and a task is offered once a day, so it may be nothing.
        val stopped = core.device.timerControl()
        assertFalse(stopped.running)
        assertNotEquals(Tile.STATE_ACTIVE, TimerTile.shown(stopped, locked = false).state)
    }
}
