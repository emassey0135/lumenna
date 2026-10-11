package io.github.emassey0135.lumenna

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.core.DayPins
import io.github.emassey0135.lumenna.core.DayPlan
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.ProposalWords
import io.github.emassey0135.lumenna.core.Replan
import io.github.emassey0135.lumenna.core.ReplanPins
import io.github.emassey0135.lumenna.core.proposalWords

// Replan from Now and Plan the Day, for the phone and the watch alike: what the core proposes,
// which of it is checked, and applying it. Every sentence is the core's; each app only shows
// the proposal its own way.

/**
 * The moment a proposal is made against, `YYYY-MM-DDTHH:MM`: none in the app, which plans from
 * now. A test gives one, so what is proposed does not depend on the hour it runs.
 */
val LocalPlannerNow = staticCompositionLocalOf<String?> { null }

/** One line of a proposal, to accept or not: a replan's change, a day plan's sitting. */
data class ProposalItem(val sentence: String, val reason: String?)

/**
 * A proposal's screen: [kind] is [ActionKind.REPLAN] or [ActionKind.PLAN_DAY], for [date]
 * (the day action's target; a replan is always of the rest of today).
 *
 * Every change starts checked. Applying makes those checked and, while the core proposes
 * something more around what was left unchecked, shows that as the new proposal: a change
 * taken alone can leave the day crowded.
 */
class ProposalModel(private val core: Core, val kind: ActionKind, private val date: String?, private val now: String? = null) {
    /** The screen's words: its title, the check box's name, the buttons, the hint. */
    val words: ProposalWords = proposalWords(kind)

    private var replan by mutableStateOf<Replan?>(null)
    private var plan by mutableStateOf<DayPlan?>(null)

    /** Which of [items] are checked, by position. */
    var checked by mutableStateOf<Set<Int>>(emptySet())
        private set

    /** Why the last apply was refused, shown on the screen until the next. */
    var refused by mutableStateOf<String?>(null)
        private set

    /** Whether the core has answered at all. */
    val ready: Boolean get() = replan != null || plan != null

    val items: List<ProposalItem>
        get() = replan?.changes?.map { ProposalItem(it.sentence, null) }
            ?: plan?.sittings?.map { ProposalItem(it.sentence, it.reason) }.orEmpty()

    /** What does not fit, said, under the heading [ProposalWords.unfit]. */
    val unfit: List<String> get() = replan?.unplaceable ?: plan?.unfit.orEmpty()

    /** The whole proposal in a sentence, as the core said it. */
    val announcement: String
        get() = replan?.let { sentence(it.announcement, it.notices) } ?: plan?.let { sentence(it.announcement, it.notices) }.orEmpty()

    /** The screen's title, in sentence case as Android and Wear OS write. */
    val title: String get() = button(words.title)

    fun toggle(index: Int) {
        checked = if (index in checked) checked - index else checked + index
    }

    /**
     * Asks the core what it proposes, keeping to what was decided so far, and says it: the
     * announcement is the whole proposal, which for someone who cannot glance at a timeline is
     * the check. False if the core could not say, which is said instead.
     */
    fun propose(): Boolean {
        val style = Clock.style
        val proposed = core.attempt {
            when (kind) {
                ActionKind.PLAN_DAY -> plan = core.lumenna.proposeDayPlan(
                    plan?.date ?: date, style, plan?.pins ?: DayPins(emptyList(), emptyList()), now,
                )
                else -> replan = core.lumenna.replan(style, replan?.pins ?: ReplanPins(emptyList(), emptyList(), emptyList()), now)
            }
        } != null
        if (proposed) {
            checked = items.indices.toSet()
            core.say(announcement)
        }
        return proposed
    }

    /**
     * Makes the changes checked, says what was made, and takes up what is proposed now.
     * Returns whether the screen is finished: nothing more is proposed. A refusal — the day
     * moved since — is kept in [refused], and the day is proposed again.
     */
    fun apply(): Boolean {
        refused = null
        return try {
            val (said, more) = when (kind) {
                ActionKind.PLAN_DAY -> {
                    val shown = plan ?: return true
                    val done = core.lumenna.applyDayPlan(shown, shown.sittings.filterIndexed { i, _ -> i in checked }, now)
                    plan = done.next
                    sentence(done.announcement, done.notices) to done.next.sittings.isNotEmpty()
                }
                else -> {
                    val shown = replan ?: return true
                    val done = core.lumenna.applyReplan(shown, shown.changes.filterIndexed { i, _ -> i in checked }, now)
                    replan = done.next
                    sentence(done.announcement, done.notices) to done.next.changes.isNotEmpty()
                }
            }
            checked = items.indices.toSet()
            core.changed()
            core.say(said)
            !more
        } catch (error: LumennaException) {
            refused = error.sentence
            core.say(error.sentence)
            propose()
            false
        }
    }
}
