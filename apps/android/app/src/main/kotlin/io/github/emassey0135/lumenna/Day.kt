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
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import io.github.emassey0135.lumenna.app.Action
import io.github.emassey0135.lumenna.app.ActionKind
import io.github.emassey0135.lumenna.app.Answer
import io.github.emassey0135.lumenna.app.BlockEvent
import io.github.emassey0135.lumenna.app.BlockFields
import io.github.emassey0135.lumenna.app.DayKind
import io.github.emassey0135.lumenna.app.DayMove
import io.github.emassey0135.lumenna.app.FieldKind
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.ShellEvent

/**
 * The planner: a day as it is lived, as a list — the app's day (`ViewModel.day`).
 *
 * Blocks in time order with their sittings beneath them, free time as rows of its own, and now
 * as a position rather than a highlight. The summary above says what a glance at a timeline
 * would. Which day, what is in hand, opening on now, folding, Go to Day and which occurrences a
 * change is to are the app's.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DayScreen(core: Core, navigator: Navigator, screen: Screen.Day) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(screen) {
        app.send(ShellEvent.Open(Place.Today))
        // A day asked for by date: a reminder's block on another day.
        screen.date?.let { date -> app.send(ShellEvent.Day(DayMove.ASK)); app.send(ShellEvent.Answer(Answer.Text(date))) }
    }
    val day = view?.takeIf { it.place == Place.Today }?.day
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { day != null }
    // Each row with its line; the summary is the heading's.
    val rows = day?.items.orEmpty().mapIndexed { index, item -> item to lines?.day?.getOrNull(index) }
    val heading = rows.firstOrNull { it.first.kind == DayKind.SUMMARY }?.second?.title
    val shown = rows.filter { !it.first.hidden && it.first.kind != DayKind.SUMMARY }
    val items = shown.map { it.first }
    val scroll = rememberLazyListState()
    val focus = rememberLanding(core, items.map { it.key }, scroll)

    // Lands on now once a day is opened, so the morning's past blocks are not in the way.
    LaunchedEffect(day?.date) {
        val now = items.indexOfFirst { it.kind == DayKind.NOW }
        if (now >= 0) scroll.scrollToItem(now)
    }

    FollowBlockForm(core, navigator)
    FollowProposal(core, navigator)
    FollowReminders(core, navigator)

    val move = { step: DayMove -> app.send(ShellEvent.Day(step)) }
    val addBlock = { app.send(ShellEvent.Block(BlockEvent.New(null, null))) }

    Offer(Command.PREVIOUS_DAY) { move(DayMove.PREVIOUS) }
    Offer(Command.NEXT_DAY) { move(DayMove.NEXT) }
    Offer(Command.GO_TO_NOW) { move(DayMove.NOW) }
    Offer(Command.GO_TO_DAY) { move(DayMove.ASK) }
    Offer(Command.NEW_BLOCK) { addBlock() }

    // The day's own actions, the app's, in the core's order.
    val dayActions = day?.let { app.rowActions("summary", it.actions) }.orEmpty()

    ScreenFrame(lines?.dayTitle?.ifEmpty { null } ?: "Today", core, navigator, actions = {
        IconButton(onClick = { addBlock() }) {
            Icon(Icons.Filled.Add, contentDescription = "Add block")
        }
    }) {
        val summary = day?.summary.orEmpty()
        Text(
            summary,
            style = MaterialTheme.typography.bodyMedium,
            color = quiet(),
            // The day's heading, as every app says it: "<day>. <summary>". The day is the top
            // bar's to show, beside it.
            modifier = Modifier.padding(horizontal = 16.dp).semantics {
                heading()
                contentDescription = heading ?: summary
                // Where the day is, its actions are: on its heading, as on its buttons below.
                if (dayActions.isNotEmpty()) {
                    customActions = dayActions.map { action -> CustomAccessibilityAction(action.name) { action.run(); true } }
                }
            },
        )
        FlowRow(
            Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedButton(modifier = Target, onClick = { move(DayMove.PREVIOUS) }) { Text("Previous day") }
            OutlinedButton(modifier = Target, onClick = { move(DayMove.NOW) }) { Text("Now") }
            OutlinedButton(modifier = Target, onClick = { move(DayMove.NEXT) }) { Text("Next day") }
            OutlinedButton(modifier = Target, onClick = { move(DayMove.ASK) }) { Text("Go to day") }
            dayActions.forEach { action ->
                OutlinedButton(modifier = Target, onClick = action.run) {
                    Text(action.name, color = if (action.destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                }
            }
        }
        LazyColumn(
            Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(items.size, 1) },
            state = scroll,
        ) {
            itemsIndexed(shown, key = { _, (item, _) -> item.key }) { index, (item, line) ->
                val edit = item.actions.firstOrNull { it.kind == ActionKind.EDIT }
                ListRow(
                    title = line?.title.orEmpty(),
                    detail = line?.detail.orEmpty(),
                    speech = spoken(line, item.expanded, item.level),
                    index = index,
                    depth = item.depth.toInt(),
                    actions = app.rowActions(item.key, item.actions),
                    focus = focus.requester(item.key),
                    key = item.key,
                    say = core::say,
                    open = when {
                        item.kind == DayKind.BLOCK && edit != null -> ({
                            app.send(ShellEvent.Focus(item.key))
                            app.send(ShellEvent.Perform(edit))
                        })
                        item.kind == DayKind.SITTING -> item.sitting?.let { sitting -> { navigator.push(Screen.Task(sitting.task)) } }
                        else -> null
                    },
                    openLabel = if (item.kind == DayKind.SITTING) "Show the task" else "Edit",
                )
                HorizontalDivider()
            }
        }
    }
}

/** The block form is the app's: shown over [navigator]'s screen while the app has one open. */
@Composable
fun FollowBlockForm(core: Core, navigator: Navigator) {
    val view by core.app.view.collectAsState()
    val open = view?.blockForm != null
    LaunchedEffect(open) { if (open && navigator.current !is Screen.BlockForm) navigator.push(Screen.BlockForm) }
}

/**
 * A block, added or changed: the app's block form (`ViewModel.block_form`), open from the day
 * until the app closes it. Times and days are typed as they are said — "9am", "14:30", "next
 * monday" — and the core reads them. Which fields it shows, what a new block starts from, the
 * flags a kind brings, and which fields a change sends are the app's.
 */
@Composable
fun BlockFormScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val form = view?.blockForm
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(form == null) { if (form != null) shown = true else if (shown) navigator.back() }
    // Leaving the form gives it up.
    DisposableEffect(Unit) { onDispose { if (app.view.value?.blockForm != null) app.send(ShellEvent.Block(BlockEvent.Cancel)) } }
    // Each line keeps its own text while typing is on its way.
    val typed = remember { mutableStateMapOf<String, String>() }
    val set = { key: String, value: String -> app.send(ShellEvent.Block(BlockEvent.Field(key, value))) }
    val save = { app.send(ShellEvent.Block(BlockEvent.Save)) }
    Offer(Command.SAVE) { save() }

    ScreenFrame(form?.heading ?: "Block", core, navigator, actions = {
        IconButton(onClick = { save() }) { Icon(Icons.Filled.Check, contentDescription = form?.yes ?: "Save") }
    }) {
        if (form == null) return@ScreenFrame
        val fields = form.values
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            form.refusal?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 8.dp).semantics { liveRegion = LiveRegionMode.Polite }) }
            // The app's fields, in the core's order: the kind a choice among its options, the
            // flags switches, the rest lines of text the core reads.
            form.fields.forEach { field ->
                when (field.kind) {
                    FieldKind.CHOICE -> {
                        Heading(field.label)
                        Column(Modifier.selectableGroup()) {
                            field.options.forEach { option ->
                                Row(
                                    Modifier
                                        .fillMaxWidth()
                                        .heightIn(min = 48.dp)
                                        .selectable(fields.kind == option.id, role = Role.RadioButton) { set(field.key, option.id) }
                                        .padding(vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    RadioButton(selected = fields.kind == option.id, onClick = null)
                                    Text(option.title, Modifier.padding(start = 12.dp))
                                }
                            }
                        }
                        field.hint.ifEmpty { null }?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = quiet()) }
                    }
                    FieldKind.TOGGLE -> Flag(field.label, fields.flag(field.key)) { set(field.key, it.toString()) }
                    else -> {
                        val minutes = field.kind == FieldKind.MINUTES
                        val value = typed[field.key] ?: if (field.key == "date") form.date else fields.text(field.key)
                        // The length is read back as it is typed.
                        val help = if (field.key == "minutes") {
                            value.trim().toUIntOrNull()?.let { "Lasts ${Clock.length(it)}." } ?: field.hint.ifEmpty { null }
                        } else {
                            field.hint.ifEmpty { null }
                        }
                        BlockField(
                            field.label, value, field.example, help,
                            number = minutes,
                            prose = field.key == "title" || field.key == "notes",
                            lines = if (field.kind == FieldKind.LINES) 3 else 1,
                        ) {
                            typed[field.key] = it
                            set(field.key, it)
                        }
                    }
                }
            }
        }
    }
}

/** A block form field's text, by the core's key; a toggle or the kind reads empty here. */
private fun BlockFields.text(key: String): String = when (key) {
    "title" -> title
    "start" -> start
    "minutes" -> minutes
    "repeat" -> repeat
    "until" -> until
    "min_minutes" -> minMinutes
    "task_filter" -> taskFilter
    "colour" -> colour
    "notes" -> notes
    else -> ""
}

/** A block form toggle, by the core's key. */
private fun BlockFields.flag(key: String): Boolean = when (key) {
    "accepts_tasks" -> acceptsTasks
    "counts_capacity" -> countsCapacity
    "anchored" -> anchored
    else -> false
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
