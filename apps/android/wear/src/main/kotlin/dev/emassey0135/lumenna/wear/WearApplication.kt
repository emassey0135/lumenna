package dev.emassey0135.lumenna.wear

import android.app.Application
import android.content.Context
import dev.emassey0135.lumenna.GlanceShower
import dev.emassey0135.lumenna.Glances
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.SyncWorker
import androidx.wear.phone.interactions.notifications.BridgingConfig
import androidx.wear.phone.interactions.notifications.BridgingManager

/** Holds the one open store for the life of the process, as the phone app does. */
class WearApplication : Application(), CoreHolder, GlanceShower {
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
        // The store opens off the main thread, as the phone's does: a cold start for an alarm,
        // the tile or a complication has nothing else to do on it.
        Thread({
            core.getOrNull()?.let {
                Reminders.follow(this, it)
                // The tile and the complications are drawn again when the store changes and at
                // each moment their glance names.
                Glances.follow(this, it)
            }
        }, "lumenna-open").start()
    }

    /** The glance is shown on the tile and the watch faces' complications. */
    override fun showGlances(context: Context) {
        LumennaTile.refresh(context)
        LumennaComplication.refresh(context)
    }
}
