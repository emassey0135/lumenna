package io.github.emassey0135.lumenna

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.CollectionItemInfo
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.collectionItemInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.Preview
import io.github.emassey0135.lumenna.core.RowView
import io.github.emassey0135.lumenna.core.Rows
import io.github.emassey0135.lumenna.core.Syntax

/** Something to do to a row: its name, as TalkBack and the long-press menu both say it. */
data class RowAction(val name: String, val run: () -> Unit)

/**
 * Tasks, with a filter field above them (§16.1: task list + filter entry), or the trash.
 *
 * The filter is applied as it is typed, and read back: how it was understood and how many
 * tasks it found. A misread filter shows wrong results silently, and wrong results are
 * invisible (§6.3).
 */
@Composable
fun TaskListScreen(core: Core, navigator: Navigator, screen: Screen.Tasks, changes: Long) {
    var filter by rememberSaveable(screen, stateSaver = TextFieldValue.Saver) {
        mutableStateOf(TextFieldValue(screen.query))
    }
    var erasing by remember { mutableStateOf<RowView?>(null) }
    // A filter part-typed is often not one yet; its error is the readback, not an alert.
    val listing: Result<Rows> = remember(filter.text, changes) {
        runCatching { core.lumenna.listTasks(filter.text) }
    }

    ScreenFrame(screen.title, core, navigator, actions = {
        if (!screen.trash) {
            IconButton(onClick = { navigator.push(Screen.QuickAdd(screen.prefix)) }) {
                Icon(Icons.Filled.Add, contentDescription = "Add task")
            }
        }
    }) {
        if (!screen.trash) {
            CompletingField(
                core = core,
                syntax = Syntax.FILTER,
                label = "Filter",
                example = "#Work & overdue",
                value = filter,
                onValueChange = { filter = it },
                onSubmit = {},
                imeAction = ImeAction.Search,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
        Text(
            readback(listing),
            style = MaterialTheme.typography.bodySmall,
            color = quiet(),
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )
        val rows = listing.getOrNull()?.rows.orEmpty()
        TaskRows(
            core,
            rows,
            actions = { row ->
                if (screen.trash) {
                    listOf(
                        RowAction("Restore") { core.change { it.restoreTask(row.id) } },
                        RowAction("Erase") { erasing = row },
                    )
                } else {
                    listOf(
                        RowAction(if (row.checked == true) "Mark Not Done" else "Mark Done") {
                            core.change { if (row.checked == true) it.uncompleteTask(row.id) else it.completeTask(row.id) }
                        },
                        RowAction("Delete") { core.change { it.trashTask(row.id) } },
                    )
                }
            },
            open = if (screen.trash) null else { row -> navigator.push(Screen.Task(row.id)) },
        )
    }

    erasing?.let { row ->
        AlertDialog(
            onDismissRequest = { erasing = null },
            title = { Text("Erase ${row.title}?") },
            text = { Text("It goes for good, with its history. This cannot be undone.") },
            confirmButton = {
                TextButton(modifier = Target, onClick = {
                    erasing = null
                    core.change { it.eraseTask(row.id) }
                }) { Text("Erase") }
            },
            dismissButton = { TextButton(modifier = Target, onClick = { erasing = null }) { Text("Cancel") } },
        )
    }
}

/** How the filter was understood, and what it found — or why it is not one yet. */
private fun readback(listing: Result<Rows>): String = listing.fold(
    onSuccess = { rows ->
        val unresolved = rows.query?.unresolved.orEmpty().map { name ->
            "no ${name.kind} called ${name.name}" + (name.suggestion?.let { ", did you mean $it?" } ?: "")
        }
        (listOfNotNull(rows.query?.description?.takeIf { it.isNotBlank() }, rows.announcement) + unresolved)
            .joinToString(". ").replaceFirstChar { it.uppercase() }
    },
    onFailure = { (it as? LumennaException)?.sentence ?: it.message.orEmpty() },
)

/**
 * Task rows: one TalkBack stop each, its title the description and everything else its
 * state; its actions are custom accessibility actions, and the same ones on a long press.
 */
@Composable
fun TaskRows(core: Core, rows: List<RowView>, actions: (RowView) -> List<RowAction>, open: ((RowView) -> Unit)?) {
    val state = rememberLazyListState()
    val focus = rememberRowFocus(core, rows.map { it.id }, state)
    LazyColumn(
        Modifier
            .fillMaxSize()
            .semantics { collectionInfo = CollectionInfo(rowCount = rows.size, columnCount = 1) },
        state = state,
    ) {
        itemsIndexed(rows, key = { _, row -> row.id }) { index, row ->
            val previous = if (index > 0) rows[index - 1].depth else null
            ListRow(
                title = row.title,
                detail = listOfNotNull(row.value).plus(row.state.filter { it != "ready" }).joinToString(", "),
                speech = RowSpeech.value(row, previous),
                done = row.checked,
                depth = row.depth.toInt(),
                index = index,
                actions = focus.actions(row.id, index, actions(row)),
                open = open?.let { { it(row) } },
                openLabel = "Show details",
                focus = focus.requester(row.id),
                key = row.id,
            )
            HorizontalDivider()
        }
    }
}

/**
 * One row of any list: a title, a quieter line beneath it, indented by depth for the eye.
 * TalkBack hears [title], then [speech]; the indentation says nothing to it, which is why
 * [speech] carries the level where it changes.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ListRow(
    title: String,
    detail: String,
    speech: String,
    index: Int,
    actions: List<RowAction>,
    open: (() -> Unit)?,
    openLabel: String,
    done: Boolean? = null,
    depth: Int = 0,
    focus: FocusRequester? = null,
    key: String? = null,
) {
    var menu by remember { mutableStateOf(false) }
    // A row with nothing to do — now, a free hour with no block to add — is text, not a button.
    val acts = open != null || actions.isNotEmpty()
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                // Focusable even under touch, so focus — and TalkBack with it — can be put
                // back on the row after a change (`RowFocus`).
                .then(
                    if (focus == null) Modifier
                    else Modifier.focusRequester(focus).focusProperties { canFocus = true }
                        .then(if (acts) Modifier else Modifier.focusable()),
                )
                .then(
                    if (!acts) Modifier
                    else Modifier.combinedClickable(
                        onClick = { if (open != null) open() else menu = true },
                        onClickLabel = if (open != null) openLabel else "Actions",
                        onLongClick = if (actions.isEmpty()) null else ({ menu = true }),
                        onLongClickLabel = "Actions",
                    ),
                )
                .clearAndSetSemantics {
                    if (key != null) rowKey = key
                    contentDescription = title
                    stateDescription = speech
                    collectionItemInfo = CollectionItemInfo(index, 1, 0, 1)
                    customActions = actions.map { action -> CustomAccessibilityAction(action.name) { action.run(); true } }
                    if (acts) {
                        onClick(label = if (open != null) openLabel else "Actions") {
                            if (open != null) open() else menu = true
                            true
                        }
                    }
                }
                .padding(start = (16 + 24 * depth).dp, end = 16.dp, top = 12.dp, bottom = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (done != null) {
                Icon(
                    if (done) Icons.Filled.CheckCircle else Glyphs.circle,
                    contentDescription = null,
                    tint = if (done) MaterialTheme.colorScheme.primary else quiet(),
                    modifier = Modifier.padding(end = 12.dp).size(24.dp),
                )
            }
            Column(Modifier.weight(1f)) {
                Text(
                    title,
                    style = MaterialTheme.typography.bodyLarge,
                    textDecoration = if (done == true) TextDecoration.LineThrough else null,
                )
                if (detail.isNotEmpty()) {
                    Text(detail, style = MaterialTheme.typography.bodyMedium, color = quiet())
                }
            }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            actions.forEach { action ->
                DropdownMenuItem(text = { Text(action.name) }, onClick = {
                    menu = false
                    action.run()
                })
            }
        }
    }
}

/**
 * Adding a task in a line, the way it would be said (§6.1). The readback beneath the field
 * says how the line is understood as it is typed — the date, priority, project and labels —
 * since there is no highlighting under the text to show it; an unknown project is an error,
 * and nothing is added until it is fixed.
 */
@Composable
fun QuickAddScreen(core: Core, navigator: Navigator, screen: Screen.QuickAdd) {
    var line by rememberSaveable(stateSaver = TextFieldValue.Saver) {
        mutableStateOf(TextFieldValue(screen.prefix, androidx.compose.ui.text.TextRange(screen.prefix.length)))
    }
    val preview: Preview? = remember(line.text) {
        if (line.text.isBlank()) null else runCatching { core.lumenna.previewTask(line.text) }.getOrNull()
    }
    val add = {
        when {
            line.text.isBlank() -> core.say("Nothing to add")
            preview?.hasErrors == true ->
                core.say(preview.diagnostics.filter { it.severity == "error" }.joinToString("; ") { it.message })
            else -> core.change { it.addTask(line.text) }?.let { navigator.back() }
        }
    }

    ScreenFrame("New Task", core, navigator, actions = {
        IconButton(onClick = { add() }) { Icon(Icons.Filled.Check, contentDescription = "Add") }
    }) {
        CompletingField(
            core = core,
            syntax = Syntax.QUICK_ADD,
            label = "Task",
            example = "call the bank tomorrow at 3pm p1 #Home @calls",
            value = line,
            onValueChange = { line = it },
            onSubmit = { add() },
            modifier = Modifier.padding(horizontal = 16.dp),
            // Typing starts at once, as it does on the iPhone.
            focused = true,
        )
        val said = preview?.let { sentence(it.announcement, it.diagnostics.map { d -> d.message }) }.orEmpty()
        Text(
            said,
            style = MaterialTheme.typography.bodyMedium,
            color = quiet(),
            modifier = Modifier
                .padding(16.dp)
                .semantics { contentDescription = if (said.isEmpty()) "" else "Will add: $said" },
        )
    }
}
