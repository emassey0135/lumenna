package dev.emassey0135.lumenna.orbit

import android.app.Application
import android.content.Context
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.GlanceShower
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.SyncWorker

/** Holds the one open store for the life of the process, and keeps reminders and sync going. */
class OrbitApplication : Application(), CoreHolder, GlanceShower {
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
}
