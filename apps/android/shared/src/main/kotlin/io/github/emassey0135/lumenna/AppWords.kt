package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.Action
import io.github.emassey0135.lumenna.app.Choice
import io.github.emassey0135.lumenna.app.Question
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.sentenceCase

// The words Asking.kt gives the UniFFI core's records, for the app's (crates/mobile), while
// screens move onto the app: the two are one set of records generated twice.

/** An action's name in this platform's capitals: sentence case, as Material and Wear OS write. */
val Action.name: String get() = sentence.ifEmpty { title }

/** A question's title in sentence case where it is the action's own name; one naming something keeps its capitals. */
fun Action.asks(title: String): String = if (title == this.title) name else title

/** A text question's title in sentence case: the core's, which knows which words are names. */
val Question.Text.said: String get() = sentence.ifEmpty { title }

/** A fixed button's words in sentence case. */
fun appButton(text: String): String = sentenceCase(text)

/** How a choice the app offers reads, as [option] reads the UniFFI core's. */
fun option(choice: Choice): Option {
    val date = choice.date
    val start = choice.start
    return if (date != null && start != null) {
        val span = choice.end?.let { "${Clock.time(start)} to ${Clock.time(it)}" } ?: Clock.time(start)
        Option(choice.id, "${Clock.spokenDay(date)}, $span, ${choice.title}", "", choice.depth.toInt(), choice.reason.orEmpty())
    } else {
        Option(choice.id, choice.title, choice.detail.orEmpty(), choice.depth.toInt(), choice.reason.orEmpty())
    }
}

/**
 * The app's actions as a row's: each puts its row in hand, then runs. [kind] and [subject] are
 * the UniFFI core's while lists share `RowAction` with screens still on it; the two enums name
 * the same variants.
 */
fun AppModel.rowActions(row: String, actions: List<Action>): List<RowAction> = actions.map { action ->
    RowAction(
        action.name,
        io.github.emassey0135.lumenna.records.ActionKind.valueOf(action.kind.name),
        action.destructive,
        io.github.emassey0135.lumenna.records.Subject.valueOf(action.subject.name),
        action.primary,
    ) {
        send(ShellEvent.Focus(row))
        send(ShellEvent.Perform(action))
    }
}

/** How an app row is said, as [RowSpeech] says the UniFFI core's: the title, then everything else. */
object AppRowSpeech {
    /** When the row is due, its time in this phone's clock, then the rest of its value. */
    fun details(row: io.github.emassey0135.lumenna.app.RowView): String? {
        val due = row.due?.let { due -> row.dueTime?.let { "$due at ${Clock.time(it)}" } ?: due }
        return listOfNotNull(due, row.value).joinToString(", ").ifEmpty { null }
    }

    /**
     * Everything after the title: done or not, when it is due, notable states, whether what is
     * under it is folded away, and the level where it changes from the row shown before (the
     * app's word for it, which folding keeps right).
     */
    fun value(item: io.github.emassey0135.lumenna.app.Item): String {
        val row = item.row
        val parts = mutableListOf<String>()
        if (row.checked == true) parts += "done"
        details(row)?.let { parts += it }
        // `ready` is true of almost every task; saying it everywhere buries the states that mean something.
        parts += row.state.filter { it != "ready" }
        row.expanded?.let { parts += if (it) "expanded" else "collapsed" }
        item.level?.let { parts += it }
        return parts.joinToString(", ")
    }
}

/** A place of the UniFFI core's as the app's, for screens on the app opened from those not yet. */
fun io.github.emassey0135.lumenna.records.Place.toApp(): io.github.emassey0135.lumenna.app.Place = when (this) {
    io.github.emassey0135.lumenna.records.Place.Today -> io.github.emassey0135.lumenna.app.Place.Today
    io.github.emassey0135.lumenna.records.Place.Tasks -> io.github.emassey0135.lumenna.app.Place.Tasks
    is io.github.emassey0135.lumenna.records.Place.Project -> io.github.emassey0135.lumenna.app.Place.Project(v1)
    is io.github.emassey0135.lumenna.records.Place.Label -> io.github.emassey0135.lumenna.app.Place.Label(v1)
    is io.github.emassey0135.lumenna.records.Place.Filter -> io.github.emassey0135.lumenna.app.Place.Filter(name, query)
    io.github.emassey0135.lumenna.records.Place.Blocks -> io.github.emassey0135.lumenna.app.Place.Blocks
    io.github.emassey0135.lumenna.records.Place.Trash -> io.github.emassey0135.lumenna.app.Place.Trash
}
