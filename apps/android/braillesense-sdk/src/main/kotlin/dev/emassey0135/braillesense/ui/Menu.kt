package dev.emassey0135.braillesense.ui

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.reading.ControlRole

/** One item of a menu: a submenu when it has [children]. */
public class MenuEntry(
    /** What it is called. */
    public val title: String,
    /** Its items, for a submenu. */
    public val children: List<MenuEntry> = emptyList(),
    /** The hotkey shown after the name, as the device's own menus do. */
    public val hotkey: String? = null,
    /** Its letter where the core gives one; else the first of its name not taken, if there is one. */
    public val letter: Char? = null,
    /** Whether it opens a dialog, which the device's menus mark. */
    public val asks: Boolean = false,
    /** What choosing it does, for an item. */
    public val run: () -> Unit = {},
)

/**
 * The device's own menus (the manual's "Menu Concepts", and its SDK's menu list): a submenu reads
 * `mn File (f) 3o` and is said "File (f) pulldown Menu", an item `mi New (n)`, said "New (n) menu
 * item", and an item that opens a dialog adds its sign and the word. Space-1 and Space-4 move and
 * wrap, Space-1-2-3 and Space-4-5-6 go to the ends, Enter opens or runs, an item's letter jumps to
 * it and runs it (or only jumps, as the person's setting has it), Backspace or Escape goes up a level or, at the top, closes the menu, and the
 * menu key or Exit closes it all. A hot key an item shows runs it while the menu is open too, as
 * it does from the screen under the menu.
 */
public class MenuFrame(
    private val top: List<MenuEntry>,
    private val letters: Letters,
    private val readings: Readings,
    /** Automatically Open Menus with Shortcut: whether a letter opens its item or only goes to it. */
    private val opensOnLetter: () -> Boolean = { true },
    /** Says that nothing here is on the letter. */
    private val notFound: (Char) -> Unit = {},
    /** The menu has closed: `cancelled` when it was left without choosing. */
    private val closed: (cancelled: Boolean) -> Unit,
) : Frame("menu") {
    private class Level(val entries: List<MenuEntry>, var index: Int = 0)

    private val levels = ArrayDeque(listOf(Level(top)))
    private val shortcuts = HashMap<List<MenuEntry>, Map<MenuEntry, Char>>()

    private val here get() = levels.last()

    private fun shortcutsOf(entries: List<MenuEntry>): Map<MenuEntry, Char> = shortcuts.getOrPut(entries) {
        entries.zip(letters.shortcuts(entries.map { it.title to it.letter }))
            .mapNotNull { (entry, letter) -> letter?.let { entry to it } }
            .toMap()
    }

    override val rendered: Rendered
        get() {
            val entry = here.entries.getOrNull(here.index) ?: return Rendered("")
            val role = if (entry.children.isNotEmpty()) ControlRole.MENU else ControlRole.MENU_ITEM
            return readings.read(line(role, entry.title, letter = shortcutsOf(here.entries)[entry], hotKey = entry.hotkey, asks = entry.asks)).rendered()
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
            closed(false)
            entry.run()
        }
    }

    /** Up a level, or out of the menu from its top. */
    private fun leave(): Boolean {
        if (levels.size > 1) levels.removeLast() else closed(true)
        return true
    }

    override fun key(key: Key): Boolean = when (key) {
        Key.Up -> move(-1)
        Key.Down -> move(+1)
        Key.First -> run { here.index = 0; true }
        Key.Last -> run { here.index = here.entries.lastIndex; true }
        Key.Enter -> run { here.entries.getOrNull(here.index)?.let(::choose); true }
        Key.Escape, Key.Backspace -> leave()
        Key.Menu, Key.Exit -> run { closed(true); true }
        is Key.SpaceDots, is Key.EnterDots, is Key.SpaceLetter, is Key.EnterLetter -> run {
            val press = when (key) {
                is Key.SpaceDots -> letters.letter(Key.Cell(key.dots))?.let { "Space-$it" }
                is Key.EnterDots -> letters.letter(Key.Cell(key.dots))?.let { "Enter-$it" }
                is Key.SpaceLetter -> "Space-${key.letter.lowercaseChar()}"
                is Key.EnterLetter -> "Enter-${key.letter.lowercaseChar()}"
                // Not reached: the branch above takes these four alone (an older Kotlin cannot tell).
                else -> null
            }
            val item = press?.let { hotKeyed(top, it) }
            if (item != null) {
                closed(false)
                item.run()
            } else {
                press?.last()?.let(notFound)
            }
            true
        }
        is Key.Cell, is Key.Typed -> run {
            val letter = letters.letter(key)?.lowercaseChar()
            val match = shortcutsOf(here.entries).entries.firstOrNull { it.value == letter }?.key
            if (match != null) {
                here.index = here.entries.indexOf(match)
                if (opensOnLetter()) choose(match)
            } else if (letter != null) {
                notFound(letter)
            }
            true
        }
        else -> false
    }

    /** The item anywhere in [entries] whose hot key is [press]. */
    private fun hotKeyed(entries: List<MenuEntry>, press: String): MenuEntry? {
        entries.forEach { entry ->
            if (entry.children.isEmpty() && entry.hotkey.equals(press, ignoreCase = true)) return entry
            hotKeyed(entry.children, press)?.let { return it }
        }
        return null
    }

    override fun routed(at: Int) {
        here.entries.getOrNull(here.index)?.let(::choose)
    }
}
