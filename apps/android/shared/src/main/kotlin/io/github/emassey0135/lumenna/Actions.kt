package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.core.PlanAssignment
import io.github.emassey0135.lumenna.core.PlanBlock

// What can be done to a row, in the order offered, as the phone and the watch both offer it:
// which actions apply when, their spoken names, and the words their questions ask. Each app
// turns these into its own actions and menus and runs them; neither decides which apply by
// itself, so the two cannot drift apart on it.

/** What can be done to a row of the day. */
enum class DayAction(val title: String, val destructive: Boolean = false) {
    ASSIGN_TASK("Assign Task"),
    EDIT("Edit"),
    CANCEL_THIS_DAY("Cancel This Day"),
    RESTORE_THIS_DAY("Restore This Day"),
    DELETE_BLOCK("Delete Block", destructive = true),
    START_TIMER("Start Timer"),
    RESUME_TIMER("Resume Timer"),
    PAUSE_TIMER("Pause Timer"),
    STOP_TIMER("Stop Timer"),
    PLANNED_LENGTH("Planned Length"),
    LOG_MINUTES("Log Minutes"),
    SHOW_THE_TASK("Show the Task"),
    UNASSIGN("Unassign", destructive = true),
    ADD_BLOCK_HERE("Add Block Here");

    companion object {
        /** A block's: tasks go in one that takes them, a repeating one can skip a day, a day
         *  changed from the series can go back to it. */
        fun of(block: PlanBlock): List<DayAction> = buildList {
            if (block.acceptsTasks) add(ASSIGN_TASK)
            add(EDIT)
            if (block.repeats) add(CANCEL_THIS_DAY)
            if (block.changedForThisDay) add(RESTORE_THIS_DAY)
            add(DELETE_BLOCK)
        }

        /** A sitting's: start, pause and stop — stopping a running or a paused sitting ends
         *  it — then its length, its time, its task. */
        fun of(sitting: PlanAssignment): List<DayAction> = buildList {
            val paused = sitting.status == "paused"
            add(if (sitting.running) PAUSE_TIMER else if (paused) RESUME_TIMER else START_TIMER)
            if (sitting.running || paused) add(STOP_TIMER)
            addAll(listOf(PLANNED_LENGTH, LOG_MINUTES, SHOW_THE_TASK, UNASSIGN))
        }

        val ofFreeTime = listOf(ADD_BLOCK_HERE)
        val ofCancelled = listOf(RESTORE_THIS_DAY)

        /** What deleting a block asks, which differs for one that repeats. */
        fun deleting(block: PlanBlock): String =
            if (block.repeats) "Every occurrence goes, not only this day. To skip one day, cancel it instead."
            else "The block and its sittings go."
    }
}

/** What can be done to a project, a label or a saved filter, wherever one is listed. */
enum class PlaceAction(val title: String, val destructive: Boolean = false) {
    RENAME("Rename"),
    MOVE_UP("Move Up"),
    MOVE_DOWN("Move Down"),
    MOVE_UNDER("Move Under"),
    ADD_PROJECT_INSIDE("Add Project Inside"),
    WEIGHT("Weight"),
    ARCHIVE("Archive"),
    UNARCHIVE("Unarchive"),
    MERGE_INTO("Merge Into"),
    COLOUR("Colour"),
    CHANGE_QUERY("Change Query"),
    DELETE("Delete", destructive = true);

    companion object {
        /** A project's: rename, reorder, nest, weigh, archive, delete. */
        fun ofProject(archived: Boolean) =
            listOf(RENAME, MOVE_UP, MOVE_DOWN, MOVE_UNDER, ADD_PROJECT_INSIDE, WEIGHT, if (archived) UNARCHIVE else ARCHIVE, DELETE)

        /** A label's: rename, reorder, merge, colour, delete. */
        val ofLabel = listOf(RENAME, MOVE_UP, MOVE_DOWN, MERGE_INTO, COLOUR, DELETE)

        /** A saved filter's: rename, change its query, reorder, delete. */
        val ofFilter = listOf(RENAME, CHANGE_QUERY, MOVE_UP, MOVE_DOWN, DELETE)

        const val DELETING_PROJECT = "Its tasks can go to the trash with it, or move to the Inbox."
        const val DELETE_AND_TRASH = "Delete and Trash Its Tasks"
        const val DELETE_AND_KEEP = "Delete and Keep Its Tasks"
        const val DELETING_LABEL = "Tasks wearing it stay; they just stop showing it."
        const val DELETING_FILTER = "The tasks it shows are not touched."
        const val COLOUR_HELP = "A colour name, such as teal or orange. Empty for none."
        const val WEIGHT_HELP = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again."
    }
}

/** What can be done to a paired device. */
enum class DeviceAction(val title: String, val destructive: Boolean = false) {
    SYNC_NOW("Sync Now"),
    RENAME("Rename"),
    STOP_SYNCING("Stop Syncing With It", destructive = true);

    companion object {
        /** A device cannot unpair itself, so that is not offered on its own row; a watch, which
         *  syncs through its phone, offers no Sync Now on a row. */
        fun of(thisDevice: Boolean, syncs: Boolean = true) = buildList {
            if (syncs) add(SYNC_NOW)
            add(RENAME)
            if (!thisDevice) add(STOP_SYNCING)
        }

        const val STOPPING = "It keeps what it already has: this is for a device you replaced, not one that was stolen."
    }
}
