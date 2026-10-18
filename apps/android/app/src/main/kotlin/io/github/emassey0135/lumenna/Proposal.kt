package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.ProposalEvent
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * Replan from Now, or Plan the Day: the app's proposal (`ViewModel.proposal`), said in full as
 * the screen opens, then each change a check box, all checked to begin with. Applying makes
 * those checked; while the core proposes more around what was left unchecked, that is the
 * screen's new proposal, and once nothing is left the app closes it. Every word is the core's
 * (`proposalWords`), in sentence case.
 */
@Composable
fun ProposalScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val proposal = view?.proposal
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(proposal == null) { if (proposal != null) shown = true else if (shown) navigator.back() }
    // Leaving it gives it up.
    DisposableEffect(Unit) { onDispose { if (app.view.value?.proposal != null) app.send(ShellEvent.Proposal(ProposalEvent.Close)) } }
    val send = { event: ProposalEvent -> app.send(ShellEvent.Proposal(event)) }

    ScreenFrame(proposal?.words?.title ?: "", core, navigator) {
        if (proposal == null) return@ScreenFrame
        val words = proposal.words
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            proposal.refusal?.let { Text(it, Modifier.padding(vertical = 8.dp), color = MaterialTheme.colorScheme.error) }
            val items = proposal.changes
            if (proposal.ready && items.isEmpty()) {
                Text(words.nothing, Modifier.padding(vertical = 8.dp))
            }
            items.forEachIndexed { index, item ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 48.dp)
                        .toggleable(item.checked, role = Role.Checkbox) { send(ProposalEvent.Check(index.toUInt(), it)) }
                        .padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(item.sentence, style = MaterialTheme.typography.bodyLarge)
                        // Why this task, as its description.
                        item.reason?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = quiet()) }
                    }
                    // The check box's name is the core's: "Make this change".
                    Checkbox(checked = item.checked, onCheckedChange = null, modifier = Modifier.semantics { contentDescription = words.item })
                }
            }
            if (proposal.unfit.isNotEmpty()) {
                Heading(words.unfit)
                proposal.unfit.forEach { Text(it, Modifier.padding(vertical = 4.dp)) }
            }
            // Other ways through the day, each named by what it is, costs and leads to.
            if (proposal.others.isNotEmpty() && words.others.isNotEmpty()) {
                Heading(words.others)
                proposal.others.forEachIndexed { index, sentence ->
                    OutlinedButton(
                        modifier = Target.fillMaxWidth().padding(vertical = 4.dp),
                        enabled = !proposal.busy,
                        onClick = { send(ProposalEvent.Take(index.toUInt())) },
                    ) { Text(sentence, Modifier.fillMaxWidth()) }
                }
            }
            if (items.isNotEmpty()) {
                Text(words.hint, Modifier.padding(top = 16.dp), style = MaterialTheme.typography.bodyMedium, color = quiet())
            }
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            ) {
                TextButton(modifier = Target, onClick = navigator::back) { Text(words.cancel) }
                if (items.isNotEmpty()) {
                    Button(modifier = Target, enabled = !proposal.busy, onClick = { send(ProposalEvent.Apply) }) { Text(words.apply) }
                }
            }
        }
    }
}

/** The app's proposal screen: shown over [navigator]'s screen while the app has one open. */
@Composable
fun FollowProposal(core: Core, navigator: Navigator) {
    val view by core.app.view.collectAsState()
    val open = view?.proposal != null
    LaunchedEffect(open) { if (open && navigator.current != Screen.Proposal) navigator.push(Screen.Proposal) }
}
