package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
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
import io.github.emassey0135.lumenna.core.Candidate
import io.github.emassey0135.lumenna.core.Syntax

/**
 * A line in the quick-add or filter language, with what could go at the cursor offered as
 * buttons beneath it — one TalkBack swipe past the field, and named as the core names
 * them: "project Work", not "#Work", since the sigil is punctuation speech may skip.
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
) {
    if (focused) LaunchedEffect(Unit) { focus.requestFocus() }
    // What could go at the cursor, and the span it would replace, in UTF-16 units.
    val offered: Pair<List<Candidate>, IntRange>? = remember(value.text, value.selection) {
        if (value.text.isBlank()) null
        else core.attempt {
            core.lumenna.completeText(value.text, TextOffsets.bytes(value.selection.end, value.text), syntax)
        }?.let { found ->
            found.candidates to (TextOffsets.utf16(found.start, value.text) until TextOffsets.utf16(found.end, value.text))
        }
    }

    Column(modifier) {
        OutlinedTextField(
            value = value,
            onValueChange = onValueChange,
            label = { Text(label) },
            placeholder = { Text(example) },
            modifier = Modifier.fillMaxWidth().focusRequester(focus),
            keyboardOptions = KeyboardOptions(
                capitalization = KeyboardCapitalization.None,
                autoCorrectEnabled = false,
                imeAction = imeAction,
            ),
            keyboardActions = KeyboardActions(onDone = { onSubmit() }, onSearch = { onSubmit() }),
        )
        val (candidates, span) = offered ?: (emptyList<Candidate>() to IntRange.EMPTY)
        if (candidates.isNotEmpty()) {
            FlowRow(Modifier.padding(top = 4.dp)) {
                candidates.take(8).forEach { candidate ->
                    SuggestionChip(
                        onClick = {
                            val text = value.text.replaceRange(span.first, span.last + 1, candidate.text)
                            onValueChange(TextFieldValue(text, TextRange(span.first + candidate.text.length)))
                        },
                        label = { Text(candidate.text) },
                        modifier = Modifier
                            .padding(end = 8.dp)
                            .semantics { contentDescription = "Complete with ${candidate.label}" },
                    )
                }
            }
        }
    }
}
