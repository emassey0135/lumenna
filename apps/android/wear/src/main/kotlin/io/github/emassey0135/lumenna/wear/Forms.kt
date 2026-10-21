package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.ShowingBlockForm
import io.github.emassey0135.lumenna.text
import io.github.emassey0135.lumenna.flag
import io.github.emassey0135.lumenna.option
import io.github.emassey0135.lumenna.TextOffsets
import io.github.emassey0135.lumenna.app.Typing
import io.github.emassey0135.lumenna.app.TaskFields
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.FieldKind
import io.github.emassey0135.lumenna.app.DetailsEvent
import io.github.emassey0135.lumenna.app.BlockFields
import io.github.emassey0135.lumenna.app.BlockEvent
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
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
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.name

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
 * Quick add: the app's line, read back by the core before anything is saved, as on every app.
 * The input screen replaces a whole line, so the line is changed or added to; what could
 * finish its last word is offered beneath it, and put in where the app says.
 */
@Composable
fun AddTaskScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val line = view?.line
    val lines by app.lines.collectAsState()
    val entry = LocalTextEntry.current
    var opened by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { app.send(ShellEvent.Typing(Typing.Open)) }
    // Added, the app lets the line go, and the screen goes back.
    LaunchedEffect(line == null) { if (line != null) opened = true else if (opened) navigator.back() }
    DisposableEffect(Unit) { onDispose { if (app.view.value?.line != null) app.send(ShellEvent.Typing(Typing.Close)) } }
    val text = line?.text.orEmpty()
    val set = { value: String -> app.send(ShellEvent.Typing(Typing.Text(value, TextOffsets.bytes(value.length, value)))) }
    // Straight to the input screen, as the phone's quick add opens with the keyboard up.
    LaunchedEffect(opened) { if (opened && text.isBlank()) entry?.ask("New task") { set(it) } }
    WearList {
        item { Heading("New task") }
        item { TextFieldButton("Task", text, "call the bank tomorrow") { set(it) } }
        if (text.isNotBlank()) {
            item {
                Button(onClick = { entry?.ask("Add to the line") { more -> set("${text.trimEnd()} ${more.trim()}") } }, modifier = Modifier.fillMaxWidth(), label = { Text("Add to the line") })
            }
        }
        line?.offer?.candidates.orEmpty().take(8).forEachIndexed { index, candidate ->
            item {
                Button(
                    onClick = { app.send(ShellEvent.Typing(Typing.Choose(index.toUInt()))) },
                    // Named as the core says it, name first: "Work, project", not "#Work".
                    modifier = Modifier.fillMaxWidth().semantics {
                        contentDescription = "Complete with ${lines?.candidates?.getOrNull(index) ?: candidate.label}"
                    },
                    label = { Text(candidate.text) },
                )
            }
        }
        // What will be saved, before anything is: a misheard date is caught here.
        line?.readback?.takeIf { it.isNotEmpty() }?.let { readback ->
            item { Text(readback.replaceFirstChar { it.uppercase() }) }
        }
        item {
            Button(
                onClick = { app.send(ShellEvent.QuickAdd(text)) },
                enabled = text.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Add") },
            )
        }
    }
}

/**
 * One task, the app's details, edited as the phone edits it: every field, through the system's
 * input screen, saved by the app as only what changed, then what can be done to it.
 */
@Composable
fun TaskScreen(core: Core, navigator: Navigator, screen: Screen.Task) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(screen.id) { app.send(ShellEvent.Details(DetailsEvent.Show(screen.id))) }
    val details = view?.details?.takeIf { it.task.id == screen.id }
    var shown by remember(screen.id) { mutableStateOf(false) }
    // Gone from under it — deleted here or elsewhere — the screen goes back.
    LaunchedEffect(details == null) { if (details != null) shown = true else if (shown) navigator.back() }
    val set = { key: String, value: String -> app.send(ShellEvent.Details(DetailsEvent.Field(key, value))) }
    WearList {
        if (details == null) {
            if (shown) item { Text("This task is no longer here.") }
            return@WearList
        }
        val current = details.task
        val form = details.fields
        item { Heading(current.title) }
        // The core's fields, in its order, as the phone has them: the priority a choice among
        // the core's options, the project one of the projects, the rest lines from the
        // system's input screen.
        details.form.forEach { field ->
            if (field.key == "project") {
                // A list to choose from, as the watch's other picks are: the projects in tree
                // order, each saying its level where it changes.
                item {
                    Button(
                        onClick = { navigator.choose(field.label, details.projects.map(::option)) { set("project", it.key) } },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(field.label) },
                        secondaryLabel = { Text(form.project) },
                    )
                }
                return@forEach
            }
            val text = form.text(field.key)
            if (text != null) {
                item { TextFieldButton(field.label, text, field.example) { set(field.key, it) } }
                return@forEach
            }
            item { Heading(field.label) }
            field.options.forEach { option ->
                item {
                    RadioButton(
                        selected = form.priority == option.id.toUByte(),
                        onSelect = { set("priority", option.id) },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(option.title) },
                    )
                }
            }
        }
        item { Button(onClick = { app.send(ShellEvent.Details(DetailsEvent.Save)) }, modifier = Modifier.fillMaxWidth(), label = { Text("Save") }) }

        if (current.depends.isNotEmpty()) {
            item { Heading("Waits for") }
            current.depends.forEach { other -> item { Text(other.title) } }
        }

        item { Heading("About") }
        if (current.repetition == null && current.recurrence != null) item { Text("Repeats by the rule ${current.recurrence}") }
        item { Text("State: ${current.state.joinToString(", ")}") }

        // What the records hold of it, the core's lines: on its own screen only, never a list.
        if (current.history.isNotEmpty()) {
            item { Heading("History") }
            current.history.forEach { line -> item { Text(line) } }
        }

        // The app's, in the core's order: this screen is the task's form, so no Edit Details.
        item { Heading("Actions") }
        current.actions.forEach { action ->
            item { Button(onClick = { app.send(ShellEvent.Perform(action)) }, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) }) }
        }
    }
}

/** A length in minutes, from the lengths a sitting usually takes, or, given [without], none. */
fun chooseLength(navigator: Navigator, title: String, without: String?, cancelled: () -> Unit = {}, chosen: (UInt?) -> Unit) {
    val lengths = listOf(15, 25, 30, 45, 60, 90, 120).map { Option(it.toString(), Clock.length(it.toUInt())) }
    navigator.choose(title, lengths + listOfNotNull(without?.let { Option("", it) }), cancelled = cancelled) { choice -> chosen(choice.key.toUIntOrNull()) }
}

/** A block, added or changed: the app's block form, as the phone has it. */
@Composable
fun BlockFormScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val form = view?.blockForm
    ShowingBlockForm(app, navigator::back)
    val set = { key: String, value: String -> app.send(ShellEvent.Block(BlockEvent.Field(key, value))) }
    WearList {
        if (form == null) return@WearList
        val fields = form.values
        item { Heading(form.heading) }
        form.refusal?.let { item { Text(it) } }
        // The app's fields, in the core's order, as the phone has them.
        form.fields.forEach { field ->
            when (field.kind) {
                FieldKind.CHOICE -> {
                    item { Heading(field.label) }
                    // The kind's options, or the colours, each with its swatch.
                    val chosen = fields.text(field.key)
                    field.options.forEach { option ->
                        item {
                            RadioButton(
                                selected = chosen == option.id,
                                onSelect = { set(field.key, option.id) },
                                modifier = Modifier.fillMaxWidth(),
                                icon = option.swatch?.let { { SwatchDot(it) } },
                                label = { Text(option.title) },
                            )
                        }
                    }
                    field.hint.ifEmpty { null }?.let { item { Text(it) } }
                }
                FieldKind.TOGGLE -> item { Flag(field.label, fields.flag(field.key)) { set(field.key, it.toString()) } }
                else -> {
                    item {
                        // A new block's day is the form's; the length is said in words beside it.
                        val value = when (field.key) {
                            "date" -> form.date
                            "minutes" -> fields.minutes.trim().toUIntOrNull()?.let { "${fields.minutes}, ${Clock.length(it)}" } ?: fields.minutes
                            else -> fields.text(field.key)
                        }
                        TextFieldButton(field.label, value, field.example) { set(field.key, if (field.key == "minutes") it.trim() else it) }
                    }
                    // A day or a time has the watch's own picker beside its line, filling it.
                    when (field.kind) {
                        FieldKind.DATE -> item { ChooseDay(form.date) { set(field.key, it) } }
                        FieldKind.TIME -> item { ChooseTime(fields.text(field.key)) { set(field.key, it) } }
                        else -> {}
                    }
                    if (field.key == "repeat") field.hint.ifEmpty { null }?.let { item { Text(it) } }
                }
            }
        }
        item { Button(onClick = { app.send(ShellEvent.Block(BlockEvent.Save)) }, modifier = Modifier.fillMaxWidth(), label = { Text(form.yes) }) }
    }
}

/** A flag of the block, as a switch: one TalkBack stop that says it and whether it is on. */
@Composable
private fun Flag(name: String, on: Boolean, changed: (Boolean) -> Unit) {
    SwitchButton(checked = on, onCheckedChange = changed, modifier = Modifier.fillMaxWidth(), label = { Text(name) })
}
