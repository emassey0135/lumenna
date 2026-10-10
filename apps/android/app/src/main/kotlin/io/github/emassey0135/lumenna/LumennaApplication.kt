package io.github.emassey0135.lumenna

import android.app.Application

/** Holds the one open store for the life of the process. */
class LumennaApplication : Application(), CoreHolder {
    /** Opened on first use, so a failure to open is shown in the activity rather than crashing. */
    override val core: Result<Core> by lazy { runCatching { Core(Core.profileDirectory(this)) } }

    override fun onCreate() {
        super.onCreate()
        SyncWorker.schedulePeriodic(this)
    }
}
