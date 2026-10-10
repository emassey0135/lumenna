package io.github.emassey0135.lumenna.wear

import android.app.Application
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.CoreHolder
import io.github.emassey0135.lumenna.SyncWorker

/** Holds the one open store for the life of the process, as the phone app does. */
class WearApplication : Application(), CoreHolder {
    /** Opened on first use, so a failure to open is shown rather than crashing. */
    override val core: Result<Core> by lazy { runCatching { Core(Core.profileDirectory(this)) } }

    override fun onCreate() {
        super.onCreate()
        SyncWorker.schedulePeriodic(this)
    }
}
