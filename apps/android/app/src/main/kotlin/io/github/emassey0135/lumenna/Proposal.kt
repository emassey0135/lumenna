package io.github.emassey0135.lumenna

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
 * Replan from Now, or Plan the Day: what the core proposes, said in full as the screen opens,
 * then each change a check box, all checked to begin with. Applying makes those checked; while
 * the core proposes more around what was left unchecked, that is the screen's new proposal.
 * Every word is the core's (`proposalWords`), in sentence case.
 */
@Composable
fun ProposalScreen(core: Core, navigator: Navigator, screen: Screen.Proposal) {
    val now = LocalPlannerNow.current
    val model = remember(screen) { ProposalModel(core, screen.kind, screen.date, now) }
    val words = model.words
    LaunchedEffect(model) { model.propose() }
    val apply = { if (model.apply()) navigator.back() }

    ScreenFrame(model.title, core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            model.refused?.let { Text(it, Modifier.padding(vertical = 8.dp), color = MaterialTheme.colorScheme.error) }
            val items = model.items
            if (model.ready && items.isEmpty()) {
                Text(words.nothing, Modifier.padding(vertical = 8.dp))
            }
            items.forEachIndexed { index, item ->
                val on = index in model.checked
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 48.dp)
                        .toggleable(on, role = Role.Checkbox) { model.toggle(index) }
                        .padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(item.sentence, style = MaterialTheme.typography.bodyLarge)
                        // Why this task, as its description.
                        item.reason?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = quiet()) }
                    }
                    // The check box's name is the core's: "Make this change".
                    Checkbox(checked = on, onCheckedChange = null, modifier = Modifier.semantics { contentDescription = words.item })
                }
            }
            if (model.unfit.isNotEmpty()) {
                Heading(button(words.unfit))
                model.unfit.forEach { Text(it, Modifier.padding(vertical = 4.dp)) }
            }
            if (items.isNotEmpty()) {
                Text(words.hint, Modifier.padding(top = 16.dp), style = MaterialTheme.typography.bodyMedium, color = quiet())
            }
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            ) {
                TextButton(modifier = Target, onClick = navigator::back) { Text(button(words.cancel)) }
                if (items.isNotEmpty()) {
                    Button(modifier = Target, onClick = { apply() }) { Text(button(words.apply)) }
                }
            }
        }
    }
}
