package io.github.emassey0135.lumenna

import android.text.format.DateFormat
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimeInput
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/*
 * Android's own day and time pickers, beside the line they fill: the line stays first and the
 * way in ("next friday", "9am"), which the core reads; a day or time chosen here is put in it,
 * ISO or HH:MM, for the core to read the same way. Material's dialogs say their own values; only
 * the button that opens one and the dialog's answers are the core's words (`picker_words`).
 */

private val isoDay = Regex("""\d{4}-\d{2}-\d{2}""")
private val clockTime = Regex("""(\d{1,2}):(\d{2})""")

/** A button that opens a calendar on the day `line` names, if it is a date, else `from`. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChooseDay(line: String, from: String? = null, modifier: Modifier = Modifier, picked: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    OutlinedButton(onClick = { open = true }, modifier = modifier.then(Target)) { Text(pickers.day) }
    if (!open) return
    val start = (line.trim().takeIf { isoDay.matches(it) } ?: from)
        ?.let { runCatching { LocalDate.parse(it) }.getOrNull() }
        ?: LocalDate.now()
    // Material's calendar counts days as UTC midnights.
    val state = rememberDatePickerState(initialSelectedDateMillis = start.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli())
    DatePickerDialog(
        onDismissRequest = { open = false },
        confirmButton = {
            TextButton(modifier = Target, onClick = {
                open = false
                state.selectedDateMillis?.let { picked(Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate().toString()) }
            }) { Text(pickers.choose) }
        },
        dismissButton = { TextButton(modifier = Target, onClick = { open = false }) { Text(pickers.cancel) } },
    ) {
        // No switch to typing a date: the line beside is that, and the switch is a 40dp target.
        DatePicker(state, showModeToggle = false)
    }
}

/**
 * A button that opens a clock on the time `line` holds, if it is one in HH:MM: Material's time
 * input, two number fields, rather than its dial, whose inner numbers are 32dp targets and
 * which TalkBack moves through a number at a time. On a 12-hour clock its AM and PM are 36dp,
 * Material's own and not ours to size.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChooseTime(line: String, modifier: Modifier = Modifier, picked: (String) -> Unit) {
    var open by rememberSaveable { mutableStateOf(false) }
    OutlinedButton(onClick = { open = true }, modifier = modifier.then(Target)) { Text(pickers.time) }
    if (!open) return
    val shown = clockTime.matchEntire(line.trim())
    val hour = shown?.groupValues?.get(1)?.toIntOrNull()?.takeIf { it in 0..23 } ?: 9
    val minute = shown?.groupValues?.get(2)?.toIntOrNull()?.takeIf { it in 0..59 } ?: 0
    // The clock the person set: 13:00 or 1:00 PM.
    val state = rememberTimePickerState(hour, minute, is24Hour = DateFormat.is24HourFormat(LocalContext.current))
    AlertDialog(
        onDismissRequest = { open = false },
        title = { Text(pickers.time) },
        text = { TimeInput(state) },
        confirmButton = {
            TextButton(modifier = Target, onClick = {
                open = false
                picked("%02d:%02d".format(state.hour, state.minute))
            }) { Text(pickers.choose) }
        },
        dismissButton = { TextButton(modifier = Target, onClick = { open = false }) { Text(pickers.cancel) } },
    )
}
