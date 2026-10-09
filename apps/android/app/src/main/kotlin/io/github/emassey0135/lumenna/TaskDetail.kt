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
import io.github.emassey0135.lumenna.core.MoveTarget
import io.github.emassey0135.lumenna.core.TaskDetail
import io.github.emassey0135.lumenna.core.TaskFields
import io.github.emassey0135.lumenna.core.taskEdit
import io.github.emassey0135.lumenna.core.taskFields

/** What the detail screen is asking, if anything. */
private sealed interface Asking {
    data object Block : Asking
    data class Length(val block: String, val date: String) : Asking
    data object WaitFor : Asking
    data object Parent : Asking
}

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
    var asking by remember { mutableStateOf<Asking?>(null) }

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
    val save = save@{
        val before = current ?: return@save
        val edit = form?.let { taskEdit(before, it) }
        if (edit == null) core.say("Nothing changed") else core.change { it.editTask(before.id, edit) }
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
            Field("Title", form.title, "What to do") { fields = form.copy(title = it) }
            Field(
                "Due", form.due, "tomorrow",
                "A date, such as tomorrow or next Friday. Empty for none. A new date keeps how it repeats.",
            ) { fields = form.copy(due = it) }
            Field(
                "Repeats", form.repeat, "every monday",
                "Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition.",
            ) { fields = form.copy(repeat = it) }
            Field("Estimate", form.estimate, "45m", "Such as 45m or 1h30m. Empty for none.") {
                fields = form.copy(estimate = it)
            }
            Field("Project", form.project, "Inbox") { fields = form.copy(project = it) }
            Field("Labels", form.labels, "calls, errands", "Names separated by commas. A new name becomes a label.") {
                fields = form.copy(labels = it)
            }

            Heading("Priority")
            Column(Modifier.selectableGroup()) {
                listOf(1 to "Priority 1, highest", 2 to "Priority 2", 3 to "Priority 3", 4 to "Priority 4, none")
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

            Heading("Waits for")
            current.depends.forEach { other ->
                TextButton(modifier = Target, onClick = { core.change { it.removeDependency(current.id, other.id) } }) {
                    Text("Stop Waiting for ${other.title}")
                }
            }
            TextButton(modifier = Target, onClick = { asking = Asking.WaitFor }) { Text("Add Something It Waits For…") }

            Heading("About")
            if (current.repetition == null && current.recurrence != null) {
                Text("Repeats by the rule ${current.recurrence}")
            }
            Text("State: ${current.state.joinToString(", ")}")

            Heading("Actions")
            val done = "completed" in current.state
            TextButton(modifier = Target, onClick = {
                core.change { if (done) it.uncompleteTask(current.id) else it.completeTask(current.id) }
            }) { Text(if (done) "Mark Not Done" else "Mark Done") }
            TextButton(modifier = Target, onClick = { asking = Asking.Block }) { Text("Put in a Block…") }
            TextButton(modifier = Target, onClick = { asking = Asking.Parent }) { Text("Make Subtask Of…") }
            if (current.parent != null) {
                TextButton(modifier = Target, onClick = { core.change { it.moveTask(current.id, MoveTarget.Top) } }) {
                    Text("Move to Top Level")
                }
            }
            TextButton(modifier = Target, onClick = { core.change { it.trashTask(current.id) }?.let { navigator.back() } }) {
                Text("Move to Trash", color = MaterialTheme.colorScheme.error)
            }
        }
    }

    val dismiss = { asking = null }
    when (val question = asking) {
        Asking.Block -> {
            val blocks = remember { blockChoices(core) }
            Choose("Put ${current?.title} in a Block", blocks.map { it.first }, "There are no work blocks this week. Add one from Today.", dismiss) { block ->
                asking = Asking.Length(block.key, blocks.first { it.first.key == block.key }.second)
            }
        }
        is Asking.Length -> AskLength(core, "How long is this sitting meant to take?", null, "no planned length", dismiss) { minutes ->
            asking = null
            current?.let { task -> core.change { it.assign(task.id, question.block, question.date, minutes) } }
        }
        Asking.WaitFor -> current?.let { task ->
            val excluded = task.depends.map { it.id }.toSet() + task.id
            Choose("Waits For", taskChoices(core, excluded), "There are no other open tasks.", dismiss) { other ->
                asking = null
                core.change { it.addDependency(task.id, other.key) }
            }
        }
        Asking.Parent -> current?.let { task ->
            Choose("Make Subtask Of", taskChoices(core, setOf(task.id)), "There are no other open tasks.", dismiss) { parent ->
                asking = null
                core.change { it.moveTask(task.id, MoveTarget.Parent(parent.key)) }
            }
        }
        null -> {}
    }
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
