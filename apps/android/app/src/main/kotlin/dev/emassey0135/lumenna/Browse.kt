package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.placeTitle
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.Swatch
import dev.emassey0135.lumenna.app.TabKind
import dev.emassey0135.lumenna.app.ActionKind
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import dev.emassey0135.lumenna.app.ShellEvent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.emassey0135.lumenna.app.Action

/** One entry of a menu: what identifies it, its name, what it holds, and its colour, drawn beside the name. */
data class Item(val key: String, val title: String, val detail: String = "", val swatch: Swatch? = null)

/**
 * A fixed menu — Browse's places and groups, Settings' pages — one TalkBack stop per entry, each
 * opening its screen. Nothing here changes under a row, so there is no row to land on again.
 */
@Composable
fun MenuScreen(title: String, core: Core, navigator: Navigator, items: List<Item>, open: (Item) -> Unit) {
    ScreenFrame(title, core, navigator) {
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(items.size, 1) }) {
            itemsIndexed(items, key = { _, item -> item.key }) { index, item ->
                ListRow(
                    title = item.title,
                    detail = item.detail,
                    speech = item.detail,
                    index = index,
                    actions = emptyList(),
                    open = { open(item) },
                    openLabel = appWords.open,
                    key = item.key,
                    say = core::say,
                    swatch = item.swatch,
                )
                HorizontalDivider()
            }
        }
    }
}

/**
 * Browse, the core's (`Arrangement.browse`): every place that is no tab of its own, in the
 * sidebar's order with what is in it, and each group with how many places it holds. A place opens
 * here; a group opens its own screen.
 */
@Composable
fun BrowseScreen(core: Core, navigator: Navigator) {
    val view by core.app.view.collectAsState()
    val sidebar = view?.sidebar.orEmpty()
    val arranged = remember(sidebar) { arrangement(sidebar) }
    val entries = arranged.browse
    val items = entries.map { Item(it.key, it.title, it.detail, it.swatch) }
    MenuScreen(
        arranged.tabs.firstOrNull { it.kind == TabKind.Browse }?.title.orEmpty(), core, navigator, items,
        open = { item ->
            when (val kind = entries.firstOrNull { it.key == item.key }?.kind) {
                is SidebarKind.Place -> navigator.push(Destination.At(kind.field0).screen())
                is SidebarKind.Group -> navigator.push(Screen.Group(kind.field0))
                null -> {}
            }
        },
    )
}

/**
 * One group's screen, the core's (`GroupScreen`): the project tree, the labels or the saved
 * filters, named by its heading, with the heading's New as the add button, each place's actions
 * and folding the app's, and the core's words when there are none.
 */
@Composable
fun GroupScreen(core: Core, navigator: Navigator, group: SidebarGroup) {
    val app = core.app
    val view by app.view.collectAsState()
    val sidebar = view?.sidebar.orEmpty()
    val screen = remember(sidebar) { arrangement(sidebar).groups.firstOrNull { it.group == group } }
    val heading = screen?.heading
    // What choosing one does, in the core's words: a label's tasks are the ones wearing it.
    val openLabel = if (group == SidebarGroup.LABELS) appWords.showTasksWearingIt else appWords.showItsTasks
    ScreenFrame(heading?.title.orEmpty(), core, navigator, actions = {
        heading?.offered?.firstOrNull { it.kind == ActionKind.NEW }?.let { new ->
            IconButton(onClick = { app.send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, heading.key), new)) }) { Icon(Icons.Filled.Add, contentDescription = new.name) }
        }
    }) {
        // Nothing in it yet, said in the core's words: "No labels."
        if (screen != null && screen.entries.isEmpty()) {
            Text(screen.empty, style = MaterialTheme.typography.bodyMedium, color = quiet(), modifier = Modifier.padding(16.dp))
        }
        PlaceRows(
            core, placeRows(screen?.entries.orEmpty()),
            open = { row -> (row.entry.kind as? SidebarKind.Place)?.field0?.let { place -> { navigator.push(Screen.Tasks(place)) } } },
            openLabel = { openLabel },
        )
    }
}

/**
 * Every block series, as the app lists them (its Blocks place): said as a task row is, when in
 * this device's clock ("every weekday at 9:00 AM"), then how long. Opening one changes every
 * occurrence, in the app's block form, as New block in the top bar opens it for a new one.
 */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Open(dev.emassey0135.lumenna.app.Place.Blocks)) }
    val here = view?.takeIf { it.place == dev.emassey0135.lumenna.app.Place.Blocks }
    val list = here?.list
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { here != null }
    FollowBlockForm(core, navigator)
    // The place's own actions, the core's (New block, Undo, Redo): the app opens the block form.
    ScreenFrame(placeTitle(Place.Blocks), core, navigator, screen = ScreenButtons(here?.screenActions.orEmpty())) {
        val count = lines?.readback.orEmpty()
        if (count.isNotEmpty()) {
            Text(
                count,
                style = MaterialTheme.typography.bodyMedium,
                color = quiet(),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        AppTaskRows(core, shownRows(list?.items.orEmpty(), lines?.list.orEmpty()), list, open = { row ->
            // A block's own screen is its form, for every occurrence: its Edit, on the row in hand.
            app.rowKeys(row.id).key("open")
        }, openLabel = appWords.editEveryOccurrence)
    }
}
