package dev.emassey0135.lumenna

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
    val style: dev.emassey0135.lumenna.app.TimeStyle
        get() = if (twentyFourHour) dev.emassey0135.lumenna.app.TimeStyle.TWENTY_FOUR_HOUR
        else dev.emassey0135.lumenna.app.TimeStyle.TWELVE_HOUR

    /** `14:30` as this phone says it: "2:30 PM", or "14:30". */
    fun time(clock: String): String {
        val time = runCatching { LocalTime.parse(clock) }.getOrNull() ?: return clock
        return format(time.hour, time.minute)
    }

    private fun format(hour: Int, minute: Int): String {
        val pattern = DateFormat.getBestDateTimePattern(Locale.getDefault(), if (twentyFourHour) "Hm" else "hma")
        return LocalTime.of(hour, minute).format(DateTimeFormatter.ofPattern(pattern))
    }

    /**
     * What this phone's locale writes for the core, which words every line: a time, and a date
     * with its weekday and month named. "Today", "Tomorrow" and when to give the year are the core's.
     */
    val locale = object : dev.emassey0135.lumenna.app.Locale {
        override fun time(hour: Byte, minute: Byte): String = format(hour.toInt(), minute.toInt())

        override fun date(year: Short, month: Byte, day: Byte, withYear: Boolean): String {
            val date = LocalDate.of(year.toInt(), month.toInt(), day.toInt())
            val weekday = date.dayOfWeek.getDisplayName(TextStyle.FULL, Locale.getDefault())
            val name = date.month.getDisplayName(TextStyle.FULL, Locale.getDefault())
            return if (withYear) "$weekday ${date.dayOfMonth} $name ${date.year}" else "$weekday ${date.dayOfMonth} $name"
        }
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


