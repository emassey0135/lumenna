package dev.emassey0135.lumenna

import android.view.KeyEvent
import android.view.KeyboardShortcutGroup
import android.view.KeyboardShortcutInfo
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.staticCompositionLocalOf
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.KeyCommand
import dev.emassey0135.lumenna.app.keyboardShortcuts
import kotlinx.coroutines.flow.MutableStateFlow

/** A key with the modifiers it needs, and no others. */
data class Keys(val code: Int, val ctrl: Boolean = false, val shift: Boolean = false) {
    fun matches(event: KeyEvent): Boolean =
        event.keyCode == code && event.isCtrlPressed == ctrl && event.isShiftPressed == shift &&
            !event.isAltPressed && !event.isMetaPressed

    /** As `KeyboardShortcutInfo` takes them. */
    val modifiers: Int get() = (if (ctrl) KeyEvent.META_CTRL_ON else 0) or (if (shift) KeyEvent.META_SHIFT_ON else 0)
}

/**
 * The keyboard commands every desktop app shares (`lumenna_app::keys`, through the core), by
 * their identifiers: the keys Windows, GTK and the web bind, and their names, so a keyboard on
 * Android does what it does there, and the system's shortcuts helper says so.
 */
private val shared: Map<String, KeyCommand> by lazy { keyboardShortcuts().flatMap { it.commands }.associateBy { it.id } }

/** A key as the shared table writes it ("Ctrl+Shift+N", "Shift+F10"), in Android's terms. */
internal fun keysOf(written: String): Keys? {
    val parts = written.split("+")
    val ctrl = "Ctrl" in parts.dropLast(1)
    val shift = "Shift" in parts.dropLast(1)
    val code = when (val key = parts.last()) {
        "Comma" -> KeyEvent.KEYCODE_COMMA
        "Page Up" -> KeyEvent.KEYCODE_PAGE_UP
        "Page Down" -> KeyEvent.KEYCODE_PAGE_DOWN
        // A PC keyboard's Delete, not Backspace (KEYCODE_DEL).
        "Delete" -> KeyEvent.KEYCODE_FORWARD_DEL
        "Enter" -> KeyEvent.KEYCODE_ENTER
        "Space" -> KeyEvent.KEYCODE_SPACE
        "Applications key" -> KeyEvent.KEYCODE_MENU
        else -> when {
            key.length == 1 && key[0] in 'A'..'Z' -> KeyEvent.KEYCODE_A + (key[0] - 'A')
            key.length == 1 && key[0] in '0'..'9' -> KeyEvent.KEYCODE_0 + (key[0] - '0')
            key.startsWith("F") && key.drop(1).toIntOrNull() in 1..12 -> KeyEvent.KEYCODE_F1 + (key.drop(1).toInt() - 1)
            else -> return null
        }
    }
    return Keys(code, ctrl = ctrl, shift = shift)
}

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
enum class Command(val id: String?, val place: Place, private val own: List<Keys> = emptyList(), private val ownTitle: String? = null) {
    NEW_TASK("new-task", Place.APP),
    NEW_BLOCK("new-block", Place.APP),
    FILTER("filter", Place.APP),
    UNDO("undo", Place.APP),
    REDO("redo", Place.APP),
    GO_TODAY("go-today", Place.APP),
    GO_TASKS("go-tasks", Place.APP),
    GO_BLOCKS("go-blocks", Place.APP),
    GO_TRASH("go-trash", Place.APP),
    GO_HISTORY("go-history", Place.APP),
    SETTINGS("settings", Place.APP),
    SYNC_NOW("sync-now", Place.APP),
    NEXT_PANE("next-pane", Place.APP),
    PREVIOUS_PANE("previous-pane", Place.APP),
    SAVE("save-task", Place.APP),
    PREVIOUS_DAY("previous-day", Place.DAY),
    NEXT_DAY("next-day", Place.DAY),
    GO_TO_NOW("go-to-now", Place.DAY),
    GO_TO_DAY("go-to-day", Place.DAY),
    MARK_DONE("mark-done", Place.ROW),
    DELETE("delete", Place.ROW),
    ACTIONS("actions", Place.ROW),
    // Android's own: a tree control's folding keys, which a Compose list does not give.
    EXPAND(null, Place.ROW, listOf(Keys(KeyEvent.KEYCODE_DPAD_RIGHT)), "Expand"),
    COLLAPSE(null, Place.ROW, listOf(Keys(KeyEvent.KEYCODE_DPAD_LEFT)), "Collapse"),
    ;

    /** What it is called, in the shared table's words. */
    val title: String by lazy { id?.let { shared[it]?.title } ?: ownTitle ?: name }

    /** The keys that run it: the shared table's, read as Android's key codes. */
    val keys: List<Keys> by lazy { id?.let { shared[it]?.keys?.mapNotNull(::keysOf) } ?: own }

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
 * row is in hand; folding too, which is among a row's actions.
 */
fun Command.rowAction(actions: List<RowAction>): RowAction? = when (this) {
    Command.MARK_DONE -> actions.ofKind(ActionKind.MARK_DONE, ActionKind.MARK_NOT_DONE)
    Command.DELETE -> actions.ofKind(ActionKind.DELETE, ActionKind.DELETE_FOR_GOOD)
    Command.EXPAND -> actions.ofKind(ActionKind.EXPAND)
    Command.COLLAPSE -> actions.ofKind(ActionKind.COLLAPSE)
    else -> null
}

/** The kind of action a row command asks for, to say why a row without one does nothing. */
val Command.kind: ActionKind?
    get() = when (this) {
        Command.MARK_DONE -> ActionKind.MARK_DONE
        Command.DELETE -> ActionKind.DELETE
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
