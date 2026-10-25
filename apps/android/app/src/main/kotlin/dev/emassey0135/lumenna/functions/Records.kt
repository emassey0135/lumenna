package dev.emassey0135.lumenna.functions

import androidx.appfunctions.AppFunctionSerializable

// What Gemini is answered with: the core's records (`Assisted`, each a record the surface
// already has) in the shapes App Functions carry, field for field where a record goes back to
// the core (a replan, a day plan), and otherwise what an assistant needs to say the answer and
// act on it next: each record's announcement, its notices, and the identifiers it names. Each
// property's description is the core's own, from the record's documentation.

/** A change made: what it did, in the sentence to say to the person. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ChangeMade(
    /** What the change did, in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say, after the announcement. */
    val notices: List<String>,
    /** Whether anything changed. */
    val changed: Boolean,
    /** The task it was made to, as it is now, if it was made to one. */
    val task: TaskWhole?,
)

/** A task, whole. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class TaskWhole(
    /** Its identifier. */
    val id: String,
    /** Its title. */
    val title: String,
    /** Its notes. */
    val notes: String,
    /** The project it is in, by name. */
    val project: String?,
    /** The identifier of the task it is a subtask of. */
    val parent: String?,
    /** Its priority, 1 to 4, 1 highest. */
    val priority: Int,
    /** Its labels, by name. */
    val labels: List<String>,
    /** The titles of the tasks it waits for. */
    val waitsFor: List<String>,
    /** When it is due, in words without the time ("due tomorrow"). */
    val due: String?,
    /** Its due time, `HH:MM`, when it has one. */
    val dueTime: String?,
    /** How it repeats, in words. */
    val repetition: String?,
    /** How long it should take, in minutes. */
    val estimateMinutes: Int?,
    /** What is true of it, in words: done, overdue, blocked. */
    val state: List<String>,
    /** Its history: sittings, time worked, completions, postponements. */
    val history: List<String>,
)

/** One task, whole, with the sentence to say. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class TaskShown(
    /** The task in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** The task. */
    val task: TaskWhole,
)

/** A listing: tasks, blocks, projects or labels. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class Listing(
    /** What was listed, in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** What the rows are, in the plural: "tasks", "blocks". */
    val noun: String,
    /** How many rows there are. */
    val count: Int,
    /** The rows, in order, a subtask after its task. */
    val rows: List<ListedRow>,
    /** What to say when there are no rows. */
    val empty: String,
)

/** One row of a listing. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ListedRow(
    /** Its identifier: what another function is given to act on it. */
    val id: String,
    /** What it is: "task", "block", "project", "label". */
    val role: String,
    /** How deep it is, 0 at the top: a subtask is one deeper than its task. */
    val depth: Int,
    /** Its title. */
    val title: String,
    /** Whether it is done, for a row that can be. */
    val checked: Boolean?,
    /** What is true of it, in words. */
    val state: List<String>,
    /** When it is due, in words without the time; for a block, how often it happens. */
    val due: String?,
    /** Its time, `HH:MM`, when it has one. */
    val dueTime: String?,
    /** What else the row says, after when it is due: a task's priority, a project's count. */
    val value: String?,
)

/** A day, as it is lived. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class DayShown(
    /** The day in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** The day, `YYYY-MM-DD`. */
    val date: String,
    /** The day's blocks, their time and their tasks, counted. */
    val summary: String,
    /** Its blocks, in time order. */
    val blocks: List<DayBlock>,
)

/** A block on a day. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class DayBlock(
    /** The identifier of this day's occurrence of it: what Put a Task in a Block takes. */
    val id: String,
    /** Its name. */
    val title: String,
    /** When it starts, `HH:MM`. */
    val start: String,
    /** When it ends, `HH:MM`. */
    val end: String,
    /** How long it is, in minutes. */
    val minutes: Int,
    /** `work`, `break` or `event`. */
    val kind: String,
    /** `past`, `now` or `upcoming` on today; empty on any other day. */
    val time: String,
    /** Whether tasks can be put in it. */
    val acceptsTasks: Boolean,
    /** The tasks in it, its sittings, in order. */
    val sittings: List<DaySitting>,
    /** What else is said of it, in words. */
    val details: List<String>,
)

/** A sitting: one session of work on a task, in a block. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class DaySitting(
    /** Its identifier: what the timers and Take a Task out of a Block take. */
    val id: String,
    /** The task's identifier. */
    val task: String,
    /** The task's title. */
    val title: String,
    /** Where it stands, in words: `planned`, `in progress`, `worked`, `paused`. */
    val status: String,
    /** How long it is planned to take, in minutes. */
    val plannedMinutes: Int?,
    /** The minutes worked so far. */
    val minutes: Int,
    /** Whether its timer runs. */
    val running: Boolean,
    /** What else is said of it, in words. */
    val details: List<String>,
)

/** The saved filters. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class SavedFilters(
    /** What was listed, in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** The filters, in order. */
    val filters: List<SavedFilter>,
    /** What to say when there are none. */
    val empty: String,
)

/** A saved filter. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class SavedFilter(
    /** Its identifier. */
    val id: String,
    /** Its name. */
    val name: String,
    /** Its query, in the filter language, for List Tasks. */
    val query: String,
)

/** What to work on now. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class WorkChoices(
    /** Where the choices come from, in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** The choices, best first. */
    val choices: List<WorkChoice>,
)

/** One of What Should I Work On?'s choices. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class WorkChoice(
    /** Its identifier: what Start on a Choice takes. */
    val id: String,
    /** Its title. */
    val title: String,
    /** Anything more about it. */
    val detail: String?,
    /** Why it is offered, in a sentence. */
    val reason: String?,
)

/** A sitting's timer, paused or stopped. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class TimerStopped(
    /** What happened, in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** Whether anything changed. */
    val changed: Boolean,
    /** The sitting's identifier. */
    val assignment: String,
    /** The minutes it has logged. */
    val minutes: Int,
    /** Whether what it logged was cut to the block's end. */
    val capped: Boolean,
)

/** A replan of the rest of today, proposed: nothing changes until it is applied. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ReplanProposed(
    /** The whole proposal in one sentence, said before anything changes; or that nothing needs to. */
    val announcement: String,
    /** Anything else worth saying. */
    val notices: List<String>,
    /** The changes, each to accept or not. */
    val changes: List<ReplanChangeProposed>,
    /** What does not fit, said: the planner cannot make time. */
    val unplaceable: List<String>,
    /** What the person decided that this proposal keeps to, handed back to apply it. */
    val pins: ReplanPinsKept,
    /** Whether its sentences are in a 24-hour clock, for the proposal that follows. */
    val twentyFourHour: Boolean,
    /** Other options, each a different trade-off, to propose with instead. */
    val alternatives: List<ReplanOption>,
)

/** One change a replan proposes. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ReplanChangeProposed(
    /** The whole change in a sentence: "Lunch moves from 10:30 to 11:00". */
    val sentence: String,
    /** `block`, `skip` or `sitting`. */
    val kind: String,
    /** The block (series) it moves or skips, or the sitting it moves. */
    val target: String,
    /** For a block: its start before, in minutes from midnight. */
    val fromStart: Long,
    /** For a block: its start after, in minutes from midnight. */
    val toStart: Long,
    /** For a block: its length before, in minutes. */
    val fromMinutes: Int,
    /** For a block: its length after, in minutes. */
    val toMinutes: Int,
    /** For a sitting: where it goes, `<series>@<date>`, or empty for back to the list. */
    val destination: String,
)

/** What a replan keeps to. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ReplanPinsKept(
    /** Blocks left where and as long as they are. */
    val keep: List<String>,
    /** Blocks skipped today. */
    val skip: List<String>,
    /** Sittings left in their block. */
    val sittings: List<String>,
    /** Blocks let shorten below their own minimum, as an option chose. */
    val floors: List<ReplanFloorKept>,
)

/** A block let shorten below its own minimum. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ReplanFloorKept(
    /** The block's series. */
    val block: String,
    /** The least it may be, in minutes. */
    val minutes: Int,
)

/** Another option for a replan. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ReplanOption(
    /** What it is, what it costs and what follows, in a sentence or two. */
    val sentence: String,
    /** The pins to propose with, to take it. */
    val pins: ReplanPinsKept,
)

/** A replan's changes made, and the next proposal. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class ReplanApplied(
    /** What was done, in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** The change made. */
    val change: ChangeMade,
    /** The day as it now is, proposed afresh around what was turned down. */
    val next: ReplanProposed,
)

/** A plan for a day's work blocks, proposed: nothing changes until it is applied. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class DayPlanProposed(
    /** The whole plan in a sentence or two, said before anything changes. */
    val announcement: String,
    /** Anything else worth saying. */
    val notices: List<String>,
    /** The day planned, `YYYY-MM-DD`. */
    val date: String,
    /** The sittings, in the order of the day, each to accept or not. */
    val sittings: List<SittingProposed>,
    /** What is due by then and fits nowhere, by title. */
    val unfit: List<String>,
    /** Tasks kept out of the plan, as the person decided. */
    val exclude: List<String>,
    /** Sittings turned down, each kept out of its block alone. */
    val declined: List<String>,
    /** Whether its sentences are in a 24-hour clock, for the plan that follows. */
    val twentyFourHour: Boolean,
)

/** One sitting a day plan proposes. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class SittingProposed(
    /** The sitting in a sentence. */
    val sentence: String,
    /** Why this task, in a sentence. */
    val reason: String?,
    /** The task's identifier. */
    val task: String,
    /** The block it goes in, `<series>@<date>`. */
    val block: String,
    /** How long it is planned to take, in minutes. */
    val minutes: Int,
)

/** A day plan's sittings made, and the next plan. */
@AppFunctionSerializable(isDescribedByKDoc = true)
data class DayPlanApplied(
    /** What was done, in one sentence, to say to the person. */
    val announcement: String,
    /** Anything else to say. */
    val notices: List<String>,
    /** The change made. */
    val change: ChangeMade,
    /** The day as it now is, planned again around what was turned down. */
    val next: DayPlanProposed,
)
