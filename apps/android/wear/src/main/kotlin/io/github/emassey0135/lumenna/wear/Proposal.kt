package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.ShowingProposal
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.ProposalEvent
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.runtime.collectAsState
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.CheckboxButton
import androidx.wear.compose.material3.FilledTonalButton
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.LocalPlannerNow

/**
 * Replan from Now, or Plan the Day: the app's proposal, as the phone has it, said as the screen
 * opens, each change a check box the app keeps, all checked; applying makes those checked, and
 * what the core proposes next around what was left unchecked becomes the list, until nothing is
 * left and the app closes it.
 */
@Composable
fun ProposalScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val proposal = view?.proposal
    ShowingProposal(app, navigator::back)
    val send = { event: ProposalEvent -> app.send(ShellEvent.Proposal(event)) }
    WearList {
        if (proposal == null) return@WearList
        val words = proposal.words
        val items = proposal.changes
        item { Heading(words.title) }
        proposal.refusal?.let { refused -> item { Text(refused, color = MaterialTheme.colorScheme.error) } }
        if (proposal.ready && items.isEmpty()) item { Text(words.nothing) }
        items.forEachIndexed { index, change ->
            item {
                CheckboxButton(
                    checked = change.checked,
                    onCheckedChange = { send(ProposalEvent.Check(index.toUInt(), it)) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(change.sentence, maxLines = Int.MAX_VALUE) },
                    // Why this task, as its description. Every line of both shows: a sentence
                    // cut short would hide what it changes.
                    secondaryLabel = change.reason?.let { { Text(it, maxLines = Int.MAX_VALUE) } },
                )
            }
        }
        if (proposal.unfit.isNotEmpty()) {
            item { Heading(words.unfit) }
            proposal.unfit.forEach { line -> item { Text(line) } }
        }
        // Other ways through the day, each named by what it is, costs and leads to.
        if (proposal.others.isNotEmpty() && words.others.isNotEmpty()) {
            item { Heading(words.others) }
            proposal.others.forEachIndexed { index, sentence ->
                item {
                    FilledTonalButton(
                        onClick = { send(ProposalEvent.Take(index.toUInt())) },
                        enabled = !proposal.busy,
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(sentence, maxLines = Int.MAX_VALUE) },
                    )
                }
            }
        }
        if (items.isNotEmpty()) {
            item { Text(words.hint) }
            item {
                Button(
                    onClick = { send(ProposalEvent.Apply) },
                    enabled = !proposal.busy,
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(words.apply) },
                )
            }
        }
        item { FilledTonalButton(onClick = { navigator.back() }, modifier = Modifier.fillMaxWidth(), label = { Text(words.cancel) }) }
    }
}
