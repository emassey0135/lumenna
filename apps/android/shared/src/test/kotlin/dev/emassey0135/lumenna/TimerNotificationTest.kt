package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.DueReminder
import dev.emassey0135.lumenna.app.GlanceTimer
import dev.emassey0135.lumenna.app.TimerControl
import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** A running timer outside the app, and its time running out: what the core gives, drawn. */
class TimerNotificationTest {
    private val now = Instant.parse("2026-11-27T10:07:00Z")

    private fun timer(runsOutAt: String? = "2026-11-27T10:25:00Z", planned: UInt? = 25u) =
        GlanceTimer("sitting-1", "Essay", "Deep work", "2026-11-27T10:00:00Z", planned, runsOutAt)

    private fun running(timer: GlanceTimer?) = TimerControl(true, "Essay", "Stop Timer", "Stop timer", true, timer)

    // The core's `glance_countdown` and `timer_control_line`, their parts shown, without the native library.
    private val countdown = { timer: GlanceTimer, at: String -> timer.runsOutAt?.let { "left until $it at $at" } }
    private val line = { control: TimerControl, at: String, sentence: Boolean -> "${control.actionSentence}|${control.detail}|$at|$sentence" }

    @Test
    fun aTimerWithAPlannedLengthCountsDownToWhereItRunsOut() {
        val shown = TimerNotification.shown(running(timer()), now, countdown, line)!!
        assertTrue(shown.countsDown)
        assertEquals(Instant.parse("2026-11-27T10:25:00Z").toEpochMilli(), shown.at)
        assertEquals("Essay", shown.task)
        assertEquals("sitting-1", shown.sitting)
        // The core's countdown beneath the task, the block beside it.
        assertEquals("left until 2026-11-27T10:25:00Z at $now", shown.text)
        assertEquals("Deep work", shown.block)
    }

    @Test
    fun aTimerWithoutOneCountsUpFromWhereItCountsFromAndShowsItsBlock() {
        val shown = TimerNotification.shown(running(timer(runsOutAt = null, planned = null)), now, countdown, line)!!
        assertFalse(shown.countsDown)
        assertEquals(Instant.parse("2026-11-27T10:00:00Z").toEpochMilli(), shown.at)
        assertEquals("Deep work", shown.text)
        assertNull(shown.block)
    }

    @Test
    fun whatTalkBackIsToldIsTheCoresLineInSentenceCase() {
        val shown = TimerNotification.shown(running(timer()), now, countdown, line)!!
        assertEquals("Stop timer|Essay|$now|true", shown.description)
    }

    @Test
    fun nothingIsShownWhileNoTimerRuns() {
        assertNull(TimerNotification.shown(running(null), now, countdown, line))
        val stopped = TimerControl(false, "Essay", "Start Timer", "Start timer", true, timer())
        assertNull(TimerNotification.shown(stopped, now, countdown, line))
    }

    private fun due(snooze: String = "Snooze", stop: String = "", sitting: String = "", carry: String = "") = DueReminder(
        "time-up:sitting-1", "2026-11-27", "2026-11-27T10:25:00Z", "task-1", "task", "Essay",
        "Essay's 25 minutes are up", "Essay ran past its 25 minutes", "Dismiss", snooze, carry, "", stop, sitting,
    )

    @Test
    fun aTimeRunningOutHasStopTimerAndDismissAndNeverSnooze() {
        val buttons = Reminders.buttons(due(snooze = "", stop = "Stop Timer", sitting = "sitting-1")) { it }
        assertEquals(listOf(Reminders.STOP to "Stop Timer", Reminders.DISMISS to "Dismiss"), buttons)
    }

    @Test
    fun anOrdinaryReminderHasDismissAndSnooze() {
        assertEquals(listOf(Reminders.DISMISS to "Dismiss", Reminders.SNOOZE to "Snooze"), Reminders.buttons(due()) { it })
    }

    @Test
    fun carryForwardFollowsWhereTheCoreOffersIt() {
        val buttons = Reminders.buttons(due(carry = "Carry Forward")) { it }
        assertEquals(listOf(Reminders.DISMISS, Reminders.SNOOZE, Reminders.CARRY), buttons.map { it.first })
    }

    @Test
    fun stopTimerNeedsTheSittingItStops() {
        assertEquals(listOf(Reminders.DISMISS), Reminders.buttons(due(snooze = "", stop = "Stop Timer")) { it }.map { it.first })
    }
}
