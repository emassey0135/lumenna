package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.Typing
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.TextRange
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.DisposableEffect
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.OutlinedTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
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
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import io.github.emassey0135.lumenna.app.Swatch
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.collectAsState
import io.github.emassey0135.lumenna.app.Item
import io.github.emassey0135.lumenna.app.Line
import io.github.emassey0135.lumenna.app.placeTitle
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.RowView
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.ActionKind
import io.github.emassey0135.lumenna.app.Syntax


/**
 * A place's tasks, with a filter field above them, or the trash: the app's list (`ViewModel.list`)
 * while this screen shows its place, which it opens as it comes to the front.
 *
 * The filter is the app's too, applied as it is typed and read back: how it was understood and
 * how many tasks it found. A misread filter shows wrong results silently, and wrong results are
 * invisible.
 */
@Composable
fun TaskListScreen(core: Core, navigator: Navigator, screen: Screen.Tasks) {
    val app = core.app
    val view by app.view.collectAsState()
    // This screen's place is the app's while it is in front; back on it, the place again.
    LaunchedEffect(screen.place) { app.send(ShellEvent.Open(screen.place)) }
    val here = view?.takeIf { it.place == screen.place }
    val list = here?.list
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { here != null }
    val trash = screen.place == Place.Trash
    FollowReminders(core, navigator)
    var filter by rememberSaveable(screen, stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue("")) }
    // The app's filter, once nothing typed is on its way: coming back to a place, the app puts
    // back the filter it was left with, and the field shows it. While typing, the field's own.
    val query = here?.query
    LaunchedEffect(query) {
        if (query != null && query != filter.text && app.still) filter = TextFieldValue(query, TextRange(query.length))
    }

    val field = remember { FocusRequester() }
    if (!trash) {
        val shortcuts = LocalShortcuts.current
        // Collected, not a key: clearing the request would change the key, and cancel the
        // effect before the field had focus.
        LaunchedEffect(shortcuts) {
            shortcuts.filterAsked.collect { asked ->
                if (!asked) return@collect
                // The tab was only just chosen: the field takes focus once it is laid out.
                for (frame in 1..10) {
                    withFrameNanos {}
                    if (runCatching { field.requestFocus(FocusDirection.Enter) }.getOrDefault(false)) break
                }
                shortcuts.filterAsked.value = false
            }
        }
        Offer(Command.FILTER) { runCatching { field.requestFocus() } }
    }

    ScreenFrame(here?.title ?: placeTitle(screen.place), core, navigator, actions = {
        if (!trash) {
            IconButton(onClick = { navigator.push(Screen.QuickAdd) }) {
                Icon(Icons.Filled.Add, contentDescription = "Add task")
            }
        }
    }) {
        if (!trash) {
            CompletingField(
                core = core,
                syntax = Syntax.FILTER,
                label = "Filter",
                example = "#Work & overdue",
                value = filter,
                onValueChange = {
                    if (it.text != filter.text) app.send(ShellEvent.Filter(it.text))
                    filter = it
                },
                onSubmit = {},
                imeAction = ImeAction.Search,
                modifier = Modifier.padding(horizontal = 16.dp),
                focus = field,
            )
        }
        Text(
            lines?.readback.orEmpty(),
            style = MaterialTheme.typography.bodySmall,
            color = quiet(),
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )
        AppTaskRows(
            core,
            shownRows(list?.items.orEmpty(), lines?.list.orEmpty()),
            list,
            open = if (trash) null else { row ->
                app.send(ShellEvent.Focus(row.id))
                navigator.push(Screen.Task(row.id))
            },
        )
    }
}

/**
 * The app's task rows: one TalkBack stop each, its title the description and everything else
 * its state; its actions, the app's, are custom accessibility actions, and the same ones on a
 * long press. Expand and Collapse are among them where something is under a row.
 */
@Composable
fun AppTaskRows(
    core: Core,
    items: List<Pair<Item, Line?>>,
    list: io.github.emassey0135.lumenna.app.ListView?,
    open: ((RowView) -> Unit)?,
    openLabel: String = "Show details",
    local: (io.github.emassey0135.lumenna.app.Offered) -> Boolean = { false },
) {
    val state = rememberLazyListState()
    val keys = items.map { it.first.row.id }
    val focus = rememberLanding(core, keys, state)
    LazyColumn(
        Modifier
            .fillMaxSize()
            .semantics { collectionInfo = CollectionInfo(rowCount = items.size, columnCount = 1) },
        state = state,
    ) {
        itemsIndexed(items, key = { _, (item, _) -> item.row.id }) { index, (item, line) ->
            val row = item.row
            ListRow(
                title = row.title,
                detail = line?.detail.orEmpty(),
                speech = spoken(item, line),
                done = row.checked,
                depth = row.depth.toInt(),
                index = index,
                actions = core.app.listRowActions(row.id, list?.offeredOf(item).orEmpty(), local),
                refuse = core.app.refusal(row.id),
                open = open?.let { { it(row) } },
                openLabel = openLabel,
                focus = focus.requester(row.id),
                key = row.id,
                say = core::say,
                swatch = row.swatch,
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
    heading: Boolean = false,
    selected: Boolean = false,
    say: (String) -> Unit = {},
    refuse: ((ActionKind) -> Unit)? = null,
    swatch: Swatch? = null,
) {
    var menu by remember { mutableStateOf(false) }
    val pane = LocalPane.current
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
                .onFocusChanged { if (it.isFocused && focus != null) pane?.row = focus }
                // A row command runs this row's action of that kind; a key the row has no
                // action for goes on to the list, which may move focus with it.
                .onKeyEvent { key ->
                    val command = Command.of(key.nativeKeyEvent) ?: return@onKeyEvent false
                    if (command == Command.ACTIONS) {
                        if (actions.isNotEmpty()) menu = true
                        return@onKeyEvent actions.isNotEmpty()
                    }
                    val action = command.rowAction(actions)
                    if (action != null) {
                        action.run()
                        return@onKeyEvent true
                    }
                    // A row key with nothing to do here: the app says why, in the core's words,
                    // rather than nothing. A heading's actions add one more of what is under it,
                    // so it says nothing of the rest.
                    val kind = command.kind ?: return@onKeyEvent false
                    if (actions.none { it.kind != ActionKind.NEW }) return@onKeyEvent false
                    val refusing = refuse ?: return@onKeyEvent false
                    refusing(kind)
                    true
                }
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
                    rowTitle = title
                    // One description, the title first. As a separate state description the
                    // rest was said before the title.
                    contentDescription = listOf(title, speech).filter { it.isNotEmpty() }.joinToString(", ")
                    collectionItemInfo = CollectionItemInfo(index, 1, 0, 1)
                    if (heading) heading()
                    // The sidebar's row for the place shown.
                    if (selected) this.selected = true
                    customActions = actions.map { action -> CustomAccessibilityAction(action.name) { action.run(); true } }
                    if (acts) {
                        onClick(label = if (open != null) openLabel else "Actions") {
                            if (open != null) open() else menu = true
                            true
                        }
                    }
                }
                .then(if (selected) Modifier.background(MaterialTheme.colorScheme.secondaryContainer) else Modifier)
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
            if (swatch != null) SwatchDot(swatch, Modifier.padding(end = 12.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    title,
                    style = if (heading) MaterialTheme.typography.titleMedium else MaterialTheme.typography.bodyLarge,
                    textDecoration = if (done == true) TextDecoration.LineThrough else null,
                )
                if (detail.isNotEmpty()) {
                    Text(detail, style = MaterialTheme.typography.bodyMedium, color = quiet())
                }
            }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            actions.forEach { action ->
                DropdownMenuItem(text = {
                    Text(action.name, color = if (action.destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                }, onClick = {
                    menu = false
                    action.run()
                })
            }
        }
    }
}

/**
 * A colour's mark beside a name ([Swatch]): a dot in the core's value for the theme's
 * background. It adds nothing to what TalkBack says, which a row clears and sets itself.
 */
@Composable
fun SwatchDot(swatch: Swatch, modifier: Modifier = Modifier) {
    Box(modifier.size(14.dp).clip(CircleShape).background(swatch.colour(isSystemInDarkTheme())))
}

/**
 * Adding a task in a line, the way it would be said: the app's line (`ViewModel.line`). The
 * readback beneath the field says how the line is understood as it is typed — the date,
 * priority, project and labels — since there is no highlighting under the text to show it; an
 * unknown project is an error, and nothing is added until it is fixed. What fits at the cursor
 * is offered beneath, named as the core names it ("project Work", not "#Work"), and put in
 * where the app says. The line starts with what the place adds.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun QuickAddScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val line = view?.line
    val lines by app.lines.collectAsState()
    var opened by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { app.send(ShellEvent.Typing(Typing.Open)) }
    // Added, the app lets the line go, and the screen goes back.
    LaunchedEffect(line == null) { if (line != null) opened = true else if (opened) navigator.back() }
    // Leaving it lets the line go.
    DisposableEffect(Unit) { onDispose { if (app.view.value?.line != null) app.send(ShellEvent.Typing(Typing.Close)) } }
    // The field's own text while typing is on its way, so the caret stays put; the app's once none is.
    var typed by remember { mutableStateOf<TextFieldValue?>(null) }
    var pending by remember { mutableIntStateOf(0) }
    val shown = typed ?: line?.let { TextFieldValue(it.text, TextRange(TextOffsets.utf16(it.cursor, it.text))) } ?: TextFieldValue("")
    LaunchedEffect(line?.text, line?.cursor) { if (pending == 0) typed = null }
    val tell = { value: TextFieldValue ->
        typed = value
        pending++
        app.send(ShellEvent.Typing(Typing.Text(value.text, TextOffsets.bytes(value.selection.end, value.text))))
    }
    LaunchedEffect(view?.line) { if (pending > 0) pending-- }
    val add = { app.send(ShellEvent.QuickAdd(shown.text)) }

    ScreenFrame("New task", core, navigator, actions = {
        IconButton(onClick = { add() }) { Icon(Icons.Filled.Check, contentDescription = "Add") }
    }) {
        val focus = remember { FocusRequester() }
        // Typing starts at once, as it does on the iPhone.
        LaunchedEffect(Unit) { focus.requestFocus() }
        Column(Modifier.padding(horizontal = 16.dp)) {
            OutlinedTextField(
                value = shown,
                onValueChange = { if (it.text != shown.text || it.selection != shown.selection) tell(it) },
                label = { Text("Task") },
                placeholder = { Text("call the bank tomorrow at 3pm p1 #Home @calls") },
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    autoCorrectEnabled = false,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(onDone = { add() }),
            )
            val candidates = line?.offer?.candidates.orEmpty()
            if (candidates.isNotEmpty()) {
                FlowRow(Modifier.padding(top = 4.dp)) {
                    val labels = lines?.candidates.orEmpty()
                    candidates.take(8).forEachIndexed { index, candidate ->
                        SuggestionChip(
                            onClick = {
                                typed = null
                                app.send(ShellEvent.Typing(Typing.Choose(index.toUInt())))
                            },
                            label = { Text(candidate.text) },
                            modifier = Modifier
                                .padding(end = 8.dp)
                                .semantics { contentDescription = "Complete with ${labels.getOrNull(index) ?: candidate.label}" },
                        )
                    }
                }
            }
        }
        val said = line?.readback.orEmpty()
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
