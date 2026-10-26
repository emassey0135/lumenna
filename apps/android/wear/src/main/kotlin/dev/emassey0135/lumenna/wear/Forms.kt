package dev.emassey0135.lumenna.wear

import dev.emassey0135.lumenna.quickWords
import dev.emassey0135.lumenna.app.completeWith
import dev.emassey0135.lumenna.app.detailsWords
import androidx.compose.ui.semantics.semantics
import dev.emassey0135.lumenna.app.durationWords
import dev.emassey0135.lumenna.leveled
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.ShowingBlockForm
import dev.emassey0135.lumenna.text
import dev.emassey0135.lumenna.flag
import dev.emassey0135.lumenna.option
import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.Typing
import dev.emassey0135.lumenna.app.TaskFields
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.FieldKind
import dev.emassey0135.lumenna.app.DetailsEvent
import dev.emassey0135.lumenna.app.BlockFields
import dev.emassey0135.lumenna.app.BlockEvent
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
import dev.emassey0135.lumenna.Option
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.ShowingLine
import dev.emassey0135.lumenna.name

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
    // Added, the app lets the line go, and the screen goes back; leaving it lets the line go.
    val opened = ShowingLine(app, navigator::back)
    val text = line?.text.orEmpty()
    val set = { value: String -> app.send(ShellEvent.Typing(Typing.Text(value, TextOffsets.bytes(value.length, value)))) }
    // Straight to the input screen, as the phone's quick add opens with the keyboard up.
    LaunchedEffect(opened) { if (opened && text.isBlank()) entry?.ask(quickWords.title) { set(it) } }
    WearList {
        item { Heading(quickWords.title) }
        item { TextFieldButton(quickWords.label, text, quickWords.example) { set(it) } }
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
                        contentDescription = completeWith(candidate)
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
                label = { Text(quickWords.add) },
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
            if (shown) item { Text(detailsWords.gone) }
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
                        onClick = { navigator.choose(field.label, details.projects.map(::option).leveled()) { set("project", it.key) } },
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
                // What the field takes, the core's words beneath it, as the phone shows them.
                if (field.hint.isNotEmpty()) item { Text(field.hint) }
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
        item { Button(onClick = { app.send(ShellEvent.Details(DetailsEvent.Save)) }, modifier = Modifier.fillMaxWidth(), label = { Text(detailsWords.save) }) }

        // What it is, the core's lines: its state, the rule it repeats by, what it waits for, and
        // what the records hold of it. On its own screen only, never a list.
        item { Heading(detailsWords.about) }
        details.lines.forEach { line ->
            item {
                androidx.compose.foundation.layout.Column(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}) {
                    Text(line.label, style = androidx.wear.compose.material3.MaterialTheme.typography.labelSmall)
                    Text(line.value)
                }
            }
        }

        // The app's, in the core's order: this screen is the task's form, so no Edit Details.
        item { Heading(detailsWords.actions) }
        current.offered.forEach { action ->
            item { Button(onClick = { app.send(ShellEvent.ActOn(ActingOn.Details, action)) }, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) }) }
        }
    }
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
                            "minutes" -> fields.minutes.trim().toUIntOrNull()?.let { "${fields.minutes}, ${durationWords(it)}" } ?: fields.minutes
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

/** The task details' words, the core's, in sentence case. */
private val detailsWords by lazy { detailsWords(true) }
