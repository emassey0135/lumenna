package io.github.emassey0135.lumenna

import android.content.Context
import android.text.format.DateFormat
import io.github.emassey0135.lumenna.core.RowView
import java.time.LocalDate
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

/**
 * Times and days as a person says them, in their own locale.
 *
 * The core sends `HH:MM` and ISO dates, which are components (§13); whether that is "2:30 PM"
 * or "14:30" is this phone's setting, so it is decided here.
 */
object Clock {
    private var twentyFourHour = false

    /** Reads the phone's 12- or 24-hour setting, which can change while the app runs. */
    fun update(context: Context) {
        twentyFourHour = DateFormat.is24HourFormat(context)
    }

    /** `14:30` as this phone says it: "2:30 PM", or "14:30". */
    fun time(clock: String): String {
        val time = runCatching { LocalTime.parse(clock) }.getOrNull() ?: return clock
        val pattern = DateFormat.getBestDateTimePattern(Locale.getDefault(), if (twentyFourHour) "Hm" else "hma")
        return time.format(DateTimeFormatter.ofPattern(pattern))
    }

    /** "1 hour 30 minutes", as the core, the command line and the BTSpeak app say it. */
    fun length(minutes: UInt): String {
        val hours = minutes / 60u
        val rest = minutes % 60u
        val parts = buildList {
            if (hours > 0u) add(if (hours == 1u) "1 hour" else "$hours hours")
            if (rest > 0u || hours == 0u) add(if (rest == 1u) "1 minute" else "$rest minutes")
        }
        return parts.joinToString(" ")
    }

    /** Today, as the core reads a date. */
    fun today(): String = LocalDate.now().toString()

    /** An ISO date as a person says it: "Today", or "Monday 5 October". */
    fun spokenDay(iso: String): String {
        val day = runCatching { LocalDate.parse(iso) }.getOrNull() ?: return iso
        val today = LocalDate.now()
        return when (day) {
            today -> "Today"
            today.plusDays(1) -> "Tomorrow"
            today.minusDays(1) -> "Yesterday"
            else -> {
                val weekday = day.dayOfWeek.getDisplayName(TextStyle.FULL, Locale.getDefault())
                val month = day.month.getDisplayName(TextStyle.FULL, Locale.getDefault())
                if (day.year == today.year) "$weekday ${day.dayOfMonth} $month"
                else "$weekday ${day.dayOfMonth} $month ${day.year}"
            }
        }
    }
}

/**
 * How a row is spoken, assembled from the components the core sends (§13) — as the iPhone
 * says it, so the two read alike.
 *
 * The core sends components rather than a sentence because speech and braille compose them
 * differently; this is the speech half. TalkBack reads a row's description, then its state,
 * so the title — what a person is scanning for — comes first and is never abbreviated.
 */
object RowSpeech {
    /** The title, verbatim. */
    fun label(row: RowView): String = row.title

    /**
     * Everything after the title: done or not, the due date, notable states, and the level.
     *
     * The level is said only where it changes from the row before (§16.11): "level 2" on every
     * subtask is noise, and indentation, which is how a sighted reader gets it, says nothing.
     */
    fun value(row: RowView, previousDepth: UInt?): String {
        val parts = mutableListOf<String>()
        if (row.checked == true) parts += "done"
        row.value?.let { parts += it }
        // `ready` is true of almost every task; saying it everywhere buries the states that
        // mean something.
        parts += row.state.filter { it != "ready" }
        if (row.expanded != null) parts += "has subtasks"
        if (row.depth != (previousDepth ?: 0u)) parts += "level ${row.depth + 1u}"
        return parts.joinToString(", ")
    }
}
