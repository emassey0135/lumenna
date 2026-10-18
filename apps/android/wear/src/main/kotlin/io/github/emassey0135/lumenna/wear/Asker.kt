package io.github.emassey0135.lumenna.wear

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.app.ActionKind
import io.github.emassey0135.lumenna.app.Answer
import io.github.emassey0135.lumenna.app.Question
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.appButton
import io.github.emassey0135.lumenna.asks
import io.github.emassey0135.lumenna.option
import io.github.emassey0135.lumenna.said

/**
 * The question the app has open (`ViewModel.asking`), asked the watch's way: Wear's AlertDialog
 * for a confirmation, a list to choose from for a pick or a choice of answers, and the system's
 * input screen for a line of text. A length is chosen from the lengths a sitting usually takes,
 * as typing one on a watch is slow. Each question is asked once as it opens, and again when an
 * answer was refused, the app saying why; giving one up tells the app. A task's form is its own
 * screen.
 */
@Composable
fun WearQuestions(core: Core, navigator: Navigator, entry: TextEntry?) {
    val app = core.app
    val view by app.view.collectAsState()
    val asking = view?.asking
    LaunchedEffect(asking?.action?.kind, asking?.action?.target, asking?.question, asking?.choices, asking?.refusal) {
        val asked = asking ?: return@LaunchedEffect
        val action = asked.action
        val cancel = { app.send(ShellEvent.Cancel) }
        val answer = { given: Answer -> app.send(ShellEvent.Answer(given)) }
        when (val question = asked.question) {
            Question.Immediate -> {}
            Question.Form -> {
                if (action != null && action.kind == ActionKind.EDIT_TASK) navigator.open(Screen.Task(action.target))
                cancel()
            }
            is Question.Confirm -> navigator.confirm(
                action?.asks(question.title) ?: question.title, question.message, appButton(question.yes), dismissed = cancel,
            ) { answer(Answer.Yes) }
            is Question.Text -> {
                // How long, chosen: a sitting's planned length, minutes logged, or the length a
                // pick asks once something is picked.
                val length = action?.kind == ActionKind.PLANNED_LENGTH || action?.kind == ActionKind.LOG_MINUTES ||
                    action?.question is Question.Pick
                if (length) {
                    chooseLength(navigator, question.said, if (question.optional) "None" else null, cancel) { minutes ->
                        answer(Answer.Text(minutes?.let { "${it}m" }.orEmpty()))
                    }
                } else {
                    entry?.ask(action?.asks(question.said) ?: question.said) { answer(Answer.Text(it)) }
                }
            }
            is Question.Pick -> asked.choices?.let { offered ->
                navigator.choose(action?.asks(question.title) ?: question.title, offered.choices.map(::option), cancelled = cancel) { picked ->
                    answer(Answer.Picked(picked.key, null))
                }
            }
            is Question.Choose -> navigator.choose(
                action?.asks(question.title) ?: question.title,
                question.answers.map { Option(it.id, appButton(it.title)) },
                question.message,
                cancelled = cancel,
            ) { picked -> answer(Answer.Picked(picked.key, null)) }
        }
    }
}
