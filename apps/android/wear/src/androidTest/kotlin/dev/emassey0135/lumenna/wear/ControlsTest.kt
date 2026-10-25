package dev.emassey0135.lumenna.wear

import android.app.PendingIntent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.wear.protolayout.ActionBuilders
import androidx.wear.protolayout.DeviceParametersBuilders
import androidx.wear.protolayout.LayoutElementBuilders
import androidx.wear.protolayout.LayoutElementBuilders.LayoutElement
import androidx.wear.protolayout.ModifiersBuilders
import androidx.wear.protolayout.material3.materialScope
import androidx.wear.watchface.complications.data.ComplicationType
import androidx.wear.watchface.complications.data.ShortTextComplicationData
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Launching
import dev.emassey0135.lumenna.app.controls
import dev.emassey0135.lumenna.app.glanceEmpty
import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** New Task on the watch, the core's control: a complication for a watch face, and the tile's button. */
@RunWith(AndroidJUnit4::class)
class ControlsTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val words = controls().first { it.id == "new-task" }.sentence

    @Test
    fun theComplicationSaysNewTaskInTheCoresWordsAndOpensQuickAdd() {
        val data = NewTaskComplication.data(context, ComplicationType.SHORT_TEXT) as ShortTextComplicationData
        assertEquals(words, data.text.getTextAt(context.resources, Instant.now()).toString())
        assertEquals(words, data.contentDescription?.getTextAt(context.resources, Instant.now()).toString())
        assertNotNull("drawn with its icon", data.monochromaticImage)
        val intent = newTaskIntent(context)
        assertEquals(MainActivity::class.java.name, intent.component?.className)
        assertEquals("new-task", Launching.id(intent))
    }

    @Test
    fun theComplicationIsAnIconOrWordsAndNothingElse() {
        assertNotNull(NewTaskComplication.data(context, ComplicationType.MONOCHROMATIC_IMAGE))
        assertNotNull(NewTaskComplication.data(context, ComplicationType.SMALL_IMAGE))
        assertNull(NewTaskComplication.data(context, ComplicationType.RANGED_VALUE))
    }

    @Test
    fun theComplicationsTapIsNotTheNowComplicationsOpeningToday() {
        // Intents differing only in their extras are one pending intent unless their codes differ.
        val today = PendingIntent.getActivity(
            context, 0, Launching.intent(context, MainActivity::class.java, Launching.TODAY),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        assertNotEquals(today, NewTaskComplication.tap(context))
    }

    @Test
    fun theTilesNewTaskButtonIsAStopOfItsOwnBeneathTheGlanceOpeningQuickAdd() {
        Clock.update(context)
        val device = DeviceParametersBuilders.DeviceParameters.Builder()
            .setScreenWidthDp(192).setScreenHeightDp(192).setScreenDensity(2f)
            .setScreenShape(DeviceParametersBuilders.SCREEN_SHAPE_ROUND)
            .setDevicePlatform(DeviceParametersBuilders.DEVICE_PLATFORM_WEAR_OS)
            .build()
        val layout = materialScope(context, device) {
            glanceLayout(glanceEmpty(Clock.locale), launching(context, Launching.TODAY), launching(context, "new-task"))
        }
        val clickables = clickables(layout)
        val opened = clickables.associate { (clickable, _) -> launched(clickable) to clickable }
        assertTrue("the glance still opens Today: ${opened.keys}", Launching.TODAY in opened.keys)
        val button = clickables.single { (clickable, _) -> launched(clickable) == "new-task" }
        assertTrue("named by the core: ${button.second}", words in button.second)
    }

    /** The launch `id` a clickable opens the app on. */
    private fun launched(clickable: ModifiersBuilders.Clickable): String? {
        val action = clickable.onClick as? ActionBuilders.LaunchAction ?: return null
        assertEquals(MainActivity::class.java.name, action.androidActivity?.className)
        return (action.androidActivity?.keyToExtraMapping?.get(Launching.ID) as? ActionBuilders.AndroidStringExtra)?.value
    }

    /** Every clickable in [element], with what TalkBack is told of it and what it holds. */
    private fun clickables(element: LayoutElement): List<Pair<ModifiersBuilders.Clickable, List<String>>> {
        val found = mutableListOf<Pair<ModifiersBuilders.Clickable, List<String>>>()
        fun walk(element: LayoutElement) {
            val (modifiers, contents) = when (element) {
                is LayoutElementBuilders.Box -> element.modifiers to element.contents
                is LayoutElementBuilders.Column -> element.modifiers to element.contents
                is LayoutElementBuilders.Row -> element.modifiers to element.contents
                is LayoutElementBuilders.Text -> element.modifiers to emptyList()
                is LayoutElementBuilders.Spacer -> element.modifiers to emptyList()
                is LayoutElementBuilders.Image -> element.modifiers to emptyList()
                else -> null to emptyList()
            }
            modifiers?.clickable?.let { found += it to descriptions(element) }
            contents.forEach(::walk)
        }
        walk(element)
        return found
    }

    /** The content descriptions in [element] and beneath it. */
    private fun descriptions(element: LayoutElement): List<String> {
        val (modifiers, contents) = when (element) {
            is LayoutElementBuilders.Box -> element.modifiers to element.contents
            is LayoutElementBuilders.Column -> element.modifiers to element.contents
            is LayoutElementBuilders.Row -> element.modifiers to element.contents
            is LayoutElementBuilders.Text -> element.modifiers to emptyList()
            else -> null to emptyList()
        }
        return listOfNotNull(modifiers?.semantics?.contentDescription?.value) + contents.flatMap(::descriptions)
    }
}
