package io.github.emassey0135.lumenna.wear

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
import io.github.emassey0135.lumenna.ProposalModel
import io.github.emassey0135.lumenna.button

/**
 * Replan from Now, or Plan the Day, as the phone has it (`ProposalModel`): the proposal said as
 * the screen opens, each change a check box, all checked; applying makes those checked, and
 * what the core proposes next around what was left unchecked becomes the list.
 */
@Composable
fun ProposalScreen(core: Core, navigator: Navigator, screen: Screen.Proposal) {
    val now = LocalPlannerNow.current
    val model = remember(screen) { ProposalModel(core, screen.kind, screen.date, now) }
    val words = model.words
    LaunchedEffect(model) { model.propose() }
    val items = model.items
    WearList {
        item { Heading(model.title) }
        model.refused?.let { refused -> item { Text(refused, color = MaterialTheme.colorScheme.error) } }
        if (model.ready && items.isEmpty()) item { Text(words.nothing) }
        items.forEachIndexed { index, change ->
            item {
                CheckboxButton(
                    checked = index in model.checked,
                    onCheckedChange = { model.toggle(index) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(change.sentence, maxLines = Int.MAX_VALUE) },
                    // Why this task, as its description. Every line of both shows: a sentence
                    // cut short would hide what it changes.
                    secondaryLabel = change.reason?.let { { Text(it, maxLines = Int.MAX_VALUE) } },
                )
            }
        }
        if (model.unfit.isNotEmpty()) {
            item { Heading(button(words.unfit)) }
            model.unfit.forEach { line -> item { Text(line) } }
        }
        // Other ways through the day, each named by what it is, costs and leads to.
        if (model.alternatives.isNotEmpty()) {
            item { Heading(model.others) }
            model.alternatives.forEach { other ->
                item {
                    FilledTonalButton(
                        onClick = { model.take(other) },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(other.sentence, maxLines = Int.MAX_VALUE) },
                    )
                }
            }
        }
        if (items.isNotEmpty()) {
            item { Text(words.hint) }
            item {
                Button(
                    onClick = { if (model.apply()) navigator.back() },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(button(words.apply)) },
                )
            }
        }
        item { FilledTonalButton(onClick = { navigator.back() }, modifier = Modifier.fillMaxWidth(), label = { Text(button(words.cancel)) }) }
    }
}
