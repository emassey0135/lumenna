package dev.emassey0135.lumenna.orbit

import android.app.Application
import android.content.Context
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.GlanceShower
import dev.emassey0135.lumenna.Glances
import dev.emassey0135.lumenna.TimerNotification
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.Ringer
import dev.emassey0135.lumenna.Ringing
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
            core.getOrNull()?.let {
                Reminders.follow(this, it)
                // What follows the glance's moments: here only a running timer's notification.
                Glances.follow(this, it)
            }
        }, "lumenna-open").start()
    }

    /**
     * No home-screen widgets on these devices: only a running timer's ongoing notification, in
     * Satellite's shade with the reminders. How much is left is said on request too (Time Left).
     */
    override fun showGlances(context: Context) = TimerNotification.show(context)

    /**
     * A reminder going off brings the app to the front, as Orbit's own Alarm Clock rings, where it
     * is said as a dialog with its buttons. Its notification, already posted, gives the sound and
     * vibration once and stays in Satellite's shade; root refused, it is all there is.
     */
    override fun ring(context: Context, due: DueReminder, since: String, now: String) {
        Foreground.open(context, OrbitActivity::class.java, Ringing(due.key, due.occurrence, since, now).extras())
    }
}
