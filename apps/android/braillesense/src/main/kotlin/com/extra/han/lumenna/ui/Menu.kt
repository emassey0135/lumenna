package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Key

/** One item of a menu: a submenu when it has [children]. */
class MenuEntry(
    val title: String,
    val children: List<MenuEntry> = emptyList(),
    /** The hotkey shown after the name, as the device's own menus do. */
    val hotkey: String? = null,
    /** Its letter where the core gives one; else the first of its name not taken. */
    val letter: Char? = null,
    val run: () -> Unit = {},
)

/**
 * The device's own menus (the manual's "Menu Concepts"): an item reads `New (n) pull down Enter-n`,
 * Space-1 and Space-4 move, Space-1-2-3 and Space-4-5-6 go to the ends, Enter opens or runs, an
 * item's letter jumps to it and runs it, Backspace at the top of a submenu goes back up, and
 * Escape closes the whole menu.
 */
class MenuFrame(
    private val top: List<MenuEntry>,
    private val letters: Letters,
    private val pullDown: String,
    /** Says that nothing here is on the letter. */
    private val notFound: (Char) -> Unit = {},
    private val closed: () -> Unit,
) : Frame("menu") {
    private class Level(val entries: List<MenuEntry>, var index: Int = 0)

    private val levels = ArrayDeque(listOf(Level(top)))
    private val shortcuts = HashMap<List<MenuEntry>, Map<MenuEntry, Char>>()

    private val here get() = levels.last()

    private fun shortcutsOf(entries: List<MenuEntry>): Map<MenuEntry, Char> = shortcuts.getOrPut(entries) {
        val taken = HashSet<Char>()
        entries.associateWith { entry ->
            val letter = entry.letter?.takeIf { it !in taken } ?: entry.title.lowercase().firstOrNull { it.isLetterOrDigit() && it !in taken }
                ?: entry.title.lowercase().firstOrNull { it.isLetterOrDigit() } ?: ' '
            taken += letter
            letter
        }
    }

    /** An item as the device's menus read it: its name, its letter, and how else to reach it. */
    private fun reading(entry: MenuEntry): String {
        val letter = shortcutsOf(here.entries)[entry]
        return buildString {
            append(entry.title)
            if (letter != null) append(" ($letter)")
            append(' ').append(pullDown)
            if (entry.hotkey != null) append(' ').append(entry.hotkey)
        }
    }

    override val rendered: Rendered
        get() {
            val entry = here.entries.getOrNull(here.index) ?: return Rendered("")
            val text = reading(entry)
            return Rendered(text, text)
        }

    private fun move(step: Int): Boolean {
        val size = here.entries.size
        if (size > 0) here.index = (here.index + step + size) % size
        return true
    }

    private fun choose(entry: MenuEntry) {
        if (entry.children.isNotEmpty()) {
            levels.addLast(Level(entry.children))
        } else {
            closed()
            entry.run()
        }
    }

    override fun key(key: Key): Boolean = when (key) {
        Key.Up -> move(-1)
        Key.Down -> move(+1)
        Key.First -> run { here.index = 0; true }
        Key.Last -> run { here.index = here.entries.lastIndex; true }
        Key.Enter -> run { here.entries.getOrNull(here.index)?.let(::choose); true }
        Key.Escape -> run { closed(); true }
        Key.Backspace -> run {
            if (levels.size > 1) levels.removeLast() else closed()
            true
        }
        is Key.Cell, is Key.Typed -> run {
            val letter = letters.letter(key)?.lowercaseChar()
            val match = shortcutsOf(here.entries).entries.firstOrNull { it.value == letter }?.key
            if (match != null) {
                here.index = here.entries.indexOf(match)
                choose(match)
            } else if (letter != null) {
                notFound(letter)
            }
            true
        }
        else -> false
    }

    override fun routed(at: Int) {
        here.entries.getOrNull(here.index)?.let(::choose)
    }
}
