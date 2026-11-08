package com.extra.han.lumenna

import android.app.Application
import android.content.Context
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.GlanceShower
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.SyncWorker
import dev.emassey0135.lumenna.app.ThisDevice
import dev.emassey0135.lumenna.thisDevice

/** Holds the one open store for the life of the process, as the phone app does. */
class BrailleSenseApplication : Application(), CoreHolder, GlanceShower {
    // The core's sentences call this "this device", said as the process is made, before
    // anything is worded.
    init {
        thisDevice = ThisDevice.DEVICE
    }

    override val core: Result<Core> by lazy { runCatching { Core(Core.profileDirectory(this), MainActivity.PLATFORM) } }

    override fun onCreate() {
        super.onCreate()
        SyncWorker.schedulePeriodic(this)
        Thread({
            core.getOrNull()?.let { Reminders.follow(this, it) }
        }, "lumenna-open").start()
    }

    /** No widget here: the launcher offers none. */
    override fun showGlances(context: Context) {}
}
