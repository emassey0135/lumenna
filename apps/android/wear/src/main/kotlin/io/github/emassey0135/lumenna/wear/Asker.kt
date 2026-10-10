package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.Asker
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.asks
import io.github.emassey0135.lumenna.button
import io.github.emassey0135.lumenna.option
import io.github.emassey0135.lumenna.rowActions
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.core.Action
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.core.Change
import io.github.emassey0135.lumenna.core.Question

/**
 * The watch's way of asking an action's question, which the core words: a list to choose from
 * for a confirmation, a pick or a choice of answers, and the system's input screen for a line
 * of text. A length is chosen from the lengths a sitting usually takes, as typing one on a
 * watch is slow.
 */
class WearAsker(private val navigator: Navigator, private val entry: TextEntry?) : Asker {
    override fun confirm(action: Action, question: Question.Confirm, yes: () -> Unit) =
        navigator.choose(action.asks(question.title), listOf(Option("yes", button(question.yes))), question.message) { yes() }

    override fun text(action: Action, question: Question.Text, answered: (String) -> Boolean) {
        if (action.kind == ActionKind.PLANNED_LENGTH || action.kind == ActionKind.LOG_MINUTES) {
            chooseLength(navigator, action.asks(question.title), if (question.optional) "None" else null) { minutes ->
                answered(minutes?.let { "${it}m" }.orEmpty())
            }
        } else {
            entry?.ask(action.asks(question.title)) { answered(it) }
        }
    }

    override fun pick(action: Action, title: String, options: List<Option>, picked: (Option) -> Unit) =
        navigator.choose(action.asks(title), options) { picked(it) }

    override fun length(action: Action, picked: Option, hint: String, answered: (String) -> Boolean) =
        chooseLength(navigator, "How long is ${picked.title} meant to take?", "No planned length") { minutes ->
            answered(minutes?.let { "${it}m" }.orEmpty())
        }

    override fun choose(action: Action, question: Question.Choose, picked: (Option) -> Unit) =
        navigator.choose(
            action.asks(question.title),
            question.answers.map(::option).map { it.copy(title = button(it.title)) },
            question.message,
        ) { picked(it) }
}

/**
 * The core's actions as a row's, asked the watch's way; [form] opens the app's own forms. The
 * primary ones come first, as a watch row shows what is done to it most before the rest.
 */
fun Core.offered(
    actions: List<Action>,
    navigator: Navigator,
    entry: TextEntry?,
    form: (Action) -> Unit = {},
    done: (Change) -> Unit = {},
): List<RowAction> = rowActions(actions, WearAsker(navigator, entry), form, done).sortedByDescending { it.primary }
