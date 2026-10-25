package dev.emassey0135.lumenna

import android.content.Intent
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.test.platform.app.InstrumentationRegistry
import dev.emassey0135.lumenna.app.launchShortcuts
import java.io.File
import java.util.UUID
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** The ways in from outside the app: the launcher's shortcuts, the Quick Settings tile, a widget. */
@RunWith(AndroidJUnit4::class)
class NewTaskTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aNewTaskRequestOpensQuickAddOverTheTaskList() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val core = Core(File(context.cacheDir, "test-${UUID.randomUUID()}"))
        rule.registerIdlingResource(stillApp(core))
        val requests = MutableStateFlow<Pair<Long, String>?>(null)
        rule.setContent { PhoneWidth { LumennaTheme { LumennaApp(core, requests) } } }
        requests.value = 1L to Launches.NEW_TASK
        rule.waitUntil(5_000) {
            rule.onAllNodes(hasSetTextAction() and hasText("Task")).fetchSemanticsNodes().isNotEmpty()
        }
        rule.onNode(hasText("New task")).assertExists()
    }

    @Test
    fun theLaunchersShortcutsAreTheCoresInItsOrderAndWords() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        Launches.publish(context)
        val given = ShortcutManagerCompat.getDynamicShortcuts(context).sortedBy { it.rank }
        assertEquals(launchShortcuts().map { it.id to it.sentence }, given.map { it.id to it.shortLabel.toString() })
        for (shortcut in given) assertEquals(shortcut.id, Launches.id(shortcut.intent))
    }

    @Test
    fun aNewTaskShortcutPinnedBeforeTheCoreListedThemStillOpensQuickAdd() {
        assertEquals(Launches.NEW_TASK, Launches.id(Intent(MainActivity.NEW_TASK)))
    }
}
