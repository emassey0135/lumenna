package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.ListFinder
import dev.emassey0135.orbit.OrbitMenu
import dev.emassey0135.orbit.OrbitMenu.Outcome

/**
 * A menu of Orbit's own kind ([OrbitMenu]), told as its Editor tells its menu: opened with no
 * name, so Orbit says "Context menu" in its own words, then each item reached as a menu's focus.
 * A letter alone is an access key in the level shown (else the next item starting with it); Space
 * or Dot 8 with a letter is a hot key from anywhere in the menu ([hot]), else that access key. On
 * the Player a digit chooses that item and holding star finds by letter. Escape, Back and the menu
 * key dismiss it, Left from the top too, and the screen under it is told again; a choice closes it
 * without telling the screen under it again, and [chosen] says what follows.
 */
internal class MenuScreen<T>(
    items: List<MenuItem<T>>,
    private val hot: Map<Char, T> = emptyMap(),
    private val chosen: (T?) -> Unit,
) : Screen() {
    private val menu = OrbitMenu(items.map { it.orbit() })
    private var picked = false
    private var choice: T? = null
    private var finder: ListFinder? = null

    override val title: String get() = menu.current?.name.orEmpty()

    override fun tellShown() {
        reader.menuOpened()
        reader.focusMenu(menu)
    }

    override fun tellMoved() = reader.focusMenu(menu)

    override fun key(key: Key): Boolean {
        if (ui.phoneKeypad && finding().phoneKey(key)) return true
        finder?.reset()
        val outcome: Outcome<T> = when (key) {
            Key.Up, Key.BackTab -> menu.up()
            Key.Down, Key.Tab -> menu.down()
            Key.First -> menu.first()
            Key.Last -> menu.last()
            Key.Right -> menu.right()
            Key.Left, Key.Dot7 -> menu.left()
            Key.Select, Key.Dot8 -> menu.select()
            Key.Escape, Key.Back, Key.Menu -> Outcome.Closed
            // Back's press has closed it already.
            is Key.BackReleased -> return true
            is Key.Cell -> {
                letter(ui.letters.of(key.dots)?.toString())
                return true
            }
            is Key.Chord, is Key.Enter -> when (val pressed = ui.letters.pressed(key)) {
                is Letters.Pressed.Letter -> {
                    hotKey(pressed.letter)
                    return true
                }
                is Letters.Pressed.Refused -> {
                    ui.refused(pressed.said)
                    return true
                }
                Letters.Pressed.None -> return false
            }
            else -> return false
        }
        act(outcome)
        return true
    }

    /** A letter alone: its access key in the level shown, else the next item whose name starts with it. */
    private fun letter(text: String?) {
        val letter = text?.singleOrNull()?.lowercaseChar() ?: return ui.refuse()
        val outcome = menu.accessKey(letter)
        if (outcome != Outcome.Refused) return act(outcome)
        val at = ListFinder.next(menu.items.map { it.name }, menu.index, letter.toString()) ?: return ui.refuse()
        act(menu.moveTo(at))
    }

    /** Space or Dot 8 with a letter: a hot key from anywhere, else the access key, else why not. */
    private fun hotKey(letter: Char) {
        if (hot.containsKey(letter)) return act(Outcome.Chosen(hot.getValue(letter)))
        val outcome = menu.accessKey(letter)
        if (outcome != Outcome.Refused) return act(outcome)
        ui.refused(ui.words.nothingOnLetter(letter))
    }

    private fun act(outcome: Outcome<T>) {
        when (outcome) {
            Outcome.Moved, Outcome.Opened, Outcome.Back -> tellMoved()
            Outcome.Closed -> ui.close(this)
            Outcome.Refused -> ui.refuse()
            is Outcome.Chosen -> {
                picked = true
                choice = outcome.value
                ui.closeQuietly(this)
            }
        }
    }

    private fun finding(): ListFinder = finder ?: ui.finder(
        rows = { menu.items.size },
        found = ::letter,
        opened = { at ->
            menu.moveTo(at)
            act(menu.select())
        },
    ).also { finder = it }

    override fun closed() {
        reader.contextMenuClosed(dismissed = !picked)
        chosen(if (picked) choice else null)
    }
}
