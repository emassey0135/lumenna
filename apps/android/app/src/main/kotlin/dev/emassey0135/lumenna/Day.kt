package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.ActingOn
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
import androidx.compose.runtime.key
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
import dev.emassey0135.lumenna.app.Action
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.BlockFields
import dev.emassey0135.lumenna.app.DayKind
import dev.emassey0135.lumenna.app.DayMove
import dev.emassey0135.lumenna.app.FieldKind
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ShellEvent

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
        // A reminder's block: its day, with it in hand.
        screen.block?.let { app.send(ShellEvent.ShowBlock(it.series, it.date)) }
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
    val dayActions = day?.let { app.actingOn(ActingOn.Day, it.offered) }.orEmpty()

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
                val edit = item.offered.firstOrNull { it.kind == ActionKind.EDIT }
                ListRow(
                    title = line?.title.orEmpty(),
                    detail = line?.detail.orEmpty(),
                    speech = spoken(line, item.expanded, item.level),
                    index = index,
                    depth = item.depth.toInt(),
                    actions = app.rowActions(item.key, item.offered),
                    keys = app.rowKeys(item.key),
                    focus = focus.requester(item.key),
                    key = item.key,
                    say = core::say,
                    open = when {
                        item.kind == DayKind.BLOCK && edit != null -> ({
                            app.send(ShellEvent.ActOn(ActingOn.Row(item.key), edit))
                        })
                        item.kind == DayKind.SITTING -> item.sitting?.let { sitting -> { navigator.push(Screen.Task(sitting.task)) } }
                        else -> null
                    },
                    openLabel = if (item.kind == DayKind.SITTING) "Show the task" else "Edit",
                    swatch = item.block?.takeIf { item.kind == DayKind.BLOCK }?.swatch,
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
    // Leaving the form gives it up.
    ShowingBlockForm(app, navigator::back)
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
                        // The kind's options, or the colours, each with its swatch.
                        val chosen = fields.text(field.key)
                        Column(Modifier.selectableGroup()) {
                            field.options.forEach { option ->
                                Row(
                                    Modifier
                                        .fillMaxWidth()
                                        .heightIn(min = 48.dp)
                                        .selectable(chosen == option.id, role = Role.RadioButton) { set(field.key, option.id) }
                                        .padding(vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    RadioButton(selected = chosen == option.id, onClick = null)
                                    option.swatch?.let { SwatchDot(it, Modifier.padding(start = 12.dp)) }
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
                        val change = { text: String ->
                            typed[field.key] = text
                            set(field.key, text)
                        }
                        val syntax = field.syntax
                        if (syntax != null) key(field.key) {
                            // Tasks from is the filter language, which completes. The field keeps
                            // its own cursor, taking the app's text only when it differs.
                            var typing by remember { mutableStateOf(androidx.compose.ui.text.input.TextFieldValue(value, androidx.compose.ui.text.TextRange(value.length))) }
                            if (typing.text != value) typing = androidx.compose.ui.text.input.TextFieldValue(value, androidx.compose.ui.text.TextRange(value.length))
                            CompletingField(
                                core = core, syntax = syntax, label = field.label, example = field.example,
                                value = typing,
                                onValueChange = { typing = it; change(it.text) },
                                onSubmit = {},
                            )
                            help?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = quiet()) }
                        } else BlockField(
                            field.label, value, field.example, help,
                            number = minutes,
                            prose = field.key == "title" || field.key == "notes",
                            lines = if (field.kind == FieldKind.LINES) 3 else 1,
                            changed = change,
                        )
                        // A day or a time has Android's own picker beside its line, filling it.
                        when (field.kind) {
                            FieldKind.DATE -> ChooseDay(value, picked = change)
                            FieldKind.TIME -> ChooseTime(value, picked = change)
                            else -> {}
                        }
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
