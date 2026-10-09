package io.github.emassey0135.lumenna

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.ForcedSize
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.accessibility.enableAccessibilityChecks
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.requestFocus
import androidx.compose.ui.test.withKeyDown
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.google.android.apps.common.testing.accessibility.framework.AccessibilityCheckResultUtils.matchesCheck
import com.google.android.apps.common.testing.accessibility.framework.checks.TouchTargetSizeCheck
import com.google.android.apps.common.testing.accessibility.framework.integrations.espresso.AccessibilityValidator
import java.io.File
import java.time.LocalDate
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The keyboard commands the Windows and GTK apps have, on Android, and the layout of a wide
 * window: what a laptop, a tablet or a Googlebook gets. Keys are sent to whatever has focus,
 * as a keyboard sends them.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class KeyboardTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var directory: File
    private lateinit var core: Core
    private val shortcuts = Shortcuts()

    @Before
    fun open() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        directory = File(context.cacheDir, "test-${UUID.randomUUID()}")
        core = Core(directory)
        Clock.update(context)
        // As with a keyboard attached: a key press takes Android out of touch mode, and only
        // then can buttons and tabs take focus.
        InstrumentationRegistry.getInstrumentation().setInTouchMode(false)
    }

    @After
    fun close() {
        InstrumentationRegistry.getInstrumentation().setInTouchMode(true)
        directory.deleteRecursively()
    }

    /**
     * The app in a window [width] wide: a phone's by default. Only the width is forced; the
     * height is the window's, since a forced size larger than the window is drawn at a
     * smaller density to fit, and every touch target measures smaller than it is.
     */
    private fun show(width: Dp = 400.dp) {
        val screen = InstrumentationRegistry.getInstrumentation().targetContext.resources.configuration
        // Wider than this screen — a phone asked for a wide window — the density shrinks, so
        // touch targets are excused, and only then: on a tablet or a desktop they are checked.
        val shrunk = width.value > screen.screenWidthDp
        rule.enableAccessibilityChecks(
            AccessibilityValidator().setRunChecksFromRootView(true).apply {
                if (shrunk) setSuppressingResultMatcher(matchesCheck(TouchTargetSizeCheck::class.java))
            },
        )
        rule.setContent {
            BoxWithConstraints {
                DeviceConfigurationOverride(DeviceConfigurationOverride.ForcedSize(DpSize(width, maxHeight))) {
                    LumennaTheme { LumennaApp(core, shortcuts = shortcuts) }
                }
            }
        }
        rule.waitForIdle()
    }

    private fun tab(name: String): SemanticsNodeInteraction =
        rule.onNode(hasText(name) and SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Tab))

    private fun row(title: String): SemanticsNodeInteraction = rule.onNode(titled(title))

    private fun titled(title: String) = SemanticsMatcher("row titled $title") { it.config.getOrNull(RowTitle) == title }

    private fun field(label: String) = rule.onNode(hasSetTextAction() and hasText(label))

    /** Focus on [node], then [key], with Ctrl and Shift held as asked. */
    private fun press(node: SemanticsNodeInteraction, key: Key, ctrl: Boolean = false, shift: Boolean = false) {
        node.requestFocus()
        rule.waitForIdle()
        node.performKeyInput {
            val press = { pressKey(key) }
            val shifted = { if (shift) withKeyDown(Key.ShiftLeft) { press() } else press() }
            if (ctrl) withKeyDown(Key.CtrlLeft) { shifted() } else shifted()
        }
        rule.waitForIdle()
    }

    private fun shows(text: String) {
        rule.waitUntil(5_000) { rule.onAllNodes(hasText(text, substring = true)).fetchSemanticsNodes().isNotEmpty() }
    }

    private fun gone(title: String) {
        rule.waitUntil(5_000) { rule.onAllNodes(titled(title)).fetchSemanticsNodes().isEmpty() }
    }

    private fun seed(title: String) {
        core.lumenna.addTask(title)
        rule.runOnIdle { core.changed() }
        rule.waitForIdle()
    }

    @Test
    fun ctrlNOpensQuickAddFromAnotherTab() {
        show()
        press(tab("Settings"), Key.N, ctrl = true)
        field("Task").assertIsFocused()
    }

    @Test
    fun ctrlFOpensTheTaskListWithItsFilterFocused() {
        show()
        press(tab("Today"), Key.F, ctrl = true)
        rule.waitUntil(5_000) { runCatching { field("Filter").assertIsFocused() }.isSuccess }
    }

    @Test
    fun ctrlKMarksTheFocusedTaskDone() {
        show()
        seed("water the plants")
        tab("Tasks").performClick()
        press(row("water the plants"), Key.K, ctrl = true)
        gone("water the plants")
        assertTrue(core.lumenna.listTasks("completed").rows.any { it.title == "water the plants" })
    }

    @Test
    fun deleteMovesTheFocusedTaskToTheTrash() {
        show()
        seed("old errand")
        tab("Tasks").performClick()
        press(row("old errand"), Key.Delete)
        gone("old errand")
        assertTrue(core.lumenna.listTasks("deleted").rows.any { it.title == "old errand" })
    }

    @Test
    fun ctrlPageDownShowsTheNextDayAndCtrlTComesBackToNow() {
        show()
        val tomorrow = Clock.spokenDay(LocalDate.now().plusDays(1).toString())
        press(tab("Today"), Key.PageDown, ctrl = true)
        shows(tomorrow)
        press(tab("Today"), Key.T, ctrl = true)
        shows(Clock.spokenDay(LocalDate.now().toString()))
    }

    @Test
    fun theShortcutsHelperListsWhatWorksOnTheScreenShown() {
        show()
        rule.runOnIdle {
            val listed = shortcuts.groups().associate { group -> group.label.toString() to group.items.map { it.label.toString() } }
            assertTrue("New task" in listed.getValue("Lumenna"))
            assertTrue("Previous day" in listed.getValue("Day"))
            assertTrue("Mark done or not done" in listed.getValue("In a list"))
        }
    }

    @Test
    fun aWideWindowHasItsTabsAtTheSideAndATaskOpensBesideItsList() {
        show(width = 1000.dp)
        seed("plan the trip")
        tab("Tasks").performClick()
        row("plan the trip").performClick()
        rule.waitForIdle()
        // The list stays, with the task's details beside it.
        row("plan the trip").assertExists()
        rule.onNodeWithContentDescription("Save").assertExists()
        // Ctrl+S saves what is open beside the list.
        press(row("plan the trip"), Key.S, ctrl = true)
        shows("Nothing changed")
    }

    @Test
    fun f6MovesBetweenTheListAndWhatIsOpenBesideItAndBack() {
        show(width = 1000.dp)
        seed("plan the trip")
        tab("Tasks").performClick()
        row("plan the trip").performClick()
        rule.waitForIdle()
        press(row("plan the trip"), Key.F6)
        rule.onNode(titled("plan the trip")).assertIsNotFocused()
        // Back to the pane before, onto the row it left.
        press(rule.onNode(isFocused()), Key.F6, shift = true)
        row("plan the trip").assertIsFocused()
    }

    @Test
    fun aNarrowWindowOpensATaskInPlaceOfItsList() {
        show()
        seed("plan the trip")
        tab("Tasks").performClick()
        row("plan the trip").performClick()
        gone("plan the trip")
        rule.onNodeWithContentDescription("Save").assertExists()
    }
}
