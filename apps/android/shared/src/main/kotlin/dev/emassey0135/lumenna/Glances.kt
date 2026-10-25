package dev.emassey0135.lumenna

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import dev.emassey0135.lumenna.app.GlanceEntry
import java.time.Duration
import java.time.Instant
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.launch

/**
 * What the home screen's widgets, the watch's tile and its complications show: the core's
 * glance (`records::glance`, worded by `lumenna_app::glance`), a timeline of moments, each whole.
 * Each is drawn again when the store changes and at the next moment the timeline names (a block
 * starting or ending, a due time passing, midnight); a running timer counts by itself from its
 * `countingFrom`, so it needs no moment of its own. What draws a glance decides nothing about
 * what it shows.
 */
object Glances {
    /** How far ahead a glance is asked for: a day, as widget systems ask. */
    const val HOURS: UInt = 24u

    /** The core's glance from now, worded in this device's clock. */
    fun entries(context: Context, core: Core): List<GlanceEntry> {
        Clock.update(context)
        return core.glance(HOURS)
    }

    /** When each entry starts being true. */
    fun at(entry: GlanceEntry): Instant? = runCatching { Instant.parse(entry.moment.at) }.getOrNull()

    /** An instant the core gives (a block's start or end, a timer's counting from). */
    fun instant(text: String): Instant? = runCatching { Instant.parse(text) }.getOrNull()

    /** The entry true at [now]: the last that starts by then, else the first. */
    fun now(entries: List<GlanceEntry>, now: Instant = Instant.now()): GlanceEntry? =
        entries.lastOrNull { at(it)?.let { start -> !start.isAfter(now) } ?: false } ?: entries.firstOrNull()

    /**
     * Draws every glance again whenever the store changes, a moment after the last change of a
     * run, and at the next moment its timeline names, on what the app shows it on
     * ([GlanceShower]: the phone's widgets, the watch's tile and complications).
     */
    @OptIn(FlowPreview::class)
    fun follow(context: Context, core: Core) {
        val app = context.applicationContext
        CoroutineScope(SupervisorJob() + Dispatchers.Main).launch {
            core.changes.debounce(1_000).collect { refresh(app) }
        }
    }

    /** Draws every glance again now, then asks to be woken at the next moment. */
    fun refresh(context: Context) {
        val app = context.applicationContext
        val core = (app as? CoreHolder)?.core?.getOrNull() ?: return
        (app as? GlanceShower)?.showGlances(app)
        // The next moment, worked out off the main thread: the core reads the store.
        Thread {
            val next = runCatching { entries(app, core) }.getOrNull()
                ?.mapNotNull(::at)?.firstOrNull { it.isAfter(Instant.now()) }
            schedule(app, next)
        }.start()
    }

    /** Wakes [GlanceWorker] at [next], or in a day when nothing changes before then. */
    private fun schedule(context: Context, next: Instant?) {
        val delay = Duration.between(Instant.now(), next ?: Instant.now().plus(Duration.ofHours(HOURS.toLong())))
            .toMillis().coerceAtLeast(0)
        val request = OneTimeWorkRequestBuilder<GlanceWorker>().setInitialDelay(delay, TimeUnit.MILLISECONDS).build()
        WorkManager.getInstance(context).enqueueUniqueWork(NEXT, ExistingWorkPolicy.REPLACE, request)
    }

    /** The one wake-up waiting, replaced each time. */
    const val NEXT = "glance-next"
}

/** Draws the glance again at the moment the timeline named, and asks for the one after. */
class GlanceWorker(context: Context, parameters: WorkerParameters) : Worker(context, parameters) {
    override fun doWork(): Result {
        // What shows a glance runs on the main thread, as the widgets' and tiles' requests ask.
        Handler(Looper.getMainLooper()).post { Glances.refresh(applicationContext) }
        return Result.success()
    }
}

/** What an app shows a glance on, which its Application says: the phone's widgets, the watch's tile and complications. */
interface GlanceShower {
    /** Asks each to draw the glance again. */
    fun showGlances(context: Context)
}
