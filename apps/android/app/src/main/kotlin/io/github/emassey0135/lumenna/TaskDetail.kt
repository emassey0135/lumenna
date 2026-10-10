package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.core.TaskDetail
import io.github.emassey0135.lumenna.core.TaskFields
import io.github.emassey0135.lumenna.core.taskEdit
import io.github.emassey0135.lumenna.core.taskFields

/**
 * One task, its fields edited and saved together.
 *
 * Saving sends only the fields that changed — the core decides which (`taskEdit`), as for
 * every app, since a field sent unchanged would win a last-write-wins race and revert another
 * device's edit to it. While the fields differ from the task, a change arriving from elsewhere
 * does not replace them: that would lose what is being typed.
 */
@Composable
fun TaskDetailScreen(core: Core, navigator: Navigator, screen: Screen.Task, changes: Long) {
    var task by remember(screen.id) { mutableStateOf<TaskDetail?>(null) }
    var fields by remember(screen.id) { mutableStateOf<TaskFields?>(null) }
    val prompt = rememberPrompter()
    val asker = remember(prompt) { DialogAsker(core, prompt) }

    LaunchedEffect(screen.id, changes) {
        val shown = runCatching { core.lumenna.showTask(screen.id).task }.getOrNull()
        val editing = task?.let { before -> fields?.let { taskEdit(before, it) } } != null
        if (!editing || shown == null) {
            task = shown
            fields = shown?.let { taskFields(it) }
        }
    }

    val current = task
    val form = fields
    // Which fields are sent is shared with the watch (`TaskField.save`): only what changed.
    val save = save@{
        val before = current ?: return@save
        form?.let { TaskField.save(core, before, it) }
    }

    Offer(Command.SAVE) { save() }

    ScreenFrame(current?.title ?: "Task", core, navigator, actions = {
        if (current != null) {
            IconButton(onClick = { save() }) { Icon(Icons.Filled.Check, contentDescription = "Save") }
        }
    }) {
        if (current == null || form == null) {
            Text("This task is no longer here.", Modifier.padding(16.dp), color = quiet())
            return@ScreenFrame
        }
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            // The core's fields, in its order, as the watch has them too: the priority a choice
            // among the core's options, the rest lines of text.
            taskFormFields.forEach { field ->
                val text = TaskField.of(field)
                if (text != null) {
                    // Prose is corrected as it is typed; a date, a duration or a name is not.
                    val prose = text == TaskField.TITLE || text == TaskField.NOTES
                    Field(field.label, text.get(form), field.example, field.help, prose, lines = if (text == TaskField.NOTES) 3 else 1) {
                        fields = text.set(form, it)
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
                                .selectable(chosen, role = Role.RadioButton) { fields = form.copy(priority = level) }
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

            // The core's, in its order: this screen is the task's form, so no Edit Details.
            Heading("Actions")
            current.actions.forEach { action ->
                TextButton(modifier = Target, onClick = {
                    core.perform(action, asker, form = {}) { if (action.kind == ActionKind.DELETE) navigator.back() }
                }) {
                    Text(action.name, color = if (action.destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                }
            }
        }
    }
    prompt.Host()
}

/** A labelled field; [hint] beneath it says what it takes, to TalkBack and on screen. */
@Composable
private fun Field(label: String, value: String, example: String, hint: String? = null, prose: Boolean = false, lines: Int = 1, changed: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = changed,
        label = { Text(label) },
        placeholder = example.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
        supportingText = hint?.let { { Text(it) } },
        minLines = lines,
        keyboardOptions = KeyboardOptions(
            capitalization = KeyboardCapitalization.None,
            autoCorrectEnabled = prose,
        ),
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
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
