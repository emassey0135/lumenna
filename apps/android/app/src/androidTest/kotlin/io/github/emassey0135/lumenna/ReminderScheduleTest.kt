package io.github.emassey0135.lumenna

import android.Manifest
import android.app.Notification
import android.app.NotificationManager
import androidx.core.app.NotificationCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.emassey0135.lumenna.core.NewBlock
import java.io.File
import java.time.Duration
import java.time.Instant
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.ZonedDateTime
import java.time.temporal.ChronoUnit
import java.util.UUID
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * What alarms a store asks for (`ReminderSchedule`, through a recording [Alarms]), and what a
 * firing's notification says. Every block is put a set time from now, so nothing depends on
 * the hour the tests run.
 */
@RunWith(AndroidJUnit4::class)
class ReminderScheduleTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private lateinit var directory: File
    private lateinit var core: Core

    /** What was asked of the alarms, kept as AlarmManager would keep it. */
    private class Recorded(override val exact: Boolean = true) : Alarms {
        val set = mutableMapOf<String, Pair<ReminderAlarm, Boolean>>()
        val cancelled = mutableListOf<String>()
        var rearm: Instant? = null
        override var armed: Set<String> = emptySet()
        override fun set(alarm: ReminderAlarm, exact: Boolean) {
            set[alarm.id] = alarm to exact
        }
        override fun cancel(id: String) {
            cancelled += id
            set.remove(id)
        }
        override fun rearmAt(at: Instant) {
            rearm = at
        }
    }

    @Before
    fun open() {
        directory = File(context.cacheDir, "test-${UUID.randomUUID()}")
        core = Core(directory)
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    /** A one-off 30-minute block starting [minutes] from now, to the minute; its start and series. */
    private fun blockIn(title: String, minutes: Long): Pair<Instant, String> {
        val start = ZonedDateTime.now().plusMinutes(minutes).truncatedTo(ChronoUnit.MINUTES)
        core.lumenna.addBlock(
            NewBlock(
                title = title,
                at = "%02d:%02d".format(start.hour, start.minute),
                minutes = 30u,
                date = start.toLocalDate().toString(),
                kind = "work",
                repeat = null,
            ),
        )
        val series = core.lumenna.listBlocks().rows.single { it.title == title }.id.substringBefore('@')
        val at = LocalDateTime.of(start.toLocalDate(), start.toLocalTime()).atZone(ZoneId.systemDefault()).toInstant()
        return at to series
    }

    @Test
    fun aBlockAnHourAwayIsArmedFiveMinutesBeforeItStartsAtTheExactTime() {
        val (start, _) = blockIn("Deep work", 60)
        val alarms = Recorded()
        val armed = ReminderSchedule(core.lumenna, alarms).arm()
        val (alarm, exact) = alarms.set.values.single()
        assertEquals(start.minus(Duration.ofMinutes(5)), alarm.at)
        assertTrue("exact, where allowed", exact)
        assertEquals(armed, listOf(alarm))
        assertEquals(setOf(alarm.id), alarms.armed)
        assertNotNull("armed again before the window runs out", alarms.rearm)
    }

    @Test
    fun refusedExactAlarmsAreSetToComeWhenAndroidAllows() {
        blockIn("Deep work", 60)
        val alarms = Recorded(exact = false)
        ReminderSchedule(core.lumenna, alarms).arm()
        assertEquals(listOf(false), alarms.set.values.map { it.second })
    }

    @Test
    fun aDismissedFiringIsCancelledAtTheNextArming() {
        blockIn("Deep work", 60)
        blockIn("Review", 120)
        val alarms = Recorded()
        val schedule = ReminderSchedule(core.lumenna, alarms)
        val first = schedule.arm().first()
        core.lumenna.dismissReminder(first.key, first.occurrence)
        schedule.arm()
        assertEquals(listOf(first.id), alarms.cancelled)
        assertEquals(1, alarms.set.size)
    }

    @Test
    fun aSnoozedFiringIsArmedAgainTenMinutesOnUnderTheSameIdentity() {
        blockIn("Deep work", 60)
        val alarms = Recorded()
        val schedule = ReminderSchedule(core.lumenna, alarms)
        val first = schedule.arm().single()
        val before = Instant.now()
        core.lumenna.snoozeReminder(first.key, first.occurrence, null)
        val again = schedule.arm().single()
        assertEquals(first.id, again.id)
        val gap = Duration.between(before, again.at)
        assertTrue("about ten minutes on: $gap", gap >= Duration.ofMinutes(9) && gap <= Duration.ofMinutes(11))
    }

    @Test
    fun aBlocksOwnReminderReplacesTheDefault() {
        val (start, series) = blockIn("Deep work", 60)
        core.lumenna.addReminderSaying(series, "at end")
        val alarms = Recorded()
        ReminderSchedule(core.lumenna, alarms).arm()
        assertEquals(listOf(start.plus(Duration.ofMinutes(30))), alarms.set.values.map { it.first.at })
    }

    @Test
    fun withRemindersOffOnThisDeviceEveryAlarmIsCancelled() {
        blockIn("Deep work", 60)
        val alarms = Recorded()
        val schedule = ReminderSchedule(core.lumenna, alarms)
        val first = schedule.arm().single()
        core.lumenna.setSetting("reminders", "false")
        assertTrue(schedule.arm().isEmpty())
        assertEquals(listOf(first.id), alarms.cancelled)
        assertTrue(alarms.armed.isEmpty())
    }

    @Test
    fun aFiringFurtherOffThanTheWindowWaitsForALaterArming() {
        blockIn("Deep work", 60)
        blockIn("Far off", ReminderSchedule.WINDOW.toMinutes() + 60)
        val alarms = Recorded()
        ReminderSchedule(core.lumenna, alarms).arm()
        assertEquals(1, alarms.set.size)
    }

    @Test
    fun aFiringsNotificationSaysTheCoresWordsWithItsButtons() {
        blockIn("Deep work", 60)
        val due = core.lumenna.upcomingReminders(Instant.now().plus(Duration.ofHours(3)).toString()).due.single()
        val notification = Reminders.notification(context, due)
        assertEquals(due.title, notification.extras.getString(Notification.EXTRA_TITLE))
        assertEquals(due.text, notification.extras.getCharSequence(Notification.EXTRA_TEXT).toString())
        assertEquals("Deep work starts in 5 minutes — nothing assigned yet", due.text)
        assertEquals(listOf(due.dismiss, due.snooze), notification.actions.map { it.title.toString() })
        assertEquals(listOf("Dismiss", "Snooze 10 Minutes"), notification.actions.map { it.title.toString() })
        assertEquals(Notification.CATEGORY_REMINDER, notification.category)
        assertEquals(Reminders.CHANNEL, notification.channelId)
        assertEquals("not bridged to a watch that gives its own", Reminders.BRIDGE_TAG, NotificationCompat.WearableExtender(notification).bridgeTag)
    }

    @Test
    fun anAlarmGoingOffShowsItsFiringUnlessItWasDismissedSince() {
        InstrumentationRegistry.getInstrumentation().uiAutomation
            .grantRuntimePermission(context.packageName, Manifest.permission.POST_NOTIFICATIONS)
        blockIn("Deep work", 60)
        blockIn("Review", 120)
        val (deep, review) = ReminderSchedule(core.lumenna, Recorded()).arm()
        val manager = context.getSystemService(NotificationManager::class.java)
        fun shown() = manager.activeNotifications.filter { it.notification.channelId == Reminders.CHANNEL }.map { it.tag }
        try {
            Reminders.fire(context, core, deep.key, deep.occurrence, deep.at.toString())
            assertEquals(listOf(deep.id), shown())
            core.lumenna.dismissReminder(review.key, review.occurrence)
            Reminders.fire(context, core, review.key, review.occurrence, review.at.toString())
            assertEquals("a firing dismissed elsewhere since it was armed is not shown", listOf(deep.id), shown())
        } finally {
            manager.cancel(deep.id, 1)
            manager.cancel(review.id, 1)
        }
    }

    // MARK: - Catching up after the device was off

    /** What the app showed, as the alarm receiver records it. */
    private class Shown(vararg ids: String) : ShownReminders {
        val ids = ids.toMutableSet()
        override fun all(): Set<String> = ids.toSet()
        override fun forget(told: Set<String>) {
            ids -= told
        }
    }

    /**
     * The device off since an hour ago: what the core last heard of catching up is put back an
     * hour, as a phone switched off then would have left it.
     */
    private fun offSinceAnHourAgo() {
        val file = File(directory, "device-settings")
        val kept = file.takeIf { it.exists() }?.readLines().orEmpty().filter { !it.startsWith("reminders-caught-up\t") }
        file.writeText((kept + "reminders-caught-up\t${Instant.now().minus(Duration.ofHours(1))}").joinToString("\n") + "\n")
    }

    @Test
    fun aFiringNotShownWhileTheDeviceWasOffComesBackOnce() {
        val catchUp = ReminderCatchUp(core.lumenna, Shown())
        assertTrue("the first time, nothing", catchUp.catchUp().due.isEmpty())
        offSinceAnHourAgo()
        // Its default fired two or three minutes ago, while the device was off.
        blockIn("Deep work", 3)
        val found = catchUp.catchUp()
        assertEquals(listOf("Deep work"), found.due.map { it.title })
        assertTrue(found.missedSummary.isEmpty())
        assertTrue("caught up once", catchUp.catchUp().due.isEmpty())
    }

    @Test
    fun aFiringTheAppShowedIsNotCaughtUpAndIsForgottenOnceTold() {
        val first = ReminderCatchUp(core.lumenna, Shown())
        first.catchUp()
        offSinceAnHourAgo()
        blockIn("Deep work", 3)
        val fired = core.lumenna.remindersDue(Instant.now().minus(Duration.ofHours(1)).toString(), null).due.single()
        val shown = Shown("${fired.key}@${fired.occurrence}")
        assertTrue(ReminderCatchUp(core.lumenna, shown).catchUp().due.isEmpty())
        assertTrue("told, so forgotten", shown.ids.isEmpty())
    }

    @Test
    fun whatWasMissedLongAgoComesAsOneSummary() {
        val catchUp = ReminderCatchUp(core.lumenna, Shown())
        catchUp.catchUp()
        offSinceAnHourAgo()
        blockIn("Deep work", -30)
        val found = catchUp.catchUp()
        assertTrue(found.due.isEmpty())
        assertEquals("1 reminder missed: Deep work started.", found.missedSummary)
        assertEquals(found.missedSummary, Reminders.summary(context, found.missedSummary).extras.getCharSequence(Notification.EXTRA_TEXT).toString())
    }
}
