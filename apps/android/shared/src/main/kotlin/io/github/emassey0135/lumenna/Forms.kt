package io.github.emassey0135.lumenna

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.github.emassey0135.lumenna.core.BlockFields
import io.github.emassey0135.lumenna.core.BlockScope
import io.github.emassey0135.lumenna.core.FormField
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.TaskDetail
import io.github.emassey0135.lumenna.core.TaskFields
import io.github.emassey0135.lumenna.core.blockDefaults
import io.github.emassey0135.lumenna.core.blockForm
import io.github.emassey0135.lumenna.core.blockEdit
import io.github.emassey0135.lumenna.core.blockFields
import io.github.emassey0135.lumenna.core.dayBlockFields
import io.github.emassey0135.lumenna.core.newBlock
import io.github.emassey0135.lumenna.core.taskEdit
import io.github.emassey0135.lumenna.core.taskForm
import io.github.emassey0135.lumenna.core.unsayableRepeatNote

// The task and block forms as the phone and the watch both edit them: each field and how
// saving sends only what changed. What each field is called, takes and shows as an example is
// the core's (`taskForm`, `blockForm`), as for every app. Each app lays them out its own way,
// the phone with text fields, the watch through the system's input screen.

/** The task form's fields, in the core's order: the priority among them, as a choice. */
val taskFormFields: List<FormField> by lazy { taskForm() }

/** The block form's fields, in the core's order. */
val blockFormFields: List<FormField> by lazy { blockForm() }

/**
 * The projects the task form's Project field offers (`projectOptions`): every one not
 * archived, in tree order, each with its depth — and the task's own, [current], when it is
 * archived, so the field does not lose it. A choice is read by its key, never its text.
 */
fun Core.projectChoices(current: String): List<Option> {
    val offered = attempt { lumenna.projectOptions() }?.map(::option).orEmpty()
    return if (current.isEmpty() || offered.any { it.key == current }) offered else offered + Option(current, current)
}

/** What a field says beneath it, or nothing. */
val FormField.help: String? get() = hint.ifEmpty { null }

/** A text field of the task form, by the core's key: where its value lives in [TaskFields]. */
enum class TaskField(
    val key: String,
    val get: (TaskFields) -> String,
    val set: (TaskFields, String) -> TaskFields,
) {
    TITLE("title", { it.title }, { f, v -> f.copy(title = v) }),
    DUE("due", { it.due }, { f, v -> f.copy(due = v) }),
    REPEATS("repeat", { it.repeat }, { f, v -> f.copy(repeat = v) }),
    ESTIMATE("estimate", { it.estimate }, { f, v -> f.copy(estimate = v) }),
    PROJECT("project", { it.project }, { f, v -> f.copy(project = v) }),
    LABELS("labels", { it.labels }, { f, v -> f.copy(labels = v) }),
    NOTES("notes", { it.notes }, { f, v -> f.copy(notes = v) });

    companion object {
        /** Where a form field's text lives, or null for the priority, whose value is a number. */
        fun of(field: FormField): TaskField? = entries.firstOrNull { it.key == field.key }

        /** Saves only what changed: an unchanged field sent would revert another device's edit. */
        fun save(core: Core, before: TaskDetail, fields: TaskFields) {
            val edit = taskEdit(before, fields)
            if (edit == null) core.say("Nothing changed") else core.change { it.editTask(before.id, edit) }
        }
    }
}

/** A block form field's text, by the core's key; a toggle or the kind reads empty here. */
fun BlockFields.text(key: String): String = when (key) {
    "title" -> title
    "start" -> start
    "minutes" -> minutes
    "repeat" -> repeat
    "until" -> until
    "min_minutes" -> minMinutes
    "task_filter" -> taskFilter
    "colour" -> colour
    "notes" -> notes
    else -> ""
}

/** These fields with [key]'s text set to [value]. */
fun BlockFields.withText(key: String, value: String): BlockFields = when (key) {
    "title" -> copy(title = value)
    "start" -> copy(start = value)
    "minutes" -> copy(minutes = value)
    "repeat" -> copy(repeat = value)
    "until" -> copy(until = value)
    "min_minutes" -> copy(minMinutes = value)
    "task_filter" -> copy(taskFilter = value)
    "colour" -> copy(colour = value)
    "notes" -> copy(notes = value)
    else -> this
}

/** A block form toggle, by the core's key. */
fun BlockFields.flag(key: String): Boolean = when (key) {
    "accepts_tasks" -> acceptsTasks
    "counts_capacity" -> countsCapacity
    "anchored" -> anchored
    else -> false
}

/** These fields with [key]'s toggle set to [on]. */
fun BlockFields.withFlag(key: String, on: Boolean): BlockFields = when (key) {
    "accepts_tasks" -> copy(acceptsTasks = on)
    "counts_capacity" -> copy(countsCapacity = on)
    "anchored" -> copy(anchored = on)
    else -> this
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

    /** Whether it can have a last day: a block that repeats, or one being added to repeat. */
    val repeats get() = shown?.repeats == true || (purpose is BlockPurpose.Add && fields.repeat.isNotBlank())

    val title = when (purpose) {
        is BlockPurpose.Add -> "New block"
        is BlockPurpose.Series -> "Every occurrence"
        is BlockPurpose.Occurrence -> "${Clock.spokenDay(purpose.date)} only"
    }

    /**
     * Whether the form shows [field]: a new block's day only when adding; a last day only
     * while it repeats; and one day of a series changes only its time, length, kind and flags.
     */
    fun shows(field: FormField): Boolean = when {
        field.key == "date" -> purpose is BlockPurpose.Add
        oneDay && !field.oneDay -> false
        field.repeatingOnly -> repeats
        else -> true
    }

    /** The core's note for a repetition its words cannot say, which an empty field keeps. */
    private val unsayable: String? = shown?.let { unsayableRepeatNote(it) }

    /** What [field] says beneath it: the core's words, and for Repeats its note on a rule. */
    fun help(field: FormField): String? =
        if (field.key == "repeat" && unsayable != null) unsayable else field.help

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

}
