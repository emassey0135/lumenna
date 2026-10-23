package dev.emassey0135.lumenna

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.Looper
import android.system.Os
import dev.emassey0135.lumenna.app.Change
import dev.emassey0135.lumenna.app.Device
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.app.SyncReport
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
 * The screens draw the app itself ([app], the Crux app through BoltFFI); what runs without them
 * — reminders' alarms and receivers, a background round, the watch's link — asks [device], the
 * same store. This class adds only what is about running on Android: where the profile lives,
 * the time zone, syncing while the app is in front, and saying what happened.
 */
class Core(
    /** Where the store lives: also what tells its notifications from another store's. */
    val directory: File,
    /** What this device says it runs, in pairing: "android", or "wearos" on a watch. */
    val platform: String = "android",
) {
    val device: Device

    init {
        useSystemTimeZone()
        directory.mkdirs()
        device = Device(directory.absolutePath)
    }

    /**
     * The app itself over this store (`AppModel`), opened on first use, on the main thread: the
     * screens draw its view and send it what the person does.
     */
    val app: AppModel by lazy { AppModel(this, platform).also { appOpened = true } }
    private var appOpened = false

    /** Whether the app is still, or not yet opened: what the tests wait on. */
    val still: Boolean get() = !appOpened || app.still

    private val generation = MutableStateFlow(0L)

    /** Moves whenever the store changed, so every screen showing it reads it again. */
    val changes: StateFlow<Long> = generation.asStateFlow()

    private val spoken = MutableSharedFlow<String>(extraBufferCapacity = 8)

    /** What to say: a change's announcement, or why something failed. The app shows each. */
    val announcements: SharedFlow<String> = spoken.asSharedFlow()

    /** A write made outside the app — a receiver's, a test's — which the app takes in. */
    fun changed() {
        moved()
        if (appOpened) app.tick()
    }

    /** The store may have moved: what follows it (reminders' alarms) looks again. */
    fun moved() {
        generation.value += 1
    }

    /** Says [text], politely, after whatever is being said now. */
    fun say(text: String) {
        if (text.isNotBlank()) spoken.tryEmit(text)
    }

    private var held: MutableList<String>? = null

    /**
     * Holds what changes say until [release]: a list is about to put focus on a row, and the
     * announcement should follow it rather than be cut off by it, as on the iPhone. Failures
     * are never held — nothing will move focus for them.
     */
    fun hold() {
        if (held == null) held = mutableListOf()
    }

    private val dropped = MutableStateFlow(0L)

    /** Moves whenever an action is given up, so a list stops waiting to put focus back. */
    val cancels: StateFlow<Long> = dropped.asStateFlow()

    /**
     * An action's question was cancelled, or had nothing to ask: no change is coming, so
     * nothing waits for one to put focus back on the row, and what was held is said.
     */
    fun cancelled() {
        release()
        dropped.value += 1
    }

    /** Says what was held. */
    fun release() {
        val texts = held ?: return
        held = null
        texts.forEach(::say)
    }

    /** Says what a change did, now or once focus has moved. */
    fun report(text: String) {
        held?.add(text) ?: say(text)
    }

    /** Says what a change did: the core's sentence, then each notice. */
    fun say(change: Change) = report(sentence(change.announcement, change.notices))

    /**
     * Runs an operation on the store from outside the app — a receiver's, a test's seeding — then
     * has the app take it in and says what happened, or why it failed. Returns the change, or
     * null on failure.
     */
    fun change(operation: (Device) -> Change): Change? = attempt { operation(device) }?.also {
        changed()
        say(it)
    }

    /** Runs [operation], saying why if it fails. */
    fun <T> attempt(operation: () -> T): T? = try {
        operation()
    } catch (error: LumennaError) {
        say(error.sentence)
        null
    }

    private val syncThread = Executors.newSingleThreadExecutor { Thread(it, "lumenna-sync") }
    private val main = Handler(Looper.getMainLooper())

    private var multicast: WifiManager.MulticastLock? = null

    /**
     * Starts keeping this device in sync, for as long as the app is in front: the app's own
     * network, told it is in front.
     *
     * Holds a multicast lock meanwhile, so local discovery hears the other devices: Android
     * drops multicast addressed to the phone unless some app holds one. It is not exclusive —
     * every app's mDNS keeps working, and the discovery socket shares port 5353 — and it is let
     * go when the app leaves the front, so it costs battery only while Lumenna is open. Pairing
     * happens only while the app is in front, so it holds the lock too.
     */
    fun startSyncing(context: Context) {
        if (multicast == null) {
            val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            multicast = wifi.createMulticastLock("lumenna-sync").apply {
                setReferenceCounted(false)
                acquire()
            }
        }
        app.inFront(true)
    }

    /** Stops syncing and lets go of the endpoint and the multicast lock. */
    fun stopSyncing() {
        multicast?.release()
        multicast = null
        if (appOpened) app.inFront(false)
    }

    /**
     * One round with every paired device, waiting for it: through the running service while the
     * app is in front, else on an endpoint opened for the round.
     */
    fun syncRound(): SyncReport = syncThread.submit(java.util.concurrent.Callable { device.syncNow() }).get()

    /** Takes a backup if one is due, off the main thread. */
    fun backUpIfDue() {
        syncThread.execute {
            try {
                device.backUpIfDue()
            } catch (error: LumennaError) {
                main.post { say("The automatic backup failed. ${error.sentence}") }
            }
        }
    }

    /** Called when the app comes back, since the person may have travelled. */
    fun timeZoneMayHaveChanged() = useSystemTimeZone()

    companion object {
        /**
         * Where the store lives: no-backup storage, the app's own and never copied to Google
         * by Android's backup — the store reaches another device by pairing.
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
val LumennaError.sentence: String
    get() = when (this) {
        is LumennaError.Failed -> reason
        is LumennaError.SyncElsewhere -> reason
    }.replaceFirstChar { it.uppercase() }

/** An announcement, then each notice. */
fun sentence(announcement: String, notices: List<String>): String =
    (listOf(announcement) + notices).filter { it.isNotBlank() }.joinToString(". ")
        .replaceFirstChar { it.uppercase() }
