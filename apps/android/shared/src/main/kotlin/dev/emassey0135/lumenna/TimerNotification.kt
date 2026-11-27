package dev.emassey0135.lumenna

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import dev.emassey0135.lumenna.app.GlanceTimer
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.app.TimerControl
import dev.emassey0135.lumenna.app.glanceCountdown
import dev.emassey0135.lumenna.app.timerControlLine
import java.time.Instant
import java.util.concurrent.Executors

/**
 * What a running timer's ongoing notification shows, from the core's control
 * (`TimerControl.timer`): the task, how it counts, and the core's words beside it.
 */
data class TimerShown(
    /** The sitting whose timer runs, which Pause and Stop act on. */
    val sitting: String,
    /** The task it runs on: the notification's title. */
    val task: String,
    /** What it says beneath the task: the core's countdown ("12 minutes left"), else the block. */
    val text: String,
    /** The block, beside a countdown; none where [text] is the block already. */
    val block: String?,
    /** Whether it counts down to [at], where the planned length runs out; else up from it. */
    val countsDown: Boolean,
    /** The instant it counts down to, or up from (what was logged before counted in), epoch milliseconds. */
    val at: Long,
    /** The whole of it in one line, the core's (`timer_control_line`): what TalkBack is told. */
    val description: String,
)

/**
 * What adds to a running timer's notification beside it, an Application being one: the watch's
 * Ongoing Activity, on its watch face and in its launcher's recents.
 */
interface OngoingTimer {
    /** Adds to [notification], about to be posted as [id], what shows [shown]; [open] is its tap. */
    fun ongoing(context: Context, notification: NotificationCompat.Builder, shown: TimerShown, id: Int, open: PendingIntent?)
}

/**
 * A running timer, outside the app: an ongoing notification, silent, on a channel of its own, for
 * as long as the core says a timer runs (`Device.timerControl`'s `timer`). It counts by itself,
 * down to where the sitting's planned length runs out where it has one, else up from when it
 * counts from, and has the core's Pause timer and Stop timer, which act on its sitting through the
 * device. Tapping it opens Today. Posted, redrawn and taken away wherever the glances are drawn
 * again (each app's `showGlances`: the store changing, the Quick Settings tile pressed, and the
 * glance's next minute, so the countdown's words stay true). Nothing here decides what it shows.
 */
object TimerNotification {
    /** The channel, the person's to set as they like in the system's settings. */
    const val CHANNEL = "timer"

    /** Its one number; reminders' are [Reminders]' own. */
    const val ID = 2

    internal const val PAUSE = "dev.emassey0135.lumenna.timer.PAUSE"
    internal const val STOP = "dev.emassey0135.lumenna.timer.STOP"
    internal const val SITTING = "sitting"

    /** Reading the store and posting: never the main thread, one at a time, in order. */
    private val work = Executors.newSingleThreadExecutor { Thread(it, "lumenna-timer") }

    /**
     * What [control] shows at [now], or none while no timer runs: the core's countdown
     * ([countdown], `glance_countdown`) beneath the task with the block beside it, else the block;
     * counting down to `runsOutAt` where the core gives it, else up from `countingFrom`; and the
     * core's line for it whole ([line], `timer_control_line`), in sentence case.
     */
    fun shown(
        control: TimerControl,
        now: Instant,
        countdown: (GlanceTimer, String) -> String? = ::glanceCountdown,
        line: (TimerControl, String, Boolean) -> String = ::timerControlLine,
    ): TimerShown? {
        val timer = control.timer?.takeIf { control.running } ?: return null
        val runsOut = timer.runsOutAt?.let(Glances::instant)
        val left = countdown(timer, now.toString())
        return TimerShown(
            sitting = timer.sitting,
            task = timer.task,
            text = left ?: timer.block,
            block = timer.block.takeIf { left != null && it.isNotEmpty() },
            countsDown = runsOut != null,
            at = (runsOut ?: Glances.instant(timer.countingFrom) ?: now).toEpochMilli(),
            description = line(control, now.toString(), true),
        )
    }

    /** Posts, redraws or takes away the notification as the store has the timer now. From any thread. */
    fun show(context: Context) {
        val app = context.applicationContext
        val core = (app as? CoreHolder)?.core?.getOrNull() ?: return
        work.execute {
            val control = try {
                core.device.timerControl()
            } catch (_: LumennaError) {
                // The store could not say; the next change tries again.
                return@execute
            }
            post(app, shown(control, Instant.now()))
        }
    }

    private fun post(context: Context, shown: TimerShown?) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        if (shown == null) return manager.cancel(ID)
        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) return
        channel(context)
        val open = open(context, shown)
        val builder = notification(context, shown, open)
        (context.applicationContext as? OngoingTimer)?.ongoing(context, builder, shown, ID, open)
        try {
            manager.notify(ID, builder.build())
        } catch (_: SecurityException) {
            // Notifications were refused after the check.
        }
    }

    /** Today, as a glance's tap opens it. The sitting in its data, so it is a pending intent of its own. */
    private fun open(context: Context, shown: TimerShown): PendingIntent? {
        val intent = context.packageManager.getLaunchIntentForPackage(context.packageName)?.apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            data = Uri.fromParts("lumenna-timer", shown.sitting, null)
            putExtra(Launching.ID, Launching.TODAY)
        } ?: return null
        return PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    /**
     * The notification: the task, the core's countdown or the block, a chronometer counting as
     * [shown] counts, and the core's Pause timer and Stop timer. Ongoing, silent and alerting only
     * once, so a minute's redraw never sounds; a stopwatch's category. Behind a secure lock it shows
     * only that the timer runs, as the Quick Settings tile does: what a task is called is behind it.
     */
    fun notification(context: Context, shown: TimerShown, open: PendingIntent?): NotificationCompat.Builder {
        val words = appWords
        fun pressed(action: String) = PendingIntent.getBroadcast(
            context, 0,
            Intent(context, ReminderReceiver::class.java).setAction(action)
                .setData(Uri.fromParts("lumenna-timer", shown.sitting, null))
                .putExtra(SITTING, shown.sitting),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        fun NotificationCompat.Builder.counting() = this
            .setSmallIcon(dev.emassey0135.lumenna.shared.R.drawable.ic_reminder)
            .setCategory(NotificationCompat.CATEGORY_STOPWATCH)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setOngoing(true)
            .setSilent(true)
            .setOnlyAlertOnce(true)
            .setUsesChronometer(true)
            .setChronometerCountDown(shown.countsDown)
            .setWhen(shown.at)
            .setShowWhen(true)
            // The watch shows its own store's timer (`OngoingTimer`), so the phone's is not bridged there.
            .extend(NotificationCompat.WearableExtender().setBridgeTag(Reminders.BRIDGE_TAG))
        val locked = NotificationCompat.Builder(context, CHANNEL).counting().setContentTitle(words.timerChannel).build()
        return NotificationCompat.Builder(context, CHANNEL)
            .counting()
            .setContentTitle(shown.task)
            .setContentText(shown.text)
            .apply { shown.block?.let { setSubText(it) } }
            .setTicker(shown.description)
            .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
            .setPublicVersion(locked)
            .apply { open?.let { setContentIntent(it) } }
            .addAction(0, words.pauseTimer, pressed(PAUSE))
            .addAction(0, words.stopTimer, pressed(STOP))
    }

    /**
     * Its Pause timer or Stop timer pressed ([action]), on reminders' thread: the sitting's timer
     * paused or stopped through the device, said as the app says a change, then reminders armed
     * again (its time running out moves with it) and the notification redrawn or taken away.
     */
    internal fun pressed(context: Context, core: Core, action: String, sitting: String) {
        try {
            val timer = if (action == PAUSE) core.device.pauseTimer(sitting) else core.device.stopTimer(sitting, null)
            core.say(sentence(timer.announcement, timer.notices))
        } catch (error: LumennaError) {
            core.say(error.sentence)
        }
        // What follows the store (the glances, this notification among them) draws again as it moves;
        // the notification is drawn now too, so the press is answered at once.
        core.changed()
        Reminders.arm(context, core)
        show(context)
    }

    /** Its channel, named and described by the core; low importance, so it never sounds or peeks. */
    fun channel(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        if (manager.getNotificationChannel(CHANNEL) != null) return
        val words = appWords
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, words.timerChannel, NotificationManager.IMPORTANCE_LOW).apply {
                description = words.timerChannelDescription
                setShowBadge(false)
            },
        )
    }
}
