package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import io.github.emassey0135.lumenna.core.Action
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.core.Question
import io.github.emassey0135.lumenna.core.Subject

/**
 * The phone's way of asking an action's question: a dialog each, through a screen's [Prompter].
 * What is asked, and in what words, is the core's (`Action.question`); Cancel is first.
 */
class DialogAsker(private val core: Core, private val prompt: Prompter) : Asker {
    // Cancelling gives up the action: the list stops waiting for a change to put focus back.
    private val cancel = {
        prompt.close()
        core.cancelled()
    }

    override fun confirm(action: Action, question: Question.Confirm, yes: () -> Unit) = prompt.show {
        Confirm(question.title, question.message, question.yes, cancel, destructive = action.destructive) {
            prompt.close()
            yes()
        }
    }

    // A refused answer stays in its dialog, with what was typed, and the core's reason is said.
    override fun text(action: Action, question: Question.Text, answered: (String) -> Boolean) = prompt.show {
        AskText(
            question.title, question.label, "Done",
            initial = question.initial,
            hint = question.hint.ifEmpty { null },
            dismiss = cancel,
        ) { text -> if (answered(text)) prompt.close() }
    }

    override fun pick(action: Action, title: String, options: List<Option>, picked: (Option) -> Unit) = prompt.show {
        Choose(title, options, cancel) { option ->
            prompt.close()
            picked(option)
        }
    }

    override fun length(action: Action, picked: Option, hint: String, answered: (String) -> Boolean) = prompt.show {
        AskText(
            picked.title, "Planned length", "Done", example = "45m", hint = hint, dismiss = cancel,
        ) { text -> if (answered(text)) prompt.close() }
    }

    override fun choose(action: Action, question: Question.Choose, picked: (Option) -> Unit) = prompt.show {
        AlertDialog(
            onDismissRequest = cancel,
            title = { Text(question.title) },
            text = { Text(question.message) },
            confirmButton = {
                Column {
                    TextButton(modifier = Target, onClick = cancel) { Text("Cancel") }
                    question.answers.map(::option).forEach { answer ->
                        TextButton(modifier = Target, onClick = {
                            prompt.close()
                            picked(answer)
                        }) {
                            Text(answer.title, color = if (action.destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                        }
                    }
                }
            },
        )
    }
}

/**
 * A row's actions as the core gives them, asked through [prompt]. [form] opens the app's own
 * forms, which only a screen knows how to reach; the new saved filter's form is here.
 */
fun Core.offered(
    actions: List<Action>,
    prompt: Prompter,
    form: (Action) -> Unit = {},
    done: (io.github.emassey0135.lumenna.core.Change) -> Unit = {},
): List<RowAction> = rowActions(actions, DialogAsker(this, prompt), { action ->
    if (action.subject == Subject.FILTER && action.kind == ActionKind.NEW) addFilter(this, prompt) else form(action)
}, done)

/** The app's own form for a new saved filter: its name, then its query. */
fun addFilter(core: Core, prompt: Prompter) {
    prompt.show {
        AskText("New Saved Filter", "Name", "Next", dismiss = { prompt.close(); core.cancelled() }) { name ->
            prompt.show {
                AskText("Query for $name", "Query", "Save", example = "#Work & overdue", dismiss = { prompt.close(); core.cancelled() }) { query ->
                    if (core.change { it.addFilter(name.trim(), query.trim()) } != null) prompt.close()
                }
            }
        }
    }
}

/**
 * Adds a project, label or saved filter as its heading's own action does (`Lumenna.places`):
 * for a list's Add button, which is that heading's action in another place.
 */
fun Core.addNew(group: io.github.emassey0135.lumenna.core.SidebarGroup, prompt: Prompter) {
    val heading = attempt { lumenna.places() }?.entries?.firstOrNull {
        (it.kind as? io.github.emassey0135.lumenna.core.SidebarKind.Group)?.v1 == group
    }
    heading?.actions?.let { offered(it, prompt) }?.firstOrNull()?.run?.invoke()
}
