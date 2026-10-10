package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.core.BlockFields
import io.github.emassey0135.lumenna.core.CancelledBlock
import io.github.emassey0135.lumenna.core.Plan
import io.github.emassey0135.lumenna.core.PlanAssignment
import io.github.emassey0135.lumenna.core.PlanBlock
import io.github.emassey0135.lumenna.core.PlanItem
import io.github.emassey0135.lumenna.core.blockDefaults

// The day as rows, and how each reads, for the phone's day and the watch's alike.

/** One line of the day as it is lived. */
sealed interface DayRow {
    val key: String
    val depth: Int get() = 0

    data class Block(val block: PlanBlock) : DayRow {
        override val key get() = "block:${block.id}"
    }

    data class Sitting(val sitting: PlanAssignment, val block: PlanBlock) : DayRow {
        override val key get() = "sitting:${sitting.id}"
        override val depth get() = 1
    }

    data class Free(val start: String, val end: String, val minutes: UInt) : DayRow {
        override val key get() = "free:$start"
    }

    data class Now(val time: String) : DayRow {
        override val key get() = "now"
    }

    data class Cancelled(val block: CancelledBlock) : DayRow {
        override val key get() = "cancelled:${block.series}"
    }
}

fun rows(plan: Plan): List<DayRow> = plan.timeline.flatMap { item ->
    when (item) {
        is PlanItem.Block -> plan.blocks.firstOrNull { it.row == item.row }?.let { block ->
            listOf(DayRow.Block(block)) + block.assignments.map { DayRow.Sitting(it, block) }
        }.orEmpty()
        is PlanItem.Free -> listOf(DayRow.Free(item.start, item.end, item.minutes))
        is PlanItem.Now -> listOf(DayRow.Now(item.time))
    }
} + plan.cancelled.map { DayRow.Cancelled(it) }

/** What a day row says: its title, then its details. */
fun words(row: DayRow): Pair<String, List<String>> = when (row) {
    // The core words a block's and a sitting's details for every app.
    is DayRow.Block -> "${Clock.time(row.block.start)} to ${Clock.time(row.block.end)}, ${row.block.title}" to row.block.details
    is DayRow.Sitting -> row.sitting.title to row.sitting.details
    is DayRow.Free -> "Free, ${Clock.length(row.minutes)}" to listOf("${Clock.time(row.start)} to ${Clock.time(row.end)}")
    is DayRow.Now -> "Now, ${Clock.time(row.time)}" to emptyList()
    is DayRow.Cancelled -> "${Clock.time(row.block.start)}, ${row.block.title}" to listOf("cancelled for this day")
}

/** The fields a new block starts from: the time and length given, and a work block's flags. */
fun newFields(at: String, minutes: UInt): BlockFields {
    val defaults = blockDefaults("work")
    return BlockFields(
        title = "", start = at, minutes = minutes.toString(), kind = "work",
        acceptsTasks = defaults?.acceptsTasks ?: true, countsCapacity = defaults?.countsCapacity ?: true,
        anchored = defaults?.anchored ?: false, repeat = "", until = "", minMinutes = "", taskFilter = "",
        colour = "", notes = "",
    )
}

/** What the block form is for. */
sealed interface BlockPurpose {
    /** A new block, on [date] at [at] for [minutes]. */
    data class Add(val date: String? = null, val at: String = "09:00", val minutes: UInt = 60u) : BlockPurpose

    /** Every occurrence of a series. */
    data class Series(val id: String) : BlockPurpose

    /** One day of a series, from how that day stands. */
    data class Occurrence(val block: PlanBlock, val date: String) : BlockPurpose
}
