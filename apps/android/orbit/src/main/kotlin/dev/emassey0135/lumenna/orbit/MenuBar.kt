package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.LetteredMenu
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.settingsPageOf
import dev.emassey0135.lumenna.app.sidebarKey
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.orbit.ui.MenuItem

/** What an item of the menu bar runs: the row's action, handed back as it is offered, or a command. */
sealed interface BarChoice {
    data class Action(val on: ActingOn, val offered: Offered) : BarChoice
    data class Command(val id: String) : BarChoice
}

/**
 * The core's menu bar (`braille_menu_bar_on_row`) as Orbit's own menus are, for the SDK's stack to
 * open on the menu key ([dev.emassey0135.orbit.ui.MenuBar]): its menus a list of submenus, the row in
 * hand's actions first, then the screen's own, then File, Edit, View, Task, Day and Help. What does
 * not apply now (`ViewModel.commands`, `screen_actions`) is left out, as Orbit's menus leave theirs;
 * a menu left empty is too. An item's name is the core's, and a hot key's press is said after it on
 * the Speak. [hot] is what each hot key runs, from the whole bar; [run] runs a choice.
 */
fun menuBarItems(
    bar: List<LetteredMenu>,
    view: ViewModel,
    on: ActingOn?,
    withPress: (String, String?) -> String,
    hot: MutableMap<Char, () -> Unit>,
    run: (BarChoice) -> Unit,
): List<MenuItem<() -> Unit>> {
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
            val running: () -> Unit = { run(choice) }
            hotLetter?.singleOrNull()?.let { hot.putIfAbsent(it, running) }
            MenuItem(withPress(title, hotLetter), item.letter.singleOrNull(), value = running)
        }
        if (children.isEmpty()) null else MenuItem(menu.title, menu.letter.singleOrNull(), children)
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
        "close-window", "quit" -> quit()
        // A command that opens a settings page (Devices, Export and Import, Restore from a Backup) is the core's to name.
        else -> settingsPageOf(id)?.let { openSettingsPage(it) } ?: send(ShellEvent.Key(id))
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

/** A settings page the menu bar names, over Settings; the devices page is the devices screen. */
private fun Ui.openSettingsPage(id: String) {
    val page = SettingsPages.of(id) ?: return refuse()
    if (page.devices) {
        push(DevicesScreen())
        return
    }
    push(SettingsScreen())
    push(PageScreen(page))
}
