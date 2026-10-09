package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.ForcedSize
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.accessibility.enableAccessibilityChecks
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performCustomAccessibilityActionWithLabel
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.google.android.apps.common.testing.accessibility.framework.AccessibilityCheckResultUtils.matchesCheck
import com.google.android.apps.common.testing.accessibility.framework.checks.TouchTargetSizeCheck
import com.google.android.apps.common.testing.accessibility.framework.integrations.espresso.AccessibilityValidator
import java.io.File
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * A wide window's sidebar, as the desktop apps and the iPad have: every place, headings and
 * project trees that fold by their actions and say which they are, and each place's own
 * actions, driven as TalkBack reaches them.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class SidebarTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var directory: File
    private lateinit var core: Core

    @Before
    fun open() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        directory = File(context.cacheDir, "test-${UUID.randomUUID()}")
        core = Core(directory)
        Clock.update(context)
        val width = 1000.dp
        // Wider than a phone's screen, the density shrinks to fit, and with it every touch
        // target; on the desktop-sized emulator they are checked as they are (KeyboardTest).
        val shrunk = width.value > context.resources.configuration.screenWidthDp
        rule.enableAccessibilityChecks(
            AccessibilityValidator().setRunChecksFromRootView(true).apply {
                if (shrunk) setSuppressingResultMatcher(matchesCheck(TouchTargetSizeCheck::class.java))
            },
        )
        rule.setContent {
            BoxWithConstraints {
                DeviceConfigurationOverride(DeviceConfigurationOverride.ForcedSize(DpSize(width, maxHeight))) {
                    LumennaTheme { LumennaApp(core) }
                }
            }
        }
        rule.waitForIdle()
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    private fun place(title: String): SemanticsNodeInteraction =
        rule.onNode(SemanticsMatcher("row titled $title") { it.config.getOrNull(RowTitle) == title })

    private fun gone(title: String) {
        rule.waitUntil(5_000) {
            rule.onAllNodes(SemanticsMatcher("row titled $title") { it.config.getOrNull(RowTitle) == title })
                .fetchSemanticsNodes().isEmpty()
        }
    }

    private fun says(title: String, words: String): Boolean =
        rule.onAllNodes(SemanticsMatcher("row titled $title") { it.config.getOrNull(RowTitle) == title })
            .fetchSemanticsNodes()
            .any { it.config.getOrNull(SemanticsProperties.ContentDescription)?.joinToString()?.contains(words) == true }

    private fun act(title: String, action: String) {
        place(title).performCustomAccessibilityActionWithLabel(action)
        rule.waitForIdle()
    }

    @Test
    fun theSidebarListsEveryPlaceAndSaysWhichIsShown() {
        for (title in listOf("Today", "Tasks", "Projects", "Inbox", "Labels", "Saved Filters", "Blocks", "Trash", "Settings")) {
            place(title).assertExists()
        }
        place("Today").assertIsSelected()
        place("Inbox").performClick()
        rule.waitForIdle()
        place("Inbox").assertIsSelected()
        rule.onNode(hasText("Inbox") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading)).assertExists()
    }

    @Test
    fun aHeadingCollapsesAndExpandsByItsActionsAndSaysWhichItIs() {
        assertTrue(says("Projects", "expanded"))
        act("Projects", "Collapse")
        gone("Inbox")
        assertTrue(says("Projects", "collapsed"))
        act("Projects", "Expand")
        place("Inbox").assertExists()
    }

    @Test
    fun aProjectIsMadeFromItsHeadingAndShownWhenChosen() {
        act("Projects", "New Project")
        rule.onNode(hasSetTextAction() and hasText("Name")).performTextInput("Garden")
        rule.onNode(hasText("Add") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Role)).performClick()
        rule.waitUntil(5_000) {
            rule.onAllNodes(SemanticsMatcher("row titled Garden") { it.config.getOrNull(RowTitle) == "Garden" })
                .fetchSemanticsNodes().isNotEmpty()
        }
        place("Garden").performClick()
        rule.waitForIdle()
        place("Garden").assertIsSelected()
    }

    @Test
    fun aLabelHasTheActionsBrowseGivesIt() {
        core.lumenna.addLabel("calls")
        rule.runOnIdle { core.changed() }
        rule.waitForIdle()
        val actions = place("calls").fetchSemanticsNode().config.getOrNull(SemanticsActions.CustomActions).orEmpty().map { it.label }
        assertTrue(actions.toString(), listOf("Rename", "Merge Into", "Colour", "Delete").all { it in actions })
    }
}
