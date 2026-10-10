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
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.DayRow
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.folded
import io.github.emassey0135.lumenna.levelChange
import io.github.emassey0135.lumenna.rows
import io.github.emassey0135.lumenna.actions
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.words
import io.github.emassey0135.lumenna.core.Plan
import java.time.LocalDate

/**
 * The day as lived, as the phone's day lists it (`DayRow`, `words`): its summary, each block
 * with its sittings beneath it, free time, now, and what is cancelled for the day. A row's
 * actions are the core's: TalkBack's custom actions, and a long press or a tap.
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
                val actions = plan?.let { actions(core, navigator, entry, it, row) }.orEmpty() +
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

/**
 * What can be done to a day row: the core's, in its order. Only its forms are this screen's to
 * open: a block's, the task a sitting is for, and a new block in free time.
 */
private fun actions(core: Core, navigator: Navigator, entry: TextEntry?, plan: Plan, row: DayRow): List<RowAction> =
    core.offered(row.actions, navigator, entry, form = { action ->
        when (action.kind) {
            ActionKind.EDIT -> if (row is DayRow.Block) {
                val block = row.block
                val date = plan.date
                // Never guessed: one day, or every day, of a repeating block.
                if (!block.repeats) navigator.open(Screen.BlockForm(BlockPurpose.Series(block.series)))
                else navigator.choose("Change ${block.title}", listOf(
                    Option("day", "${Clock.spokenDay(date)} Only"), Option("all", "Every Occurrence"),
                ), "Which occurrences?") { which ->
                    navigator.open(Screen.BlockForm(
                        if (which.key == "day") BlockPurpose.Occurrence(block, date) else BlockPurpose.Series(block.series),
                    ))
                }
            }
            ActionKind.EDIT_TASK -> navigator.open(Screen.Task(action.target))
            ActionKind.ADD_BLOCK -> if (row is DayRow.Free) {
                navigator.open(Screen.BlockForm(BlockPurpose.Add(date = action.target, at = action.other ?: row.start, minutes = row.minutes)))
            }
            else -> {}
        }
    })
