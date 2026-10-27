package dev.emassey0135.lumenna

import android.content.Context
import android.os.Build
import android.util.Log
import androidx.appsearch.app.AppSearchSchema
import androidx.appsearch.app.AppSearchSchema.StringPropertyConfig
import androidx.appsearch.app.AppSearchSession
import androidx.appsearch.app.Features
import androidx.appsearch.app.GenericDocument
import androidx.appsearch.app.PutDocumentsRequest
import androidx.appsearch.app.RemoveByDocumentIdRequest
import androidx.appsearch.app.SetSchemaRequest
import androidx.appsearch.platformstorage.PlatformStorage
import dev.emassey0135.lumenna.app.SearchDocument
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.launch

/**
 * The core's search documents (`Device.searchDocuments()`, `records::search`) in AppSearch, the
 * system's own index (Android 12 and later), so what searches the device finds tasks, projects,
 * labels, saved filters and blocks there. Each document is the core's, field for field, a task's
 * with what its entity holds beside (`SearchTask`: its project, labels, priority, states…), for
 * an agent to tell tasks apart by; what matches a search and how a result reads are the core's,
 * never this index's to decide.
 *
 * It is kept current as the store changes: what changed is written again and what went is
 * removed, as the last indexing (remembered by each document's fingerprint) tells them apart.
 * The documents are visible to the system's UI, to the home screen's and the assistant's search,
 * and to an agent allowed to execute App Functions, which finds a task there to name it to a
 * function (`functions/`). Opening one is [MainActivity] with [Launching.ACTION] and its
 * `launch` as [Launching.ID], sent to the app as `ShellEvent.Launch`.
 */
object SearchIndex {
    /** The database, namespace and schema type the documents live under. */
    const val DATABASE = "lumenna"
    const val NAMESPACE = "lumenna"
    const val TYPE = "LumennaItem"

    private val worker = Executors.newSingleThreadExecutor { Thread(it, "lumenna-search-index") }
    private var session: AppSearchSession? = null

    /** The schema, from `SearchDocument`'s fields: names and words found by prefix, the rest kept. */
    private val schema = AppSearchSchema.Builder(TYPE)
        .addProperty(kept("kind", StringPropertyConfig.INDEXING_TYPE_EXACT_TERMS, StringPropertyConfig.TOKENIZER_TYPE_VERBATIM))
        .addProperty(kept("title", StringPropertyConfig.INDEXING_TYPE_PREFIXES, StringPropertyConfig.TOKENIZER_TYPE_PLAIN))
        .addProperty(kept("line", StringPropertyConfig.INDEXING_TYPE_PREFIXES, StringPropertyConfig.TOKENIZER_TYPE_PLAIN))
        .addProperty(
            StringPropertyConfig.Builder("keywords")
                .setCardinality(AppSearchSchema.PropertyConfig.CARDINALITY_REPEATED)
                .setIndexingType(StringPropertyConfig.INDEXING_TYPE_PREFIXES)
                .setTokenizerType(StringPropertyConfig.TOKENIZER_TYPE_PLAIN)
                .build(),
        )
        .addProperty(kept("due", StringPropertyConfig.INDEXING_TYPE_NONE, StringPropertyConfig.TOKENIZER_TYPE_NONE))
        .addProperty(kept("launch", StringPropertyConfig.INDEXING_TYPE_NONE, StringPropertyConfig.TOKENIZER_TYPE_NONE))
        // A task's own, from `SearchTask`.
        .addProperty(kept("project", StringPropertyConfig.INDEXING_TYPE_EXACT_TERMS, StringPropertyConfig.TOKENIZER_TYPE_VERBATIM))
        .addProperty(several("labels"))
        .addProperty(number("priority"))
        .addProperty(kept("dueTime", StringPropertyConfig.INDEXING_TYPE_NONE, StringPropertyConfig.TOKENIZER_TYPE_NONE))
        .addProperty(number("estimateMinutes"))
        .addProperty(kept("repetition", StringPropertyConfig.INDEXING_TYPE_NONE, StringPropertyConfig.TOKENIZER_TYPE_NONE))
        .addProperty(several("states"))
        .addProperty(kept("createdAt", StringPropertyConfig.INDEXING_TYPE_NONE, StringPropertyConfig.TOKENIZER_TYPE_NONE))
        .addProperty(flag("done"))
        .addProperty(flag("deleted"))
        .build()

    private fun several(name: String) = StringPropertyConfig.Builder(name)
        .setCardinality(AppSearchSchema.PropertyConfig.CARDINALITY_REPEATED)
        .setIndexingType(StringPropertyConfig.INDEXING_TYPE_EXACT_TERMS)
        .setTokenizerType(StringPropertyConfig.TOKENIZER_TYPE_VERBATIM)
        .build()

    private fun number(name: String) = AppSearchSchema.LongPropertyConfig.Builder(name)
        .setCardinality(AppSearchSchema.PropertyConfig.CARDINALITY_OPTIONAL)
        .build()

    private fun flag(name: String) = AppSearchSchema.BooleanPropertyConfig.Builder(name)
        .setCardinality(AppSearchSchema.PropertyConfig.CARDINALITY_OPTIONAL)
        .build()

    private fun kept(name: String, indexing: Int, tokenizer: Int) = StringPropertyConfig.Builder(name)
        .setCardinality(AppSearchSchema.PropertyConfig.CARDINALITY_OPTIONAL)
        .setIndexingType(indexing)
        .setTokenizerType(tokenizer)
        .build()

    /** Indexes the store now and again whenever it changes, a moment after the last change of a run. */
    @OptIn(FlowPreview::class)
    fun follow(context: Context, core: Core) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return
        val app = context.applicationContext
        CoroutineScope(SupervisorJob() + Dispatchers.Main).launch {
            core.changes.debounce(2_000).collect { worker.execute { runCatching { indexNow(app, core) }.onFailure { Log.w("LumennaSearch", "Indexing failed", it) } } }
        }
    }

    /** A document as AppSearch keeps it. */
    fun document(doc: SearchDocument): GenericDocument = GenericDocument.Builder<GenericDocument.Builder<*>>(NAMESPACE, doc.id, TYPE)
        .setPropertyString("kind", doc.kind.name.lowercase())
        .setPropertyString("title", doc.title)
        .setPropertyString("line", doc.line)
        .setPropertyString("keywords", *doc.keywords.toTypedArray())
        .apply { doc.due?.let { setPropertyString("due", it) } }
        .setPropertyString("launch", doc.launch)
        .apply {
            doc.task?.let { task ->
                task.project?.let { setPropertyString("project", it) }
                setPropertyString("labels", *task.labels.toTypedArray())
                setPropertyLong("priority", task.priority.toLong())
                task.dueTime?.let { setPropertyString("dueTime", it) }
                task.estimateMins?.let { setPropertyLong("estimateMinutes", it.toLong()) }
                task.repetition?.let { setPropertyString("repetition", it) }
                setPropertyString("states", *task.states.toTypedArray())
                setPropertyString("createdAt", task.createdAt)
                setPropertyBoolean("done", task.done)
                setPropertyBoolean("deleted", task.deleted)
            }
        }
        .build()

    /** What tells one version of a document from another, kept between indexings. */
    internal fun fingerprint(doc: SearchDocument): String = fingerprint(
        listOf(doc.kind.name, doc.title, doc.line, doc.keywords.joinToString("\u001f"), doc.due.orEmpty(), doc.launch, doc.task.toString()),
    )

    /**
     * A document's fields as SHA-256: a 32-bit hash let two versions of a document collide, and
     * the second was never indexed.
     */
    internal fun fingerprint(fields: List<String>): String =
        java.security.MessageDigest.getInstance("SHA-256").digest(fields.joinToString("\u001e").toByteArray())
            .joinToString("") { "%02x".format(it) }

    /**
     * Writes what changed since the last indexing and removes what went, waiting for it: on the
     * worker thread, or a test's. Returns the session, for a test to look in.
     */
    @Synchronized
    fun indexNow(context: Context, core: Core): AppSearchSession {
        val session = session ?: open(context).also { session = it }
        val documents = core.device.searchDocuments()
        val remembered = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
        val before = remembered.all.mapValues { it.value as? String }
        val now = documents.associate { it.id to fingerprint(it) }
        val changed = documents.filter { before[it.id] != now[it.id] }
        val gone = before.keys - now.keys
        if (changed.isNotEmpty()) {
            session.putAsync(PutDocumentsRequest.Builder().addGenericDocuments(changed.map(::document)).build()).get()
        }
        if (gone.isNotEmpty()) {
            session.removeAsync(RemoveByDocumentIdRequest.Builder(NAMESPACE).addIds(gone).build()).get()
        }
        if (changed.isNotEmpty() || gone.isNotEmpty()) {
            session.requestFlushAsync().get()
            remembered.edit().clear().apply { now.forEach { (id, print) -> putString(id, print) } }.apply()
        }
        return session
    }

    /**
     * Opens the platform's index and sets the schema, visible to the system's UI, the home
     * screen's and the assistant's search, and App Functions' callers where the device has them.
     */
    private fun open(context: Context): AppSearchSession {
        val session = PlatformStorage.createSearchSessionAsync(PlatformStorage.SearchContext.Builder(context, DATABASE).build()).get()
        val visible = session.features.isFeatureSupported(Features.ADD_PERMISSIONS_AND_GET_VISIBILITY)
        val readers = buildList {
            if (visible) {
                add(setOf(SetSchemaRequest.READ_HOME_APP_SEARCH_DATA))
                add(setOf(SetSchemaRequest.READ_ASSISTANT_APP_SEARCH_DATA))
            }
            if (visible && Build.VERSION.SDK_INT >= Build.VERSION_CODES.BAKLAVA) add(setOf(SetSchemaRequest.EXECUTE_APP_FUNCTIONS))
        }
        fun request(readers: List<Set<Int>>) = SetSchemaRequest.Builder()
            .addSchemas(schema)
            .setSchemaTypeDisplayedBySystem(TYPE, true)
            .setForceOverride(true)
            .apply { readers.forEach { addRequiredPermissionsForSchemaTypeVisibility(TYPE, it) } }
            .build()
        // A platform that does not know a permission refuses the whole schema: it is set again
        // with the ones every version knows.
        runCatching { session.setSchemaAsync(request(readers)).get() }
            .recoverCatching { session.setSchemaAsync(request(readers.filter { SetSchemaRequest.EXECUTE_APP_FUNCTIONS !in it })).get() }
            .getOrThrow()
        return session
    }

    private const val PREFERENCES = "search-index"
}
