package io.github.emassey0135.lumenna.wear

import android.app.Application
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.CoreHolder
import io.github.emassey0135.lumenna.Reminders
import io.github.emassey0135.lumenna.SyncWorker
import androidx.wear.phone.interactions.notifications.BridgingConfig
import androidx.wear.phone.interactions.notifications.BridgingManager

/** Holds the one open store for the life of the process, as the phone app does. */
class WearApplication : Application(), CoreHolder {
    /** Opened on first use, so a failure to open is shown rather than crashing. */
    override val core: Result<Core> by lazy { runCatching { Core(Core.profileDirectory(this), PLATFORM) } }

    override fun onCreate() {
        super.onCreate()
        SyncWorker.schedulePeriodic(this)
        // The watch gives its own store's reminders, as the phone gives its own; the phone's
        // reminder notifications are not bridged here, or each firing came twice. Only those
        // (`Reminders.BRIDGE_TAG`): anything else the phone app posts still comes. Without this
        // app installed nothing is turned off, and the phone's reminders reach the watch.
        BridgingManager.fromContext(this).setConfig(
            BridgingConfig.Builder(this, true).addExcludedTag(Reminders.BRIDGE_TAG).build(),
        )
        core.getOrNull()?.let { Reminders.follow(this, it) }
    }
}
