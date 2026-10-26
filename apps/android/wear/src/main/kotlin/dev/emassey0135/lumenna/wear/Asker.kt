package dev.emassey0135.lumenna.wear

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.Option
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Answer
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.option
import dev.emassey0135.lumenna.pickers

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
    // A question that asks for a day, answered from the watch's calendar: the day it opens on.
    var calendar by remember { mutableStateOf<String?>(null) }
    calendar?.let { from ->
        DayDialog(true, dayOf("", from), dismissed = {
            calendar = null
            app.send(ShellEvent.Cancel)
        }) { day ->
            calendar = null
            app.send(ShellEvent.Answer(Answer.Text(day)))
        }
    }
    LaunchedEffect(asking?.action?.kind, asking?.action?.target, asking?.question, asking?.choices, asking?.refusal) {
        val asked = asking ?: return@LaunchedEffect
        val action = asked.action
        val cancel = { app.send(ShellEvent.Cancel) }
        // Asked again after a refusal, the input screen's label says why first: on the
        // watch's own screen behind it, the reason was never seen.
        val label = { title: String -> listOfNotNull(asked.refusal, title).joinToString(". ") }
        val answer = { given: Answer -> app.send(ShellEvent.Answer(given)) }
        when (val question = asked.question) {
            Question.Immediate -> {}
            Question.Form -> {
                if (action != null && action.kind == ActionKind.EDIT_TASK) navigator.open(Screen.Task(action.target))
                cancel()
            }
            is Question.Confirm -> navigator.confirm(
                question.title, question.message, question.yes, dismissed = cancel,
            ) { answer(Answer.Yes) }
            is Question.Text -> {
                // How long, chosen: a sitting's planned length, minutes logged, or the length a
                // pick asks once something is picked.
                val length = action?.kind == ActionKind.PLANNED_LENGTH || action?.kind == ActionKind.LOG_MINUTES ||
                    action?.question is Question.Pick
                if (length) {
                    chooseLength(navigator, question.title, if (question.optional) "None" else null, cancel) { minutes ->
                        answer(Answer.Text(minutes?.let { "${it}m" }.orEmpty()))
                    }
                } else if (asked.day != null) {
                    // A day: the line first, said or typed, then the watch's own calendar.
                    val from = asked.day
                    navigator.choose(question.title, listOf(Option("line", question.label), Option("calendar", pickers.day)), cancelled = cancel) { way ->
                        if (way.key == "calendar") calendar = from else entry?.ask(label(question.title), cancel) { answer(Answer.Text(it)) }
                    }
                } else {
                    // Backing out of the input screen gives the question up, as Cancel would.
                    entry?.ask(label(question.title), cancel) { answer(Answer.Text(it)) }
                }
            }
            is Question.Pick -> asked.choices?.let { offered ->
                val worded = app.lines.value?.choices.orEmpty()
                val options = offered.choices.mapIndexed { index, choice -> worded.getOrNull(index)?.let { option(choice, it) } ?: option(choice) }
                navigator.choose(question.title, options, cancelled = cancel) { picked ->
                    answer(Answer.Picked(picked.key, null))
                }
            }
            is Question.Choose -> navigator.choose(
                question.title,
                question.answers.map { Option(it.id, it.title) },
                question.message,
                cancelled = cancel,
            ) { picked -> answer(Answer.Picked(picked.key, null)) }
        }
    }
}
