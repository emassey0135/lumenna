package dev.emassey0135.lumenna.wear

import android.app.Application
import android.app.PendingIntent
import android.content.Context
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import androidx.wear.ongoing.OngoingActivity
import androidx.wear.ongoing.Status
import dev.emassey0135.lumenna.GlanceShower
import dev.emassey0135.lumenna.OngoingTimer
import dev.emassey0135.lumenna.TimerNotification
import dev.emassey0135.lumenna.TimerShown
import dev.emassey0135.lumenna.Glances
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.SyncWorker
import dev.emassey0135.lumenna.app.ThisDevice
import dev.emassey0135.lumenna.thisDevice
import androidx.wear.phone.interactions.notifications.BridgingConfig
import androidx.wear.phone.interactions.notifications.BridgingManager

/** Holds the one open store for the life of the process, as the phone app does. */
class WearApplication : Application(), CoreHolder, GlanceShower, OngoingTimer {
    // The core's sentences call this device "this watch", which every word is asked for as: said
    // as the process is made, before an alarm, the tile or an activity can word anything.
    init {
        thisDevice = ThisDevice.WATCH
    }

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

    /**
     * The glance is shown on the tile and the watch faces' complications; a running timer as an
     * ongoing activity too, drawn again with them.
     */
    override fun showGlances(context: Context) {
        LumennaTile.refresh(context)
        LumennaComplication.refresh(context)
        TimerNotification.show(context)
    }

    /**
     * A running timer as Wear OS shows what goes on: an Ongoing Activity over its notification, on
     * the watch face and in the launcher's recents, counting as the notification counts (down to
     * where the planned length runs out, else up), titled by the task and said whole by the core's
     * line. The watch counts in its elapsed-realtime clock, so the instant is moved onto it.
     */
    override fun ongoing(context: Context, notification: NotificationCompat.Builder, shown: TimerShown, id: Int, open: PendingIntent?) {
        open ?: return
        val elapsed = shown.at - System.currentTimeMillis() + SystemClock.elapsedRealtime()
        val time = if (shown.countsDown) Status.TimerPart(elapsed) else Status.StopwatchPart(elapsed)
        OngoingActivity.Builder(context, id, notification)
            .setStaticIcon(dev.emassey0135.lumenna.shared.R.drawable.ic_reminder)
            .setTouchIntent(open)
            .setTitle(shown.task)
            .setContentDescription(shown.description)
            .setCategory(NotificationCompat.CATEGORY_STOPWATCH)
            .setStatus(Status.Builder().addTemplate("#time#").addPart("time", time).build())
            .build()
            .apply(context)
    }
}
