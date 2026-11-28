package dev.emassey0135.lumenna.functions

import android.os.Build
import androidx.annotation.RequiresApi
import androidx.appfunctions.AppFunctionAppUnknownException
import androidx.appfunctions.AppFunctionDeclaration
import androidx.appfunctions.AppFunctionInstruction
import androidx.appfunctions.AppFunctionService
import androidx.appfunctions.AppFunctionServiceEntryPoint
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.app.Argument
import dev.emassey0135.lumenna.app.Assisted
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Lumenna's App Functions, for Gemini: one per operation of the core's table
 * (`assistant_operations()`, `records::assistant`), named after its `id` and described in its
 * words, each parameter by its name and description, so Gemini offers what `lum mcp` does.
 * Each runs through the core's one dispatcher (`Device.assist`). The library reads a
 * function's description only from an annotation written ahead, so the words are the table's
 * copied here, and `AppFunctionsTest` fails when one differs from the table, is missing, or
 * is not in it. An operation added to the table needs its function here.
 *
 * The library generates the service (`LumennaFunctionService`) and the XML the system indexes
 * (`lumenna_functions.xml`) from this.
 */
@RequiresApi(Build.VERSION_CODES.BAKLAVA)
@AppFunctionServiceEntryPoint(serviceName = "LumennaFunctionService", appFunctionXmlFileName = "lumenna_functions")
abstract class LumennaFunctions : AppFunctionService() {
    private fun core(): Core = (application as CoreHolder).core.getOrElse {
        throw AppFunctionAppUnknownException(dev.emassey0135.lumenna.app.storeUnopened(it.message.orEmpty()))
    }

    /** The core's answer to [operation], off the main thread, where every function is called. */
    private suspend fun assist(operation: String, vararg given: Argument?): Assisted =
        withContext(Dispatchers.IO) { Assistant.run(core(), operation, *given) }

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Lists tasks: by default every open task, most urgent first, subtasks under their task; or those a filter admits.")
    suspend fun listTasks(
        @AppFunctionInstruction("A filter in Lumenna's filter language, such as `overdue`, `today & #Work`, `@calls`, `#Work & /Errands` (a section), `p1`, or `search: words`; empty for every open task.")
        query: String? = null,
    ): Listing =
        Assistant.listing(assist("list_tasks", Assistant.text("query", query)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Searches task titles and notes for words.")
    suspend fun searchTasks(
        @AppFunctionInstruction("Words to search titles and notes for.")
        text: String,
    ): Listing =
        Assistant.listing(assist("search_tasks", Assistant.text("text", text)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("One task whole: its fields, states, subtasks, what it waits for, its sittings and its history.")
    suspend fun showTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
    ): TaskShown =
        Assistant.taskShown(assist("show_task", Assistant.text("id", id)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("A day as it is lived: its blocks in time order, each with the tasks in it (its sittings), the free time between them, and where now falls.")
    suspend fun day(
        @AppFunctionInstruction("The day, as a person says it (\"tomorrow\", \"friday\", \"2026-11-03\"); today if left out.")
        date: String? = null,
    ): DayShown =
        Assistant.dayShown(assist("day", Assistant.text("date", date)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("The days ahead at once, today first: each day's blocks in time order, then the tasks due, starting or with their deadline that day.")
    suspend fun listUpcoming(): Listing =
        Assistant.listing(assist("list_upcoming"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Every time block, each with when it happens and how often.")
    suspend fun listBlocks(): Listing =
        Assistant.listing(assist("list_blocks"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Every project, its subprojects under it.")
    suspend fun listProjects(): Listing =
        Assistant.listing(assist("list_projects"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("A project's sections, the headings its tasks sit under, in order, each with how many open tasks it holds.")
    suspend fun listSections(
        @AppFunctionInstruction("The project, by name.")
        project: String,
    ): Listing =
        Assistant.listing(assist("list_sections", Assistant.text("project", project)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Every label.")
    suspend fun listLabels(): Listing =
        Assistant.listing(assist("list_labels"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Every saved filter, with its query.")
    suspend fun listFilters(): SavedFilters =
        Assistant.savedFilters(assist("list_filters"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("What to work on now, ranked, each choice with the reason it is offered: in a block under way its own unfinished sittings, then its suggestions; in free time, what fits before the next block. Starting on one is `start_on`.")
    suspend fun whatShouldIWorkOn(): WorkChoices =
        Assistant.workChoices(assist("what_should_i_work_on"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Proposes how to replan the rest of today, when the day has slipped: every change as a whole sentence. Nothing changes until `apply_replan` is given the ones the person accepted; say them to the person first.")
    suspend fun proposeReplan(): ReplanProposed =
        Assistant.replanProposed(assist("propose_replan"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("What the timer is doing now: running, on which task, how long so far and how much of the sitting's planned length is left; else what `toggle_timer` would start, or why nothing would.")
    suspend fun timer(): TimerState =
        Assistant.timer(assist("timer"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("What every sitting logged over the last four weeks, summed by week, by project and by label, each with the estimates of the tasks worked beside it.")
    suspend fun report(): Listing =
        Assistant.listing(assist("report"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Proposes which tasks to put in each of a day's work blocks still to come, each with why. Nothing changes until `apply_day_plan` is given the sittings the person accepted.")
    suspend fun proposeDayPlan(
        @AppFunctionInstruction("The day, as a person says it (\"tomorrow\", \"friday\", \"2026-11-03\"); today if left out.")
        date: String? = null,
    ): DayPlanProposed =
        Assistant.dayPlanProposed(assist("propose_day_plan", Assistant.text("date", date)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Adds a task (or, from a line starting with `block`, a time block) from one quick-add line, and says how it read it.")
    suspend fun addTask(
        @AppFunctionInstruction("A quick-add line, read as a person's would be: the title, then any of a due date or time (\"tomorrow at 3pm\", \"every weekday\"), a priority (p1 to p4), labels (@calls), a project (#Work, #\"Home Office\") and a section of it (/Errands), an estimate (~30m), how long a sitting of it is (\"3h in 45m sittings\"), when it may be begun (\"from monday\", or \"someday\"), the day it must be done by (\"by friday\"). A line starting with `block` makes a time block instead (\"block Deep work 9-11am weekdays\"). The answer says how it was read.")
        text: String,
    ): ChangeMade =
        Assistant.changeMade(assist("add_task", Assistant.text("text", text)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Changes a task's fields: only those given.")
    suspend fun editTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
        @AppFunctionInstruction("A new title.")
        title: String? = null,
        @AppFunctionInstruction("A date phrase (\"next monday\", \"in 3 days at 9am\"), or `none` to clear it.")
        due: String? = null,
        @AppFunctionInstruction("A repetition (\"every monday\", \"every! 2 weeks\"), or `none` to stop it repeating.")
        repeat: String? = null,
        @AppFunctionInstruction("1 to 4, where 1 is highest.")
        priority: Int? = null,
        @AppFunctionInstruction("How long it should take (\"45m\", \"1h30m\"), or `none`.")
        estimate: String? = null,
        @AppFunctionInstruction("How long one sitting of it is (\"45m\"), for work better done in pieces: planning gives it a sitting at a time. `none` to plan it whole.")
        sittings: String? = null,
        @AppFunctionInstruction("When it may be begun, as a person says a day (\"next monday\"): until then it is left out of the lists and the planner. `someday` keeps it until it is given a date; `none` starts it now.")
        start: String? = null,
        @AppFunctionInstruction("The day it must be done by (\"friday\"), or `none`.")
        deadline: String? = null,
        @AppFunctionInstruction("What it takes of the person's energy: `low`, `medium` or `high`, or `none` for not rated. Planning puts demanding work in their best hours and plans less on a day they are low.")
        energy: String? = null,
        @AppFunctionInstruction("One emoji to draw beside its title (\"🎯\"), never read aloud, or `none` to take it off.")
        emoji: String? = null,
        @AppFunctionInstruction("Its notes, whole.")
        notes: String? = null,
        @AppFunctionInstruction("The project to move it to, by name.")
        project: String? = null,
        @AppFunctionInstruction("Its labels, all of them, by name.")
        labels: List<String>? = null,
    ): ChangeMade =
        Assistant.changeMade(assist("edit_task", Assistant.text("id", id), Assistant.text("title", title), Assistant.text("due", due), Assistant.text("repeat", repeat), Assistant.number("priority", priority), Assistant.text("estimate", estimate), Assistant.text("sittings", sittings), Assistant.text("start", start), Assistant.text("deadline", deadline), Assistant.text("energy", energy), Assistant.text("emoji", emoji), Assistant.text("notes", notes), Assistant.text("project", project), Assistant.texts("labels", labels)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Marks a task done. A repeating one moves on to its next date.")
    suspend fun completeTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
    ): ChangeMade =
        Assistant.changeMade(assist("complete_task", Assistant.text("id", id)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Marks a done task not done.")
    suspend fun uncompleteTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
    ): ChangeMade =
        Assistant.changeMade(assist("uncomplete_task", Assistant.text("id", id)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Moves a task to the trash, from which `restore_task` brings it back.")
    suspend fun trashTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
    ): ChangeMade =
        Assistant.changeMade(assist("trash_task", Assistant.text("id", id)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Erases a task in the trash permanently: from the history every device keeps, and with it the history of everything deleted before. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.")
    suspend fun eraseTaskPermanently(
        @AppFunctionInstruction("The identifier of a task in the trash.")
        id: String,
        @AppFunctionInstruction("True only once the person has said yes to the question this asks without it.")
        confirm: Boolean? = null,
    ): ChangeMade =
        Assistant.changeMade(assist("erase_task_permanently", Assistant.text("id", id), Assistant.flag("confirm", confirm)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Erases everything deleted permanently: every task in the trash, and the history of everything ever deleted, on every device. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.")
    suspend fun eraseEverythingDeleted(
        @AppFunctionInstruction("True only once the person has said yes to the question this asks without it.")
        confirm: Boolean? = null,
    ): ChangeMade =
        Assistant.changeMade(assist("erase_everything_deleted", Assistant.flag("confirm", confirm)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Brings a task back from the trash.")
    suspend fun restoreTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
    ): ChangeMade =
        Assistant.changeMade(assist("restore_task", Assistant.text("id", id)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Makes a task a subtask of another, moves it to a project or under a section of one, or makes it a task of its own.")
    suspend fun moveTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
        @AppFunctionInstruction("A task to make it a subtask of, by identifier.")
        parent: String? = null,
        @AppFunctionInstruction("A project to move it to, by name.")
        project: String? = null,
        @AppFunctionInstruction("True to make it a task of its own again, under no other.")
        top: Boolean? = null,
        @AppFunctionInstruction("A section of its project to put it under, by name (Project/Section for another project's), or `none` to take it out of the one it is under.")
        section: String? = null,
    ): ChangeMade =
        Assistant.changeMade(assist("move_task", Assistant.text("id", id), Assistant.text("parent", parent), Assistant.text("project", project), Assistant.flag("top", top), Assistant.text("section", section)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Adds a section to a project, after its others: a heading its tasks can sit under.")
    suspend fun addSection(
        @AppFunctionInstruction("The project, by name.")
        project: String,
        @AppFunctionInstruction("The section's name.")
        name: String,
    ): ChangeMade =
        Assistant.changeMade(assist("add_section", Assistant.text("project", project), Assistant.text("name", name)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Puts a task in a block, as a sitting: one session of work on it there.")
    suspend fun assign(
        @AppFunctionInstruction("The task's identifier.")
        task: String,
        @AppFunctionInstruction("The block's identifier, as the day lists it.")
        block: String,
        @AppFunctionInstruction("The day, for a block that repeats; the block's own day otherwise.")
        date: String? = null,
        @AppFunctionInstruction("How long the sitting is planned to take, in minutes.")
        minutes: Int? = null,
    ): ChangeMade =
        Assistant.changeMade(assist("assign", Assistant.text("task", task), Assistant.text("block", block), Assistant.text("date", date), Assistant.number("minutes", minutes)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Takes a sitting out of its block; the task itself stays.")
    suspend fun unassign(
        @AppFunctionInstruction("The sitting's identifier: a task's place in a block, as the day lists it.")
        assignment: String,
    ): ChangeMade =
        Assistant.changeMade(assist("unassign", Assistant.text("assignment", assignment)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Starts, or resumes, a sitting's timer.")
    suspend fun startTimer(
        @AppFunctionInstruction("The sitting's identifier: a task's place in a block, as the day lists it.")
        assignment: String,
    ): ChangeMade =
        Assistant.changeMade(assist("start_timer", Assistant.text("assignment", assignment)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Pauses a sitting's timer, keeping what it has logged: the timer running, unless a sitting is named. Refused when none runs.")
    suspend fun pauseTimer(
        @AppFunctionInstruction("The sitting's identifier, as the day lists it; the sitting whose timer runs when left out.")
        assignment: String? = null,
    ): TimerState =
        Assistant.timer(assist("pause_timer", Assistant.text("assignment", assignment)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Stops a sitting's timer and logs its time: the timer running, unless a sitting is named. Refused when none runs.")
    suspend fun stopTimer(
        @AppFunctionInstruction("The sitting's identifier, as the day lists it; the sitting whose timer runs when left out.")
        assignment: String? = null,
        @AppFunctionInstruction("The minutes to log instead of the timer's, when it ran on by mistake.")
        minutes: Int? = null,
    ): TimerState =
        Assistant.timer(assist("stop_timer", Assistant.text("assignment", assignment), Assistant.number("minutes", minutes)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Presses the timer's one control: stops the timer running, or with none running starts on the first of What Should I Work On?'s choices, its timer running. `timer` says first which it would do.")
    suspend fun toggleTimer(): ChangeMade =
        Assistant.changeMade(assist("toggle_timer"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Starts on one of What Should I Work On?'s choices at once, its timer running: in the block under way, or in a block of its own in free time.")
    suspend fun startOn(
        @AppFunctionInstruction("The `id` of one of the choices What Should I Work On? offered.")
        choice: String,
    ): ChangeMade =
        Assistant.changeMade(assist("start_on", Assistant.text("choice", choice)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Makes the replan changes the person accepted, as one change they can undo, and proposes again around what they turned down.")
    suspend fun applyReplan(
        @AppFunctionInstruction("The proposal as `propose_replan` returned it, whole.")
        proposal: ReplanProposed,
        @AppFunctionInstruction("Those of its `changes` the person accepted, each as it was proposed.")
        accepted: List<ReplanChangeProposed>,
    ): ReplanApplied =
        Assistant.replanApplied(assist("apply_replan", Assistant.replan("proposal", proposal), Assistant.replanChanges("accepted", accepted)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Puts the sittings the person accepted into their blocks, as one change they can undo, and plans again around what they turned down.")
    suspend fun applyDayPlan(
        @AppFunctionInstruction("The proposal as `propose_day_plan` returned it, whole.")
        proposal: DayPlanProposed,
        @AppFunctionInstruction("Those of its `sittings` the person accepted, each as it was proposed.")
        accepted: List<SittingProposed>,
    ): DayPlanApplied =
        Assistant.dayPlanApplied(assist("apply_day_plan", Assistant.dayPlan("proposal", proposal), Assistant.dayPlanSittings("accepted", accepted)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("What is due for a second look: the projects due for review, each with when it was last reviewed and how often it is, then the tasks the planner asks about, each with its question in short. A task's whole question is its `asking` in `show_task`.")
    suspend fun review(): Listing =
        Assistant.listing(assist("review"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Marks a project reviewed now: it leaves the review until its interval comes round.")
    suspend fun markReviewed(
        @AppFunctionInstruction("The project, by name.")
        project: String,
    ): ChangeMade =
        Assistant.changeMade(assist("mark_reviewed", Assistant.text("project", project)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Says how the person's energy is today, which planning weighs for today alone: low plans less and leaves what takes the most for another day, high plans more. On a low day, `propose_replan` then proposes a lighter rest of the day.")
    suspend fun checkInEnergy(
        @AppFunctionInstruction("How the person says they are today: `low`, `usual` or `high`. Left out, says what they said today.")
        level: String? = null,
    ): ChangeMade =
        Assistant.changeMade(assist("check_in_energy", Assistant.text("level", level)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Sets how often a project comes up for review.")
    suspend fun reviewEvery(
        @AppFunctionInstruction("The project, by name.")
        project: String,
        @AppFunctionInstruction("How often, such as every week, every 2 weeks, every month, or never.")
        every: String,
    ): ChangeMade =
        Assistant.changeMade(assist("review_every", Assistant.text("project", project), Assistant.text("every", every)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Answers the planner's question about a task (its `asking` in `show_task`, put to the person first): keeps it, drops it, makes it repeat less often, or takes its date away.")
    suspend fun reconsiderTask(
        @AppFunctionInstruction("The task's identifier: a whole UUID, or a prefix that names one task.")
        id: String,
        @AppFunctionInstruction("One of the answers its question offers: `keep` (asked again later), `drop` (to the trash), `less-often` (a repeating task half as often), or `no-date` (its due date taken away).")
        answer: String,
    ): ChangeMade =
        Assistant.changeMade(assist("reconsider_task", Assistant.text("id", id), Assistant.text("answer", answer)))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Undoes the last change made on this device, whoever made it: an app, a command, or an assistant.")
    suspend fun undo(): ChangeMade =
        Assistant.changeMade(assist("undo"))

    @AppFunctionDeclaration(isDescribedByKDoc = false)
    @AppFunctionInstruction("Redoes what was last undone.")
    suspend fun redo(): ChangeMade =
        Assistant.changeMade(assist("redo"))
}
