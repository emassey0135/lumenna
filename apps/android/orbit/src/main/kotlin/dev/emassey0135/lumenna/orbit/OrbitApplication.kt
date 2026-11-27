package dev.emassey0135.lumenna.orbit

import android.app.Application
import android.content.Context
import android.content.Intent
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.GlanceShower
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.Ringer
import dev.emassey0135.lumenna.SyncWorker
import dev.emassey0135.lumenna.app.DueReminder
import dev.emassey0135.orbit.Foreground

/** Holds the one open store for the life of the process, and keeps reminders and sync going. */
class OrbitApplication : Application(), CoreHolder, GlanceShower, Ringer {
    override val core: Result<Core> by lazy { runCatching { Core(Core.profileDirectory(this)) } }

    override fun onCreate() {
        super.onCreate()
        SyncWorker.schedulePeriodic(this)
        Thread({
            core.getOrNull()?.let { Reminders.follow(this, it) }
        }, "lumenna-open").start()
    }

    // No home-screen widgets on these devices.
    override fun showGlances(context: Context) {}

    /**
     * A reminder going off brings the app to the front, as Orbit's own Alarm Clock rings, where it
     * is said as a dialog with its buttons. Its notification, already posted, gives the sound and
     * vibration once and stays in Satellite's shade; root refused, it is all there is.
     */
    override fun ring(context: Context, due: DueReminder, since: String, now: String) {
        Foreground.open(
            context, OrbitActivity::class.java,
            mapOf(RING_KEY to due.key, RING_OCCURRENCE to due.occurrence, RING_SINCE to since, RING_NOW to now),
        )
    }

    /** A reminder that went off, as the activity is started for it: its firing, and the moments it was found between. */
    data class Ringing(val key: String, val occurrence: String, val since: String, val now: String)

    companion object {
        /** The firing that brought the activity to the front, in its intent. */
        const val RING_KEY = "dev.emassey0135.lumenna.ring.key"
        const val RING_OCCURRENCE = "dev.emassey0135.lumenna.ring.occurrence"
        const val RING_SINCE = "dev.emassey0135.lumenna.ring.since"
        const val RING_NOW = "dev.emassey0135.lumenna.ring.now"

        /** The firing [intent] was started for, if a reminder going off started it. */
        fun ringing(intent: Intent?): Ringing? {
            intent ?: return null
            return Ringing(
                intent.getStringExtra(RING_KEY) ?: return null,
                intent.getStringExtra(RING_OCCURRENCE) ?: return null,
                intent.getStringExtra(RING_SINCE) ?: return null,
                intent.getStringExtra(RING_NOW) ?: return null,
            )
        }

        /** Takes the ring out of [intent] once heard, so the activity made again keeps no ringing of the past. */
        fun heard(intent: Intent?) {
            listOf(RING_KEY, RING_OCCURRENCE, RING_SINCE, RING_NOW).forEach { intent?.removeExtra(it) }
        }
    }
}
