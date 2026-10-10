package io.github.emassey0135.lumenna.wear

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.accessibility.disableAccessibilityChecks
import androidx.compose.ui.test.junit4.accessibility.enableAccessibilityChecks
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performCustomAccessibilityActionWithLabel
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import java.io.File
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The watch app driven as TalkBack reaches it: rows and buttons by what they say, actions by
 * their names. Each test has a store of its own, and Google's accessibility checks run on
 * every interaction, as in the phone's tests. What the system's input screen would return
 * is scripted (`answers`), since a test cannot speak or write on it.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class WearTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var directory: File
    private lateinit var core: Core
    private val answers = ArrayDeque<String>()
    private val asked = mutableListOf<String>()

    @Before
    fun open() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        directory = File(context.cacheDir, "test-${UUID.randomUUID()}")
        core = Core(directory)
        Clock.update(context)
        rule.enableAccessibilityChecks()
        val entry = TextEntry { label, answered ->
            asked += label
            answers.removeFirstOrNull()?.let(answered)
        }
        rule.setContent { androidx.wear.compose.material3.MaterialTheme { WearApp(core, entry) } }
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    /** What is on screen with `text`, scrolled to first: a list holds only the rows it shows. */
    private fun shown(text: String, substring: Boolean = false): SemanticsNodeInteraction {
        val matcher = hasText(text, substring = substring) and hasClickAction()
        runCatching { rule.onNode(hasScrollToNodeAction()).performScrollToNode(matcher) }
        return rule.onNode(matcher)
    }

    /** Text that is only read, not pressed, scrolled to first. */
    private fun said(text: String): SemanticsNodeInteraction {
        val matcher = hasText(text, substring = true)
        runCatching { rule.onNode(hasScrollToNodeAction()).performScrollToNode(matcher) }
        return rule.onNode(matcher)
    }

    private fun press(text: String, substring: Boolean = false) {
        shown(text, substring).performClick()
        rule.waitForIdle()
    }

    private fun stateOf(text: String, substring: Boolean = false): String =
        shown(text, substring).fetchSemanticsNode().config.getOrNull(SemanticsProperties.StateDescription).orEmpty()

    @Test
    fun thePlacesAreTheCoresWithTheirHeadingsAndAProject() {
        shown("Today").assertExists()
        shown("Tasks").assertExists()
        shown("Inbox", substring = true).assertExists()
        assertEquals("expanded", stateOf("Projects"))
    }

    @Test
    fun aHeadingFoldsWhatIsUnderItAndSaysWhichItIs() {
        press("Projects")
        assertEquals("collapsed", stateOf("Projects"))
        assertTrue("what was under it is folded away", rule.onAllNodes(hasText("New project")).fetchSemanticsNodes().isEmpty())
        press("Projects")
        shown("New project").assertExists()
    }

    @Test
    fun quickAddAsksForTheLineReadsItBackAndAddsTheTask() {
        answers += "call the bank tomorrow p1"
        press("New task")
        assertEquals("the system's input screen is asked straight away", listOf("New task"), asked)
        said("priority 1").assertExists()
        press("Add")
        assertEquals(listOf("call the bank"), core.lumenna.listTasks("").rows.map { it.title })
        press("Tasks")
        assertTrue(stateOf("call the bank").contains("due tomorrow"))
    }

    @Test
    fun quickAddOffersWhatFinishesTheLastWord() {
        answers += "call the bank #In"
        press("New task")
        val offered = rule.onNode(hasContentDescription("Complete with project Inbox"))
        runCatching { rule.onNode(hasScrollToNodeAction()).performScrollToNode(hasContentDescription("Complete with project Inbox")) }
        offered.performClick()
        rule.waitForIdle()
        shown("call the bank #Inbox", substring = true).assertExists()
    }

    @Test
    fun aTasksFieldIsAskedForAndSavedAsOnlyWhatChanged() {
        core.lumenna.addTask("water plants")
        core.changed()
        press("Tasks")
        press("water plants")
        answers += "water the plants"
        press("Title")
        press("Save")
        assertEquals(listOf("water the plants"), core.lumenna.listTasks("").rows.map { it.title })
    }

    @Test
    fun aTasksProjectIsChosenFromAListOfTheProjectsEachSayingItsLevel() {
        core.lumenna.addProject("Work", null)
        core.lumenna.addProject("Reports", "Work")
        core.lumenna.addTask("water plants")
        core.changed()
        press("Tasks")
        press("water plants")
        press("Project")
        assertTrue("a list to choose from, not the input screen", asked.isEmpty())
        said("level 2").assertExists()
        press("Reports")
        press("Save")
        assertEquals("Reports", core.lumenna.showTask(core.lumenna.listTasks("").rows.single().id).task.project)
    }

    @Test
    fun aTasksActionsAreItsCustomActions() {
        core.lumenna.addTask("water plants")
        core.changed()
        press("Tasks")
        shown("water plants").performCustomAccessibilityActionWithLabel("Mark done")
        rule.waitForIdle()
        assertTrue(core.lumenna.listTasks("").rows.isEmpty())
    }

    @Test
    fun theDayGoesToADaySaidAsAPersonSaysIt() {
        press("Today")
        answers += "tomorrow"
        press("Go to day")
        said("Tomorrow").assertExists()
    }

    @Test
    fun settingsShowPlanningAndTheDevices() {
        press("Settings")
        press("Planning")
        shown("Completing a task completes its subtasks").assertExists()
    }

    @Test
    fun aProjectsOwnActionsAreTheCoresAndRenameAsksTheInputScreen() {
        core.lumenna.addProject("Work", null)
        rule.runOnIdle { core.changed() }
        press("Work", substring = true)
        answers += "Job"
        press("Rename")
        assertEquals(listOf("Rename Work"), asked)
        assertTrue(core.lumenna.listProjects().rows.any { it.title == "Job" })
    }

    @Test
    fun aDeleteAsksFirstInTheCoresWords() {
        core.lumenna.addLabel("calls")
        rule.runOnIdle { core.changed() }
        press("calls", substring = true)
        press("Delete")
        // Wear's own AlertDialog, its confirm button named by the core's words.
        rule.onNode(hasText("Tasks wearing it stay", substring = true)).assertExists()
        // Wear's dialog shrinks the screen behind it while it shows, so the checks measured the
        // label's buttons back there, out of reach, as under 48dp: the checks are left out for
        // this press alone.
        rule.disableAccessibilityChecks()
        rule.onNode(hasContentDescription("Delete label") and hasClickAction()).performClick()
        rule.waitForIdle()
        rule.enableAccessibilityChecks()
        assertTrue(core.lumenna.listLabels().rows.isEmpty())
    }

    @Test
    fun aDevicesActionsAreTheCoresAndSyncNowIsTheScreens() {
        press("Settings")
        press("Devices and sync")
        shown("Sync now").assertExists()
        val own = core.lumenna.syncStatus().devices.firstOrNull { it.thisDevice } ?: return
        val actions = shown(own.name).fetchSemanticsNode().config.getOrNull(androidx.compose.ui.semantics.SemanticsActions.CustomActions).orEmpty().map { it.label }
        assertEquals("this watch cannot unpair itself, and a row does not sync", listOf("Rename"), actions)
    }

    @Test
    fun aRowShowsItsPrimaryActionsFirst() {
        core.lumenna.addTask("water plants")
        core.changed()
        press("Tasks")
        val actions = shown("water plants").fetchSemanticsNode().config
            .getOrNull(androidx.compose.ui.semantics.SemanticsActions.CustomActions).orEmpty().map { it.label }
        assertEquals(listOf("Mark done", "Move to trash"), actions.take(2))
        assertTrue("every action is still offered", "Move to project" in actions)
    }
}
