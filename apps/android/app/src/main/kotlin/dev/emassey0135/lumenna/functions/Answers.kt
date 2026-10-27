package dev.emassey0135.lumenna.functions

import android.os.Handler
import android.os.Looper
import androidx.appfunctions.AppFunctionAppUnknownException
import androidx.appfunctions.AppFunctionInvalidArgumentException
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.app.Argument
import dev.emassey0135.lumenna.app.ArgumentValue
import dev.emassey0135.lumenna.app.Assisted
import dev.emassey0135.lumenna.app.Change
import dev.emassey0135.lumenna.app.DayPins
import dev.emassey0135.lumenna.app.DayPlan
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.app.ProposedSitting
import dev.emassey0135.lumenna.app.Replan
import dev.emassey0135.lumenna.app.ReplanAlternative
import dev.emassey0135.lumenna.app.ReplanChange
import dev.emassey0135.lumenna.app.ReplanFloor
import dev.emassey0135.lumenna.app.ReplanPins
import dev.emassey0135.lumenna.app.TaskDetail
import dev.emassey0135.lumenna.app.TimeStyle
import dev.emassey0135.lumenna.app.assistantOperations
import dev.emassey0135.lumenna.sentence

/**
 * Gemini's way to the core: every App Function runs its operation through the one dispatcher
 * `lum mcp`, Siri and Shortcuts run theirs through (`Device.assist`), so none does what another
 * cannot. What the core refuses, an operation that asks first included, is refused in the core's
 * words; a write is taken in by the app, as another process's would be.
 */
internal object Assistant {
    private val writes: Set<String> by lazy { assistantOperations().filter { it.writes }.map { it.id }.toSet() }
    private val main = Handler(Looper.getMainLooper())

    /** Runs [operation] with what was given, leaving out what was not. */
    fun run(core: Core, operation: String, vararg given: Argument?): Assisted {
        val answer = try {
            core.device.assist(operation, given.filterNotNull())
        } catch (refused: LumennaError) {
            throw AppFunctionInvalidArgumentException(refused.sentence)
        }
        if (operation in writes) main.post { core.changed() }
        return answer
    }

    fun text(name: String, value: String?) = value?.let { Argument(name, ArgumentValue.Text(it)) }
    fun number(name: String, value: Int?) = value?.let { Argument(name, ArgumentValue.Number(it.toLong())) }
    fun flag(name: String, value: Boolean?) = value?.let { Argument(name, ArgumentValue.Flag(it)) }
    fun texts(name: String, value: List<String>?) = value?.let { Argument(name, ArgumentValue.Texts(it)) }
    fun replan(name: String, value: ReplanProposed) = Argument(name, ArgumentValue.Replan(value.core()))
    fun replanChanges(name: String, value: List<ReplanChangeProposed>) =
        Argument(name, ArgumentValue.ReplanChanges(value.map { it.core() }))
    fun dayPlan(name: String, value: DayPlanProposed) = Argument(name, ArgumentValue.DayPlan(value.core()))
    fun dayPlanSittings(name: String, value: List<SittingProposed>) =
        Argument(name, ArgumentValue.DayPlanSittings(value.map { it.core() }))

    private fun unexpected(answer: Assisted): Nothing =
        throw AppFunctionAppUnknownException("The core answered with ${answer::class.simpleName}")

    fun changeMade(answer: Assisted) = (answer as? Assisted.Change)?.field0?.made() ?: unexpected(answer)

    fun listing(answer: Assisted): Listing {
        val rows = (answer as? Assisted.Rows)?.field0 ?: unexpected(answer)
        return Listing(
            rows.announcement, rows.notices, rows.noun, rows.count.toInt(),
            rows.rows.map {
                ListedRow(it.id, it.role, it.depth.toInt(), it.title, it.checked, it.state, it.due, it.dueTime, it.value)
            },
            rows.empty,
        )
    }

    fun taskShown(answer: Assisted): TaskShown {
        val shown = (answer as? Assisted.Task)?.field0 ?: unexpected(answer)
        return TaskShown(shown.announcement, shown.notices, shown.task.whole())
    }

    fun dayShown(answer: Assisted): DayShown {
        val plan = (answer as? Assisted.Plan)?.field0 ?: unexpected(answer)
        return DayShown(
            plan.announcement, plan.notices, plan.date, plan.summary,
            plan.blocks.map { block ->
                DayBlock(
                    block.id, block.title, block.start, block.end, block.durationMins.toInt(), block.kind, block.`when`,
                    block.acceptsTasks,
                    block.assignments.map {
                        DaySitting(it.id, it.task, it.title, it.status, it.plannedMins?.toInt(), it.minutes.toInt(), it.running, it.details)
                    },
                    block.details,
                )
            },
        )
    }

    fun savedFilters(answer: Assisted): SavedFilters {
        val filters = (answer as? Assisted.Filters)?.field0 ?: unexpected(answer)
        return SavedFilters(filters.announcement, filters.notices, filters.filters.map { SavedFilter(it.id, it.name, it.query) }, filters.empty)
    }

    fun workChoices(answer: Assisted): WorkChoices {
        val choices = (answer as? Assisted.Choices)?.field0 ?: unexpected(answer)
        return WorkChoices(choices.announcement, choices.notices, choices.choices.map { WorkChoice(it.id, it.title, it.detail, it.reason) })
    }

    fun timer(answer: Assisted): TimerState {
        val timer = (answer as? Assisted.Timer)?.field0 ?: unexpected(answer)
        return TimerState(timer.announcement, timer.notices, timer.changed, timer.assignment, timer.minutes.toInt(), timer.capped, timer.running)
    }

    fun replanProposed(answer: Assisted) = (answer as? Assisted.Replan)?.field0?.proposed() ?: unexpected(answer)

    fun replanApplied(answer: Assisted): ReplanApplied {
        val done = (answer as? Assisted.Replanned)?.field0 ?: unexpected(answer)
        return ReplanApplied(done.announcement, done.notices, done.change.made(), done.next.proposed())
    }

    fun dayPlanProposed(answer: Assisted) = (answer as? Assisted.DayPlan)?.field0?.proposed() ?: unexpected(answer)

    fun dayPlanApplied(answer: Assisted): DayPlanApplied {
        val done = (answer as? Assisted.DayPlanned)?.field0 ?: unexpected(answer)
        return DayPlanApplied(done.announcement, done.notices, done.change.made(), done.next.proposed())
    }
}

private fun Change.made() = ChangeMade(announcement, notices, changed, task?.whole())

private fun TaskDetail.whole() = TaskWhole(
    id, title, notes, project, parent, priority.toInt(), labels, depends.map { it.title }, due, dueTime, repetition,
    estimateMins?.toInt(), state, history,
)

// A proposal goes back to the core whole, so each field is carried both ways.

private fun ReplanPins.kept() = ReplanPinsKept(keep, skip, sittings, floors.map { ReplanFloorKept(it.block, it.minutes.toInt()) })

private fun ReplanPinsKept.core() = ReplanPins(keep, skip, sittings, floors.map { ReplanFloor(it.block, it.minutes.toUInt()) })

private fun ReplanChange.proposed() =
    ReplanChangeProposed(sentence, kind, target, fromStart, toStart, fromMins.toInt(), toMins.toInt(), destination)

private fun ReplanChangeProposed.core() =
    ReplanChange(sentence, kind, target, fromStart, toStart, fromMinutes.toUInt(), toMinutes.toUInt(), destination)

private fun Replan.proposed() = ReplanProposed(
    announcement, notices, changes.map { it.proposed() }, unplaceable, pins.kept(), style == TimeStyle.TWENTY_FOUR_HOUR,
    alternatives.map { ReplanOption(it.sentence, it.pins.kept()) },
)

private fun ReplanProposed.core() = Replan(
    announcement, notices, changes.map { it.core() }, unplaceable, pins.core(), style(twentyFourHour),
    alternatives.map { ReplanAlternative(it.sentence, it.pins.core()) },
)

private fun ProposedSitting.proposed() = SittingProposed(sentence, reason, task, block, minutes.toInt())

private fun SittingProposed.core() = ProposedSitting(sentence, reason, task, block, minutes.toUInt())

private fun DayPlan.proposed() = DayPlanProposed(
    announcement, notices, date, sittings.map { it.proposed() }, unfit, pins.exclude, pins.declined,
    style == TimeStyle.TWENTY_FOUR_HOUR,
)

private fun DayPlanProposed.core() =
    DayPlan(announcement, notices, date, sittings.map { it.core() }, unfit, DayPins(exclude, declined), style(twentyFourHour))

private fun style(twentyFourHour: Boolean) = if (twentyFourHour) TimeStyle.TWENTY_FOUR_HOUR else TimeStyle.TWELVE_HOUR
