package io.github.emassey0135.lumenna

import android.content.Context
import android.text.format.DateFormat
import java.time.LocalDate
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle
import java.util.Locale

/**
 * Times and days as a person says them, in their own locale.
 *
 * The core sends `HH:MM` and ISO dates, which are components; whether that is "2:30 PM"
 * or "14:30" is this phone's setting, so it is decided here.
 */
object Clock {
    private var twentyFourHour = false

    /** Reads the phone's 12- or 24-hour setting, which can change while the app runs. */
    fun update(context: Context) {
        twentyFourHour = DateFormat.is24HourFormat(context)
    }

    /** The clock the core words a proposal's times in, as this device has its own. */
    val style: io.github.emassey0135.lumenna.app.TimeStyle
        get() = if (twentyFourHour) io.github.emassey0135.lumenna.app.TimeStyle.TWENTY_FOUR_HOUR
        else io.github.emassey0135.lumenna.app.TimeStyle.TWELVE_HOUR

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

/** The core counts text in UTF-8 bytes, as Rust strings do; Kotlin counts UTF-16 units. */
object TextOffsets {
    /** A UTF-16 offset as the byte offset the core wants. */
    fun bytes(utf16: Int, text: String): UInt =
        text.substring(0, utf16.coerceIn(0, text.length)).encodeToByteArray().size.toUInt()

    /** A byte offset from the core as a UTF-16 offset, never splitting a character. */
    fun utf16(bytes: UInt, text: String): Int {
        val encoded = text.encodeToByteArray()
        return encoded.copyOfRange(0, bytes.toInt().coerceIn(0, encoded.size)).decodeToString().length
    }
}


