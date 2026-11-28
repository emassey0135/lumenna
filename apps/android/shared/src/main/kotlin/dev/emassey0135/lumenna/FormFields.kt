package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.BlockFields
import dev.emassey0135.lumenna.app.TaskFields

// The forms' fields by the core's keys (`task_form()`, `block_form()`), where each value lives
// in the app's records: one mapping for the phone and the watch, so a field the core adds is
// added once.

/** A text field of the task form: its value, or null for one that is not text (the priority). */
fun TaskFields.text(key: String): String? = when (key) {
    "title" -> title
    "due" -> due
    "start" -> start
    "deadline" -> deadline
    "repeat" -> repeat
    "estimate" -> estimate
    "sittings" -> sittings
    "project" -> project
    "labels" -> labels
    "emoji" -> emoji
    "notes" -> notes
    else -> null
}

/**
 * A choice of the task form other than the priority, by the core's key: the option's id it
 * holds now (the energy, empty for not rated), or null for a field that is no such choice.
 */
fun TaskFields.chosen(key: String): String? = when (key) {
    "energy" -> energy
    else -> null
}

/** The task form with [key]'s text set to [value], as shown while its typing is on its way. */
fun TaskFields.with(key: String, value: String): TaskFields = when (key) {
    "title" -> copy(title = value)
    "due" -> copy(due = value)
    "start" -> copy(start = value)
    "deadline" -> copy(deadline = value)
    "repeat" -> copy(repeat = value)
    "estimate" -> copy(estimate = value)
    "sittings" -> copy(sittings = value)
    "energy" -> copy(energy = value)
    "project" -> copy(project = value)
    "labels" -> copy(labels = value)
    "emoji" -> copy(emoji = value)
    "notes" -> copy(notes = value)
    else -> this
}

/** Whether the task form's [key] is prose, corrected as it is typed; a date, a duration, a name or an emoji is not. */
fun isProse(key: String): Boolean = key == "title" || key == "notes"

/** A text field of the block form. */
fun BlockFields.text(key: String): String = when (key) {
    "title" -> title
    "start" -> start
    "minutes" -> minutes
    "repeat" -> repeat
    "until" -> until
    "min_minutes" -> minMinutes
    "task_filter" -> taskFilter
    "colour" -> colour
    "hours" -> hours
    "emoji" -> emoji
    "notes" -> notes
    "kind" -> kind
    else -> ""
}

/** A switch of the block form. */
fun BlockFields.flag(key: String): Boolean = when (key) {
    "accepts_tasks" -> acceptsTasks
    "counts_capacity" -> countsCapacity
    "anchored" -> anchored
    else -> false
}
