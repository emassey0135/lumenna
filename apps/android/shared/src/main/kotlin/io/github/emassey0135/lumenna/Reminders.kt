package io.github.emassey0135.lumenna

import android.Manifest
import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import io.github.emassey0135.lumenna.records.DueReminder
import io.github.emassey0135.lumenna.core.Lumenna
import io.github.emassey0135.lumenna.records.LumennaException
import java.time.Duration
import java.time.Instant
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.launch

// Reminders on the phone and the watch. When each fires, what it says and the buttons' words
// are the core's (`upcomingReminders`, `DueReminder`); Android schedules them itself, so this
// hands AlarmManager the firings of the next two days and re-arms whenever they may have
// moved: on boot, on the app's start and return to the front, after the store changes, and
// after a background sync.

/** A firing's notification as shown: its tag, `<key>@<occurrence>`, and the store that showed it. */
data class ShownNotification(val tag: String, val store: String?)

/** One firing as an alarm: which reminder and occurrence (what a dismissal names), and when. */
data class ReminderAlarm(val key: String, val occurrence: String, val at: Instant) {
    /** The alarm's and its notification's identity: re-arming the same firing replaces it. */
    val id: String get() = "$key@$occurrence"
}

/** What sets alarms: Android's AlarmManager, or a test's record of what it was asked. */
interface Alarms {
    /** Whether an alarm may go off at its exact time; refused, Android brings it when it allows. */
    val exact: Boolean

    /** The alarms set last time, so what is no longer listed is cancelled. */
    var armed: Set<String>

    fun set(alarm: ReminderAlarm, exact: Boolean)

    fun cancel(id: String)

    /** One more arming at [at], so the window moves on even if nothing else happens. */
    fun rearmAt(at: Instant)
}

/**
 * Which alarms a store asks for, set through [alarms]: the seam the tests check, given a store.
 * Each firing of the next [WINDOW] is set — exactly where Android allows it — and whatever was
 * set before and is no longer listed (dismissed, done, deleted, reminders turned off) is
 * cancelled.
 */
class ReminderSchedule(private val lumenna: Lumenna, private val alarms: Alarms) {
    fun arm(now: Instant = Instant.now()): List<ReminderAlarm> {
        val upcoming = lumenna.upcomingReminders(now.plus(WINDOW).toString()).due
        val wanted = upcoming.take(MOST).mapNotNull { due ->
            runCatching { ReminderAlarm(due.key, due.occurrence, Instant.parse(due.at)) }.getOrNull()
        }
        val ids = wanted.map { it.id }.toSet()
        (alarms.armed - ids).forEach(alarms::cancel)
        val exact = alarms.exact
        wanted.forEach { alarms.set(it, exact) }
        alarms.armed = ids
        alarms.rearmAt(now.plus(REARM))
        return wanted
    }

    companion object {
        /** How far ahead alarms are set; every arming moves it on. */
        val WINDOW: Duration = Duration.ofHours(48)

        /** Android keeps at most 500 alarms an app; a day's reminders are far fewer. */
        const val MOST = 100

        /** When to arm again if nothing else has: before the window runs out. */
        val REARM: Duration = Duration.ofHours(24)
    }
}

/** The firings this app has shown since the core was last told, each `<key>@<occurrence>`. */
interface ShownReminders {
    fun all(): Set<String>

    /** Forgets [told], once the core has heard of them; any shown since stay. */
    fun forget(told: Set<String>)
}

/**
 * What was missed while nothing could show it — the device off, so its alarms gone — asked of
 * the core just before each arming (`remindersMissed`). Android's alarms do not survive a
 * restart, so the core is told what this app did show, and the rest since it last asked was
 * missed. What was told is forgotten only once the core has taken it.
 */
class ReminderCatchUp(private val lumenna: Lumenna, private val shown: ShownReminders) {
    fun catchUp(): io.github.emassey0135.lumenna.records.DueReminders {
        val told = shown.all()
        val found = lumenna.remindersMissed(told.toList(), false)
        shown.forget(told)
        return found
    }
}

/** What was shown, in the app's preferences. */
class SystemShown(context: Context) : ShownReminders {
    private val preferences = context.getSharedPreferences(Reminders.PREFERENCES, Context.MODE_PRIVATE)

    override fun all(): Set<String> = synchronized(Reminders) { preferences.getStringSet("shown", emptySet()).orEmpty().toSet() }

    override fun forget(told: Set<String>) = synchronized(Reminders) {
        preferences.edit().putStringSet("shown", all() - told).apply()
    }

    fun record(id: String) = synchronized(Reminders) {
        preferences.edit().putStringSet("shown", all() + id).apply()
    }
}

/** AlarmManager, remembering what it set in the app's preferences. */
class SystemAlarms(private val context: Context) : Alarms {
    private val manager = context.getSystemService(AlarmManager::class.java)
    private val preferences = context.getSharedPreferences(Reminders.PREFERENCES, Context.MODE_PRIVATE)

    override val exact: Boolean get() = Reminders.exactAllowed(context)

    override var armed: Set<String>
        get() = preferences.getStringSet("armed", emptySet()).orEmpty()
        set(value) = preferences.edit().putStringSet("armed", value).apply()

    override fun set(alarm: ReminderAlarm, exact: Boolean) {
        val pending = Reminders.firing(context, alarm, PendingIntent.FLAG_UPDATE_CURRENT) ?: return
        val millis = alarm.at.toEpochMilli()
        // Allowed while idle, so Doze does not hold a reminder back until the next window.
        if (exact) {
            try {
                manager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, millis, pending)
                return
            } catch (_: SecurityException) {
                // Taken back between the check and the call: inexact, below.
            }
        }
        manager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, millis, pending)
    }

    override fun cancel(id: String) {
        Reminders.firing(context, id, PendingIntent.FLAG_NO_CREATE)?.let(manager::cancel)
    }

    override fun rearmAt(at: Instant) {
        val intent = Intent(context, ReminderReceiver::class.java).setAction(Reminders.REARM)
        val pending = PendingIntent.getBroadcast(context, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        manager.set(AlarmManager.RTC, at.toEpochMilli(), pending)
    }
}

/**
 * What Android allows reminders on this device: notifications, alarms at an exact time, and
 * the system's page that grants those, if it has one. A screen reads it through
 * [LocalReminderAccess], so a test can show the page as a refusal leaves it: revoking exact
 * alarms kills the app, tests and all.
 */
data class ReminderAccess(val notifications: Boolean, val exact: Boolean, val grantExact: Intent?)

/** How a screen learns what Android allows: the system's answer, unless a test says otherwise. */
val LocalReminderAccess = staticCompositionLocalOf<(Context) -> ReminderAccess> { Reminders::access }

/** What a reminder's notification opens: a task, or a block's day. */
data class ReminderOpened(val kind: String, val target: String, val occurrence: String)

object Reminders {
    /** The notification channel, which the person can change in the system's settings. */
    const val CHANNEL = "reminders"

    /**
     * What a reminder notification is tagged for Wear OS's bridging: the watch app turns
     * bridging off for this tag (`BridgingManager`), since it gives its own store's reminders.
     */
    const val BRIDGE_TAG = "lumenna-reminder"

    const val PREFERENCES = "reminders"

    internal const val FIRE = "io.github.emassey0135.lumenna.reminder.FIRE"
    internal const val DISMISS = "io.github.emassey0135.lumenna.reminder.DISMISS"
    internal const val SNOOZE = "io.github.emassey0135.lumenna.reminder.SNOOZE"
    internal const val REARM = "io.github.emassey0135.lumenna.reminder.REARM"
    internal const val CARRY = "io.github.emassey0135.lumenna.reminder.CARRY"
    private const val KEY = "key"
    private const val TARGET = "target"
    private const val OCCURRENCE = "occurrence"
    private const val AT = "at"
    private const val OPEN_KIND = "io.github.emassey0135.lumenna.open.kind"
    private const val OPEN_TARGET = "io.github.emassey0135.lumenna.open.target"
    private const val OPEN_OCCURRENCE = "io.github.emassey0135.lumenna.open.occurrence"
    /** The notification to take away once the app has opened for it: a button's, which does not cancel itself. */
    private const val OPEN_CANCEL = "io.github.emassey0135.lumenna.open.cancel"

    /** What a notification's Replan button opens: Replan from Now, for [ReminderOpened.kind]. */
    const val REPLAN = "replan"

    /** Which store showed a firing's notification, its directory, in the notification's extras. */
    private const val STORE = "io.github.emassey0135.lumenna.store"

    /** The missed summary's tag. */
    private const val MISSED = "missed"

    /** The tag of what carrying forward from a notification said. No `@`: it is no firing. */
    private const val CARRIED = "carried"

    /** Notifications are posted under their firing's identity, with this one number. */
    private const val NOTIFICATION = 1

    /** Arms the alarms from the store; one arming at a time, from any thread. */
    @Synchronized
    fun arm(context: Context, core: Core) {
        channel(context)
        try {
            // First what was missed — after a restart, everything since the last look but what
            // was shown — then the alarms, which cancel any for a firing now past.
            catchUp(context, core)
            ReminderSchedule(core.lumenna, SystemAlarms(context)).arm()
            clearDealtWith(context, core)
        } catch (_: LumennaException) {
            // The store could not say; the next change or start tries again.
        }
    }

    /**
     * Re-arms after every change to the store, a moment after the last of a run of them: a
     * local edit, a sync bringing something in, the app coming back to the front (which says
     * the store changed so every screen reads it again). The first arming is now.
     */
    @OptIn(FlowPreview::class)
    fun follow(context: Context, core: Core) {
        val app = context.applicationContext
        CoroutineScope(SupervisorJob() + Dispatchers.Main).launch {
            core.changes.debounce(1_000).collect { arm(app, core) }
        }
    }

    /**
     * Takes away what was shown and has since been dealt with: dismissed or snoozed on another
     * device, or a task done. Best effort, as the core's acknowledgements are.
     */
    private fun clearDealtWith(context: Context, core: Core) {
        val manager = context.getSystemService(NotificationManager::class.java)
        val shown = manager.activeNotifications.filter { it.notification.channelId == CHANNEL && it.id == NOTIFICATION }
            .map { ShownNotification(it.tag.orEmpty(), it.notification.extras.getString(STORE)) }
        dealtWith(core, shown).forEach { manager.cancel(it, NOTIFICATION) }
    }

    /**
     * Of the firings [shown], the tags of those this store showed and has since dealt with.
     * Only this store's: another store in the process — a test's, beside the app's own — has
     * firings this one knows nothing of, and taking those away raced that store's showing
     * them. One shown before notifications named their store counts as this one's.
     */
    fun dealtWith(core: Core, shown: List<ShownNotification>): List<String> {
        val ours = shown.filter { it.tag.contains('@') && (it.store == null || it.store == core.directory.path) }
        if (ours.isEmpty()) return emptyList()
        val since = Instant.now().minus(Duration.ofDays(1)).toString()
        val still = core.lumenna.remindersDue(since, null).let { it.due + it.missed }.map { "${it.key}@${it.occurrence}" }.toSet()
        return ours.map { it.tag }.filter { it !in still }
    }

    /**
     * An alarm went off: shows its firing, as the core words it now, unless it has been dealt
     * with since it was set. One delayed past the core's few minutes is said of the past.
     */
    fun fire(context: Context, core: Core, key: String, occurrence: String, at: String): DueReminder? {
        val moment = runCatching { Instant.parse(at) }.getOrNull() ?: return null
        val now = maxOf(Instant.now(), moment)
        val found = core.attempt { core.lumenna.remindersDue(moment.minusSeconds(1).toString(), now.toString()) } ?: return null
        val match = { due: DueReminder -> due.key == key && due.occurrence == occurrence }
        val shown = found.due.firstOrNull(match)?.also { show(context, core, it, it.text) }
            ?: found.missed.firstOrNull(match)?.also { show(context, core, it, it.missed) }
        // Told to the core at the next catching up, so it is not counted as missed.
        if (shown != null) SystemShown(context).record("$key@$occurrence")
        return shown
    }

    /**
     * Shows what the core says was missed: each recent one as its own reminder, with its
     * buttons, and the older ones as one summary.
     */
    private fun catchUp(context: Context, core: Core) {
        val found = ReminderCatchUp(core.lumenna, SystemShown(context)).catchUp()
        found.due.forEach { show(context, core, it, it.text) }
        if (found.missedSummary.isNotEmpty() && notificationsAllowed(context)) {
            try {
                context.getSystemService(NotificationManager::class.java).notify(MISSED, NOTIFICATION, summary(context, found.missedSummary))
            } catch (_: SecurityException) {
                // Notifications were refused after the check.
            }
        }
    }

    /** The one notification for what was missed long enough ago to be said together. */
    fun summary(context: Context, text: String): Notification {
        val open = context.packageManager.getLaunchIntentForPackage(context.packageName)
        return NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(io.github.emassey0135.lumenna.shared.R.drawable.ic_reminder)
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setTicker(text)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .setAutoCancel(true)
            .extend(NotificationCompat.WearableExtender().setBridgeTag(BRIDGE_TAG))
            .apply { open?.let { setContentIntent(PendingIntent.getActivity(context, 0, it, PendingIntent.FLAG_IMMUTABLE)) } }
            .build()
    }

    /** The notification's Dismiss button: on every device, as soon as they hear of it. */
    fun dismiss(context: Context, core: Core, key: String, occurrence: String) {
        context.getSystemService(NotificationManager::class.java).cancel("$key@$occurrence", NOTIFICATION)
        core.attempt { core.lumenna.dismissReminder(key, occurrence) }
        core.changed()
        arm(context, core)
    }

    /** The notification's Snooze button: back in the core's ten minutes, on every device. */
    fun snooze(context: Context, core: Core, key: String, occurrence: String) {
        context.getSystemService(NotificationManager::class.java).cancel("$key@$occurrence", NOTIFICATION)
        core.attempt { core.lumenna.snoozeReminder(key, occurrence, null) }
        core.changed()
        arm(context, core)
    }

    /**
     * The notification's Carry Forward button, at a block's end with work left short: carries
     * the rest of each sitting to the next block that takes its task, and says what it did in
     * the notification's place, since the app may not be open to say it.
     */
    fun carry(context: Context, core: Core, key: String, target: String, occurrence: String) {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.cancel("$key@$occurrence", NOTIFICATION)
        val said = try {
            val change = core.lumenna.carryRestForward("$target@$occurrence")
            core.changed()
            core.say(change)
            sentence(change.announcement, change.notices)
        } catch (error: LumennaException) {
            core.say(error.sentence)
            error.sentence
        }
        if (notificationsAllowed(context)) {
            try {
                manager.notify(CARRIED, NOTIFICATION, told(context, said))
            } catch (_: SecurityException) {
                // Notifications were refused after the check.
            }
        }
        arm(context, core)
    }

    /** What a notification's button did, said quietly where the reminder was, and gone in a while. */
    fun told(context: Context, text: String): Notification =
        NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(io.github.emassey0135.lumenna.shared.R.drawable.ic_reminder)
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setTicker(text)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .setSilent(true)
            .setAutoCancel(true)
            .setTimeoutAfter(60_000)
            .extend(NotificationCompat.WearableExtender().setBridgeTag(BRIDGE_TAG))
            .build()

    private fun show(context: Context, core: Core, due: DueReminder, text: String) {
        if (!notificationsAllowed(context)) return
        channel(context)
        try {
            context.getSystemService(NotificationManager::class.java).notify("${due.key}@${due.occurrence}", NOTIFICATION, notification(context, due, text, core.directory.path))
        } catch (_: SecurityException) {
            // Notifications were refused after the check.
        }
    }

    /**
     * A firing's notification: the core's title, its whole message, and its Dismiss and Snooze
     * buttons, and Carry Forward where the core offers it, in sentence case as Android writes. A reminder's category, so Do Not Disturb lets it through only where the person
     * allows reminders; nothing in it is said by sound alone.
     */
    fun notification(context: Context, due: DueReminder, text: String = due.text, store: String? = null): Notification {
        val id = "${due.key}@${due.occurrence}"
        fun pressed(action: String) = PendingIntent.getBroadcast(
            context, 0,
            Intent(context, ReminderReceiver::class.java).setAction(action).setData(uri(id))
                .putExtra(KEY, due.key).putExtra(OCCURRENCE, due.occurrence).putExtra(TARGET, due.target),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val open = context.packageManager.getLaunchIntentForPackage(context.packageName)?.apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            putExtra(OPEN_KIND, due.kind)
            putExtra(OPEN_TARGET, due.target)
            putExtra(OPEN_OCCURRENCE, due.occurrence)
        }
        return NotificationCompat.Builder(context, CHANNEL)
            .apply { store?.let { addExtras(android.os.Bundle().apply { putString(STORE, it) }) } }
            .setSmallIcon(io.github.emassey0135.lumenna.shared.R.drawable.ic_reminder)
            .setContentTitle(due.title)
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setTicker(text)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setWhen(runCatching { Instant.parse(due.at).toEpochMilli() }.getOrDefault(System.currentTimeMillis()))
            .setShowWhen(true)
            .setAutoCancel(true)
            .extend(NotificationCompat.WearableExtender().setBridgeTag(BRIDGE_TAG))
            .apply {
                open?.let {
                    setContentIntent(PendingIntent.getActivity(context, id.hashCode(), it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
                }
            }
            .addAction(0, button(due.dismiss), pressed(DISMISS))
            .addAction(0, button(due.snooze), pressed(SNOOZE))
            // At a block's end with work left short, the text ends in the question this answers.
            .apply { if (due.carry.isNotEmpty()) addAction(0, button(due.carry), pressed(CARRY)) }
            // A block's work outrunning its time: Replan opens the app at Replan from Now.
            .apply {
                if (due.replan.isNotEmpty()) {
                    val replan = context.packageManager.getLaunchIntentForPackage(context.packageName)?.apply {
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
                        putExtra(OPEN_KIND, REPLAN)
                        putExtra(OPEN_TARGET, due.target)
                        putExtra(OPEN_OCCURRENCE, due.occurrence)
                        putExtra(OPEN_CANCEL, id)
                    }
                    replan?.let {
                        addAction(0, button(due.replan), PendingIntent.getActivity(context, "replan $id".hashCode(), it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
                    }
                }
            }
            .build()
    }

    /**
     * What a notification opened the app to, if one did; a button that opened it (Replan) has
     * its notification taken away, as tapping the notification itself does.
     */
    fun opened(context: Context, intent: Intent?): ReminderOpened? {
        intent?.getStringExtra(OPEN_CANCEL)?.let { context.getSystemService(NotificationManager::class.java).cancel(it, NOTIFICATION) }
        return opened(intent)
    }

    /** What a notification opened the app to, if one did. */
    fun opened(intent: Intent?): ReminderOpened? {
        val kind = intent?.getStringExtra(OPEN_KIND) ?: return null
        val target = intent.getStringExtra(OPEN_TARGET) ?: return null
        return ReminderOpened(kind, target, intent.getStringExtra(OPEN_OCCURRENCE).orEmpty())
    }

    private fun uri(id: String): Uri = Uri.fromParts("lumenna-reminder", id, null)

    internal fun firing(context: Context, alarm: ReminderAlarm, flags: Int): PendingIntent? =
        PendingIntent.getBroadcast(
            context, 0,
            Intent(context, ReminderReceiver::class.java).setAction(FIRE).setData(uri(alarm.id))
                .putExtra(KEY, alarm.key).putExtra(OCCURRENCE, alarm.occurrence).putExtra(AT, alarm.at.toString()),
            PendingIntent.FLAG_IMMUTABLE or flags,
        )

    /** The alarm set for [id], to cancel: an intent matches by its action and data, not its extras. */
    internal fun firing(context: Context, id: String, flags: Int): PendingIntent? =
        PendingIntent.getBroadcast(
            context, 0,
            Intent(context, ReminderReceiver::class.java).setAction(FIRE).setData(uri(id)),
            PendingIntent.FLAG_IMMUTABLE or flags,
        )

    internal fun handle(context: Context, intent: Intent) {
        val core = (context.applicationContext as? CoreHolder)?.core?.getOrNull() ?: return
        val key = intent.getStringExtra(KEY)
        val occurrence = intent.getStringExtra(OCCURRENCE)
        when (intent.action) {
            FIRE -> if (key != null && occurrence != null) fire(context, core, key, occurrence, intent.getStringExtra(AT).orEmpty())
            DISMISS -> if (key != null && occurrence != null) dismiss(context, core, key, occurrence)
            SNOOZE -> if (key != null && occurrence != null) snooze(context, core, key, occurrence)
            CARRY -> {
                val target = intent.getStringExtra(TARGET)
                if (key != null && target != null && occurrence != null) carry(context, core, key, target, occurrence)
            }
            else -> {
                // The phone may have moved to another zone, which moves every civil time.
                core.timeZoneMayHaveChanged()
                arm(context, core)
            }
        }
    }

    /** The channel reminders are posted on, named for them, so the person can set how they come. */
    fun channel(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java)
        if (manager.getNotificationChannel(CHANNEL) != null) return
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, "Reminders", NotificationManager.IMPORTANCE_HIGH).apply {
                description = "Tasks coming due and blocks starting or ending, as you set them."
            },
        )
    }

    /** What Android allows reminders here now. */
    fun access(context: Context): ReminderAccess =
        ReminderAccess(notificationsAllowed(context), exactAllowed(context), exactAlarmSettings(context))

    /** Whether alarms may be exact: always before Android 12, else as the person allows. */
    fun exactAllowed(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.S || context.getSystemService(AlarmManager::class.java).canScheduleExactAlarms()

    /**
     * The system's page that grants exact alarms ("Alarms and reminders"), where there is
     * one: none before Android 12, when none is needed, and none on some watches.
     */
    fun exactAlarmSettings(context: Context): Intent? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return null
        val intent = Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, Uri.parse("package:${context.packageName}"))
        return intent.takeIf { it.resolveActivity(context.packageManager) != null }
    }

    /** Whether reminders can be shown: notifications allowed, and their channel not turned off. */
    fun notificationsAllowed(context: Context): Boolean {
        if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) return false
        val channel = context.getSystemService(NotificationManager::class.java).getNotificationChannel(CHANNEL)
        return channel == null || channel.importance != NotificationManager.IMPORTANCE_NONE
    }

    /** The system's page for this app's notifications. */
    fun notificationSettings(context: Context): Intent =
        Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)

    /**
     * Whether to ask for notifications now: from Android 13 they are asked for, and asked
     * once, when the person first adds a reminder or turns reminders on. After that, Settings
     * says they are off and opens the system's page.
     */
    fun shouldAsk(context: Context): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED &&
            !context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).getBoolean("asked", false)

    /** Notes that notifications have been asked for. */
    fun asked(context: Context) {
        context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).edit().putBoolean("asked", true).apply()
    }
}

/** Alarms going off and a notification's buttons: the app's own intents, never another's. */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) = Reminders.handle(context, intent)
}

/**
 * What moves every alarm: the phone starting (alarms do not survive it), the app updated, the
 * clock or time zone changed, exact alarms granted. Each re-arms from the store.
 */
class ReminderRearmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action in rearming) Reminders.handle(context, Intent(Reminders.REARM))
    }

    private val rearming = setOf(
        Intent.ACTION_BOOT_COMPLETED,
        Intent.ACTION_MY_PACKAGE_REPLACED,
        Intent.ACTION_TIME_CHANGED,
        Intent.ACTION_TIMEZONE_CHANGED,
        "android.app.action.SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED",
    )
}
