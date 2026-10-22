package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.ActingOn
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.collectAsState
import io.github.emassey0135.lumenna.app.DetailsEvent
import io.github.emassey0135.lumenna.app.FieldKind
import io.github.emassey0135.lumenna.app.FormField
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.TaskFields

/**
 * One task, its fields edited and saved together: the app's details (`ViewModel.details`) of
 * the task this screen asks it to show.
 *
 * Which fields saving sends is the app's: only what changed, since a field sent unchanged would
 * win a last-write-wins race and revert another device's edit to it; and typing not yet saved
 * is kept when a change arrives from elsewhere. Each field keeps its own text while typing is
 * on its way, and takes the app's once none is.
 */
@Composable
fun TaskDetailScreen(core: Core, navigator: Navigator, screen: Screen.Task) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(screen.id) { app.send(ShellEvent.Details(DetailsEvent.Show(screen.id))) }
    val details = view?.details?.takeIf { it.task.id == screen.id }
    var shown by remember(screen.id) { mutableStateOf(false) }
    // Gone from under it — deleted here or elsewhere — the screen goes back.
    LaunchedEffect(details == null) {
        if (details != null) shown = true else if (shown) navigator.back()
    }
    var typed by remember(screen.id) { mutableStateOf<TaskFields?>(null) }
    val form = typed ?: details?.fields
    // The app's fields once nothing typed is on its way.
    LaunchedEffect(details?.fields, details?.dirty) {
        if (details != null && !details.dirty) typed = null
    }
    val set = { key: String, value: String, fields: TaskFields ->
        typed = fields
        app.send(ShellEvent.Details(DetailsEvent.Field(key, value)))
    }
    val save = { app.send(ShellEvent.Details(DetailsEvent.Save)) }
    FollowReminders(core, navigator)

    Offer(Command.SAVE) { save() }

    ScreenFrame(details?.task?.title ?: "Task", core, navigator, actions = {
        if (details != null) {
            IconButton(onClick = { save() }) { Icon(Icons.Filled.Check, contentDescription = "Save") }
        }
    }) {
        if (details == null || form == null) {
            Text(if (shown) "This task is no longer here." else "", Modifier.padding(16.dp), color = quiet())
            return@ScreenFrame
        }
        val current = details.task
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            // The app's fields, in the core's order: the priority a choice among the core's
            // options, the project one of the projects, the rest text.
            details.form.forEach { field ->
                val text = form.text(field.key)
                if (field.key == "project") {
                    ProjectField(field.label, form.project, details.projects.map(::option)) { set("project", it, form.copy(project = it)) }
                    return@forEach
                }
                if (text != null) {
                    // Prose is corrected as it is typed; a date, a duration or a name is not.
                    val prose = isProse(field.key)
                    // A field of one line saves on Enter, as a form's line does.
                    val line = field.kind != FieldKind.LINES
                    Field(field.label, text, field.example, field.hint.ifEmpty { null }, prose, lines = if (line) 1 else 3, onEnter = if (line) save else null) {
                        set(field.key, it, form.with(field.key, it))
                    }
                    return@forEach
                }
                Heading(field.label)
                Column(Modifier.selectableGroup()) {
                    field.options.forEach { option ->
                        val level = option.id.toUByte()
                        val chosen = form.priority == level
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 48.dp)
                                .selectable(chosen, role = Role.RadioButton) { set("priority", option.id, form.copy(priority = level)) }
                                .padding(vertical = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            RadioButton(selected = chosen, onClick = null)
                            Text(option.title, Modifier.padding(start = 12.dp))
                        }
                    }
                }
            }

            if (current.depends.isNotEmpty()) {
                Heading("Waits for")
                current.depends.forEach { other -> Text(other.title, Modifier.padding(vertical = 4.dp)) }
            }

            Heading("About")
            if (current.repetition == null && current.recurrence != null) {
                Text("Repeats by the rule ${current.recurrence}")
            }
            Text("State: ${current.state.joinToString(", ")}")

            // What the records hold of it, the core's lines: on its own screen only, never a list.
            if (current.history.isNotEmpty()) {
                Heading("History")
                current.history.forEach { line -> Text(line, Modifier.padding(vertical = 4.dp)) }
            }

            // The core's, in its order: this screen is the task's form, so no Edit Details.
            Heading("Actions")
            current.offered.forEach { action ->
                TextButton(modifier = Target, onClick = { app.send(ShellEvent.ActOn(ActingOn.Details, action)) }) {
                    Text(action.name, color = if (action.destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                }
            }
        }
    }
}

/**
 * A labelled field; [hint] beneath it says what it takes, to TalkBack and on screen. Given
 * [onEnter], it is one line, and Enter runs it.
 */
@Composable
private fun Field(
    label: String,
    value: String,
    example: String,
    hint: String? = null,
    prose: Boolean = false,
    lines: Int = 1,
    onEnter: (() -> Unit)? = null,
    changed: (String) -> Unit,
) {
    OutlinedTextField(
        value = value,
        onValueChange = changed,
        label = { Text(label) },
        placeholder = example.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
        supportingText = hint?.let { { Text(it) } },
        singleLine = onEnter != null,
        minLines = lines,
        keyboardOptions = KeyboardOptions(
            capitalization = KeyboardCapitalization.None,
            autoCorrectEnabled = prose,
            imeAction = if (onEnter != null) ImeAction.Done else ImeAction.Default,
        ),
        keyboardActions = KeyboardActions(onDone = { onEnter?.invoke() }),
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
}

/**
 * The task's project, chosen from the projects (`projectOptions`) in Material's exposed
 * dropdown. Typing narrows the list and Enter takes the first that fits; nothing typed is
 * kept that is not a project, so leaving the field shows the project it still has. Each item
 * says its level where it changes, as a tree's rows do, beside the indent.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProjectField(label: String, value: String, options: List<Option>, chosen: (String) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    var typed by remember(value) { mutableStateOf(value) }
    val narrowing = typed != value
    val shown = if (narrowing) options.filter { it.title.contains(typed.trim(), ignoreCase = true) } else options
    val take = { option: Option ->
        chosen(option.key)
        typed = option.key
        expanded = false
    }
    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }, modifier = Modifier.padding(top = 8.dp)) {
        OutlinedTextField(
            value = typed,
            onValueChange = {
                typed = it
                expanded = true
            },
            label = { Text(label) },
            singleLine = true,
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { shown.firstOrNull()?.let(take) }),
            modifier = Modifier
                .menuAnchor(ExposedDropdownMenuAnchorType.PrimaryEditable)
                .fillMaxWidth()
                .onFocusChanged { if (!it.isFocused) typed = value },
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = {
            expanded = false
            typed = value
        }) {
            if (shown.isEmpty()) {
                DropdownMenuItem(text = { Text("No project matches") }, onClick = {}, enabled = false)
            }
            shown.forEachIndexed { index, option ->
                val level = levelChange(shown, index)
                DropdownMenuItem(
                    text = { Text(option.title) },
                    onClick = { take(option) },
                    contentPadding = PaddingValues(start = (12 + 16 * option.depth).dp, end = 12.dp),
                    modifier = Modifier.semantics {
                        contentDescription = listOfNotNull(option.title, level).joinToString(", ")
                        if (option.key == value) selected = true
                    },
                )
            }
        }
    }
}

/** A section's name, as a heading TalkBack can jump between. */
@Composable
fun Heading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(top = 24.dp, bottom = 4.dp).semantics { heading() },
    )
}
