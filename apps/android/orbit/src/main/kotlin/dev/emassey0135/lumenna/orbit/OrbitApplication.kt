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
    override fun ring(context: Context, due: DueReminder) {
        Foreground.open(context, OrbitActivity::class.java, mapOf(RING_KEY to due.key, RING_OCCURRENCE to due.occurrence))
    }

    companion object {
        /** The firing that brought the activity to the front, in its intent. */
        const val RING_KEY = "dev.emassey0135.lumenna.ring.key"
        const val RING_OCCURRENCE = "dev.emassey0135.lumenna.ring.occurrence"

        /** The firing [intent] was started for, if a reminder going off started it. */
        fun ringing(intent: Intent?): Pair<String, String>? {
            val key = intent?.getStringExtra(RING_KEY) ?: return null
            val occurrence = intent.getStringExtra(RING_OCCURRENCE) ?: return null
            return key to occurrence
        }
    }
}
