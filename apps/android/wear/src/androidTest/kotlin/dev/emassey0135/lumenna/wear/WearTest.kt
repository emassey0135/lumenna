package dev.emassey0135.lumenna.wear

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
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Core
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

    /** The moment a proposal is made against: now, unless a test fixes one. */
    private val plannerNow = androidx.compose.runtime.mutableStateOf<String?>(null)

    /** What a reminder's notification opened the app to, as the activity hands it over. */
    private val opened = kotlinx.coroutines.flow.MutableStateFlow<Pair<Long, dev.emassey0135.lumenna.ReminderOpened>?>(null)
    private val asked = mutableListOf<String>()

    /** What Wear OS allows reminders, as a test says it; the system's answer when null. */
    private var access: dev.emassey0135.lumenna.ReminderAccess? = null

    @Before
    fun open() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        directory = File(context.cacheDir, "test-${UUID.randomUUID()}")
        core = Core(directory)
        rule.registerIdlingResource(stillApp(core))
        Clock.update(context)
        rule.enableAccessibilityChecks()
        val entry = TextEntry { label, answered ->
            asked += label
            answers.removeFirstOrNull()?.let(answered)
        }
        rule.setContent {
            androidx.compose.runtime.CompositionLocalProvider(
                dev.emassey0135.lumenna.LocalReminderAccess provides { context -> access ?: dev.emassey0135.lumenna.Reminders.access(context) },
                dev.emassey0135.lumenna.LocalPlannerNow provides plannerNow.value,
            ) {
                androidx.wear.compose.material3.MaterialTheme { WearApp(core, entry, opened) }
            }
        }
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
        assertEquals(listOf("call the bank"), core.device.listTasks("").rows.map { it.title })
        press("Tasks")
        assertTrue(stateOf("call the bank").contains("due tomorrow"))
    }

    @Test
    fun quickAddOffersWhatFinishesTheLastWord() {
        answers += "call the bank #In"
        press("New task")
        val offered = rule.onNode(hasContentDescription("Complete with Inbox, project"))
        runCatching { rule.onNode(hasScrollToNodeAction()).performScrollToNode(hasContentDescription("Complete with Inbox, project")) }
        offered.performClick()
        rule.waitForIdle()
        shown("call the bank #Inbox", substring = true).assertExists()
    }

    @Test
    fun aTasksFieldIsAskedForAndSavedAsOnlyWhatChanged() {
        core.device.addTask("water plants")
        core.changed()
        press("Tasks")
        press("water plants")
        answers += "water the plants"
        press("Title")
        press("Save")
        assertEquals(listOf("water the plants"), core.device.listTasks("").rows.map { it.title })
    }

    @Test
    fun aTasksProjectIsChosenFromAListOfTheProjectsEachSayingItsLevel() {
        core.device.addProject("Work", null)
        core.device.addProject("Reports", "Work")
        core.device.addTask("water plants")
        core.changed()
        press("Tasks")
        press("water plants")
        press("Project")
        assertTrue("a list to choose from, not the input screen", asked.isEmpty())
        said("level 2").assertExists()
        press("Reports")
        press("Save")
        assertEquals("Reports", core.device.showTask(core.device.listTasks("").rows.single().id).task.project)
    }

    @Test
    fun aTasksActionsAreItsCustomActions() {
        core.device.addTask("water plants")
        core.changed()
        press("Tasks")
        shown("water plants").performCustomAccessibilityActionWithLabel("Mark done")
        rule.waitForIdle()
        assertTrue(core.device.listTasks("").rows.isEmpty())
    }

    @Test
    fun theDayGoesToADaySaidAsAPersonSaysIt() {
        press("Today")
        answers += "tomorrow"
        press("Go to day")
        // The line first, then the watch's calendar.
        shown("Choose a day").assertExists()
        press("Day")
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
        core.device.addProject("Work", null)
        rule.runOnIdle { core.changed() }
        press("Work", substring = true)
        answers += "Job"
        press("Rename")
        assertEquals(listOf("Rename Work"), asked)
        assertTrue(core.device.listProjects().rows.any { it.title == "Job" })
    }

    @Test
    fun aDeleteAsksFirstInTheCoresWords() {
        core.device.addLabel("calls")
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
        assertTrue(core.device.listLabels().rows.isEmpty())
    }

    @Test
    fun aDevicesActionsAreTheCoresAndSyncNowIsTheScreens() {
        press("Settings")
        press("Devices and sync")
        shown("Sync now").assertExists()
        val own = core.device.syncStatus().devices.firstOrNull { it.thisDevice } ?: return
        val actions = shown(own.name).fetchSemanticsNode().config.getOrNull(androidx.compose.ui.semantics.SemanticsActions.CustomActions).orEmpty().map { it.label }
        assertEquals("this watch cannot unpair itself, and a row does not sync", listOf("Rename"), actions)
    }

    @Test
    fun aRowShowsItsPrimaryActionsFirst() {
        core.device.addTask("water plants")
        core.changed()
        press("Tasks")
        val actions = shown("water plants").fetchSemanticsNode().config
            .getOrNull(androidx.compose.ui.semantics.SemanticsActions.CustomActions).orEmpty().map { it.label }
        assertEquals(listOf("Mark done", "Move to trash"), actions.take(2))
        assertTrue("every action is still offered", "Move to project" in actions)
    }

    @Test
    fun anEmptyCodeSaysACodeIsNeededAndNeverReadsTheClipboard() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val copied = "lumenna-not-a-code"
        rule.runOnIdle {
            (context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager)
                .setPrimaryClip(android.content.ClipData.newPlainText("test", copied))
        }
        press("Settings")
        press("Devices and sync")
        val words = core.app.view.value!!.devices!!.words
        answers += ""
        press(words.theirCode)
        assertEquals(listOf(words.theirCode), asked)
        said(words.needCode).assertExists()
        assertTrue(
            "what was copied is not taken up",
            rule.onAllNodes(hasText(copied, substring = true)).fetchSemanticsNodes().isEmpty(),
        )
    }

    /** What a form's line says beneath its name. */
    private fun valueOf(name: String): String =
        shown(name).fetchSemanticsNode().config.getOrNull(SemanticsProperties.Text).orEmpty()
            .map { it.text }.filter { it != name }.joinToString()

    @Test
    fun aNewBlockStartsWhenTheCoreSaysAndFollowsTheDay() {
        press("Today")
        press("Add block")
        assertEquals(core.device.newBlockStart(null), valueOf("Starts at"))
        answers += "in 3 days"
        press("Day")
        assertEquals(core.device.newBlockStart("in 3 days"), valueOf("Starts at"))
    }

    @Test
    fun aBlocksDayIsFilledByTheWatchsOwnCalendarBesideItsLine() {
        press("Today")
        press("Add block")
        answers += "2030-01-15"
        press("Day")
        press("Choose a day")
        // Wear's calendar, open on the day the line names, a column at a time to its Confirm.
        // Wear's dialog shrinks the form behind it while it shows, and the checks measured the
        // form's buttons back there as under 48dp: they are left out while the dialog is open.
        rule.disableAccessibilityChecks()
        val confirm = androidx.compose.ui.test.hasContentDescription("Confirm") and hasClickAction()
        repeat(3) {
            if (rule.onAllNodes(confirm).fetchSemanticsNodes().isEmpty()) {
                rule.onNode(androidx.compose.ui.test.hasContentDescription("Next") and hasClickAction()).performClick()
                rule.waitForIdle()
            }
        }
        rule.onNode(confirm).performClick()
        rule.waitForIdle()
        rule.enableAccessibilityChecks()
        assertEquals("2030-01-15", valueOf("Day"))
    }

    /**
     * A row's custom action run on the main thread, as TalkBack runs it: one that opens a
     * screen navigates, which the test thread may not.
     */
    private fun actOnMain(row: String, label: String) {
        val actions = shown(row, substring = true).fetchSemanticsNode().config
            .getOrNull(androidx.compose.ui.semantics.SemanticsActions.CustomActions).orEmpty()
        val action = actions.single { it.label == label }
        rule.runOnUiThread { action.action() }
        rule.waitForIdle()
    }

    @Test
    fun aLabelTakesAColourFromThePaletteAndItsPlaceSaysIt() {
        core.device.addLabel("calls")
        rule.runOnIdle { core.changed() }
        press("calls", substring = true)
        // A label's actions are buttons at the foot of its list; Colour picks from the palette.
        press("Colour")
        said("its colour now").assertExists()
        press("teal")
        rule.waitUntil(5_000) { core.device.listLabels().rows.single().value == "no open tasks, teal" }
    }

    @Test
    fun aBlocksRemindersAreItsDefaultsUntilOneOfItsOwnIsAddedByAPhrase() {
        // Granted, so adding the first reminder asks nothing over the app.
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.uiAutomation.grantRuntimePermission(instrumentation.targetContext.packageName, android.Manifest.permission.POST_NOTIFICATIONS)
        core.device.addBlock(newBlock(title = "Deep work", at = "9:00", minutes = 90u, date = "today", kind = "work", repeat = null))
        rule.runOnIdle { core.changed() }
        press("Today")
        actOnMain("Deep work", "Reminders")
        said("1 reminder, the defaults").assertExists()
        said("5 minutes before it starts, a default").assertExists()
        answers += "at end"
        press("Add reminder")
        assertEquals(listOf("New reminder for Deep work"), asked)
        shown("When it ends").assertExists()
        actOnMain("When it ends", "Remove reminder")
        said("1 reminder, the defaults").assertExists()
    }

    @Test
    fun theRemindersSettingsTurnThemOffOnThisWatchAndSayWhatIsRefused() {
        access = dev.emassey0135.lumenna.ReminderAccess(notifications = false, exact = false, grantExact = android.content.Intent())
        press("Settings")
        press("Reminders")
        said("Notifications are off for Lumenna").assertExists()
        shown("Allow notifications").assertExists()
        said("not allowed to set alarms for an exact time").assertExists()
        shown("Allow exact alarms").assertExists()
        press("Reminders on this device")
        assertEquals("false", core.device.settings("reminders").settings.single().value)
    }

    // MARK: - The planner

    private fun block(title: String, date: String, at: String, minutes: UInt, kind: String = "work") =
        core.device.addBlock(newBlock(title = title, at = at, minutes = minutes, date = date, kind = kind, repeat = null))

    private fun checked(text: String): Boolean =
        shown(text, substring = true).fetchSemanticsNode().config.getOrNull(SemanticsProperties.ToggleableState) == androidx.compose.ui.state.ToggleableState.On

    @Test
    fun theDaysOwnActionsAreOnItsHeadingAndButtonsAfterItsRows() {
        block("All day", "today", "00:00", 1440u)
        rule.runOnIdle { core.changed() }
        press("Today")
        val expected = core.device.plan(null).actions.map { it.sentence }
        val heading = rule.onNode(SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading) and hasText("Today. ", substring = true))
        assertEquals(expected, heading.fetchSemanticsNode().config.getOrNull(androidx.compose.ui.semantics.SemanticsActions.CustomActions).orEmpty().map { it.label })
        assertTrue(expected.toString(), "Replan from now" in expected)
        expected.forEach { shown(it).assertExists() }
    }

    @Test
    fun replanFromNowSaysItsProposalAndAfterAChangeTakenAloneShowsTheNext() {
        // A fixed day and moment, so the proposal does not depend on when the test runs.
        plannerNow.value = "2026-05-06T10:40"
        block("All day", "today", "00:00", 1440u)
        block("Deep work", "2026-05-06", "09:00", 120u)
        block("Lunch", "2026-05-06", "10:30", 30u, "break")
        block("Review", "2026-05-06", "11:00", 60u)
        block("Walk", "2026-05-06", "12:00", 30u, "break")
        rule.runOnIdle { core.changed() }
        press("Today")
        press("Replan from now")
        assertTrue(checked("Lunch moves from"))
        // By its length: an other option's sentence also says Review moves, to 45 minutes.
        press("shortens from 1 hour to 30 minutes", substring = true)
        assertTrue("unchecked", !checked("shortens from 1 hour to 30 minutes"))
        press("Make the changes checked")
        // The list holds only the rows on screen, and the button pressed is at its foot.
        rule.waitForIdle()
        assertTrue("the next proposal starts checked", checked("Walk moves from"))
        press("Make the changes checked")
        rule.waitUntil(5_000) { rule.onAllNodes(hasText("Make the changes checked")).fetchSemanticsNodes().isEmpty() }
        assertEquals("12:00", core.device.plan("2026-05-06").blocks.single { it.title == "Lunch" }.start)
    }

    @Test
    fun planTheDayGivesEachSittingItsReasonAndPlansAgainAroundOneTurnedDown() {
        block("Morning", "tomorrow", "09:00", 30u)
        block("Afternoon", "tomorrow", "14:00", 30u)
        listOf("Report p1", "Email p2", "Filing p3").forEach { core.device.addTask(it) }
        rule.runOnIdle { core.changed() }
        press("Today")
        press("Next day")
        press("Plan the day")
        said("Suggested because priority 1").assertExists()
        press("Report goes in Morning", substring = true)
        press("Plan the ones checked")
        rule.waitForIdle()
        shown("Filing goes in Morning", substring = true).assertExists()
        press("Plan the ones checked")
        rule.waitUntil(5_000) { rule.onAllNodes(hasText("Plan the ones checked")).fetchSemanticsNodes().isEmpty() }
        val planned = core.device.plan("tomorrow").blocks.associate { b -> b.title to b.assignments.map { it.title } }
        assertEquals(mapOf("Morning" to listOf("Filing"), "Afternoon" to listOf("Email")), planned)
    }

    @Test
    fun whatToWorkOnSaysWhyEachIsOfferedBeneathItsName() {
        block("All day", "today", "00:00", 1440u)
        core.device.addTask("write the chapter p1")
        rule.runOnIdle { core.changed() }
        press("Today")
        press(core.device.plan(null).actions.single { it.kind == dev.emassey0135.lumenna.app.ActionKind.WHAT_NOW }.sentence)
        shown("write the chapter").assertExists()
        said("Suggested because").assertExists()
        press("write the chapter")
        rule.waitUntil(5_000) { core.device.plan(null).blocks.single().assignments.any { it.title == "write the chapter" } }
    }

    @Test
    fun aTasksScreenHasItsHistoryUnderAHeading() {
        core.device.addTask("draft the report")
        rule.runOnIdle { core.changed() }
        press("Tasks")
        press("draft the report")
        val history = core.device.showTask(core.device.listTasks("").rows.single().id).task.history
        assertTrue(history.isNotEmpty())
        said("History").assertExists()
        history.forEach { said(it).assertExists() }
    }

    private fun replannedDay() {
        plannerNow.value = "2026-05-06T10:40"
        block("All day", "today", "00:00", 1440u)
        block("Deep work", "2026-05-06", "09:00", 120u)
        block("Lunch", "2026-05-06", "10:30", 30u, "break")
        block("Review", "2026-05-06", "11:00", 60u)
        rule.runOnIdle { core.changed() }
    }

    @Test
    fun anotherOptionOnTheReplanScreenIsProposedInPlace() {
        replannedDay()
        val skip = core.device.replan(dev.emassey0135.lumenna.Clock.style, noPins, plannerNow.value).alternatives.first { it.sentence.startsWith("Skip Lunch today") }
        press("Today")
        press("Replan from now")
        said("Other options").assertExists()
        press(skip.sentence)
        // The list holds only the rows on screen, and the option pressed was at its foot.
        rule.waitForIdle()
        assertTrue(checked("Lunch is skipped"))
        assertTrue(rule.onAllNodes(hasText("Other options")).fetchSemanticsNodes().isEmpty())
    }

    @Test
    fun anOverrunsReplanButtonOpensReplanFromNow() {
        replannedDay()
        rule.runOnIdle { opened.value = 1L to dev.emassey0135.lumenna.ReminderOpened(dev.emassey0135.lumenna.Reminders.REPLAN, "", "") }
        rule.waitUntil(5_000) { rule.onAllNodes(hasText("Lunch moves from", substring = true)).fetchSemanticsNodes().isNotEmpty() }
        assertTrue(checked("Lunch moves from"))
    }
}
