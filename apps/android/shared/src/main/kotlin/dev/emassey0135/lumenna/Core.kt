package dev.emassey0135.lumenna

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.Looper
import dev.emassey0135.lumenna.app.Change
import dev.emassey0135.lumenna.app.Device
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.app.SyncReport
import java.io.File
import java.util.concurrent.Executors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * The open store, for the whole app.
 *
 * The screens draw the app itself ([app], the Crux app through BoltFFI); what runs without them
 * — reminders' alarms and receivers, a background round, the watch's link — asks [device], the
 * same store. This class adds only what is about running on Android: where the profile lives,
 * syncing while the app is in front, and saying what happened.
 *
 * The time zone is Android's own: with `TZ` unset, the core reads `persist.sys.timezone`, the
 * zone the app's own clock reads too, and follows a change as the person travels. Setting `TZ`
 * from here raced the core's threads reading it, and took effect no sooner.
 */
class Core(
    /** Where the store lives: also what tells its notifications from another store's. */
    val directory: File,
    /** What this device says it runs, in pairing: "android", or "wearos" on a watch. */
    val platform: String = "android",
) {
    val device: Device

    init {
        directory.mkdirs()
        device = Device(directory.absolutePath)
    }

    /**
     * The app itself over this store (`AppModel`), opened on first use, on the main thread: the
     * screens draw its view and send it what the person does.
     */
    val app: AppModel by lazy { AppModel(this, platform).also { appOpened = true } }
    private var appOpened = false

    /**
     * What a widget, tile or complication shows for the next [hours] (`Glances`): the app's
     * glance while it runs, else the device's, both worded in this device's clock.
     */
    fun glance(hours: UInt): List<dev.emassey0135.lumenna.app.GlanceEntry> =
        if (appOpened) app.glance(hours) else device.glance(hours, Clock.locale)

    /** Whether the app is still, or not yet opened: what the tests wait on. */
    val still: Boolean get() = !appOpened || app.still

    private val generation = MutableStateFlow(0L)

    /**
     * Moves whenever the store changed, so what follows it — reminders' alarms, the widgets, the
     * search index, the watch's link — looks again. Not for every turn of the app's: a keystroke
     * or a new minute changes nothing in the store, and each look reads all of it.
     */
    val changes: StateFlow<Long> = generation.asStateFlow()

    private val spoken = announcementFlow()

    /** What to say: a change's announcement, or why something failed. The app shows each. */
    val announcements: SharedFlow<String> = spoken.asSharedFlow()

    /** A write made outside the app — a receiver's, a test's — which the app takes in. From any thread. */
    fun changed() {
        moved()
        if (appOpened) app.tick()
    }

    /** The store may have moved: what follows it (reminders' alarms) looks again. From any thread. */
    fun moved() {
        generation.update { it + 1 }
    }

    private val headsThread = Executors.newSingleThreadExecutor { Thread(it, "lumenna-heads") }
    private val heads = Heads(read = { runCatching { device.heads() }.getOrNull() }, run = headsThread::execute, moved = ::moved)

    /**
     * The app left a turn: [changes] moves only if the store's heads did, which a write here or
     * one taken in from elsewhere moves, and a keystroke or a new minute does not.
     */
    fun turned() = heads.look()

    /** Says [text], politely, after whatever is being said now. From any thread. */
    fun say(text: String) {
        if (text.isNotBlank()) spoken.tryEmit(text)
    }

    private val main = Handler(Looper.getMainLooper())
    private var held: MutableList<String>? = null

    /**
     * Holds what changes say until [release]: a list is about to put focus on a row, and the
     * announcement should follow it rather than be cut off by it, as on the iPhone. Failures
     * are never held — nothing will move focus for them.
     */
    fun hold() {
        if (held == null) held = mutableListOf()
    }

    /** Says what was held. */
    fun release() {
        val texts = held ?: return
        held = null
        texts.forEach(::say)
    }

    /** Says what a change did, now or once focus has moved. From any thread: what is held is the main thread's. */
    fun report(text: String) {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            main.post { report(text) }
            return
        }
        held?.add(text) ?: say(text)
    }

    /** Says what a change did: the core's sentence, then each notice. */
    fun say(change: Change) = report(sentence(change.announcement, change.notices))

    /** Runs [operation], saying why if it fails. */
    fun <T> attempt(operation: () -> T): T? = try {
        operation()
    } catch (error: LumennaError) {
        say(error.sentence)
        null
    }

    private val syncThread = Executors.newSingleThreadExecutor { Thread(it, "lumenna-sync") }

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

    /** Takes a backup now, into this device's backups, off the main thread: says what it did, or why not. */
    fun backUpNow() {
        syncThread.execute { attempt { device.backup() }?.let { say(sentence(it.announcement, it.notices)) } }
    }

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

    companion object {
        /**
         * Where the store lives: no-backup storage, the app's own and never copied to Google
         * by Android's backup — the store reaches another device by pairing.
         */
        fun profileDirectory(context: Context): File = File(context.noBackupFilesDir, "lumenna")
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
