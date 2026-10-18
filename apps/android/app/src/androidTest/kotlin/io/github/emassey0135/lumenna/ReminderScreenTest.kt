package io.github.emassey0135.lumenna

import android.Manifest
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.accessibility.enableAccessibilityChecks
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performCustomAccessibilityActionWithLabel
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
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
 * Setting reminders as TalkBack reaches them: a block's and a task's Reminders action, the
 * list in the core's words, Add reminder taking a phrase, and the settings for them — this
 * device's on the Reminders page, the defaults on Planning.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class ReminderScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private lateinit var directory: File
    private lateinit var core: Core

    /** What Android allows, as a test says it; the system's answer when null. */
    private var access: ReminderAccess? = null

    @Before
    fun open() {
        // Granted, so adding the first reminder asks nothing over the app.
        instrumentation.uiAutomation.grantRuntimePermission(context.packageName, Manifest.permission.POST_NOTIFICATIONS)
        directory = File(context.cacheDir, "test-${UUID.randomUUID()}")
        core = Core(directory)
        rule.registerIdlingResource(stillApp(core))
        Clock.update(context)
        rule.enableAccessibilityChecks()
        rule.setContent {
            CompositionLocalProvider(LocalReminderAccess provides { context -> access ?: Reminders.access(context) }) {
                PhoneWidth { LumennaTheme { LumennaApp(core) } }
            }
        }
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    private fun tab(name: String) {
        rule.onNode(hasText(name) and SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Tab)).performClick()
        rule.waitForIdle()
    }

    private fun titled(title: String, substring: Boolean = false) = SemanticsMatcher("row titled $title") { node ->
        node.config.getOrNull(RowTitle)?.let { if (substring) title in it else it == title } == true
    }

    private fun row(title: String, substring: Boolean = false): SemanticsNodeInteraction = rule.onNode(titled(title, substring))

    private fun act(title: String, action: String, substring: Boolean = false) {
        row(title, substring).performCustomAccessibilityActionWithLabel(action)
        rule.waitForIdle()
    }

    private fun button(text: String) {
        val node = rule.onNode(hasText(text) and SemanticsMatcher.keyIsDefined(SemanticsProperties.Role))
        runCatching { node.performScrollTo() }
        node.performClick()
        rule.waitForIdle()
    }

    private fun field(label: String): SemanticsNodeInteraction = rule.onNode(hasSetTextAction() and hasText(label))

    private fun replace(label: String, text: String) {
        field(label).performTextClearance()
        field(label).performTextInput(text)
        rule.waitForIdle()
    }

    private fun shows(text: String) {
        rule.waitUntil(5_000) { rule.onAllNodes(hasText(text, substring = true, ignoreCase = true)).fetchSemanticsNodes().isNotEmpty() }
    }

    private fun seed(operation: (io.github.emassey0135.lumenna.app.Device) -> Unit) {
        operation(core.device)
        rule.runOnIdle { core.changed() }
        rule.waitForIdle()
    }

    @Test
    fun aBlocksRemindersAreItsDefaultsUntilOneOfItsOwnIsAddedAndRemoved() {
        seed { it.addBlock(newBlock(title = "Deep work", at = "9:00", minutes = 90u, date = "today", kind = "work", repeat = null)) }
        tab("Today")
        act("Deep work", "Reminders", substring = true)
        rule.onNode(isHeading() and hasText("1 reminder, the defaults")).assertExists()
        row("5 minutes before it starts").assertExists()

        rule.onNodeWithContentDescription("Add reminder").performClick()
        rule.waitForIdle()
        // A phrase the core cannot read stays in its dialog, and the core says why.
        replace("When", "soon")
        button("Add")
        shows("'soon' is not a reminder")
        field("When").assertExists()
        replace("When", "at end")
        button("Add")
        row("When it ends").assertExists()
        rule.onNode(isHeading() and hasText("1 reminder")).assertExists()
        assertTrue(row("5 minutes before it starts").let { runCatching { it.assertExists() }.isFailure })

        act("When it ends", "Remove reminder")
        rule.onNode(isHeading() and hasText("1 reminder, the defaults")).assertExists()
    }

    @Test
    fun aTaskWithADueDateOffersItsReminders() {
        seed { it.addTask("call the bank tomorrow 3pm") }
        tab("Tasks")
        act("call the bank", "Reminders")
        rule.onNode(isHeading() and hasText("reminder", substring = true)).assertExists()
        rule.onNodeWithContentDescription("Add reminder").assertExists()
    }

    @Test
    fun theRemindersPageTurnsThemOffOnThisDeviceAlone() {
        tab("Settings")
        row("Reminders").performClick()
        rule.waitForIdle()
        rule.onNode(hasText("Reminders on this device") and SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Switch)).performClick()
        rule.waitForIdle()
        assertEquals("false", core.device.settings("reminders").settings.single().value)
        assertEquals("Reminders are off on this device", core.device.upcomingReminders("2100-01-01T00:00:00Z").announcement)
    }

    @Test
    fun theRemindersPageSaysWhatAndroidRefusesAndOffersTheSystemsPages() {
        // Refused as a person can refuse them: a test cannot revoke exact alarms for real,
        // since Android kills the app when they are.
        access = ReminderAccess(notifications = false, exact = false, grantExact = android.content.Intent())
        tab("Settings")
        row("Reminders").performClick()
        rule.waitForIdle()
        shows("Notifications are off for Lumenna")
        rule.onNode(hasText("Allow notifications") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Role)).assertExists()
        shows("not allowed to set alarms for an exact time")
        rule.onNode(hasText("Allow exact alarms") and SemanticsMatcher.keyIsDefined(SemanticsProperties.Role)).assertExists()

        // Granted, as on coming back from the system's page.
        access = ReminderAccess(notifications = true, exact = true, grantExact = null)
        rule.runOnIdle { core.changed() }
        rule.waitUntil(5_000) {
            rule.onAllNodes(hasText("Allow exact alarms")).fetchSemanticsNodes().isEmpty() &&
                rule.onAllNodes(hasText("Allow notifications")).fetchSemanticsNodes().isEmpty()
        }
    }

    @Test
    fun theDefaultRemindersAreAPhraseOnPlanningAndARefusalStaysInItsDialog() {
        tab("Settings")
        row("Planning").performClick()
        rule.waitForIdle()
        button("Reminders for a block")
        replace("Reminders for a block", "soon")
        button("Set")
        field("Reminders for a block").assertExists()
        replace("Reminders for a block", "10m before start, at end")
        button("Set")
        assertEquals("10m before start, at end", core.device.settings("default-block-reminders").settings.single().value)
    }
}
