package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.Candidate
import dev.emassey0135.lumenna.app.Syntax
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp

/**
 * A line in the quick-add or filter language, with what could go at the cursor offered two
 * ways: as buttons beneath it that change as one types — one TalkBack swipe past the field, quick
 * to check for whether anything completes, and named as the core names them: "project Work", not
 * "#Work", since the sigil is punctuation speech may skip — and as a list in a dialog, with clear
 * edges, for someone who knows they want one: the field's Show completions action, its button,
 * or Down (or Ctrl+Space) on a keyboard. Nothing opens by itself.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CompletingField(
    core: Core,
    syntax: Syntax,
    label: String,
    example: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    onSubmit: () -> Unit,
    modifier: Modifier = Modifier,
    imeAction: ImeAction = ImeAction.Done,
    focused: Boolean = false,
    focus: FocusRequester = remember { FocusRequester() },
    /** Beneath the field, read by TalkBack with it: why an answer was refused, else the question's hint. */
    hint: String? = null,
) {
    if (focused) LaunchedEffect(Unit) { focus.requestFocus() }
    // What could go at the cursor, and the span it would replace, in UTF-16 units.
    val offered: Pair<List<Candidate>, IntRange>? = remember(value.text, value.selection) {
        if (value.text.isBlank()) null
        else runCatching {
            core.app.complete(value.text, TextOffsets.bytes(value.selection.end, value.text), syntax)
        }.getOrNull()?.let { found ->
            found.candidates to (TextOffsets.utf16(found.start, value.text) until TextOffsets.utf16(found.end, value.text))
        }
    }

    var listing by remember { mutableStateOf(false) }
    val (candidates, span) = offered ?: (emptyList<Candidate>() to IntRange.EMPTY)
    val choose = { candidate: Candidate ->
        val text = value.text.replaceRange(span.first, span.last + 1, candidate.text)
        onValueChange(TextFieldValue(text, TextRange(span.first + candidate.text.length)))
    }

    Column(modifier) {
        OutlinedTextField(
            value = value,
            onValueChange = onValueChange,
            label = { Text(label) },
            placeholder = { Text(example) },
            supportingText = hint?.let { { Text(it) } },
            modifier = Modifier
                .fillMaxWidth()
                .focusRequester(focus)
                .semantics { customActions = listOf(CustomAccessibilityAction(SHOW_COMPLETIONS) { listing = true; true }) }
                .onPreviewKeyEvent { event ->
                    // Down, or Ctrl+Space, opens the list on a keyboard, as on every desktop.
                    val asked = event.key == Key.DirectionDown || (event.key == Key.Spacebar && event.isCtrlPressed)
                    if (event.type == KeyEventType.KeyDown && asked) { listing = true; true } else false
                },
            keyboardOptions = KeyboardOptions(
                capitalization = KeyboardCapitalization.None,
                autoCorrectEnabled = false,
                imeAction = imeAction,
            ),
            keyboardActions = KeyboardActions(onDone = { onSubmit() }, onSearch = { onSubmit() }),
        )
        if (candidates.isNotEmpty()) {
            FlowRow(Modifier.padding(top = 4.dp)) {
                candidates.take(SHOWN_COMPLETIONS).forEach { candidate ->
                    SuggestionChip(
                        onClick = { choose(candidate) },
                        label = { Text(candidate.text) },
                        modifier = Modifier
                            .padding(end = 8.dp)
                            .semantics { contentDescription = dev.emassey0135.lumenna.app.completeWith(candidate) },
                    )
                }
            }
            OutlinedButton(onClick = { listing = true }, modifier = Target) { Text(SHOW_COMPLETIONS) }
        }
    }
    if (listing) {
        AlertDialog(
            onDismissRequest = { listing = false },
            title = { Text(dev.emassey0135.lumenna.app.completions(true)) },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    if (candidates.isEmpty()) Text(quickWords.nothingToComplete)
                    candidates.forEach { candidate ->
                        Text(
                            dev.emassey0135.lumenna.app.candidateLine(candidate),
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 48.dp)
                                .clickable(role = androidx.compose.ui.semantics.Role.Button) { listing = false; choose(candidate) }
                                .padding(vertical = 12.dp),
                        )
                    }
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(modifier = Target, onClick = { listing = false }) { Text(quickWords.cancel) } },
        )
    }
}

/** The field's action and button that open the list, the core's words. */
val SHOW_COMPLETIONS: String get() = quickWords.showCompletions
