package io.github.emassey0135.lumenna

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.accessibility.enableAccessibilityChecks
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performCustomAccessibilityActionWithLabel
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import android.app.UiAutomation
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.emassey0135.lumenna.core.NewBlock
import io.github.emassey0135.lumenna.core.TaskEdit
import java.io.File
import java.time.LocalDate
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The app driven as TalkBack reaches it: rows by what they say, actions by their names, fields
 * by their labels — never by position. Each test has a store of its own, and Google's
 * accessibility checks run on every interaction, as Apple's audit does in the iPhone's tests.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class LumennaTest {
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
        rule.enableAccessibilityChecks()
        rule.setContent { LumennaTheme { LumennaApp(core) } }
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    // MARK: - Moving about

    private fun tab(name: String) {
        rule.onNode(hasText(name) and SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Tab)).performClick()
        rule.waitForIdle()
    }

    /** A row, by the title it says first. */
    private fun row(title: String, substring: Boolean = false): SemanticsNodeInteraction =
        rule.onNode(hasContentDescription(title, substring = substring) and hasStateDescription())

    private fun hasStateDescription() = SemanticsMatcher.keyIsDefined(SemanticsProperties.StateDescription)

    private fun act(title: String, action: String, substring: Boolean = false) {
        row(title, substring).performCustomAccessibilityActionWithLabel(action)
        rule.waitForIdle()
    }

    private fun press(name: String) {
        rule.onNodeWithContentDescription(name).performClick()
        rule.waitForIdle()
    }

    /** A button by its text, scrolled into view first, as TalkBack would bring it. */
    private fun button(text: String) {
        val node = rule.onNode(hasText(text) and SemanticsMatcher.keyIsDefined(SemanticsProperties.Role))
        runCatching { node.performScrollTo() }
        node.performClick()
        rule.waitForIdle()
    }

    private fun field(label: String): SemanticsNodeInteraction = rule.onNode(hasSetTextAction() and hasText(label))

    private fun type(label: String, text: String) {
        field(label).performTextInput(text)
        rule.waitForIdle()
    }

    private fun replace(label: String, text: String) {
        field(label).performTextClearance()
        field(label).performTextInput(text)
        rule.waitForIdle()
    }

    private fun shows(text: String, substring: Boolean = true) {
        rule.waitUntil(5_000) {
            rule.onAllNodes(hasText(text, substring = substring, ignoreCase = true)).fetchSemanticsNodes().isNotEmpty()
        }
    }

    private fun says(row: String, state: String) {
        rule.waitUntil(5_000) {
            rule.onAllNodes(hasContentDescription(row, substring = true)).fetchSemanticsNodes().any { node ->
                node.config.getOrNull(SemanticsProperties.StateDescription)?.contains(state) == true
            }
        }
    }

    private fun gone(title: String) {
        rule.waitUntil(5_000) { rule.onAllNodes(hasContentDescription(title)).fetchSemanticsNodes().isEmpty() }
    }

    private fun seed(operation: (io.github.emassey0135.lumenna.core.Lumenna) -> Unit) {
        operation(core.lumenna)
        rule.runOnIdle { core.changed() }
        rule.waitForIdle()
    }

    private fun titles(query: String = ""): List<String> = core.lumenna.listTasks(query).rows.map { it.title }

    // MARK: - Tasks

    @Test
    fun addingATaskReadsItBackAndListsIt() {
        tab("Tasks")
        press("Add task")
        type("Task", "call the bank tomorrow p1")
        shows("call the bank, due tomorrow")
        shows("priority 1")
        press("Add")
        row("call the bank").assertExists()
        assertEquals(listOf("call the bank"), titles())
    }

    @Test
    fun completingATaskPutsFocusOnTheOneNowInItsPlaceThenSaysWhatHappened() {
        seed {
            it.addTask("first")
            it.addTask("second")
            it.addTask("third")
        }
        tab("Tasks")
        act("second", "Mark Done")
        gone("second")
        row("third").assertIsFocused()
        shows("Completed second")
    }

    /**
     * With TalkBack running, its own focus — not only input focus — lands on the row now in
     * the completed one's place. Skipped without TalkBack: turn it on in the emulator first.
     */
    @Test
    fun talkBackFollowsFocusToTheRowNowInTheCompletedOnesPlace() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val manager = context.getSystemService(AccessibilityManager::class.java)
        assumeTrue("TalkBack is not running", manager.isTouchExplorationEnabled)
        val automation = InstrumentationRegistry.getInstrumentation()
            .getUiAutomation(UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES)
        seed {
            it.addTask("first")
            it.addTask("second")
            it.addTask("third")
        }
        tab("Tasks")
        act("second", "Mark Done")
        gone("second")
        val focused = {
            val root = automation.rootInActiveWindow
            val a11y = root?.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY)
            val input = root?.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
            "root=${root != null} a11y=${a11y?.contentDescription ?: a11y?.text} input=${input?.contentDescription ?: input?.text}"
        }
        runCatching {
            rule.waitUntil(5_000) {
                automation.rootInActiveWindow?.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY)
                    ?.contentDescription?.toString() == "third"
            }
        }.onFailure { throw AssertionError("TalkBack's focus is not on third: ${focused()}") }
    }

    @Test
    fun movingAProjectKeepsFocusOnIt() {
        seed {
            it.addProject("Home", null)
            it.addProject("Work", null)
        }
        tab("Browse")
        row("Projects").performClick()
        rule.waitForIdle()
        act("Home", "Move Down")
        assertEquals(listOf("Work", "Home"), core.lumenna.listProjects().rows.map { it.title }.filter { it != "Inbox" })
        row("Home").assertIsFocused()
    }

    @Test
    fun deletingTheLastTaskPutsFocusOnTheNewLastOne() {
        seed {
            it.addTask("keep")
            it.addTask("lose")
        }
        tab("Tasks")
        act("lose", "Delete")
        gone("lose")
        row("keep").assertIsFocused()
    }

    @Test
    fun quickAddRefusesAnUnknownProjectAndSaysWhy() {
        tab("Tasks")
        press("Add task")
        type("Task", "water the plants #Nowhere")
        shows("Nowhere")
        press("Add")
        assertEquals(emptyList<String>(), titles())
    }

    @Test
    fun todayMeansTodayWhereThePhoneIs() {
        assertEquals(LocalDate.now().toString(), core.lumenna.plan(null).date)
        rule.onNode(hasText("Today") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading)).assertExists()
    }

    @Test
    fun completingRemovesTheTaskAndUndoBringsItBack() {
        seed { it.addTask("buy milk") }
        tab("Tasks")
        act("buy milk", "Mark Done")
        gone("buy milk")
        press("Undo")
        row("buy milk").assertExists()
        assertEquals(listOf("buy milk"), titles())
    }

    @Test
    fun aFilterSaysHowItWasUnderstood() {
        seed {
            it.addProject("Work", null)
            it.addTask("ring the client #Work")
            it.addTask("water the plants")
        }
        tab("Tasks")
        type("Filter", "#Work")
        shows("1 task")
        row("ring the client").assertExists()
        gone("water the plants")
    }

    @Test
    fun editingATaskSavesOnlyWhatChanged() {
        seed { it.addTask("draft report") }
        val id = core.lumenna.listTasks("").rows.single().id
        tab("Tasks")
        row("draft report").performClick()
        rule.waitForIdle()
        replace("Title", "draft the report")
        // Another device moves the date while the title is being typed.
        seed { it.editTask(id, TaskEdit(null, "friday", null, null, null, null, null, null)) }
        press("Save")
        val task = core.lumenna.showTask(id).task
        assertEquals("draft the report", task.title)
        assertTrue("the other device's date stands: ${task.due}", task.due != null)
    }

    @Test
    fun taskDetailLabelsAreSavedByName() {
        seed { it.addTask("call mum") }
        tab("Tasks")
        row("call mum").performClick()
        rule.waitForIdle()
        type("Labels", "calls, @errands")
        press("Save")
        assertEquals(listOf("calls", "errands"), core.lumenna.listTasks("").rows.single().let { core.lumenna.showTask(it.id).task.labels })
    }

    @Test
    fun aRepetitionIsShownInWordsAndKeptThroughANewDate() {
        seed { it.addTask("water plants every monday") }
        tab("Tasks")
        row("water plants").performClick()
        rule.waitForIdle()
        field("Repeats").assert(SemanticsMatcher("shows every monday") {
            it.config.getOrNull(SemanticsProperties.EditableText)?.text == "every monday"
        })
        replace("Due", "2026-12-14")
        press("Save")
        val task = core.lumenna.showTask(core.lumenna.listTasks("").rows.single().id).task
        assertEquals("2026-12-14", task.due)
        assertEquals("every monday", task.repetition)
    }

    @Test
    fun aTrashedTaskCanBeRestoredFromTheTrash() {
        seed {
            it.addTask("keep me")
            it.trashTask(it.listTasks("").rows.single().id)
        }
        tab("Browse")
        row("Trash").performClick()
        rule.waitForIdle()
        act("keep me", "Restore")
        gone("keep me")
        assertEquals(listOf("keep me"), titles())
    }

    // MARK: - The day

    @Test
    fun theDaySaysWhatItHoldsAndShowsFreeTime() {
        seed { it.addBlock(NewBlock("Focus", "9:00", 90u, "today", "work", null)) }
        tab("Today")
        says("Focus", "work block")
        // Before the block and after it.
        assertEquals(2, rule.onAllNodes(hasContentDescription("Free", substring = true) and hasStateDescription()).fetchSemanticsNodes().size)
        shows("1 block", substring = true)
    }

    @Test
    fun aTaskCanBeAssignedToABlockAndTimed() {
        seed {
            it.addTask("write the chapter")
            it.addBlock(NewBlock("All day", "00:00", 1439u, "today", "work", null))
        }
        tab("Today")
        act("All day", "Assign Task", substring = true)
        button("write the chapter")
        type("Minutes", "45")
        button("Done")
        says("write the chapter", "planned for 45 minutes")
        act("write the chapter", "Start Timer")
        says("write the chapter", "in progress")
        act("write the chapter", "Planned Length")
        replace("Minutes", "")
        button("Done")
        says("write the chapter", "in progress")
        assertEquals(null, core.lumenna.plan(null).blocks.single().assignments.single().plannedMins)
    }

    @Test
    fun aTaskIsPutInABlockFromItsOwnPage() {
        seed {
            it.addTask("tidy the desk")
            it.addBlock(NewBlock("Chores", "10:00", 60u, "tomorrow", "work", null))
            it.addBlock(NewBlock("Lunch", "12:00", 30u, "tomorrow", "break", null))
        }
        tab("Tasks")
        row("tidy the desk").performClick()
        rule.waitForIdle()
        button("Put in a Block…")
        rule.onAllNodes(hasText("Lunch", substring = true)).fetchSemanticsNodes().let { assertTrue("a break takes no tasks", it.isEmpty()) }
        button("Tomorrow, ${Clock.time("10:00")}, Chores")
        type("Minutes", "30")
        button("Done")
        val sitting = core.lumenna.plan("tomorrow").blocks.first { it.title == "Chores" }.assignments.single()
        assertEquals("tidy the desk", sitting.title)
        assertEquals(30u, sitting.plannedMins)
    }

    @Test
    fun aCancelledDayIsListedAndCanBePutBack() {
        seed { it.addBlock(NewBlock("Run", "7:00", 30u, "today", "work", "every day")) }
        tab("Today")
        act("Run", "Cancel This Day", substring = true)
        says("Run", "cancelled for this day")
        act("Run", "Restore This Day", substring = true)
        says("Run", "work block")
    }

    @Test
    fun undoWorksFromTheDayToo() {
        seed { it.addBlock(NewBlock("Run", "7:00", 30u, "today", "work", "every day")) }
        tab("Today")
        act("Run", "Cancel This Day", substring = true)
        says("Run", "cancelled for this day")
        press("Undo")
        says("Run", "work block")
    }

    // MARK: - Browse

    @Test
    fun aProjectHoldsTheTasksAddedInIt() {
        tab("Browse")
        row("Projects").performClick()
        rule.waitForIdle()
        press("Add project")
        type("Name", "Work")
        button("Add")
        row("Work").performClick()
        rule.waitForIdle()
        press("Add task")
        type("Task", "ship it")
        press("Add")
        row("ship it").assertExists()
        assertEquals(listOf("ship it"), titles("#Work"))
    }

    @Test
    fun labelsAndSavedFiltersCanBeMade() {
        tab("Browse")
        row("Labels").performClick()
        rule.waitForIdle()
        press("Add label")
        type("Name", "calls")
        button("Add")
        row("calls").assertExists()
        press("Back")
        row("Saved filters").performClick()
        rule.waitForIdle()
        press("Add filter")
        type("Name", "Urgent")
        button("Next")
        type("Query", "p1")
        button("Save")
        row("Urgent").assertExists()
        assertEquals("p1", core.lumenna.listFilters().filters.single().query)
    }

    @Test
    fun everyBlockIsListedUnderBrowseAndAsksBeforeDeleting() {
        seed { it.addBlock(NewBlock("Review", "16:00", 60u, "in 10 days", "work", null)) }
        tab("Browse")
        row("Blocks").performClick()
        rule.waitForIdle()
        act("Review", "Delete")
        shows("Delete Review?")
        button("Delete")
        gone("Review")
        assertEquals(0u, core.lumenna.listBlocks().count)
    }

    // MARK: - Settings

    @Test
    fun aTimeSettingTakesHoursAndMinutesAsSaid() {
        tab("Settings")
        row("Planning").performClick()
        rule.waitForIdle()
        rule.onNode(hasText("Day starts") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Role)).performClick()
        rule.waitForIdle()
        replace("Time", "9:30am")
        button("Set")
        assertEquals("09:30", core.lumenna.settings("day-start").settings.single().value)
    }

    @Test
    fun theDevicesPageSaysThisDevice() {
        tab("Settings")
        row("Devices and Sync").performClick()
        rule.waitForIdle()
        rule.onNodeWithText("Devices and Sync").assertExists()
        rule.onNodeWithContentDescription("Pair a device").assertExists()
    }
}
