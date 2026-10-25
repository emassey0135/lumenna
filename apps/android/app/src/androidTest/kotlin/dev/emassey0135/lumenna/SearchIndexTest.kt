package dev.emassey0135.lumenna

import android.os.Build
import androidx.appsearch.app.GetByDocumentIdRequest
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/** The system's search index: the core's documents, field for field, and only those still there. */
@RunWith(AndroidJUnit4::class)
class SearchIndexTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val directory = File(context.cacheDir, "search-${UUID.randomUUID()}")

    @After
    fun tearDown() {
        directory.deleteRecursively()
    }

    @Test
    fun aTaskIsIndexedAsTheCoreDescribesItAndRemovedOnceTrashed() {
        assumeTrue(Build.VERSION.SDK_INT >= Build.VERSION_CODES.S)
        val core = Core(directory)
        val task = core.device.addTask("Renew the passport #Errands").task ?: error("the change names its task")
        val document = core.device.searchDocuments().first { it.id == "task:${task.id}" }
        val session = SearchIndex.indexNow(context, core)
        val ids = listOf(document.id)
        val found = session.getByDocumentIdAsync(GetByDocumentIdRequest.Builder(SearchIndex.NAMESPACE).addIds(ids).build()).get()
        val indexed = found.successes[document.id] ?: error("the task is indexed")
        assertEquals("Renew the passport", indexed.getPropertyString("title"))
        assertEquals(document.line, indexed.getPropertyString("line"))
        assertEquals(document.keywords, indexed.getPropertyStringArray("keywords")?.toList().orEmpty())
        assertEquals(document.launch, indexed.getPropertyString("launch"))
        core.device.trashTask(task.id)
        SearchIndex.indexNow(context, core)
        val after = session.getByDocumentIdAsync(GetByDocumentIdRequest.Builder(SearchIndex.NAMESPACE).addIds(ids).build()).get()
        assertTrue("a trashed task is no longer found", after.successes.isEmpty())
    }
}
