package io.github.emassey0135.lumenna.wear

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.BlockPurpose
import io.github.emassey0135.lumenna.Choice
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.DayAction
import io.github.emassey0135.lumenna.DayRow
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.folded
import io.github.emassey0135.lumenna.levelChange
import io.github.emassey0135.lumenna.rows
import io.github.emassey0135.lumenna.sentence
import io.github.emassey0135.lumenna.taskChoices
import io.github.emassey0135.lumenna.words
import io.github.emassey0135.lumenna.core.Plan
import java.time.LocalDate

/**
 * The day as lived, as the phone's day lists it (`DayRow`, `words`): its summary, each block
 * with its sittings beneath it, free time, now, and what is cancelled for the day. A row's
 * actions are the phone's (`DayAction`): TalkBack's custom actions, and a long press or a tap.
 * A block with sittings folds. Another day is a button away.
 */
@Composable
fun DayScreen(core: Core, navigator: Navigator, changes: Long) {
    var day by rememberSaveable { mutableStateOf<String?>(null) }
    var collapsed by rememberSaveable { mutableStateOf(emptySet<String>()) }
    val plan = remember(changes, day) { core.attempt { core.lumenna.plan(day) } }
    val entry = LocalTextEntry.current
    val shown = plan?.let { folded(rows(it), { row -> row.depth }, { row -> row.key }, collapsed) }.orEmpty()
    WearList {
        item { Heading(plan?.let { Clock.spokenDay(it.date) } ?: "Day") }
        plan?.let { item { Text(it.summary.ifEmpty { it.announcement.replaceFirstChar { c -> c.uppercase() } }) } }
        shown.forEachIndexed { index, shownRow ->
            item {
                val row = shownRow.item
                val (title, details) = words(row)
                val actions = plan?.let { actions(core, navigator, it, row) }.orEmpty() +
                    if (shownRow.parent) listOf(RowAction(if (shownRow.collapsed) "Expand" else "Collapse") {
                        collapsed = if (row.key in collapsed) collapsed - row.key else collapsed + row.key
                        core.say(if (row.key in collapsed) "Collapsed" else "Expanded")
                    }) else emptyList()
                RowButton(
                    title,
                    detail = details.joinToString(", "),
                    state = listOfNotNull(shownRow.state, levelChange(shown, index)).joinToString(", ").ifEmpty { null },
                    actions = actions,
                    onLongClick = { navigator.actions(title, actions) },
                    onClick = { if (actions.isNotEmpty()) navigator.actions(title, actions) },
                )
            }
        }
        item { Button(onClick = { navigator.open(Screen.BlockForm(BlockPurpose.Add(date = plan?.date))) }, modifier = Modifier.fillMaxWidth(), label = { Text("Add Block") }) }
        item { Button(onClick = { day = step(plan, -1) }, modifier = Modifier.fillMaxWidth(), label = { Text("Previous Day") }) }
        if (day != null) item { Button(onClick = { day = null }, modifier = Modifier.fillMaxWidth(), label = { Text("Today") }) }
        item { Button(onClick = { day = step(plan, 1) }, modifier = Modifier.fillMaxWidth(), label = { Text("Next Day") }) }
        item {
            // As the phone asks: a day as it is said, read by the core.
            Button(onClick = {
                entry?.ask("Go to day") { text -> core.attempt { core.lumenna.plan(text) }?.let { day = it.date } }
            }, modifier = Modifier.fillMaxWidth(), label = { Text("Go to Day") })
        }
    }
}

private fun step(plan: Plan?, days: Long): String? {
    val from = plan?.date?.let { LocalDate.parse(it) } ?: LocalDate.now()
    val next = from.plusDays(days)
    return if (next == LocalDate.now()) null else next.toString()
}

/** What can be done to a day row: which apply is the phone's too (`DayAction`). */
private fun actions(core: Core, navigator: Navigator, plan: Plan, row: DayRow): List<RowAction> {
    val date = plan.date
    val timed = { operation: () -> io.github.emassey0135.lumenna.core.Timer ->
        core.attempt(operation)?.let {
            core.changed()
            core.report(sentence(it.announcement, it.notices))
        }
    }
    val listed = when (row) {
        is DayRow.Block -> DayAction.of(row.block)
        is DayRow.Sitting -> DayAction.of(row.sitting)
        is DayRow.Free -> DayAction.ofFreeTime
        is DayRow.Cancelled -> DayAction.ofCancelled
        is DayRow.Now -> emptyList()
    }
    return listed.mapNotNull { action ->
        val run: (() -> Unit)? = when (row) {
            is DayRow.Block -> {
                val block = row.block
                when (action) {
                    DayAction.ASSIGN_TASK -> { {
                        navigator.choose("Assign to ${block.title}", taskChoices(core)) { task ->
                            chooseLength(navigator, "How long is this sitting meant to take?", "No Planned Length") { minutes ->
                                core.change { it.assign(task.key, block.series, date, minutes) }
                            }
                        }
                    } }
                    DayAction.EDIT -> { {
                        // Never guessed: one day, or every day, of a repeating block.
                        if (!block.repeats) navigator.open(Screen.BlockForm(BlockPurpose.Series(block.series)))
                        else navigator.choose("Change ${block.title}", listOf(
                            Choice("day", "${Clock.spokenDay(date)} Only"), Choice("all", "Every Occurrence"),
                        ), "Which occurrences?") { which ->
                            navigator.open(Screen.BlockForm(
                                if (which.key == "day") BlockPurpose.Occurrence(block, date) else BlockPurpose.Series(block.series),
                            ))
                        }
                    } }
                    DayAction.CANCEL_THIS_DAY -> { { core.change { it.cancelOccurrence(block.series, date) } } }
                    DayAction.RESTORE_THIS_DAY -> { { core.change { it.restoreOccurrence(block.series, date) } } }
                    DayAction.DELETE_BLOCK -> { {
                        navigator.choose("Delete ${block.title}?", listOf(Choice("delete", "Delete")), DayAction.deleting(block)) {
                            core.change { it.deleteBlock(block.series) }
                        }
                    } }
                    else -> null
                }
            }
            is DayRow.Sitting -> {
                val sitting = row.sitting
                when (action) {
                    DayAction.START_TIMER, DayAction.RESUME_TIMER -> { { core.change { it.startTimer(sitting.id) } } }
                    DayAction.PAUSE_TIMER -> { { timed { core.lumenna.pauseTimer(sitting.id) } } }
                    DayAction.STOP_TIMER -> { { timed { core.lumenna.stopTimer(sitting.id, null) } } }
                    DayAction.PLANNED_LENGTH -> { {
                        chooseLength(navigator, "Planned length of ${sitting.title}", "No Planned Length") { minutes ->
                            core.change { it.planMinutes(sitting.id, minutes) }
                        }
                    } }
                    DayAction.LOG_MINUTES -> { {
                        chooseLength(navigator, "Minutes on ${sitting.title}", "Cancel") { minutes ->
                            if (minutes != null) timed { core.lumenna.stopTimer(sitting.id, minutes) }
                        }
                    } }
                    DayAction.SHOW_THE_TASK -> { { navigator.open(Screen.Task(sitting.task)) } }
                    DayAction.UNASSIGN -> { { core.change { it.unassign(sitting.id) } } }
                    else -> null
                }
            }
            is DayRow.Free -> { { navigator.open(Screen.BlockForm(BlockPurpose.Add(date = date, at = row.start, minutes = row.minutes))) } }
            is DayRow.Cancelled -> { { core.change { it.restoreOccurrence(row.block.series, date) } } }
            is DayRow.Now -> null
        }
        run?.let { RowAction(action.title, it) }
    }
}
