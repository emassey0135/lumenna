package dev.emassey0135.lumenna

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.ProposalEvent
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Typing

/**
 * A screen showing something the app has open — a proposal, a reminder list, the block form —
 * goes [back] once the app lets it go, having shown it; and leaving the screen gives it up in
 * the app ([close]) while [stillOpen]. The same on the phone and the watch.
 */
@Composable
fun ShowingApp(open: Boolean, stillOpen: () -> Boolean, back: () -> Unit, close: () -> Unit) {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(open) { if (open) shown = true else if (shown) back() }
    DisposableEffect(Unit) { onDispose { if (stillOpen()) close() } }
}

/** Replan from Now's or Plan the Day's screen, on the app's proposal. */
@Composable
fun ShowingProposal(app: AppModel, back: () -> Unit) {
    val view by app.view.collectAsState()
    ShowingApp(
        open = view?.proposal != null,
        stillOpen = { app.view.value?.proposal != null },
        back = back,
        close = { app.send(ShellEvent.Proposal(ProposalEvent.Close)) },
    )
}

/** A reminder list's screen, on the app's open list. */
@Composable
fun ShowingReminders(app: AppModel, back: () -> Unit) {
    val view by app.view.collectAsState()
    ShowingApp(
        open = view?.reminders != null,
        stillOpen = { app.view.value?.reminders != null },
        back = back,
        close = { app.send(ShellEvent.CloseReminders) },
    )
}

/**
 * Quick add's screen, on the app's line: opened with the screen, which goes [back] once the
 * line is added and let go, and leaving the screen lets it go. Whether the app has it open.
 */
@Composable
fun ShowingLine(app: AppModel, back: () -> Unit): Boolean {
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Typing(Typing.Open)) }
    ShowingApp(
        open = view?.line != null,
        stillOpen = { app.view.value?.line != null },
        back = back,
        close = { app.send(ShellEvent.Typing(Typing.Close)) },
    )
    return view?.line != null
}

/** The block form's screen, on the app's open form. */
@Composable
fun ShowingBlockForm(app: AppModel, back: () -> Unit) {
    val view by app.view.collectAsState()
    ShowingApp(
        open = view?.blockForm != null,
        stillOpen = { app.view.value?.blockForm != null },
        back = back,
        close = { app.send(ShellEvent.Block(BlockEvent.Cancel)) },
    )
}
