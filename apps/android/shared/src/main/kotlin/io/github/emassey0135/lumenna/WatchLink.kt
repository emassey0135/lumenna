package io.github.emassey0135.lumenna

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Log
import com.google.android.gms.tasks.Tasks
import com.google.android.gms.wearable.CapabilityClient
import com.google.android.gms.wearable.ChannelClient
import com.google.android.gms.wearable.Wearable
import com.google.android.gms.wearable.WearableListenerService
import io.github.emassey0135.lumenna.core.LinkStream
import io.github.emassey0135.lumenna.core.LinkSynced
import java.io.InputStream
import java.io.OutputStream
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * A Wear OS watch's own link to its phone, Google's Wearable Data Layer, used in preference to
 * Iroh between the two while they are near each other: straight over Bluetooth or the local
 * network, where Iroh would go out to the internet and often through a relay. Iroh stays for
 * them when they are apart, and for every other device.
 *
 * Only a node Google reports as nearby is used: further off, the Data Layer can carry messages
 * through Google's servers, and what syncs is the store itself. The Data Layer joins only the
 * phone and the watch paired with it, between apps with the same package and signing key, so
 * the first link pairs the two without words (`syncOverLink`), and the watch joins every
 * device's list.
 */
object WatchLink {
    /** What both apps say they offer, in `res/values/wear.xml`. */
    const val CAPABILITY = "lumenna_sync"
    const val PATH = "/lumenna/sync"
    private const val TAG = "LumennaLink"

    private val worker = Executors.newSingleThreadExecutor { Thread(it, "lumenna-link") }
    private val main = Handler(Looper.getMainLooper())

    /**
     * Syncs with the other side, phone or watch, if it is near: off the main thread, telling
     * the app when anything came across. `platform` is what this device runs: `android` or
     * `wearos`.
     */
    fun syncNearby(context: Context, core: Core, platform: String) {
        val app = context.applicationContext
        worker.execute {
            runCatching {
                val capability = Tasks.await(
                    Wearable.getCapabilityClient(app).getCapability(CAPABILITY, CapabilityClient.FILTER_REACHABLE),
                    10, TimeUnit.SECONDS,
                )
                for (node in capability.nodes.filter { it.isNearby }) {
                    val channels = Wearable.getChannelClient(app)
                    val channel = Tasks.await(channels.openChannel(node.id, PATH), 10, TimeUnit.SECONDS)
                    try {
                        run(app, core, platform, channel)
                    } finally {
                        runCatching { Tasks.await(channels.close(channel), 5, TimeUnit.SECONDS) }
                    }
                }
            }.onFailure { Log.i(TAG, "No sync over the watch's link: ${it.message}") }
        }
    }

    /** Runs the session over `channel`, on this thread, and tells the app what it did. */
    fun run(context: Context, core: Core, platform: String, channel: ChannelClient.Channel) {
        val channels = Wearable.getChannelClient(context)
        val input = Tasks.await(channels.getInputStream(channel), 10, TimeUnit.SECONDS)
        val output = Tasks.await(channels.getOutputStream(channel), 10, TimeUnit.SECONDS)
        val synced: LinkSynced = core.lumenna.syncOverLink(ChannelStreams(input, output), PairingSession.deviceName(context), platform)
        main.post {
            if (synced.changed || synced.paired) core.changed()
            // Pairing is worth saying; a sync over the link is not, any more than an Iroh round.
            if (synced.paired) core.say(sentence(synced.announcement, synced.notices))
        }
    }

    /** A channel's two streams, as the core reads and writes them; any pair of streams in a test. */
    class ChannelStreams(private val input: InputStream, private val output: OutputStream) : LinkStream {
        override fun read(max: UInt): ByteArray {
            val buffer = ByteArray(max.toInt())
            val read = runCatching { input.read(buffer) }.getOrDefault(-1)
            return if (read <= 0) ByteArray(0) else buffer.copyOf(read)
        }

        override fun write(bytes: ByteArray): Boolean = runCatching {
            output.write(bytes)
            output.flush()
        }.isSuccess

        override fun finish() {
            runCatching { output.close() }
        }
    }
}

/**
 * Answers the other side's link when it opens one, so neither app needs to be in front: the
 * system starts this for a channel on [WatchLink.PATH]. Each app's manifest declares it.
 */
abstract class LinkService : WearableListenerService() {
    /** What this device runs: `android` or `wearos`. */
    abstract val platform: String

    override fun onChannelOpened(channel: ChannelClient.Channel) {
        if (channel.path != WatchLink.PATH) return
        val core = (application as CoreHolder).core.getOrNull() ?: return
        // A listener's callbacks run on a thread of its own, which the session may hold.
        runCatching { WatchLink.run(this, core, platform, channel) }
            .onFailure { Log.i("LumennaLink", "The watch's link stopped: ${it.message}") }
    }
}
