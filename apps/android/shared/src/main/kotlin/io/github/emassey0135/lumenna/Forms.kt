package io.github.emassey0135.lumenna

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.github.emassey0135.lumenna.core.BlockFields
import io.github.emassey0135.lumenna.core.BlockScope
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.TaskDetail
import io.github.emassey0135.lumenna.core.TaskFields
import io.github.emassey0135.lumenna.core.blockDefaults
import io.github.emassey0135.lumenna.core.blockEdit
import io.github.emassey0135.lumenna.core.blockFields
import io.github.emassey0135.lumenna.core.dayBlockFields
import io.github.emassey0135.lumenna.core.newBlock
import io.github.emassey0135.lumenna.core.taskEdit

// The task and block forms as the phone and the watch both edit them: each field, what it
// takes, and how saving sends only what changed. Each app lays them out its own way, the
// phone with text fields, the watch through the system's input screen.

/** A field of the task form: its name, an example, what it takes, and where it lives. */
enum class TaskField(
    val title: String,
    val example: String,
    val hint: String?,
    val get: (TaskFields) -> String,
    val set: (TaskFields, String) -> TaskFields,
) {
    TITLE("Title", "What to do", null, { it.title }, { f, v -> f.copy(title = v) }),
    DUE(
        "Due", "tomorrow", "A date, such as tomorrow or next Friday. Empty for none. A new date keeps how it repeats.",
        { it.due }, { f, v -> f.copy(due = v) },
    ),
    REPEATS(
        "Repeats", "every monday", "Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition.",
        { it.repeat }, { f, v -> f.copy(repeat = v) },
    ),
    ESTIMATE("Estimate", "45m", "Such as 45m or 1h30m. Empty for none.", { it.estimate }, { f, v -> f.copy(estimate = v) }),
    PROJECT("Project", "Inbox", null, { it.project }, { f, v -> f.copy(project = v) }),
    LABELS(
        "Labels", "calls, errands", "Names separated by commas. A new name becomes a label.",
        { it.labels }, { f, v -> f.copy(labels = v) },
    ),
    NOTES("Notes", "Anything else", null, { it.notes }, { f, v -> f.copy(notes = v) });

    companion object {
        /** The fields above the priority, as both forms order them; Notes follows it. */
        val beforePriority = listOf(TITLE, DUE, REPEATS, ESTIMATE, PROJECT, LABELS)

        /** Saves only what changed: an unchanged field sent would revert another device's edit. */
        fun save(core: Core, before: TaskDetail, fields: TaskFields) {
            val edit = taskEdit(before, fields)
            if (edit == null) core.say("Nothing changed") else core.change { it.editTask(before.id, edit) }
        }
    }
}

/**
 * The block form: a new block, or a change to one — every occurrence, or one day, as the
 * person already chose. What a block is made from, and which fields a change sends, are the
 * core's (`newBlock`, `blockEdit`), as for every app.
 */
class BlockFormModel(private val core: Core, val purpose: BlockPurpose) {
    private val shown = (purpose as? BlockPurpose.Series)?.let { core.attempt { core.lumenna.showBlock(it.id) } }

    /** The fields as the form opened, which saving compares against. */
    val initial: BlockFields = when (purpose) {
        is BlockPurpose.Add -> newFields(purpose.at, purpose.minutes)
        is BlockPurpose.Series -> shown?.let { blockFields(it) } ?: newFields("09:00", 60u)
        is BlockPurpose.Occurrence -> dayBlockFields(purpose.block)
    }

    var fields by mutableStateOf(initial)
    var day by mutableStateOf((purpose as? BlockPurpose.Add)?.date ?: Clock.today())

    val oneDay get() = purpose is BlockPurpose.Occurrence

    /** A rule the date grammar cannot say, shown beside an empty Repeats field, and kept. */
    val rule: String? = shown?.rrule?.takeIf { shown.repetition == null }

    /** Whether it can have a last day: a block that repeats, or one being added to repeat. */
    val repeats get() = shown?.repeats == true || (purpose is BlockPurpose.Add && fields.repeat.isNotBlank())

    val title = when (purpose) {
        is BlockPurpose.Add -> "New Block"
        is BlockPurpose.Series -> "Every Occurrence"
        is BlockPurpose.Occurrence -> "${Clock.spokenDay(purpose.date)} Only"
    }

    /** What the Repeats field means, which differs between adding and changing. */
    val repeatsHelp
        get() = when {
            purpose is BlockPurpose.Add -> "Such as every weekday. Empty for a block that happens once."
            rule != null -> "It repeats by the rule $rule, which this cannot show in words. Empty keeps it; none makes it happen once."
            initial.repeat.isEmpty() -> "It happens once now. Such as every weekday to make it repeat."
            else -> "Empty makes it happen once."
        }

    /** A kind brings its own flags with it, which can then be set apart from it. */
    fun kind(new: String) {
        val defaults = blockDefaults(new)
        fields = fields.copy(
            kind = new,
            acceptsTasks = defaults?.acceptsTasks ?: fields.acceptsTasks,
            countsCapacity = defaults?.countsCapacity ?: fields.countsCapacity,
            anchored = defaults?.anchored ?: fields.anchored,
        )
    }

    /** Saves, or says why not. Returns whether the form is done with: saved, or nothing to save. */
    fun save(): Boolean {
        if (fields.title.isBlank()) {
            core.say("A block needs a name.")
            return false
        }
        val change = when (purpose) {
            is BlockPurpose.Add -> core.attempt { newBlock(fields, day.trim().ifEmpty { null }) }?.let { block ->
                core.change { it.addBlock(block) }
            }
            is BlockPurpose.Series, is BlockPurpose.Occurrence -> {
                val edit = try {
                    blockEdit(initial, fields)
                } catch (error: LumennaException) {
                    core.say(error.sentence)
                    return false
                }
                if (edit == null) {
                    core.say("Nothing changed")
                    return true
                }
                val (id, scope) = when (purpose) {
                    is BlockPurpose.Series -> purpose.id to BlockScope.Series
                    is BlockPurpose.Occurrence -> purpose.block.series to BlockScope.Occurrence(purpose.date)
                }
                core.change { it.editBlock(id, edit, scope) }
            }
        }
        return change != null
    }

    companion object {
        /** The kinds, as each is said. */
        val kinds = listOf("work" to "Work, takes tasks", "break" to "Break", "event" to "Event")
        const val FLAGS_HELP = "The kind sets these; change any of them to set it apart."
    }
}
