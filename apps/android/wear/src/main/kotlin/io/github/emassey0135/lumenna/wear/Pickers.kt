package io.github.emassey0135.lumenna.wear

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.DatePicker
import androidx.wear.compose.material3.Dialog
import androidx.wear.compose.material3.Text
import androidx.wear.compose.material3.TimePicker
import io.github.emassey0135.lumenna.pickers
import java.time.LocalDate
import java.time.LocalTime

/*
 * The watch's own day and time pickers, beside the line they fill: the line stays first and the
 * way in, said or typed on the system's input screen; a day or time chosen here, by the crown or
 * a swipe, is put in it, ISO or HH:MM, for the core to read the same way. Wear's pickers say
 * their own columns and confirm with their own button; only the button that opens one is the
 * core's words (`picker_words`).
 */

private val isoDay = Regex("""\d{4}-\d{2}-\d{2}""")
private val clockTime = Regex("""(\d{1,2}):(\d{2})""")

/** The day `line` names, if it is a date, else `from`, else today. */
fun dayOf(line: String, from: String? = null): LocalDate =
    (line.trim().takeIf { isoDay.matches(it) } ?: from)?.let { runCatching { LocalDate.parse(it) }.getOrNull() } ?: LocalDate.now()

/** Wear's calendar, over everything, open on `start`; a day chosen is given back ISO. */
@Composable
fun DayDialog(visible: Boolean, start: LocalDate, dismissed: () -> Unit, picked: (String) -> Unit) {
    Dialog(visible = visible, onDismissRequest = dismissed) {
        DatePicker(initialDate = start, onDatePicked = { picked(it.toString()) })
    }
}

/** A button that opens a calendar on the day `line` names, if it is a date, else `from`. */
@Composable
fun ChooseDay(line: String, from: String? = null, picked: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    Button(onClick = { open = true }, modifier = Modifier.fillMaxWidth(), label = { Text(pickers.day) })
    DayDialog(open, dayOf(line, from), dismissed = { open = false }) {
        open = false
        picked(it)
    }
}

/**
 * A button that opens a clock on the time `line` holds, if it is one in HH:MM; `of`, beneath,
 * says which line it fills where there are several.
 */
@Composable
fun ChooseTime(line: String, of: String? = null, picked: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    Button(
        onClick = { open = true },
        modifier = Modifier.fillMaxWidth(),
        label = { Text(pickers.time) },
        secondaryLabel = of?.let { { Text(it) } },
    )
    val shown = clockTime.matchEntire(line.trim())
    val start = shown?.let { runCatching { LocalTime.of(it.groupValues[1].toInt(), it.groupValues[2].toInt()) }.getOrNull() } ?: LocalTime.of(9, 0)
    Dialog(visible = open, onDismissRequest = { open = false }) {
        // The clock the person set: Wear's picker follows it by itself.
        TimePicker(initialTime = start, onTimePicked = {
            open = false
            picked("%02d:%02d".format(it.hour, it.minute))
        })
    }
}
