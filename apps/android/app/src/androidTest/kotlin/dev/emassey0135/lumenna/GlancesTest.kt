package dev.emassey0135.lumenna

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/** What the widgets, the tile and the complications show: the core's glance, at the moment it names. */
@RunWith(AndroidJUnit4::class)
class GlancesTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val directory = File(context.cacheDir, "glance-${UUID.randomUUID()}")

    @After
    fun tearDown() {
        directory.deleteRecursively()
    }

    @Test
    fun aBlockLaterTodayIsWhatTheGlanceShowsOnceItHasStarted() {
        val start = LocalTime.now().plusHours(2).withMinute(0).withSecond(0).withNano(0)
        // A block two hours on must still be today and end by midnight.
        assumeTrue(LocalTime.now().isBefore(LocalTime.of(20, 30)))
        val core = Core(directory)
        core.device.addBlock(newBlock("Deep work", "%02d:00".format(start.hour), 60u, LocalDate.now().toString(), "work", null))
        val entries = Glances.entries(context, core)
        assertEquals("Free", Glances.now(entries)?.headline?.title)
        val starts = start.atDate(LocalDate.now()).atZone(ZoneId.systemDefault()).toInstant()
        val then = Glances.now(entries, starts.plusSeconds(60))
        assertNotNull("the timeline names the block's start", then?.moment?.current)
        assertEquals("Deep work", then?.headline?.title)
        assertTrue("TalkBack hears it whole: ${then?.spoken}", then?.spoken.orEmpty().startsWith("Deep work"))
    }
}
