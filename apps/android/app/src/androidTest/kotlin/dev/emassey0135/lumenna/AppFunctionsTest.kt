package dev.emassey0135.lumenna

import androidx.appfunctions.AppFunctionInvalidArgumentException
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.emassey0135.lumenna.app.assistantOperations
import dev.emassey0135.lumenna.functions.Assistant
import java.io.File
import java.util.UUID
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import org.w3c.dom.Element

/**
 * Gemini's App Functions: exactly the core's assistant operations, in the core's words, as the
 * system indexes them (the XML the library wrote into the app's assets), and each answered by
 * the core's dispatcher.
 */
@RunWith(AndroidJUnit4::class)
class AppFunctionsTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val directory = File(context.cacheDir, "functions-${UUID.randomUUID()}")

    @After
    fun tearDown() {
        directory.deleteRecursively()
    }

    /** A function as the indexed XML describes it. */
    private data class Declared(val description: String, val parameters: Map<String, Pair<String, Boolean>>)

    private fun Element.child(name: String): String? =
        (0 until childNodes.length).map { childNodes.item(it) }.filterIsInstance<Element>().firstOrNull { it.tagName == name }?.textContent

    private fun Element.children(name: String): List<Element> =
        (0 until childNodes.length).map { childNodes.item(it) }.filterIsInstance<Element>().filter { it.tagName == name }

    /** Every function the system indexes, by identifier. */
    private fun declared(file: String): Map<String, Declared> {
        val document = context.assets.open(file).use { DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(it) }
        return document.documentElement.children("appfunction").associate { function ->
            val parameters = function.children("parameters").associate {
                it.child("name").orEmpty() to (it.child("description").orEmpty() to (it.child("isRequired") == "true"))
            }
            function.child("id").orEmpty() to Declared(function.child("description").orEmpty(), parameters)
        }
    }

    /** `complete_task` as its function is named: `LumennaFunctions#completeTask`. */
    private fun functionId(operation: String): String {
        val words = operation.split('_')
        return "dev.emassey0135.lumenna.functions.LumennaFunctions#" +
            words.first() + words.drop(1).joinToString("") { it.replaceFirstChar(Char::uppercase) }
    }

    @Test
    fun everyAssistantOperationIsAnAppFunctionInTheCoresWordsAndNoOtherIs() {
        val functions = declared("lumenna_functions.xml")
        val operations = assistantOperations()
        for (operation in operations) {
            val function = functions[functionId(operation.id)]
            if (function == null) {
                fail("${operation.id} has no App Function: add it to LumennaFunctions")
                return
            }
            assertEquals("${operation.id}'s description", operation.description, function.description)
            assertEquals(
                "${operation.id}'s parameters",
                operation.parameters.associate { it.name to (it.description to !it.optional) },
                function.parameters,
            )
        }
        assertEquals("functions that are no operation", emptySet<String>(), functions.keys - operations.map { functionId(it.id) }.toSet())
    }

    @Test
    fun androidSixteensIndexListsTheSameFunctions() {
        val document = context.assets.open("lumenna_functions-v1.xml").use { DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(it) }
        val listed = document.documentElement.children("appfunction").map { it.child("function_id").orEmpty() }.toSet()
        assertEquals(assistantOperations().map { functionId(it.id) }.toSet(), listed)
    }

    @Test
    fun aFunctionRunsItsOperationThroughTheCoreAndSaysWhatItDid() {
        val core = Core(directory)
        val added = Assistant.changeMade(Assistant.run(core, "add_task", Assistant.text("text", "Call the dentist tomorrow")))
        assertTrue(added.changed)
        val task = added.task ?: error("the change names its task")
        assertEquals("Call the dentist", task.title)
        val listed = Assistant.listing(Assistant.run(core, "list_tasks"))
        assertEquals(listOf(task.id), listed.rows.map { it.id })
        val done = Assistant.changeMade(Assistant.run(core, "complete_task", Assistant.text("id", task.id)))
        assertTrue(done.announcement.isNotBlank())
        assertEquals(emptyList<String>(), Assistant.listing(Assistant.run(core, "list_tasks")).rows.map { it.id })
    }

    @Test
    fun anOperationThatAsksFirstAnswersWithTheCoresQuestionUntilConfirmed() {
        val core = Core(directory)
        try {
            Assistant.run(core, "erase_everything_deleted")
            fail("erasing everything deleted went ahead without confirm")
        } catch (refused: AppFunctionInvalidArgumentException) {
            assertTrue(refused.errorMessage.orEmpty(), refused.errorMessage.orEmpty().contains("Ask the person first"))
        }
        val erased = Assistant.changeMade(Assistant.run(core, "erase_everything_deleted", Assistant.flag("confirm", true)))
        assertFalse(erased.announcement.isBlank())
    }

    @Test
    fun aReplanProposedGoesBackToTheCoreWhole() {
        val core = Core(directory)
        val proposed = Assistant.replanProposed(Assistant.run(core, "propose_replan"))
        val applied = Assistant.replanApplied(
            Assistant.run(core, "apply_replan", Assistant.replan("proposal", proposed), Assistant.replanChanges("accepted", proposed.changes)),
        )
        assertFalse(applied.announcement.isBlank())
    }
}
