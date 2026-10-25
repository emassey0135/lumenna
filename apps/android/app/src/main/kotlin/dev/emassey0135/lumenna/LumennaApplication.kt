package dev.emassey0135.lumenna

import android.app.Application
import android.content.Context
import dev.emassey0135.lumenna.widgets.Widgets

/** Holds the one open store for the life of the process. */
class LumennaApplication : Application(), CoreHolder, GlanceShower {
    /** Opened on first use, so a failure to open is shown in the activity rather than crashing. */
    override val core: Result<Core> by lazy { runCatching { Core(Core.profileDirectory(this)) } }

    override fun onCreate() {
        super.onCreate()
        SyncWorker.schedulePeriodic(this)
        // The icon's shortcuts are the core's, given to the launcher as the process starts.
        Launches.publish(this)
        core.getOrNull()?.let {
            // Reminders are armed now and after every change to the store (`Reminders.follow`),
            // and the widgets drawn again then and at each moment their glance names.
            Reminders.follow(this, it)
            Glances.follow(this, it)
            // The system's search index too (`SearchIndex`).
            SearchIndex.follow(this, it)
        }
    }

    /** The glance is shown on the home screen's widgets. */
    override fun showGlances(context: Context) = Widgets.refresh(context)
}
