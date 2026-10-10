package io.github.emassey0135.lumenna

import android.view.KeyEvent
import android.view.KeyboardShortcutGroup
import android.view.KeyboardShortcutInfo
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.staticCompositionLocalOf
import io.github.emassey0135.lumenna.core.ActionKind
import kotlinx.coroutines.flow.MutableStateFlow

/** A key with the modifiers it needs, and no others. */
data class Keys(val code: Int, val ctrl: Boolean = false, val shift: Boolean = false) {
    fun matches(event: KeyEvent): Boolean =
        event.keyCode == code && event.isCtrlPressed == ctrl && event.isShiftPressed == shift &&
            !event.isAltPressed && !event.isMetaPressed

    /** As `KeyboardShortcutInfo` takes them. */
    val modifiers: Int get() = (if (ctrl) KeyEvent.META_CTRL_ON else 0) or (if (shift) KeyEvent.META_SHIFT_ON else 0)
}

private fun ctrl(code: Int, shift: Boolean = false) = Keys(code, ctrl = true, shift = shift)

/** Where a command is listed in the system's keyboard shortcuts helper. */
enum class Place(val title: String) {
    APP("Lumenna"),
    DAY("Day"),
    ROW("In a list"),
}

/**
 * Every keyboard command, with the keys the Windows and GTK apps give it, so a person moving
 * between devices keeps the keys they know. Ctrl, not Alt: TalkBack's keyboard commands are
 * on Alt, or on Search, which Android also reserves for the system.
 */
enum class Command(val title: String, val place: Place, vararg val keys: Keys) {
    NEW_TASK("New task", Place.APP, ctrl(KeyEvent.KEYCODE_N)),
    NEW_BLOCK("New block", Place.APP, ctrl(KeyEvent.KEYCODE_N, shift = true)),
    FILTER("Filter tasks", Place.APP, ctrl(KeyEvent.KEYCODE_F)),
    UNDO("Undo", Place.APP, ctrl(KeyEvent.KEYCODE_Z)),
    REDO("Redo", Place.APP, ctrl(KeyEvent.KEYCODE_Y), ctrl(KeyEvent.KEYCODE_Z, shift = true)),
    GO_TODAY("Today", Place.APP, ctrl(KeyEvent.KEYCODE_1)),
    GO_TASKS("Tasks", Place.APP, ctrl(KeyEvent.KEYCODE_2)),
    GO_BLOCKS("Blocks", Place.APP, ctrl(KeyEvent.KEYCODE_3)),
    GO_TRASH("Trash", Place.APP, ctrl(KeyEvent.KEYCODE_4)),
    SETTINGS("Settings", Place.APP, ctrl(KeyEvent.KEYCODE_COMMA)),
    SYNC_NOW("Sync now", Place.APP, Keys(KeyEvent.KEYCODE_F5)),
    NEXT_PANE("Next pane", Place.APP, Keys(KeyEvent.KEYCODE_F6)),
    PREVIOUS_PANE("Previous pane", Place.APP, Keys(KeyEvent.KEYCODE_F6, shift = true)),
    SAVE("Save changes", Place.APP, ctrl(KeyEvent.KEYCODE_S)),

    PREVIOUS_DAY("Previous day", Place.DAY, ctrl(KeyEvent.KEYCODE_PAGE_UP)),
    NEXT_DAY("Next day", Place.DAY, ctrl(KeyEvent.KEYCODE_PAGE_DOWN)),
    GO_TO_NOW("Go to now", Place.DAY, ctrl(KeyEvent.KEYCODE_T)),
    GO_TO_DAY("Go to day", Place.DAY, ctrl(KeyEvent.KEYCODE_G)),

    // A row's own: each runs the focused row's action of that kind (`ListRow`), so a key does
    // exactly what the row's action list offers, and nothing a row does not.
    MARK_DONE("Mark done or not done", Place.ROW, ctrl(KeyEvent.KEYCODE_K)),
    DELETE("Delete", Place.ROW, Keys(KeyEvent.KEYCODE_FORWARD_DEL)),
    ACTIONS("Actions", Place.ROW, Keys(KeyEvent.KEYCODE_F10, shift = true), Keys(KeyEvent.KEYCODE_MENU)),
    EXPAND("Expand", Place.ROW, Keys(KeyEvent.KEYCODE_DPAD_RIGHT)),
    COLLAPSE("Collapse", Place.ROW, Keys(KeyEvent.KEYCODE_DPAD_LEFT)),
    ;

    fun matches(event: KeyEvent) = keys.any { it.matches(event) }

    companion object {
        /** The command [event] asks for, if any: only as the key goes down, and only once. */
        fun of(event: KeyEvent): Command? =
            if (event.action != KeyEvent.ACTION_DOWN || event.repeatCount > 0) null
            else entries.firstOrNull { it.matches(event) }
    }
}

/**
 * The row's action a row command runs: by the core's kind, so Delete is Delete on whatever
 * row is in hand; folding, which is the app's, by name.
 */
fun Command.rowAction(actions: List<RowAction>): RowAction? = when (this) {
    Command.MARK_DONE -> actions.ofKind(ActionKind.MARK_DONE, ActionKind.MARK_NOT_DONE)
    Command.DELETE -> actions.ofKind(ActionKind.DELETE, ActionKind.DELETE_FOR_GOOD)
    Command.EXPAND -> actions.firstOrNull { it.kind == null && it.name == "Expand" }
    Command.COLLAPSE -> actions.firstOrNull { it.kind == null && it.name == "Collapse" }
    else -> null
}

/**
 * What each command does now. A screen offers a command while it is shown ([Offer]); the most
 * recently shown wins, so the pane in front answers before the one behind it.
 */
class Shortcuts {
    /** Ctrl+F from elsewhere: the Tasks tab opens, and its filter field takes focus. */
    val filterAsked = MutableStateFlow(false)

    private val offers = LinkedHashMap<Command, ArrayDeque<() -> Unit>>()

    internal fun offer(command: Command, run: () -> Unit): () -> Unit {
        offers.getOrPut(command) { ArrayDeque() }.addLast(run)
        return { offers[command]?.remove(run) }
    }

    /** Runs what [event] asks for, if anything now offers it. */
    fun handle(event: KeyEvent): Boolean {
        val command = Command.of(event) ?: return false
        val run = offers[command]?.lastOrNull() ?: return false
        run()
        return true
    }

    /**
     * For the system's keyboard shortcuts helper (Meta+/): what can be done now, and what a
     * row in a list answers. A key is shown once, however many commands could take it.
     */
    fun groups(): List<KeyboardShortcutGroup> = Place.entries.mapNotNull { place ->
        val shown = Command.entries.filter { it.place == place && (place == Place.ROW || offers[it]?.isNotEmpty() == true) }
        if (shown.isEmpty()) null
        else KeyboardShortcutGroup(
            place.title,
            shown.flatMap { command -> command.keys.map { KeyboardShortcutInfo(command.title, it.code, it.modifiers) } },
        )
    }
}

/** The app's [Shortcuts], for any screen to offer commands to. */
val LocalShortcuts = staticCompositionLocalOf { Shortcuts() }

/** Offers [command] while this is composed, running [run] — the newest [run] each time. */
@Composable
fun Offer(command: Command, run: () -> Unit) {
    val shortcuts = LocalShortcuts.current
    val latest = rememberUpdatedState(run)
    DisposableEffect(shortcuts, command) {
        val withdraw = shortcuts.offer(command) { latest.value() }
        onDispose { withdraw() }
    }
}
