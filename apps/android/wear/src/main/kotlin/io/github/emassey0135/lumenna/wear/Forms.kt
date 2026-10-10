package io.github.emassey0135.lumenna.wear

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.RadioButton
import androidx.wear.compose.material3.SwitchButton
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.BlockFormModel
import io.github.emassey0135.lumenna.BlockPurpose
import io.github.emassey0135.lumenna.Choice
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Completing
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.TaskField
import io.github.emassey0135.lumenna.blockChoices
import io.github.emassey0135.lumenna.taskChoices
import io.github.emassey0135.lumenna.core.MoveTarget
import io.github.emassey0135.lumenna.core.Syntax
import io.github.emassey0135.lumenna.core.TaskDetail
import io.github.emassey0135.lumenna.core.TaskFields
import io.github.emassey0135.lumenna.core.taskEdit
import io.github.emassey0135.lumenna.core.taskFields

/**
 * A field the system's input screen fills: its name, then what it holds, one TalkBack stop.
 * Pressing it asks for a new value; on a watch there is no text field to type into.
 */
@Composable
fun TextFieldButton(name: String, value: String, example: String = "", ask: (String) -> Unit) {
    val entry = LocalTextEntry.current
    Button(
        onClick = { entry?.ask(name) { ask(it) } },
        modifier = Modifier.fillMaxWidth(),
        label = { Text(name) },
        secondaryLabel = { Text(value.ifEmpty { example.takeIf { it.isNotEmpty() }?.let { "For example, $it" } ?: "Empty" }) },
    )
}

/**
 * Quick add: one line, read back by the core before anything is saved, as on every app.
 * The input screen replaces a whole line, so the line is changed or added to; what could
 * finish its last word is offered beneath it.
 */
@Composable
fun AddTaskScreen(core: Core, navigator: Navigator, prefix: String) {
    var line by remember { mutableStateOf(prefix) }
    val entry = LocalTextEntry.current
    // Straight to the input screen, as the phone's quick add opens with the keyboard up.
    LaunchedEffect(Unit) { if (line.isBlank()) entry?.ask("New task") { line = it } }
    val readback = remember(line) { if (line.isBlank()) null else core.attempt { core.lumenna.previewTask(line) } }
    val offered = remember(line) { Completing.offered(core, line, Syntax.QUICK_ADD) }
    WearList {
        item { Heading("New Task") }
        item { TextFieldButton("Task", line, "call the bank tomorrow") { line = it } }
        if (line.isNotBlank()) {
            item {
                Button(onClick = { entry?.ask("Add to the line") { more -> line = "${line.trimEnd()} ${more.trim()}" } }, modifier = Modifier.fillMaxWidth(), label = { Text("Add to the Line") })
            }
        }
        offered?.let { (candidates, span) ->
            candidates.take(8).forEach { candidate ->
                item {
                    Button(
                        onClick = { line = Completing.insert(line, span, candidate) },
                        // Named as the core names it: "project Work", not "#Work".
                        modifier = Modifier.fillMaxWidth().semantics { contentDescription = "Complete with ${candidate.label}" },
                        label = { Text(candidate.text) },
                    )
                }
            }
        }
        // What will be saved, before anything is: a misheard date is caught here.
        readback?.let { preview ->
            item { Text(preview.announcement.replaceFirstChar { it.uppercase() }) }
            preview.notices.forEach { notice -> item { Text(notice) } }
        }
        item {
            Button(
                onClick = { if (core.change { it.addTask(line) } != null) navigator.back() },
                enabled = line.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Add") },
            )
        }
    }
}

/**
 * One task, edited as the phone edits it (`TaskField`): every field, saved as only what
 * changed, then what can be done to it.
 */
@Composable
fun TaskScreen(core: Core, navigator: Navigator, id: String, changes: Long) {
    var task by remember(id) { mutableStateOf<TaskDetail?>(null) }
    var fields by remember(id) { mutableStateOf<TaskFields?>(null) }
    LaunchedEffect(id, changes) {
        val shown = runCatching { core.lumenna.showTask(id).task }.getOrNull()
        // Another device's change follows unless this one is part way through editing.
        val editing = task?.let { before -> fields?.let { taskEdit(before, it) } } != null
        if (!editing || shown == null) {
            task = shown
            fields = shown?.let { taskFields(it) }
        }
    }
    val current = task
    val form = fields
    WearList {
        if (current == null || form == null) {
            item { Text("This task is no longer here.") }
            return@WearList
        }
        item { Heading(current.title) }
        TaskField.beforePriority.forEach { field ->
            item { TextFieldButton(field.title, field.get(form), field.example) { fields = field.set(form, it) } }
        }
        item { Heading("Priority") }
        TaskField.priorities.forEach { (level, name) ->
            item {
                RadioButton(
                    selected = form.priority.toInt() == level,
                    onSelect = { fields = form.copy(priority = level.toUByte()) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(name) },
                )
            }
        }
        item { TextFieldButton(TaskField.NOTES.title, form.notes, TaskField.NOTES.example) { fields = form.copy(notes = it) } }
        item { Button(onClick = { TaskField.save(core, current, form) }, modifier = Modifier.fillMaxWidth(), label = { Text("Save") }) }

        item { Heading("Waits for") }
        current.depends.forEach { other ->
            item {
                Button(onClick = { core.change { it.removeDependency(current.id, other.id) } }, modifier = Modifier.fillMaxWidth(), label = { Text("Stop Waiting for ${other.title}") })
            }
        }
        item {
            Button(onClick = {
                val excluded = current.depends.map { it.id }.toSet() + current.id
                navigator.choose("Waits For", taskChoices(core, excluded)) { other -> core.change { it.addDependency(current.id, other.key) } }
            }, modifier = Modifier.fillMaxWidth(), label = { Text("Add Something It Waits For") })
        }

        item { Heading("About") }
        if (current.repetition == null && current.recurrence != null) item { Text("Repeats by the rule ${current.recurrence}") }
        item { Text("State: ${current.state.joinToString(", ")}") }

        item { Heading("Actions") }
        val done = "completed" in current.state
        item {
            Button(onClick = { core.change { if (done) it.uncompleteTask(current.id) else it.completeTask(current.id) } }, modifier = Modifier.fillMaxWidth(), label = { Text(if (done) "Mark Not Done" else "Mark Done") })
        }
        item {
            Button(onClick = {
                val blocks = blockChoices(core)
                navigator.choose("Put ${current.title} in a Block", blocks.map { it.first }, if (blocks.isEmpty()) "There are no work blocks this week. Add one from Today." else null) { block ->
                    val date = blocks.first { it.first.key == block.key }.second
                    chooseLength(navigator, "How long is this sitting meant to take?", "No Planned Length") { minutes ->
                        core.change { it.assign(current.id, block.key, date, minutes) }
                    }
                }
            }, modifier = Modifier.fillMaxWidth(), label = { Text("Put in a Block") })
        }
        item {
            Button(onClick = {
                navigator.choose("Make Subtask Of", taskChoices(core, setOf(current.id))) { parent ->
                    core.change { it.moveTask(current.id, MoveTarget.Parent(parent.key)) }
                }
            }, modifier = Modifier.fillMaxWidth(), label = { Text("Make Subtask Of") })
        }
        if (current.parent != null) {
            item { Button(onClick = { core.change { it.moveTask(current.id, MoveTarget.Top) } }, modifier = Modifier.fillMaxWidth(), label = { Text("Move to Top Level") }) }
        }
        item {
            Button(onClick = { core.change { it.trashTask(current.id) }?.let { navigator.back() } }, modifier = Modifier.fillMaxWidth(), label = { Text("Move to Trash") })
        }
    }
}

/** A length in minutes, from the lengths a sitting usually takes, or none. */
fun chooseLength(navigator: Navigator, title: String, without: String, chosen: (UInt?) -> Unit) {
    val lengths = listOf(15, 25, 30, 45, 60, 90, 120).map { Choice(it.toString(), Clock.length(it.toUInt())) }
    navigator.choose(title, lengths + Choice("", without)) { choice -> chosen(choice.key.toUIntOrNull()) }
}

/** A block, added or changed, in the form the phone has (`BlockFormModel`). */
@Composable
fun BlockFormScreen(core: Core, navigator: Navigator, purpose: BlockPurpose) {
    val model = remember(purpose) { BlockFormModel(core, purpose) }
    val fields = model.fields
    WearList {
        item { Heading(model.title) }
        item { TextFieldButton("Name", fields.title, "Deep work") { model.fields = model.fields.copy(title = it) } }
        if (purpose is BlockPurpose.Add) {
            item { TextFieldButton("Day", model.day, "tomorrow") { model.day = it } }
        }
        item { TextFieldButton("Starts", fields.start, "9am") { model.fields = model.fields.copy(start = it) } }
        item {
            TextFieldButton(
                "Lasts, in minutes",
                fields.minutes.trim().toUIntOrNull()?.let { "${fields.minutes}, ${Clock.length(it)}" } ?: fields.minutes,
                "60",
            ) { model.fields = model.fields.copy(minutes = it.trim()) }
        }
        item { Heading("Kind") }
        BlockFormModel.kinds.forEach { (value, name) ->
            item {
                RadioButton(selected = fields.kind == value, onSelect = { model.kind(value) }, modifier = Modifier.fillMaxWidth(), label = { Text(name) })
            }
        }
        if (!model.oneDay) {
            item { TextFieldButton("Repeats", fields.repeat, "every weekday") { model.fields = model.fields.copy(repeat = it) } }
            item { Text(model.repeatsHelp) }
        }
        item { Heading("What it does") }
        item { Flag("Takes tasks", fields.acceptsTasks) { model.fields = model.fields.copy(acceptsTasks = it) } }
        item { Flag("Counts toward hours for work", fields.countsCapacity) { model.fields = model.fields.copy(countsCapacity = it) } }
        item { Flag("Anchored, never moved when the day slips", fields.anchored) { model.fields = model.fields.copy(anchored = it) } }
        item { Text(BlockFormModel.FLAGS_HELP) }
        if (!model.oneDay) {
            item { Heading("More") }
            if (model.repeats) item { TextFieldButton("Until", fields.until, "31 January") { model.fields = model.fields.copy(until = it) } }
            item { TextFieldButton("Shortest length, in minutes", fields.minMinutes, "30") { model.fields = model.fields.copy(minMinutes = it) } }
            item { TextFieldButton("Tasks from", fields.taskFilter, "#Work") { model.fields = model.fields.copy(taskFilter = it) } }
            item { TextFieldButton("Colour", fields.colour, "teal") { model.fields = model.fields.copy(colour = it) } }
            item { TextFieldButton("Notes", fields.notes, "Anything else") { model.fields = model.fields.copy(notes = it) } }
        }
        item { Button(onClick = { if (model.save()) navigator.back() }, modifier = Modifier.fillMaxWidth(), label = { Text("Save") }) }
    }
}

/** A flag of the block, as a switch: one TalkBack stop that says it and whether it is on. */
@Composable
private fun Flag(name: String, on: Boolean, changed: (Boolean) -> Unit) {
    SwitchButton(checked = on, onCheckedChange = changed, modifier = Modifier.fillMaxWidth(), label = { Text(name) })
}
