package dev.emassey0135.lumenna

import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.datastore.preferences.core.emptyPreferences
import androidx.glance.appwidget.compose
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
import dev.emassey0135.lumenna.app.GlancePart
import dev.emassey0135.lumenna.widgets.TodayWidget
import dev.emassey0135.lumenna.widgets.Widgets
import kotlinx.coroutines.runBlocking
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

    @Test
    fun todaysWidgetEndsInNewTaskAndNowsNeverHasIt() {
        assertEquals(GlancePart.NEW_TASK, Widgets.parts("today").last())
        assertTrue(GlancePart.NEW_TASK !in Widgets.parts("now"))
        assertEquals(Launches.NEW_TASK, Launches.id(Widgets.newTask(context)))
        assertEquals(Launches.shortcut(Launches.NEW_TASK)?.sentence, Widgets.newTaskWords())
    }

    @Test
    fun todaysWidgetDrawsNewTaskAsAButtonOfItsOwnNamedByTheCore() {
        val views = runBlocking { TodayWidget().compose(context, size = DpSize(320.dp, 480.dp), state = emptyPreferences()) }
        var drawn: View? = null
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            drawn = views.apply(context, FrameLayout(context))
        }
        val words = Widgets.newTaskWords()
        val named = all(drawn!!).filter { it.contentDescription?.toString() == words }
        assertTrue("a view named $words", named.isNotEmpty())
        // Pressed by itself, not by the whole widget's tap that opens Today.
        val widget = all(drawn!!).first { it.hasOnClickListeners() }
        val pressed = named.mapNotNull { view -> generateSequence(view) { it.parent as? View }.firstOrNull { it.hasOnClickListeners() } }
        assertTrue("its own press", pressed.any { it !== widget })
    }

    private fun all(view: View): List<View> =
        listOf(view) + if (view is ViewGroup) (0 until view.childCount).flatMap { all(view.getChildAt(it)) } else emptyList()
}
