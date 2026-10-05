package io.github.emassey0135.lumenna

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.system.Os
import io.github.emassey0135.lumenna.core.Change
import io.github.emassey0135.lumenna.core.Lumenna
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.Reach
import io.github.emassey0135.lumenna.core.SyncListener
import io.github.emassey0135.lumenna.core.SyncReport
import io.github.emassey0135.lumenna.core.SyncService
import java.io.File
import java.util.TimeZone
import java.util.concurrent.Executors
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The open store, for the whole app.
 *
 * Every operation is a method on [lumenna], generated from the Rust surface: this class adds
 * only what is about running on Android — where the profile lives, the time zone, syncing while
 * the app is in front, and saying what happened. Operations are milliseconds against a local
 * SQLite file, so they run on the main thread and a screen never shows a state the store has
 * moved past, as on the iPhone.
 */
class Core(directory: File) {
    val lumenna: Lumenna

    init {
        useSystemTimeZone()
        directory.mkdirs()
        lumenna = Lumenna.open(directory.absolutePath)
    }

    private val generation = MutableStateFlow(0L)

    /** Moves whenever the store changed, so every screen showing it reads it again. */
    val changes: StateFlow<Long> = generation.asStateFlow()

    private val spoken = MutableSharedFlow<String>(extraBufferCapacity = 8)

    /** What to say: a change's announcement, or why something failed. The app shows each. */
    val announcements: SharedFlow<String> = spoken.asSharedFlow()

    /** Every screen showing the store reads it again. */
    fun changed() {
        generation.value += 1
    }

    /** Says [text], politely, after whatever is being said now. */
    fun say(text: String) {
        if (text.isNotBlank()) spoken.tryEmit(text)
    }

    /** Says what a change did: the core's sentence, then each notice. */
    fun say(change: Change) = say(sentence(change.announcement, change.notices))

    /**
     * Runs an operation that changes the store, then has every screen read it again and says
     * what happened — or, if it failed, says why. Returns the change, or null on failure.
     */
    fun change(operation: (Lumenna) -> Change): Change? = attempt { operation(lumenna) }?.also {
        changed()
        say(it)
    }

    /** Runs [operation], saying why if it fails. */
    fun <T> attempt(operation: () -> T): T? = try {
        operation()
    } catch (error: LumennaException) {
        say(error.sentence)
        null
    }

    private val syncThread = Executors.newSingleThreadExecutor { Thread(it, "lumenna-sync") }
    private var sync: SyncService? = null
    private val main = Handler(Looper.getMainLooper())

    /** Starts keeping this device in sync (§8), for as long as the app is in front. */
    fun startSyncing() {
        syncThread.execute {
            if (sync == null) {
                sync = try {
                    lumenna.startSync(Reach.INTERNET, Arrivals())
                } catch (_: LumennaException) {
                    null
                }
            }
        }
    }

    /** Stops syncing and lets go of the endpoint. */
    fun stopSyncing() {
        syncThread.execute {
            sync?.stop()
            sync = null
        }
    }

    /** Syncs with every paired device now, off the main thread. */
    fun syncNow(finished: (Result<SyncReport>) -> Unit) {
        syncThread.execute {
            val result = runCatching { sync?.syncNow() ?: lumenna.syncNow(Reach.INTERNET) }
            main.post { finished(result) }
        }
    }

    /** Takes a backup if one is due (§9), off the main thread. */
    fun backUpIfDue() {
        syncThread.execute {
            try {
                lumenna.backUpIfDue()
            } catch (error: LumennaException) {
                main.post { say("The automatic backup failed. ${error.sentence}") }
            }
        }
    }

    /** Called when the app comes back, since the person may have travelled. */
    fun timeZoneMayHaveChanged() = useSystemTimeZone()

    /** What a sync brought in: every screen reads the store again. */
    private inner class Arrivals : SyncListener {
        override fun changed() {
            main.post { this@Core.changed() }
        }
    }

    companion object {
        /**
         * Where the store lives: no-backup storage, the app's own and never copied to Google
         * by Android's backup — the store reaches another device by pairing (§9).
         */
        fun profileDirectory(context: Context): File = File(context.noBackupFilesDir, "lumenna")

        /**
         * Tells the core which time zone "today" is in. The core reads Android's own setting
         * when `TZ` is unset, but this is the zone the app's own clock uses, so they agree.
         */
        private fun useSystemTimeZone() {
            Os.setenv("TZ", TimeZone.getDefault().id, true)
        }
    }
}

/** The sentence the core wrote, which is already phrased to be read aloud. */
val LumennaException.sentence: String
    get() = message.orEmpty().replaceFirstChar { it.uppercase() }

/** An announcement, then each notice. */
fun sentence(announcement: String, notices: List<String>): String =
    (listOf(announcement) + notices).filter { it.isNotBlank() }.joinToString(". ")
        .replaceFirstChar { it.uppercase() }
