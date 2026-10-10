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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.core.BlockFields
import io.github.emassey0135.lumenna.core.BlockScope
import io.github.emassey0135.lumenna.core.FieldKind
import io.github.emassey0135.lumenna.core.Question
import io.github.emassey0135.lumenna.core.goToDayQuestion
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

/** What the day screen is asking. */
private sealed interface DayAsk {
    data class Which(val block: PlanBlock) : DayAsk
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
    val prompt = rememberPrompter()
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
        val summary = plan.fold({ it.summary }, { (it as? io.github.emassey0135.lumenna.core.LumennaException)?.sentence.orEmpty() })
        Text(
            summary,
            style = MaterialTheme.typography.bodyMedium,
            color = quiet(),
            // The day's heading, as every app says it: "<day>. <summary>". The day is the top
            // bar's to show, beside it.
            modifier = Modifier.padding(horizontal = 16.dp).semantics {
                heading()
                contentDescription = listOfNotNull(shown?.let { Clock.spokenDay(it.date) }, summary.ifEmpty { null })
                    .joinToString(". ")
            },
        )
        FlowRow(
            Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedButton(modifier = Target, onClick = { step(-1) }) { Text("Previous day") }
            OutlinedButton(modifier = Target, onClick = { day = null }) { Text("Now") }
            OutlinedButton(modifier = Target, onClick = { step(1) }) { Text("Next day") }
            OutlinedButton(modifier = Target, onClick = { asking = DayAsk.GoTo }) { Text("Go to day") }
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
                    actions = focus.actions(row.key, index, actions(core, navigator, prompt, row, edit)) +
                        listOfNotNull(foldAction(core, folds[index], row.key, folding)),
                    focus = focus.requester(row.key),
                    key = row.key,
                    say = core::say,
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
                        Column(horizontalAlignment = Alignment.End) {
                            TextButton(modifier = Target, onClick = {
                                asking = null
                                navigator.push(
                                    Screen.BlockForm(
                                        BlockPurpose.Occurrence(block, date),
                                    ),
                                )
                            }) { Text("${Clock.spokenDay(date)} only") }
                            TextButton(modifier = Target, onClick = {
                                asking = null
                                navigator.push(Screen.BlockForm(BlockPurpose.Series(block.series)))
                            }) { Text("Every occurrence") }
                            TextButton(modifier = Target, onClick = dismiss) { Text("Cancel") }
                        }
                    },
                )
            }
        }
        DayAsk.GoTo -> {
            // The core's question, as every app asks it.
            val q = goToDayQuestion() as Question.Text
            AskText(q.said, q.label, q.yes, example = "next friday", hint = q.hint.ifEmpty { null }, dismiss = dismiss) { text ->
                core.attempt { core.lumenna.plan(text) }?.let {
                    asking = null
                    day = it.date
                }
            }
        }
        null -> {}
    }
    prompt.Host()
}

/**
 * What can be done to a day row, as custom actions and on a long press: the core's, in its
 * order. Only its forms are this screen's to open: a block's, the task a sitting is for, and a
 * new block in free time.
 */
private fun actions(
    core: Core,
    navigator: Navigator,
    prompt: Prompter,
    row: DayRow,
    edit: (PlanBlock) -> Unit,
): List<RowAction> = core.offered(row.actions, prompt, form = { action ->
    when (action.kind) {
        ActionKind.EDIT -> if (row is DayRow.Block) edit(row.block)
        ActionKind.EDIT_TASK -> navigator.push(Screen.Task(action.target))
        ActionKind.ADD_BLOCK -> if (row is DayRow.Free) {
            navigator.push(Screen.BlockForm(BlockPurpose.Add(date = action.target, at = action.other ?: row.start, minutes = row.minutes)))
        }
        else -> {}
    }
})

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
    // What it edits and how it saves are shared with the watch (`BlockFormModel`).
    val model = remember(purpose) { BlockFormModel(core, purpose) }
    var fields by model::fields
    var day by model::day
    val title = model.title
    val kind = model::kind
    val save = { if (model.save()) navigator.back() }
    Offer(Command.SAVE) { save() }

    ScreenFrame(title, core, navigator, actions = {
        IconButton(onClick = { save() }) { Icon(Icons.Filled.Check, contentDescription = "Save") }
    }) {
        // The core's fields, in its order (`blockForm`): the kind a choice among its options,
        // the flags switches, the rest lines of text the core reads.
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            blockFormFields.filter(model::shows).forEach { field ->
                when (field.kind) {
                    FieldKind.CHOICE -> {
                        Heading(field.label)
                        Column(Modifier.selectableGroup()) {
                            field.options.forEach { option ->
                                Row(
                                    Modifier
                                        .fillMaxWidth()
                                        .heightIn(min = 48.dp)
                                        .selectable(fields.kind == option.id, role = Role.RadioButton) { kind(option.id) }
                                        .padding(vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    RadioButton(selected = fields.kind == option.id, onClick = null)
                                    Text(option.title, Modifier.padding(start = 12.dp))
                                }
                            }
                        }
                        field.help?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = quiet()) }
                    }
                    FieldKind.TOGGLE -> Flag(field.label, fields.flag(field.key)) { fields = fields.withFlag(field.key, it) }
                    else -> {
                        val minutes = field.kind == FieldKind.MINUTES
                        // A new block's day is the model's; the length is read back as it is typed.
                        val value = if (field.key == "date") day else fields.text(field.key)
                        val help = if (field.key == "minutes") {
                            fields.minutes.trim().toUIntOrNull()?.let { "Lasts ${Clock.length(it)}." } ?: model.help(field)
                        } else {
                            model.help(field)
                        }
                        BlockField(
                            field.label, value, field.example, help,
                            number = minutes,
                            prose = field.key == "title" || field.key == "notes",
                            lines = if (field.kind == FieldKind.LINES) 3 else 1,
                        ) { if (field.key == "date") day = it else fields = fields.withText(field.key, it) }
                    }
                }
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
    prose: Boolean = false,
    lines: Int = 1,
    changed: (String) -> Unit,
) {
    OutlinedTextField(
        value = value,
        onValueChange = changed,
        label = { Text(label) },
        placeholder = example.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
        supportingText = hint?.let { { Text(it) } },
        minLines = lines,
        keyboardOptions = KeyboardOptions(
            capitalization = if (prose) KeyboardCapitalization.Sentences else KeyboardCapitalization.None,
            keyboardType = if (number) KeyboardType.Number else KeyboardType.Text,
            autoCorrectEnabled = prose,
        ),
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
}
