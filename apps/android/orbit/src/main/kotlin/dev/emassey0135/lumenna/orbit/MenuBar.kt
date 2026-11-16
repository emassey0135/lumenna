package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.LetteredMenu
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.SpeakLetter
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.brailleLetter
import dev.emassey0135.lumenna.app.nothingOnLetter
import dev.emassey0135.lumenna.app.sidebarKey
import dev.emassey0135.lumenna.app.speakLetter
import dev.emassey0135.lumenna.app.speakPressInstead
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.OrbitMenu
import dev.emassey0135.orbit.OrbitMenu.Outcome

/** What an item of the menu bar runs: the row's action, handed back as it is offered, or a command. */
sealed interface BarChoice {
    data class Action(val on: ActingOn, val offered: Offered) : BarChoice
    data class Command(val id: String) : BarChoice
}

/**
 * The core's menu bar (`braille_menu_bar_on_row`) as Orbit's own menus are: its menus a list of
 * submenus, the row in hand's actions first, then the screen's own, then File, Edit, View, Task,
 * Day and Help. What does not apply now (`ViewModel.commands`, `screen_actions`) is left out, as
 * Orbit's menus leave theirs; a menu left empty is too. An item's name is the core's, and a hot
 * key's press is said after it on the Speak. [hot] is what each hot key runs, from the whole bar.
 */
fun menuBarItems(
    bar: List<LetteredMenu>,
    view: ViewModel,
    on: ActingOn?,
    withPress: (String, String?) -> String,
    hot: MutableMap<String, BarChoice>,
): List<OrbitMenu.Item<BarChoice>> {
    val states = view.commands.associateBy { it.id }
    val actions = view.screenActions.associateBy { it.id }
    return bar.mapNotNull { menu ->
        val children = menu.items.mapNotNull { item ->
            val action = item.action
            val choice = when {
                action != null -> BarChoice.Action(on ?: return@mapNotNull null, action)
                states[item.id]?.enabled == false || actions[item.id]?.enabled == false -> return@mapNotNull null
                else -> BarChoice.Command(item.id)
            }
            val title = if (action != null) item.title else states[item.id]?.title?.ifEmpty { null } ?: actions[item.id]?.title ?: item.title
            val hotLetter = item.letter.takeIf { item.hotKey && it.isNotEmpty() }
            if (hotLetter != null) hot.putIfAbsent(hotLetter, choice)
            OrbitMenu.Item(withPress(title, hotLetter), item.letter.singleOrNull(), value = choice)
        }
        if (children.isEmpty()) null else OrbitMenu.Item(menu.title, menu.letter.singleOrNull(), children)
    }
}

/**
 * The menu bar open (Space with M on the Speak, Menu on the Player), moving as Orbit's Editor's
 * menu does ([OrbitMenu]) and told to Orbit's reader as its own: the menu opened, then each item
 * as a menu's (role 8), a submenu said as one. A letter alone is an item's access key in the level
 * shown; Space with a letter (or Dot 8 with one the Speak keeps) is a hot key from anywhere in the
 * bar, as from the screen under it. Escape, Back and the menu key close it; Left closes it from
 * the top. A choice closes it and runs, the screen under it not said again.
 */
class MenuBarScreen(
    private val menu: OrbitMenu<BarChoice>,
    private val hot: Map<String, BarChoice>,
) : Screen() {
    private var chose = false

    override val title: String get() = appWords.braille.actionsMenu

    override fun rows(): List<Row> = menu.items.mapIndexed { index, item ->
        Row("${menu.depth}:$index", item.name, {
            menu.moveTo(index)
            act(menu.select())
        }, title = item.name)
    }

    /** The Player's digits and T9 letters move here as in any list ([FirstLetters]). */
    override fun moved(row: Row) {
        row.key.substringAfter(':').toIntOrNull()?.let(menu::moveTo)
    }

    /** What the reader is told: the menu opened as it first shows, then the item in hand. */
    fun draw(newlyShown: Boolean, why: Why, textView: android.widget.TextView) {
        index = menu.index
        selectedKey = "${menu.depth}:${menu.index}"
        textView.text = buildString {
            append(title).append("\n\n").append(menu.current?.name.orEmpty())
            if (menu.current?.isSubMenu == true) append(" …")
            append("\n\n").append(menu.index + 1).append(" of ").append(menu.items.size)
        }
        val item = menu.current?.let { ReaderCall.FocusMenu(it.name, it.isSubMenu, it.accessKey, menu.index + 1, menu.items.size) }
        describeMenu(newlyShown, why, item).forEach(ui.reader::perform)
    }

    override fun key(key: Key): Boolean {
        val outcome = when (key) {
            Key.Up, Key.BackTab -> menu.up()
            Key.Down, Key.Tab -> menu.down()
            Key.First -> menu.first()
            Key.Last -> menu.last()
            Key.Right -> menu.right()
            Key.Left, Key.Dot7 -> menu.left()
            Key.Select, Key.Dot8 -> menu.select()
            Key.Escape, Key.Back, Key.Menu -> Outcome.Closed
            is Key.BackReleased -> return true
            is Key.Cell -> brailleLetter(key.dots.toUByte())?.singleOrNull()?.let(menu::accessKey) ?: Outcome.Refused
            is Key.Chord -> {
                val letter = brailleLetter(key.dots.toUByte())
                if (letter != null && speakLetter(letter) != SpeakLetter.SPACE) {
                    ui.refuse()
                    ui.say(speakPressInstead(letter))
                    return true
                }
                hotKey(letter) ?: return true
            }
            is Key.Enter -> hotKey(brailleLetter(key.dots.toUByte())) ?: return true
            else -> return false
        }
        act(outcome)
        return true
    }

    /** A hot key's choice; none once a letter that runs nothing has been refused in the core's words. */
    private fun hotKey(letter: String?): Outcome<BarChoice>? {
        letter ?: return Outcome.Refused
        val choice = hot[letter]
        if (choice == null) {
            ui.refuse()
            ui.say(nothingOnLetter(letter))
            return null
        }
        return Outcome.Chosen(choice)
    }

    private fun act(outcome: Outcome<BarChoice>) {
        when (outcome) {
            Outcome.Moved, Outcome.Opened, Outcome.Back -> ui.redraw(Why.Moved)
            Outcome.Closed -> ui.remove(this)
            Outcome.Refused -> ui.refuse()
            is Outcome.Chosen -> {
                chose = true
                ui.closeMenu(this) { ui.runBarChoice(outcome.value) }
            }
        }
    }

    override fun leaving() {
        ui.reader.contextMenuClosed(dismissed = !chose)
    }
}

/** What an item of the menu bar runs: the row's action, the shell's own, else the core's `Key(id)`. */
fun Ui.runBarChoice(choice: BarChoice) {
    when (choice) {
        is BarChoice.Action -> send(ShellEvent.ActOn(choice.on, choice.offered))
        is BarChoice.Command -> runMenuCommand(choice.id)
    }
}

/**
 * A command of the menu bar: what is this app's own to run (a screen action marked `shell`, a
 * heading's places, a settings page, leaving), else the core's `Key(id)`.
 */
fun Ui.runMenuCommand(id: String) {
    view.screenActions.firstOrNull { it.id == id && it.shell }?.let {
        runScreenAction(it)
        return
    }
    when (id) {
        "filter" -> view.place?.let { push(FilterScreen(it)) } ?: refuse()
        "keyboard-help" -> push(KeyHelpScreen())
        "reminders-due" -> push(DueScreen())
        "about" -> appWords.metadata.let { say("${it.name} ${it.version}. ${it.description}") }
        "projects" -> openGroup(SidebarGroup.PROJECTS)
        "labels" -> openGroup(SidebarGroup.LABELS)
        "filters" -> openGroup(SidebarGroup.FILTERS)
        "devices" -> push(DevicesScreen())
        "export-import" -> openSettingsPage("export")
        "restore-backup" -> openSettingsPage("backups")
        "close-window", "quit" -> quit()
        else -> send(ShellEvent.Key(id))
    }
}

/** A heading's places, from anywhere: Home, on the heading, unfolded. */
private fun Ui.openGroup(group: SidebarGroup) {
    goHome()
    val key = sidebarKey(SidebarKind.Group(group))
    send(ShellEvent.Hold(Side.SIDEBAR, key))
    view.sidebar.firstOrNull { it.key == key }?.offered?.firstOrNull { it.kind == ActionKind.EXPAND }
        ?.let { send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, key), it)) }
}

/** A settings page the menu bar names, over Settings. */
private fun Ui.openSettingsPage(id: String) {
    val page = SettingsPages.of(id) ?: return refuse()
    push(SettingsScreen())
    push(PageScreen(page))
}
