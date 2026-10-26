package dev.emassey0135.lumenna

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Bundle
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.accessibility.enableAccessibilityChecks
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.core.content.ContextCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.emassey0135.lumenna.app.ParameterKind
import dev.emassey0135.lumenna.app.assistantOperations
import dev.emassey0135.lumenna.app.automationOperations
import dev.emassey0135.lumenna.app.automationWords
import dev.emassey0135.lumenna.tasker.Tasker
import dev.emassey0135.lumenna.tasker.TaskerAction
import dev.emassey0135.lumenna.tasker.TaskerFireReceiver
import dev.emassey0135.lumenna.tasker.TaskerSecrets
import dev.emassey0135.lumenna.tasker.TaskerSetup
import java.io.File
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Tasker's action plugin: its screen offers exactly the core's operations that may run
 * unattended, in the core's words; saving one makes a secret only this device keeps; a firing
 * without it is refused in the core's words; and one with it answers the core's variables, to
 * Tasker's waiting broadcast or its completion intent.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class TaskerTest {
    @get:Rule
    val rule = createComposeRule()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val directory = File(context.cacheDir, "tasker-${UUID.randomUUID()}")
    private lateinit var core: Core
    private val secrets by lazy { TaskerSecrets(context) }

    /** What the screen saved: the operation, the values as typed, and as shown. */
    private var saved: Triple<String, Map<String, String>, Map<String, String>>? = null

    @Before
    fun open() {
        core = Core(directory)
        rule.registerIdlingResource(stillApp(core))
        rule.enableAccessibilityChecks()
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    private fun setUp(before: TaskerAction? = null) {
        rule.setContent { LumennaTheme { TaskerSetup(core, before) { operation, values, shown -> saved = Triple(operation, values, shown) } } }
    }

    private fun press(text: String) {
        val node = rule.onNode(hasText(text) and hasClickAction())
        node.performScrollTo()
        node.performClick()
        rule.waitForIdle()
    }

    /** The action the screen saved, with the secret the edit activity makes and keeps for it. */
    private fun savedAction(): TaskerAction {
        val (operation, values, shown) = checkNotNull(saved) { "nothing was saved" }
        val secret = secrets.make()
        secrets.keep(secret, operation)
        return TaskerAction(operation, values, shown, secret)
    }

    private fun Bundle.text(name: String): String? = getString(name)

    @Test
    fun theScreenOffersEveryOperationOfTheTableButWhatAsksFirstOrTakesAProposal() {
        val proposals = setOf(ParameterKind.REPLAN, ParameterKind.REPLAN_CHANGES, ParameterKind.DAY_PLAN, ParameterKind.DAY_PLAN_SITTINGS)
        val unattended = assistantOperations().filter { op -> !op.confirms && op.parameters.none { it.kind in proposals } }
        assertEquals(unattended.map { it.id }, automationOperations().map { it.id })
        assertTrue(automationOperations().none { it.confirms })

        setUp()
        for (operation in automationOperations()) {
            rule.onNode(hasText(operation.sentence) and hasClickAction()).performScrollTo()
        }
        for (operation in assistantOperations().filter { it.confirms }) {
            assertTrue(operation.id, rule.onAllNodes(hasText(operation.sentence)).fetchSemanticsNodes().isEmpty())
        }
        // A destructive operation says so with its name and description.
        val trash = automationOperations().first { it.id == "trash_task" }
        rule.onNode(hasText(trash.sentence) and hasText(automationWords().destructive) and hasClickAction()).performScrollTo()
    }

    @Test
    fun anActionKeepsItsBundleWholeThroughTasker() {
        val action = TaskerAction("add_task", mapOf("text" to "%line"), mapOf("text" to "Buy milk"), secrets.make())
        assertEquals(action, TaskerAction.from(action.bundle()))
    }

    @Test
    fun aSecretTheSetupIsHandedIsKeptOnlyIfItWasMadeHere() {
        val stranger = "0".repeat(64)
        assertTrue(stranger != secrets.forSaving(stranger))
        val ours = secrets.make().also { secrets.keep(it, "add_task") }
        assertEquals(ours, secrets.forSaving(ours))
        assertTrue(secrets.forSaving(null).length == 64)
    }

    @Test
    fun anActionSetUpOnTheScreenFiresAndAnswersTheCoresVariables() {
        setUp()
        press("Add a task")
        rule.onNode(hasSetTextAction() and hasText("Line")).performTextInput("Buy milk tomorrow")
        press("Save")
        val adding = savedAction()
        assertEquals("add_task", adding.operation)
        assertEquals("Add a task. Line: Buy milk tomorrow", adding.blurb())
        // Tasker fills its variables into the values, and never into the secret.
        val replaced = adding.bundle().getString(TaskerAction.REPLACE_KEYS).orEmpty().split(' ')
        assertEquals(listOf(TaskerAction.VALUE + "text"), replaced)

        val added = Tasker.fire(core, secrets, adding.bundle())
        assertTrue(added.variables.text("%errmsg"), added.ok)
        assertEquals("Buy milk", added.variables.text("%task_title"))
        assertTrue(added.variables.text("%announcement").orEmpty().isNotEmpty())
        val id = checkNotNull(added.variables.text("%task_id"))

        // What Tasker lists as this action's variables: the core's, in sentence case.
        assertTrue(Tasker.relevant("add_task").any { it.startsWith("%task_id\nTask identifier\n") })
        assertTrue(Tasker.relevant("list_tasks").any { it.startsWith("%ids()\n") })

        val listed = Tasker.fire(core, secrets, TaskerAction("list_tasks", emptyMap(), emptyMap(), secrets.make().also { secrets.keep(it, "list_tasks") }).bundle())
        assertEquals(id, listed.variables.text("%ids1"))
        assertEquals("Buy milk", listed.variables.text("%titles1"))
        assertEquals("1", listed.variables.text("%count"))
    }

    @Test
    fun aTaskIsChosenFromTheCoresChoicesAndShownByItsTitle() {
        val id = core.device.addTask("Write the report").task!!.id
        rule.runOnIdle { core.changed() }
        setUp()
        press("Complete a task")
        press("Choose a task")
        press("Write the report")
        press("Save")
        val completing = savedAction()
        assertEquals(mapOf("id" to id), completing.values)
        assertEquals("Complete a task. Task: Write the report", completing.blurb())
        assertTrue(Tasker.fire(core, secrets, completing.bundle()).ok)
        // Done, so no longer among the open tasks.
        assertTrue(core.device.listTasks("").rows.none { it.id == id })
    }

    @Test
    fun saveSaysWhatIsNeededAndAVariableWaitsUntilItFires() {
        setUp()
        press("Complete a task")
        press("Save")
        rule.onNode(hasText("Task cannot be left empty.")).assertExists()
        assertEquals(null, saved)
        rule.onNode(hasSetTextAction() and hasText("Task")).performTextInput("%task")
        press("Save")
        assertEquals(mapOf("id" to "%task"), savedAction().values)
    }

    @Test
    fun aFiringWhoseSecretWasNotSetUpHereIsRefusedInTheCoresWords() {
        val notSetUp = automationWords().notSetUp
        val stranger = Tasker.fire(core, secrets, TaskerAction("undo", emptyMap(), emptyMap(), "made elsewhere").bundle())
        assertFalse(stranger.ok)
        assertEquals(notSetUp, stranger.variables.text("%errmsg"))

        // A secret set up for one operation does not run another.
        val secret = secrets.make()
        secrets.keep(secret, "list_tasks")
        val other = Tasker.fire(core, secrets, TaskerAction("trash_task", mapOf("id" to "x"), emptyMap(), secret).bundle())
        assertFalse(other.ok)
        assertEquals(notSetUp, other.variables.text("%errmsg"))

        // Nor does one with no secret at all.
        assertFalse(Tasker.fire(core, secrets, Bundle()).ok)
    }

    @Test
    fun theReceiverAnswersTaskersWaitingBroadcastAndRefusesAStrangersFiring() {
        // Through the receiver itself, as Tasker sends it: an ordered broadcast, answered in its
        // result code and extras. A stranger's secret writes nothing, so the app's own store is safe.
        val answered = CountDownLatch(1)
        var code = 0
        var extras: Bundle? = null
        val fire = Intent("com.twofortyfouram.locale.intent.action.FIRE_SETTING")
            .setComponent(ComponentName(context, TaskerFireReceiver::class.java))
            .putExtra(Tasker.EXTRA_BUNDLE, TaskerAction("undo", emptyMap(), emptyMap(), "made elsewhere").bundle())
        context.sendOrderedBroadcast(
            fire, null,
            object : BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) {
                    code = resultCode
                    extras = getResultExtras(true)
                    answered.countDown()
                }
            },
            null, Activity.RESULT_OK, null, null,
        )
        assertTrue(answered.await(30, TimeUnit.SECONDS))
        assertEquals(Tasker.RESULT_FAILED, code)
        assertEquals(automationWords().notSetUp, extras?.getBundle("net.dinglisch.android.tasker.extras.VARIABLES")?.getString("%errmsg"))
    }

    @Test
    fun aFiringTaskerDidNotWaitOnIsAnsweredThroughItsCompletionIntent() {
        val action = "dev.emassey0135.lumenna.test.TASKER_COMPLETED"
        val completed = CountDownLatch(1)
        var completion: Intent? = null
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                completion = intent
                completed.countDown()
            }
        }
        ContextCompat.registerReceiver(context, receiver, IntentFilter(action), ContextCompat.RECEIVER_NOT_EXPORTED)
        try {
            val secret = secrets.make().also { secrets.keep(it, "add_task") }
            val fired = Tasker.fire(core, secrets, TaskerAction("add_task", mapOf("text" to "Call the bank"), emptyMap(), secret).bundle())
            val fire = Intent().putExtra(
                "net.dinglisch.android.tasker.extras.COMPLETION_INTENT",
                Intent(action).setPackage(context.packageName).toUri(Intent.URI_INTENT_SCHEME),
            )
            assertTrue(Tasker.finish(context, fire, fired))
            assertTrue(completed.await(10, TimeUnit.SECONDS))
            val answer = checkNotNull(completion)
            assertEquals(Tasker.RESULT_OK, answer.getIntExtra("net.dinglisch.android.tasker.extras.RESULT_CODE", 0))
            val variables = answer.getBundleExtra("net.dinglisch.android.tasker.extras.VARIABLES")
            assertEquals("Call the bank", variables?.getString("%task_title"))
        } finally {
            context.unregisterReceiver(receiver)
        }
    }
}
