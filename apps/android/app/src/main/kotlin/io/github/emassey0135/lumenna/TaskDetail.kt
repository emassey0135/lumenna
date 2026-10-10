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
    val asker = remember(prompt) { DialogAsker(prompt) }

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
            // The fields, as the watch has them too (`TaskField`).
            TaskField.beforePriority.forEach { field ->
                Field(field.title, field.get(form), field.example, field.hint) { fields = field.set(form, it) }
            }

            Heading("Priority")
            Column(Modifier.selectableGroup()) {
                TaskField.priorities
                    .forEach { (level, name) ->
                        val chosen = form.priority.toInt() == level
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 48.dp)
                                .selectable(chosen, role = Role.RadioButton) {
                                    fields = form.copy(priority = level.toUByte())
                                }
                                .padding(vertical = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            RadioButton(selected = chosen, onClick = null)
                            Text(name, Modifier.padding(start = 12.dp))
                        }
                    }
            }

            Heading("Notes")
            OutlinedTextField(
                value = form.notes,
                onValueChange = { fields = form.copy(notes = it) },
                label = { Text("Notes") },
                placeholder = { Text("Anything else") },
                minLines = 3,
                modifier = Modifier.fillMaxWidth(),
            )

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
                    Text(action.title, color = if (action.destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                }
            }
        }
    }
    prompt.Host()
}

/** A labelled field; [hint] beneath it says what it takes, to TalkBack and on screen. */
@Composable
private fun Field(label: String, value: String, example: String, hint: String? = null, changed: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = changed,
        label = { Text(label) },
        placeholder = { Text(example) },
        supportingText = hint?.let { { Text(it) } },
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = label == "Title"),
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
