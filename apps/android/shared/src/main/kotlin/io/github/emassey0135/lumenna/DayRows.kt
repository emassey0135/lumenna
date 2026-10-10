package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.core.Action
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

    data class Free(
        val start: String,
        val end: String,
        val minutes: UInt,
        val title: String,
        val details: List<String>,
        val actions: List<Action>,
    ) : DayRow {
        override val key get() = "free:$start"
    }

    data class Now(val time: String, val title: String) : DayRow {
        override val key get() = "now"
    }

    data class Cancelled(val block: CancelledBlock) : DayRow {
        override val key get() = "cancelled:${block.series}"
    }
}

/** What can be done to a day row: the core's, for each. */
val DayRow.actions: List<Action>
    get() = when (this) {
        is DayRow.Block -> block.actions
        is DayRow.Sitting -> sitting.actions
        is DayRow.Free -> actions
        is DayRow.Cancelled -> block.actions
        is DayRow.Now -> emptyList()
    }

fun rows(plan: Plan): List<DayRow> = plan.timeline.flatMap { item ->
    when (item) {
        is PlanItem.Block -> plan.blocks.firstOrNull { it.row == item.row }?.let { block ->
            listOf(DayRow.Block(block)) + block.assignments.map { DayRow.Sitting(it, block) }
        }.orEmpty()
        is PlanItem.Free -> listOf(DayRow.Free(item.start, item.end, item.minutes, item.title, item.details, item.actions))
        is PlanItem.Now -> listOf(DayRow.Now(item.time, item.title))
    }
} + plan.cancelled.map { DayRow.Cancelled(it) }

/**
 * What a day row says: its first line, then the rest. The words are the core's; only the clock
 * is this device's. Every app joins them in one order: a block "<start> to <end>, <title>,
 * <details>"; a sitting "<title>, <details>"; free time "<title>, <details>, <start> to
 * <end>"; now "<title>, <time>"; a cancelled day "<start>, <title>, <details>".
 */
fun words(row: DayRow): Pair<String, List<String>> = when (row) {
    is DayRow.Block -> "${Clock.time(row.block.start)} to ${Clock.time(row.block.end)}, ${row.block.title}" to row.block.details
    is DayRow.Sitting -> row.sitting.title to row.sitting.details
    is DayRow.Free -> row.title to row.details + "${Clock.time(row.start)} to ${Clock.time(row.end)}"
    is DayRow.Now -> "${row.title}, ${Clock.time(row.time)}" to emptyList()
    is DayRow.Cancelled -> "${Clock.time(row.block.start)}, ${row.block.title}" to row.block.details
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
