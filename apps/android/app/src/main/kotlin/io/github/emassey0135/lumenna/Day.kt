package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import io.github.emassey0135.lumenna.core.BlockFields
import io.github.emassey0135.lumenna.core.BlockScope
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.blockDefaults
import io.github.emassey0135.lumenna.core.blockEdit
import io.github.emassey0135.lumenna.core.blockFields
import io.github.emassey0135.lumenna.core.dayBlockFields
import io.github.emassey0135.lumenna.core.newBlock
import io.github.emassey0135.lumenna.core.CancelledBlock
import io.github.emassey0135.lumenna.core.Plan
import io.github.emassey0135.lumenna.core.PlanAssignment
import io.github.emassey0135.lumenna.core.PlanBlock
import io.github.emassey0135.lumenna.core.PlanItem

/** One line of the day as it is lived. */
private sealed interface DayRow {
    val key: String
    val depth: Int get() = 0

    data class Block(val block: PlanBlock) : DayRow {
        override val key get() = "block:${block.id}"
    }

    data class Sitting(val sitting: PlanAssignment, val block: PlanBlock) : DayRow {
        override val key get() = "sitting:${sitting.id}"
        override val depth get() = 1
    }

    data class Free(val start: String, val end: String, val minutes: UInt) : DayRow {
        override val key get() = "free:$start"
    }

    data class Now(val time: String) : DayRow {
        override val key get() = "now"
    }

    data class Cancelled(val block: CancelledBlock) : DayRow {
        override val key get() = "cancelled:${block.series}"
    }
}

private fun rows(plan: Plan): List<DayRow> = plan.timeline.flatMap { item ->
    when (item) {
        is PlanItem.Block -> plan.blocks.firstOrNull { it.row == item.row }?.let { block ->
            listOf(DayRow.Block(block)) + block.assignments.map { DayRow.Sitting(it, block) }
        }.orEmpty()
        is PlanItem.Free -> listOf(DayRow.Free(item.start, item.end, item.minutes))
        is PlanItem.Now -> listOf(DayRow.Now(item.time))
    }
} + plan.cancelled.map { DayRow.Cancelled(it) }

/** What a day row says: its title, then its details. */
private fun words(row: DayRow): Pair<String, List<String>> = when (row) {
    // The core words a block's and a sitting's details for every app.
    is DayRow.Block -> "${Clock.time(row.block.start)} to ${Clock.time(row.block.end)}, ${row.block.title}" to row.block.details
    is DayRow.Sitting -> row.sitting.title to row.sitting.details
    is DayRow.Free -> "Free, ${Clock.length(row.minutes)}" to listOf("${Clock.time(row.start)} to ${Clock.time(row.end)}")
    is DayRow.Now -> "Now, ${Clock.time(row.time)}" to emptyList()
    is DayRow.Cancelled -> "${Clock.time(row.block.start)}, ${row.block.title}" to listOf("cancelled for this day")
}

/** What the day screen is asking. */
private sealed interface DayAsk {
    data class Assign(val block: PlanBlock) : DayAsk
    data class AssignLength(val block: PlanBlock, val task: Choice) : DayAsk
    data class Plan(val sitting: PlanAssignment) : DayAsk
    data class Log(val sitting: PlanAssignment) : DayAsk
    data class Which(val block: PlanBlock) : DayAsk
    data class Delete(val block: PlanBlock) : DayAsk
    data object GoTo : DayAsk
}

/**
 * The planner: a day as it is lived, as a list.
 *
 * Blocks in time order with their sittings beneath them, free time as rows of its own, and now
 * as a position rather than a highlight. The summary above says what a glance at a timeline
 * would. Opening on today scrolls to now, not to midnight.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DayScreen(core: Core, navigator: Navigator, screen: Screen.Day, changes: Long) {
    var day by rememberSaveable(screen) { mutableStateOf(screen.date) }
    var asking by remember { mutableStateOf<DayAsk?>(null) }
    val plan: Result<Plan> = remember(day, changes) { runCatching { core.lumenna.plan(day) } }
    val shown = plan.getOrNull()
    val folding = rememberFolding()
    val folds = folded(shown?.let { rows(it) }.orEmpty(), { it.depth }, { it.key }, folding.value)
    val list = folds.map { it.item }
    val scroll = rememberLazyListState()
    val focus = rememberRowFocus(core, list.map { it.key }, scroll)

    // Lands on now once a day is opened, so the morning's past blocks are not in the way.
    LaunchedEffect(shown?.date) {
        val now = list.indexOfFirst { it is DayRow.Now }
        if (now >= 0) scroll.scrollToItem(now)
    }

    // A block that happens once opens its form; a repeating one asks which days first.
    val edit: (PlanBlock) -> Unit = { block ->
        if (block.repeats && shown != null) asking = DayAsk.Which(block)
        else navigator.push(Screen.BlockForm(BlockPurpose.Series(block.series)))
    }

    fun step(days: Long) {
        val from = java.time.LocalDate.parse(shown?.date ?: Clock.today())
        day = from.plusDays(days).toString()
    }

    Offer(Command.PREVIOUS_DAY) { step(-1) }
    Offer(Command.NEXT_DAY) { step(1) }
    Offer(Command.GO_TO_NOW) { day = null }
    Offer(Command.GO_TO_DAY) { asking = DayAsk.GoTo }
    Offer(Command.NEW_BLOCK) { navigator.push(Screen.BlockForm(BlockPurpose.Add(date = shown?.date))) }

    ScreenFrame(shown?.let { Clock.spokenDay(it.date) } ?: "Today", core, navigator, actions = {
        IconButton(onClick = { navigator.push(Screen.BlockForm(BlockPurpose.Add(date = shown?.date))) }) {
            Icon(Icons.Filled.Add, contentDescription = "Add block")
        }
    }) {
        Text(
            plan.fold({ it.summary }, { (it as? io.github.emassey0135.lumenna.core.LumennaException)?.sentence.orEmpty() }),
            style = MaterialTheme.typography.bodyMedium,
            color = quiet(),
            modifier = Modifier.padding(horizontal = 16.dp).semantics { heading() },
        )
        FlowRow(
            Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedButton(modifier = Target, onClick = { step(-1) }) { Text("Previous Day") }
            OutlinedButton(modifier = Target, onClick = { day = null }) { Text("Now") }
            OutlinedButton(modifier = Target, onClick = { step(1) }) { Text("Next Day") }
            OutlinedButton(modifier = Target, onClick = { asking = DayAsk.GoTo }) { Text("Go to Day") }
        }
        LazyColumn(
            Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(list.size, 1) },
            state = scroll,
        ) {
            itemsIndexed(list, key = { _, row -> row.key }) { index, row ->
                val (title, details) = words(row)
                val speech = details + listOfNotNull(folds[index].state, levelChange(folds, index))
                ListRow(
                    title = title,
                    detail = details.joinToString(", "),
                    speech = speech.joinToString(", "),
                    index = index,
                    depth = row.depth,
                    actions = focus.actions(row.key, index, actions(core, navigator, shown, row, edit) { asking = it }) +
                        listOfNotNull(foldAction(core, folds[index], row.key, folding)),
                    focus = focus.requester(row.key),
                    key = row.key,
                    open = when (row) {
                        is DayRow.Block -> ({ edit(row.block) })
                        is DayRow.Sitting -> ({ navigator.push(Screen.Task(row.sitting.task)) })
                        else -> null
                    },
                    openLabel = if (row is DayRow.Sitting) "Show the task" else "Edit",
                )
                HorizontalDivider()
            }
        }
    }

    val dismiss = { asking = null }
    when (val question = asking) {
        is DayAsk.Assign -> Choose("Assign to ${question.block.title}", taskChoices(core), "There are no open tasks.", dismiss) {
            asking = DayAsk.AssignLength(question.block, it)
        }
        is DayAsk.AssignLength -> AskLength(core, "How long is ${question.task.title} meant to take?", null, "no planned length", dismiss) { minutes ->
            asking = null
            core.change { it.assign(question.task.key, question.block.series, shown?.date, minutes) }
        }
        is DayAsk.Plan -> AskLength(core, "Planned length of ${question.sitting.title}", question.sitting.plannedMins, "no planned length", dismiss) { minutes ->
            asking = null
            core.change { it.planMinutes(question.sitting.id, minutes) }
        }
        is DayAsk.Log -> AskText(
            "Minutes on ${question.sitting.title}", "Minutes", "Log", example = "45",
            hint = "The whole of this sitting, replacing what is logged.", number = true, dismiss = dismiss,
        ) { text ->
            val minutes = text.trim().toUIntOrNull()
            if (minutes == null) {
                core.say("That is not a number of minutes.")
            } else {
                asking = null
                core.attempt { core.lumenna.stopTimer(question.sitting.id, minutes) }?.let {
                    core.changed()
                    core.report(sentence(it.announcement, it.notices))
                }
            }
        }
        is DayAsk.Which -> {
            val block = question.block
            val date = shown?.date ?: Clock.today()
            run {
                // "This day, or every day?" is asked, never guessed.
                AlertDialog(
                    onDismissRequest = dismiss,
                    title = { Text("Change ${block.title}") },
                    text = { Text("Which occurrences?") },
                    confirmButton = {
                        Column {
                            TextButton(modifier = Target, onClick = {
                                asking = null
                                navigator.push(
                                    Screen.BlockForm(
                                        BlockPurpose.Occurrence(block, date),
                                    ),
                                )
                            }) { Text("${Clock.spokenDay(date)} Only") }
                            TextButton(modifier = Target, onClick = {
                                asking = null
                                navigator.push(Screen.BlockForm(BlockPurpose.Series(block.series)))
                            }) { Text("Every Occurrence") }
                            TextButton(modifier = Target, onClick = dismiss) { Text("Cancel") }
                        }
                    },
                )
            }
        }
        is DayAsk.Delete -> Confirm(
            "Delete ${question.block.title}?",
            if (question.block.repeats) "Every occurrence goes, not only this day. To skip one day, cancel it instead." else "The block and its sittings go.",
            "Delete",
            dismiss,
        ) {
            asking = null
            core.change { it.deleteBlock(question.block.series) }
        }
        DayAsk.GoTo -> AskText("Go to Day", "Day", "Go", example = "next friday", hint = "A date, such as tomorrow or 12 October.", dismiss = dismiss) { text ->
            core.attempt { core.lumenna.plan(text) }?.let {
                asking = null
                day = it.date
            }
        }
        null -> {}
    }
}

/** What can be done to a day row, as custom actions and on a long press. */
private fun actions(
    core: Core,
    navigator: Navigator,
    plan: Plan?,
    row: DayRow,
    edit: (PlanBlock) -> Unit,
    ask: (DayAsk) -> Unit,
): List<RowAction> {
    val date = plan?.date
    return when (row) {
        is DayRow.Block -> buildList {
            val block = row.block
            if (block.acceptsTasks) add(RowAction("Assign Task") { ask(DayAsk.Assign(block)) })
            add(RowAction("Edit") { edit(block) })
            if (block.repeats && date != null) {
                add(RowAction("Cancel This Day") { core.change { it.cancelOccurrence(block.series, date) } })
            }
            if (block.changedForThisDay && date != null) {
                add(RowAction("Restore This Day") { core.change { it.restoreOccurrence(block.series, date) } })
            }
            add(RowAction("Delete Block") { ask(DayAsk.Delete(block)) })
        }
        is DayRow.Sitting -> {
            val sitting = row.sitting
            val paused = sitting.status == "paused"
            val timed = { operation: () -> io.github.emassey0135.lumenna.core.Timer ->
                core.attempt(operation)?.let {
                    core.changed()
                    core.report(sentence(it.announcement, it.notices))
                }
            }
            // Start, pause and stop: stopping a running or a paused sitting ends it.
            buildList {
                if (sitting.running) {
                    add(RowAction("Pause Timer") { timed { core.lumenna.pauseTimer(sitting.id) } })
                } else {
                    add(RowAction(if (paused) "Resume Timer" else "Start Timer") { core.change { it.startTimer(sitting.id) } })
                }
                if (sitting.running || paused) {
                    add(RowAction("Stop Timer") { timed { core.lumenna.stopTimer(sitting.id, null) } })
                }
            } + listOf(
                RowAction("Planned Length") { ask(DayAsk.Plan(sitting)) },
                RowAction("Log Minutes") { ask(DayAsk.Log(sitting)) },
                RowAction("Show the Task") { navigator.push(Screen.Task(sitting.task)) },
                RowAction("Unassign") { core.change { it.unassign(sitting.id) } },
            )
        }
        is DayRow.Free -> listOf(
            RowAction("Add Block Here") {
                navigator.push(Screen.BlockForm(BlockPurpose.Add(date = date, at = row.start, minutes = row.minutes)))
            },
        )
        is DayRow.Cancelled -> if (date == null) emptyList() else listOf(
            RowAction("Restore This Day") { core.change { it.restoreOccurrence(row.block.series, date) } },
        )
        is DayRow.Now -> emptyList()
    }
}

/** What the block form is for. */
sealed interface BlockPurpose {
    /** A new block, on [date] at [at] for [minutes]. */
    data class Add(val date: String? = null, val at: String = "09:00", val minutes: UInt = 60u) : BlockPurpose

    /** Every occurrence of a series. */
    data class Series(val id: String) : BlockPurpose

    /** One day of a series, from how that day stands. */
    data class Occurrence(val block: PlanBlock, val date: String) : BlockPurpose
}

/** The fields a new block starts from: the time and length given, and a work block's flags. */
private fun newFields(at: String, minutes: UInt): BlockFields {
    val defaults = blockDefaults("work")
    return BlockFields(
        title = "", start = at, minutes = minutes.toString(), kind = "work",
        acceptsTasks = defaults?.acceptsTasks ?: true, countsCapacity = defaults?.countsCapacity ?: true,
        anchored = defaults?.anchored ?: false, repeat = "", until = "", minMinutes = "", taskFilter = "",
        colour = "", notes = "",
    )
}

/**
 * A block, added or changed. Times and days are typed as they are said —
 * "9am", "14:30", "next monday" — and the core reads them, as on the command line.
 *
 * What a block is made from, and which fields a change sends, are the core's (`newBlock`,
 * `blockEdit`), as for every app: only what changed, so an edit made elsewhere to another field
 * stands. A change of kind brings the kind's flags with it (`blockDefaults`); a flag can then
 * be set apart from it.
 */
@Composable
fun BlockFormScreen(core: Core, navigator: Navigator, purpose: BlockPurpose) {
    val shown = remember(purpose) {
        (purpose as? BlockPurpose.Series)?.let { core.attempt { core.lumenna.showBlock(it.id) } }
    }
    val initial = remember(purpose) {
        when (purpose) {
            is BlockPurpose.Add -> newFields(purpose.at, purpose.minutes)
            is BlockPurpose.Series -> shown?.let { blockFields(it) } ?: newFields("09:00", 60u)
            is BlockPurpose.Occurrence -> dayBlockFields(purpose.block)
        }
    }
    var fields by remember(purpose) { mutableStateOf(initial) }
    var day by remember(purpose) { mutableStateOf((purpose as? BlockPurpose.Add)?.date ?: Clock.today()) }
    val oneDay = purpose is BlockPurpose.Occurrence
    // A rule the date grammar cannot say is shown beside an empty Repeats field, and kept.
    val rule = shown?.rrule?.takeIf { shown.repetition == null }
    val repeats = shown?.repeats == true || (purpose is BlockPurpose.Add && fields.repeat.isNotBlank())
    val title = when (purpose) {
        is BlockPurpose.Add -> "New Block"
        is BlockPurpose.Series -> "Every Occurrence"
        is BlockPurpose.Occurrence -> "${Clock.spokenDay(purpose.date)} Only"
    }
    val kind = { new: String ->
        val defaults = blockDefaults(new)
        fields = fields.copy(
            kind = new,
            acceptsTasks = defaults?.acceptsTasks ?: fields.acceptsTasks,
            countsCapacity = defaults?.countsCapacity ?: fields.countsCapacity,
            anchored = defaults?.anchored ?: fields.anchored,
        )
    }

    val save = save@{
        if (fields.title.isBlank()) {
            core.say("A block needs a name.")
            return@save
        }
        val change = when (purpose) {
            is BlockPurpose.Add -> core.attempt { newBlock(fields, day.trim().ifEmpty { null }) }?.let { block ->
                core.change { it.addBlock(block) }
            }
            is BlockPurpose.Series, is BlockPurpose.Occurrence -> {
                val edit = try {
                    blockEdit(initial, fields)
                } catch (error: LumennaException) {
                    core.say(error.sentence)
                    return@save
                }
                if (edit == null) {
                    core.say("Nothing changed")
                    navigator.back()
                    return@save
                }
                val (id, scope) = when (purpose) {
                    is BlockPurpose.Series -> purpose.id to BlockScope.Series
                    is BlockPurpose.Occurrence -> purpose.block.series to BlockScope.Occurrence(purpose.date)
                    else -> return@save
                }
                core.change { it.editBlock(id, edit, scope) }
            }
        }
        if (change != null) navigator.back()
    }
    Offer(Command.SAVE) { save() }

    ScreenFrame(title, core, navigator, actions = {
        IconButton(onClick = { save() }) { Icon(Icons.Filled.Check, contentDescription = "Save") }
    }) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            BlockField("Name", fields.title, "Deep work") { fields = fields.copy(title = it) }
            if (purpose is BlockPurpose.Add) {
                BlockField("Day", day, "tomorrow", "A date, such as tomorrow or next Monday.") { day = it }
            }
            BlockField("Starts", fields.start, "9am", "Such as 9am or 14:30.") { fields = fields.copy(start = it) }
            BlockField(
                "Lasts, in minutes", fields.minutes, "60",
                fields.minutes.trim().toUIntOrNull()?.let { "Lasts ${Clock.length(it)}." },
                number = true,
            ) { fields = fields.copy(minutes = it) }

            Heading("Kind")
            Column(Modifier.selectableGroup()) {
                listOf("work" to "Work, takes tasks", "break" to "Break", "event" to "Event").forEach { (value, name) ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 48.dp)
                            .selectable(fields.kind == value, role = Role.RadioButton) { kind(value) }
                            .padding(vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = fields.kind == value, onClick = null)
                        Text(name, Modifier.padding(start = 12.dp))
                    }
                }
            }

            if (!oneDay) {
                BlockField(
                    "Repeats", fields.repeat, "every weekday",
                    when {
                        purpose is BlockPurpose.Add -> "Such as every weekday. Empty for a block that happens once."
                        rule != null -> "It repeats by the rule $rule, which this cannot show in words. Empty keeps it; none makes it happen once."
                        initial.repeat.isEmpty() -> "It happens once now. Such as every weekday to make it repeat."
                        else -> "Empty makes it happen once."
                    },
                ) { fields = fields.copy(repeat = it) }
            }

            Heading("What it does")
            Flag("Takes tasks", fields.acceptsTasks) { fields = fields.copy(acceptsTasks = it) }
            Flag("Counts toward hours for work", fields.countsCapacity) { fields = fields.copy(countsCapacity = it) }
            Flag("Anchored, never moved when the day slips", fields.anchored) { fields = fields.copy(anchored = it) }
            Text(
                "The kind sets these; change any of them to set it apart.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
            )

            if (!oneDay) {
                Heading("More")
                if (repeats) {
                    BlockField("Until", fields.until, "31 January", "Its last day. Empty to repeat for good.") {
                        fields = fields.copy(until = it)
                    }
                }
                BlockField(
                    "Shortest length, in minutes", fields.minMinutes, "30",
                    "How far a slipping day may shorten it. Empty for the kind's own.", number = true,
                ) { fields = fields.copy(minMinutes = it) }
                BlockField("Tasks from", fields.taskFilter, "#Work", "A filter for which tasks it is meant for.") {
                    fields = fields.copy(taskFilter = it)
                }
                BlockField("Colour", fields.colour, "teal") { fields = fields.copy(colour = it) }
                BlockField("Notes", fields.notes, "Anything else") { fields = fields.copy(notes = it) }
            }
        }
    }
}

/** A flag of the block, as a switch row: one TalkBack stop that says it and whether it is on. */
@Composable
private fun Flag(name: String, on: Boolean, changed: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .toggleable(on, role = Role.Switch, onValueChange = changed)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(name, Modifier.weight(1f))
        Switch(checked = on, onCheckedChange = null)
    }
}

@Composable
private fun BlockField(
    label: String,
    value: String,
    example: String,
    hint: String? = null,
    number: Boolean = false,
    changed: (String) -> Unit,
) {
    OutlinedTextField(
        value = value,
        onValueChange = changed,
        label = { Text(label) },
        placeholder = { Text(example) },
        supportingText = hint?.let { { Text(it) } },
        keyboardOptions = KeyboardOptions(
            capitalization = if (label == "Name") KeyboardCapitalization.Sentences else KeyboardCapitalization.None,
            keyboardType = if (number) KeyboardType.Number else KeyboardType.Text,
            autoCorrectEnabled = label == "Name",
        ),
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
}
