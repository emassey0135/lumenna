package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import io.github.emassey0135.lumenna.app.Action
import io.github.emassey0135.lumenna.app.ActionKind
import io.github.emassey0135.lumenna.app.Answer
import io.github.emassey0135.lumenna.app.Question
import io.github.emassey0135.lumenna.app.ShellEvent

/**
 * The question the app has open, asked as the phone asks: a dialog each. Which question is open, its words, what a pick offers and why an answer
 * was refused are the app's (`ViewModel.asking`); the dialog closes when the app closes it. A
 * task's form is its details, which [form] opens.
 */
@Composable
fun AppQuestions(core: Core, form: (Action) -> Unit) {
    val app = core.app
    val view by app.view.collectAsState()
    val lines by app.lines.collectAsState()
    val asking = view?.asking ?: return
    val action = asking.action
    val cancel = { app.send(ShellEvent.Cancel) }
    val answer = { given: Answer -> app.send(ShellEvent.Answer(given)) }
    // A new question is a new dialog, so what was typed in the last does not carry over; a
    // refusal is the same question, and keeps what was typed.
    key(action?.kind, action?.target, asking.question) {
        when (val question = asking.question) {
            Question.Immediate -> {}
            Question.Form -> LaunchedEffect(Unit) {
                if (action != null && action.kind == ActionKind.EDIT_TASK) form(action)
                cancel()
            }
            is Question.Confirm -> Confirm(
                question.title, question.message, question.yes, cancel,
                destructive = action?.destructive == true,
            ) { answer(Answer.Yes) }
            is Question.Text -> AskText(
                question.title, question.label, question.yes,
                initial = question.initial,
                // A refused answer stays, with what was typed, and the app's reason beneath it.
                hint = asking.refusal ?: question.hint.ifEmpty { null },
                dismiss = cancel,
            ) { text -> answer(Answer.Text(text)) }
            // What a pick offers comes with the question, once the app has worked it out.
            is Question.Pick -> asking.choices?.let { offered ->
                val yes = question.yes.ifEmpty { action?.name.orEmpty() }
                val worded = lines?.choices.orEmpty()
                val options = offered.choices.mapIndexed { index, choice -> worded.getOrNull(index)?.let { option(choice, it) } ?: option(choice) }
                Choose(question.title, options, yes, cancel) { picked ->
                    answer(Answer.Picked(picked.key, null))
                }
            }
            is Question.Choose -> AlertDialog(
                onDismissRequest = cancel,
                title = { Text(question.title) },
                text = { Text(question.message) },
                confirmButton = {
                    // Stacked, as Material stacks buttons too long for a row: the answers, then
                    // Cancel at the foot.
                    Column(horizontalAlignment = Alignment.End) {
                        question.answers.forEach { choice ->
                            TextButton(modifier = Target, onClick = { answer(Answer.Picked(choice.id, null)) }) {
                                Text(choice.title, color = if (action?.destructive == true) MaterialTheme.colorScheme.error else Color.Unspecified)
                            }
                        }
                        TextButton(modifier = Target, onClick = cancel) { Text("Cancel") }
                    }
                },
            )
        }
    }
}
