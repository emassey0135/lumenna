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
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Completing
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.TaskField
import io.github.emassey0135.lumenna.blockFormFields
import io.github.emassey0135.lumenna.flag
import io.github.emassey0135.lumenna.help
import io.github.emassey0135.lumenna.name
import io.github.emassey0135.lumenna.taskFormFields
import io.github.emassey0135.lumenna.text
import io.github.emassey0135.lumenna.withFlag
import io.github.emassey0135.lumenna.withText
import io.github.emassey0135.lumenna.core.FieldKind
import io.github.emassey0135.lumenna.perform
import io.github.emassey0135.lumenna.core.ActionKind
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
        item { Heading("New task") }
        item { TextFieldButton("Task", line, "call the bank tomorrow") { line = it } }
        if (line.isNotBlank()) {
            item {
                Button(onClick = { entry?.ask("Add to the line") { more -> line = "${line.trimEnd()} ${more.trim()}" } }, modifier = Modifier.fillMaxWidth(), label = { Text("Add to the line") })
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
        // The core's fields, in its order, as the phone has them: the priority a choice among
        // the core's options, the rest lines from the system's input screen.
        taskFormFields.forEach { field ->
            val text = TaskField.of(field)
            if (text != null) {
                item { TextFieldButton(field.label, text.get(form), field.example) { fields = text.set(form, it) } }
                return@forEach
            }
            item { Heading(field.label) }
            field.options.forEach { option ->
                item {
                    RadioButton(
                        selected = form.priority == option.id.toUByte(),
                        onSelect = { fields = form.copy(priority = option.id.toUByte()) },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(option.title) },
                    )
                }
            }
        }
        item { Button(onClick = { TaskField.save(core, current, form) }, modifier = Modifier.fillMaxWidth(), label = { Text("Save") }) }

        if (current.depends.isNotEmpty()) {
            item { Heading("Waits for") }
            current.depends.forEach { other -> item { Text(other.title) } }
        }

        item { Heading("About") }
        if (current.repetition == null && current.recurrence != null) item { Text("Repeats by the rule ${current.recurrence}") }
        item { Text("State: ${current.state.joinToString(", ")}") }

        // The core's, in its order: this screen is the task's form, so no Edit Details.
        item { Heading("Actions") }
        current.actions.forEach { action ->
            item {
                val entry = LocalTextEntry.current
                Button(onClick = {
                    core.perform(action, WearAsker(navigator, entry), form = {}) {
                        if (action.kind == ActionKind.DELETE) navigator.back()
                    }
                }, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) })
            }
        }
    }
}

/** A length in minutes, from the lengths a sitting usually takes, or, given [without], none. */
fun chooseLength(navigator: Navigator, title: String, without: String?, chosen: (UInt?) -> Unit) {
    val lengths = listOf(15, 25, 30, 45, 60, 90, 120).map { Option(it.toString(), Clock.length(it.toUInt())) }
    navigator.choose(title, lengths + listOfNotNull(without?.let { Option("", it) })) { choice -> chosen(choice.key.toUIntOrNull()) }
}

/** A block, added or changed, in the form the phone has (`BlockFormModel`). */
@Composable
fun BlockFormScreen(core: Core, navigator: Navigator, purpose: BlockPurpose) {
    val model = remember(purpose) { BlockFormModel(core, purpose) }
    val fields = model.fields
    WearList {
        item { Heading(model.title) }
        // The core's fields, in its order (`blockForm`), as the phone has them.
        blockFormFields.filter(model::shows).forEach { field ->
            when (field.kind) {
                FieldKind.CHOICE -> {
                    item { Heading(field.label) }
                    field.options.forEach { option ->
                        item {
                            RadioButton(
                                selected = fields.kind == option.id,
                                onSelect = { model.kind(option.id) },
                                modifier = Modifier.fillMaxWidth(),
                                label = { Text(option.title) },
                            )
                        }
                    }
                    field.help?.let { item { Text(it) } }
                }
                FieldKind.TOGGLE -> item {
                    Flag(field.label, fields.flag(field.key)) { model.fields = model.fields.withFlag(field.key, it) }
                }
                else -> {
                    item {
                        // A new block's day is the model's; the length is said in words beside it.
                        val value = when (field.key) {
                            "date" -> model.day
                            "minutes" -> fields.minutes.trim().toUIntOrNull()?.let { "${fields.minutes}, ${Clock.length(it)}" } ?: fields.minutes
                            else -> fields.text(field.key)
                        }
                        TextFieldButton(field.label, value, field.example) {
                            if (field.key == "date") model.day = it else model.fields = model.fields.withText(field.key, if (field.key == "minutes") it.trim() else it)
                        }
                    }
                    if (field.key == "repeat") model.help(field)?.let { item { Text(it) } }
                }
            }
        }
        item { Button(onClick = { if (model.save()) navigator.back() }, modifier = Modifier.fillMaxWidth(), label = { Text("Save") }) }
    }
}

/** A flag of the block, as a switch: one TalkBack stop that says it and whether it is on. */
@Composable
private fun Flag(name: String, on: Boolean, changed: (Boolean) -> Unit) {
    SwitchButton(checked = on, onCheckedChange = changed, modifier = Modifier.fillMaxWidth(), label = { Text(name) })
}
