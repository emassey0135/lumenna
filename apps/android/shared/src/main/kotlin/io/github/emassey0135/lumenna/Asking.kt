package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.core.Action
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.core.Answer
import io.github.emassey0135.lumenna.core.Change
import io.github.emassey0135.lumenna.core.Choice
import io.github.emassey0135.lumenna.core.Question
import io.github.emassey0135.lumenna.core.sentenceCase

// Running a row's actions, for the phone and the watch alike. Which actions a row has, their
// names, what each asks and what a pick offers are the core's (`Action`, `Lumenna.choices`);
// each app only asks the question its own way (an `Asker`) and hands the answer back to
// `Lumenna.act`.

/** Something offered for choosing, as a chooser shows it. */
data class Option(val key: String, val title: String, val detail: String = "", val depth: Int = 0)

/**
 * How a choice the core offers reads, in every app's order: a block "<day>, <start> to <end>,
 * <title>" in this device's clock, as one line; anything else "<title>, <detail>".
 */
fun option(choice: Choice): Option {
    val date = choice.date
    val start = choice.start
    return if (date != null && start != null) {
        val span = choice.end?.let { "${Clock.time(start)} to ${Clock.time(it)}" } ?: Clock.time(start)
        Option(choice.id, "${Clock.spokenDay(date)}, $span, ${choice.title}", "", choice.depth.toInt())
    } else {
        Option(choice.id, choice.title, choice.detail.orEmpty(), choice.depth.toInt())
    }
}

/** An action's name in this platform's capitals: sentence case, as Material and Wear OS write. */
val Action.name: String get() = sentence.ifEmpty { title }

/**
 * A question's title in sentence case where it is the action's own name ("New project");
 * one that names something ("Rename Home Office") keeps it as the core wrote it, since only
 * the core knows which words are a name.
 */
fun Action.asks(title: String): String = if (title == this.title) name else title

/** A fixed button's words — "Delete Label", an answer to choose — in sentence case. */
fun button(text: String): String = sentenceCase(text)

/** How an app asks an action's question. Cancel is always the app's own, placed as its platform places it. */
interface Asker {
    /** Whether to go ahead; [yes] goes ahead. */
    fun confirm(action: Action, question: Question.Confirm, yes: () -> Unit)

    /** A line of text; [answered] says whether it was taken, and a refused one stays asked. */
    fun text(action: Action, question: Question.Text, answered: (String) -> Boolean)

    /** One of [options], which the core chose. */
    fun pick(action: Action, title: String, options: List<Option>, picked: (Option) -> Unit)

    /** How long a sitting of [picked] is meant to take, after a pick that asks it; empty is none. */
    fun length(action: Action, picked: Option, hint: String, answered: (String) -> Boolean)

    /** One of a few answers, each its own button. */
    fun choose(action: Action, question: Question.Choose, picked: (Option) -> Unit)
}

/**
 * Runs [action]: asks its question through [asker], or opens the app's own [form], then hands
 * the answer to the core, says what it did, and passes the change to [done].
 */
fun Core.perform(action: Action, asker: Asker, form: (Action) -> Unit, done: (Change) -> Unit = {}) {
    val act = { answer: Answer -> change { it.act(action, answer) }?.also(done) }
    when (val question = action.question) {
        Question.Immediate -> act(Answer.Yes)
        Question.Form -> form(action)
        is Question.Confirm -> asker.confirm(action, question) { act(Answer.Yes) }
        is Question.Text -> asker.text(action, question) { text -> act(Answer.Text(text)) != null }
        is Question.Pick -> {
            val offered = attempt { lumenna.choices(action) } ?: return
            // When there is nothing to pick, the core says why, and that is all.
            if (offered.choices.isEmpty()) {
                say(sentence(offered.announcement, offered.notices))
                cancelled()
                return
            }
            asker.pick(action, question.title, offered.choices.map(::option)) { picked ->
                val length = question.length
                if (length == null) act(Answer.Picked(picked.key, null))
                else asker.length(action, picked, length) { text -> act(Answer.Picked(picked.key, text)) != null }
            }
        }
        is Question.Choose -> asker.choose(action, question) { picked -> act(Answer.Picked(picked.key, null)) }
    }
}

/** The core's actions as a row's, each run through [perform]. */
fun Core.rowActions(
    actions: List<Action>,
    asker: Asker,
    form: (Action) -> Unit = {},
    done: (Change) -> Unit = {},
): List<RowAction> = actions.map { action ->
    RowAction(action.name, action.kind, action.destructive, action.subject, action.primary) { perform(action, asker, form, done) }
}

/** The first of [actions] of one of [kinds]: a key's action on the row in hand. */
fun List<RowAction>.ofKind(vararg kinds: ActionKind): RowAction? = firstOrNull { it.kind != null && it.kind in kinds }
