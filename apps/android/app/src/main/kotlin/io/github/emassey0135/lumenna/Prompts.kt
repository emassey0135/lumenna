package io.github.emassey0135.lumenna

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.CollectionItemInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.collectionItemInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp

/**
 * One question with a line for the answer: a name, a query, a number of minutes.
 *
 * [hint] is shown beneath the field, where TalkBack reads it with the field and a sighted
 * person sees it too. The field takes focus when the dialog opens, so typing can start at once.
 */
@Composable
fun AskText(
    title: String,
    label: String,
    action: String,
    initial: String = "",
    example: String = "",
    hint: String? = null,
    number: Boolean = false,
    dismiss: () -> Unit,
    done: (String) -> Unit,
) {
    var text by remember { mutableStateOf(TextFieldValue(initial, TextRange(initial.length))) }
    val focus = remember { FocusRequester() }
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                label = { Text(label) },
                placeholder = { if (example.isNotEmpty()) Text(example) },
                supportingText = hint?.let { { Text(it) } },
                keyboardOptions = KeyboardOptions(
                    keyboardType = if (number) KeyboardType.Number else KeyboardType.Text,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(onDone = { done(text.text) }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
        },
        confirmButton = { TextButton(modifier = Target, onClick = { done(text.text) }) { Text(action) } },
        dismissButton = { TextButton(modifier = Target, onClick = dismiss) { Text("Cancel") } },
    )
    LaunchedEffect(Unit) { focus.requestFocus() }
}


/**
 * Choosing one of many, narrowed by typing: a task to wait for or to go under, a block to put
 * a task in. What is offered is the core's, and never empty: when there is nothing, the core
 * says why instead. Each choice is one TalkBack stop, said as "Deep work, 9:00 AM to 11:00 AM".
 */
@Composable
fun Choose(title: String, choices: List<Option>, yes: String, dismiss: () -> Unit, chosen: (Option) -> Unit) {
    var narrow by remember { mutableStateOf("") }
    val shown = choices.filter {
        narrow.isBlank() || it.title.contains(narrow, ignoreCase = true) || it.detail.contains(narrow, ignoreCase = true)
    }
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text(title) },
        text = {
            Column {
                if (choices.size > 6) {
                    OutlinedTextField(
                        value = narrow,
                        onValueChange = { narrow = it },
                        label = { Text("Narrow the list") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (shown.isEmpty()) {
                    Text("Nothing matches", Modifier.padding(vertical = 12.dp))
                }
                LazyColumn(
                    Modifier
                        .heightIn(max = 360.dp)
                        .semantics { collectionInfo = CollectionInfo(shown.size, 1) },
                ) {
                    itemsIndexed(shown, key = { _, choice -> choice.key }) { index, choice ->
                        Column(
                            Modifier
                                .fillMaxWidth()
                                // TalkBack says what choosing it does: "double-tap to move".
                                .clickable(role = Role.Button, onClickLabel = yes) { chosen(choice) }
                                .semantics(mergeDescendants = true) {
                                    collectionItemInfo = CollectionItemInfo(index, 1, 0, 1)
                                }
                                .padding(start = (16 * choice.depth).dp, top = 12.dp, bottom = 12.dp),
                        ) {
                            Text(choice.title, style = MaterialTheme.typography.bodyLarge)
                            if (choice.detail.isNotEmpty()) {
                                Text(choice.detail, style = MaterialTheme.typography.bodyMedium, color = quiet())
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(modifier = Target, onClick = dismiss) { Text("Cancel") } },
    )
}

/** A yes-or-no question before something that cannot be taken back. */
@Composable
fun Confirm(
    title: String,
    message: String,
    action: String,
    dismiss: () -> Unit,
    destructive: Boolean = false,
    confirmed: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text(title) },
        text = { Text(message) },
        confirmButton = {
            TextButton(modifier = Target, onClick = confirmed) {
                Text(action, color = if (destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
            }
        },
        dismissButton = { TextButton(modifier = Target, onClick = dismiss) { Text("Cancel") } },
    )
}

/** The one question a screen is asking, if any. */
class Prompter {
    var current by mutableStateOf<(@Composable () -> Unit)?>(null)
        private set

    /** Asks [question]; it closes itself with [close]. */
    fun show(question: @Composable () -> Unit) {
        current = question
    }

    fun close() {
        current = null
    }
}

@Composable
fun rememberPrompter(): Prompter = remember { Prompter() }

/** Shows whatever is being asked. */
@Composable
fun Prompter.Host() {
    current?.invoke()
}
