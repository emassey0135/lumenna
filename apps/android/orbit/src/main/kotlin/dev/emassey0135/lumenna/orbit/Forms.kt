package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.DetailsEvent
import dev.emassey0135.lumenna.app.DevicesEvent
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.SettingsPage
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.buttonLetter
import dev.emassey0135.lumenna.app.pageLetter
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.text
import dev.emassey0135.orbit.Key

/**
 * The details of a task, as the core's task form (`App::task_form_on`): its fields, the lines of
 * its details, its own actions, Save and Cancel; the task's other actions a menu away.
 */
class DetailsScreen(private val id: String) : DialogScreen() {
    override fun shown(): DialogShown? = ui.app.taskForm()?.takeIf { ui.view.details?.task?.id == id }

    override fun openedAs(): Opened = Opened.List

    override fun resumed() {
        ui.send(ShellEvent.Details(DetailsEvent.Show(id)))
    }

    override val offered: List<Offered> get() = ui.view.details?.task?.offered.orEmpty()
    override val on: ActingOn get() = ActingOn.Details

    private var saving = false

    override fun stands(view: ViewModel): Boolean {
        val details = view.details ?: return true
        if (details.task.id != id) return false
        if (saving) {
            // A save refused keeps the details open with what was typed, the app's sentence said.
            if (details.dirty) saving = false else return false
        }
        return true
    }

    /** Back is Cancel, on both: what was typed is thrown away, never saved. */
    override fun beforeBack(): Boolean {
        cancelDialog()
        return true
    }

    /** Saving closes the form once the app has it; Cancel (Discard) closes it at once. */
    override fun sending(event: ShellEvent) {
        val details = (event as? ShellEvent.Details)?.field0 ?: return
        if (details is DetailsEvent.Save) saving = true
        if (details is DetailsEvent.Discard) ui.later(0) { ui.remove(this) }
    }

    override fun cancelDialog() {
        dialog?.cancel()
        ui.remove(this)
    }
}

/** The block form, the core's while it is open: its fields, Save (or Add) and Cancel. */
class BlockFormScreen : DialogScreen() {
    override fun shown(): DialogShown? = ui.screenView.dialog?.takeIf { it.id == "block" }

    override fun openedAs(): Opened = Opened.List

    override fun stands(view: ViewModel): Boolean = view.blockForm != null

    override fun leaving() {
        if (ui.view.blockForm != null) ui.send(ShellEvent.Block(BlockEvent.Cancel))
    }

    override fun cancelDialog() {
        dialog?.cancel()
    }
}

/** The core's settings pages. */
class SettingsScreen : ListScreen() {
    override val title: String get() = titleOf("settings")

    override fun loaded(view: ViewModel): Boolean = view.settings?.settings?.isNotEmpty() == true

    // Opened by the core (Settings on Home, its letter) or here: opened once either way.
    override fun resumed() {
        if (ui.view.settings == null) ui.send(ShellEvent.Settings(SettingsEvent.Open))
    }

    override fun leaving() {
        ui.settingsClosed = true
        ui.send(ShellEvent.Settings(SettingsEvent.Close))
    }

    // The core lists only the pages with something on them here.
    override fun rows(): List<Row> {
        val view = ui.view.settings ?: return emptyList()
        return view.pages.map { page ->
                Row(page.id, listOf(page.title, page.description).filter { it.isNotEmpty() }.joinToString(". "), {
                    ui.push(if (page.id == "devices") DevicesScreen() else PageScreen(page))
                }, letter = pageLetter(page.id))
            }
    }
}

/**
 * One of the core's settings pages (`App::settings_page_on`): each setting as its control, applied
 * as it changes, the page's caption, its buttons and the store's own actions. A page has nothing to
 * cancel: Escape leaves it.
 */
class PageScreen(private val page: SettingsPage) : DialogScreen() {
    override fun shown(): DialogShown? = ui.app.settingsPage(page.id)

    override val title: String get() = shown()?.title?.ifEmpty { null } ?: page.title

    override fun openedAs(): Opened = Opened.List

    override fun cancelDialog() {
        dialog?.commit()
        ui.remove(this)
    }

    override fun beforeBack(): Boolean {
        dialog?.commit()
        return false
    }

    /** What the core leaves to this app: a file to pick, a backup into the folder, pairing. */
    override fun shell(control: ControlShown) {
        when (control.key) {
            "back-up" -> ui.core.backUpNow()
            "restore-backup", "import" -> ui.pickImport()
            "pair" -> ui.push(DevicesScreen())
            // A folder is chosen with a file picker Orbit's keys cannot work.
            else -> ui.refuse()
        }
    }
}

/** The paired devices, how syncing goes, and pairing another. */
class DevicesScreen : ListScreen() {
    override val lands: Lands get() = Lands.SIDE

    override val title: String get() = SettingsPages.of("devices")?.title.orEmpty()

    override fun loaded(view: ViewModel): Boolean = view.devices != null

    override fun resumed() {
        ui.send(ShellEvent.Devices(DevicesEvent.Show))
    }

    override fun inHand(): String? = ui.view.devicesHeld.focus

    override fun moved(row: Row) {
        if (row.on != null) ui.send(ShellEvent.Hold(Side.DEVICES, row.key))
    }

    override fun rows(): List<Row> {
        val view = ui.view.devices ?: return emptyList()
        val lines = ui.lines?.devices.orEmpty()
        return buildList {
            view.status.takeIf { it.isNotEmpty() }?.let { add(Row("status", it)) }
            view.list?.devices?.forEachIndexed { index, device ->
                add(Row(device.nodeId, lines.getOrNull(index)?.text ?: device.name, null, device.offered, ActingOn.Held(Side.DEVICES, device.nodeId)))
            }
            add(Row("sync", titleOf("sync-now"), { ui.send(ShellEvent.Key("sync-now")) }, letter = buttonLetter("sync-now")))
            add(Row("wait", view.words.wait, { pair(null) }, letter = buttonLetter("pair")))
            add(Row("join", view.words.join, {
                ui.push(FieldEditScreen(view.words.theirCode, "", view.words.intro) { code ->
                    // As typed, spaces and all: the core reads the code.
                    pair(code)
                })
            }))
        }
    }

    private fun pair(code: String?) {
        if (code != null && code.isEmpty()) {
            ui.say(ui.view.devices?.words?.needCode.orEmpty())
            return
        }
        ui.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Open(deviceName()))))
        if (code != null) {
            ui.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Code(code))))
            ui.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Join)))
        } else {
            ui.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Wait)))
        }
        ui.push(PairingScreen())
    }

    private fun deviceName(): String = android.os.Build.MODEL.orEmpty()
}

/** Pairing under way: the code to give, the words to compare, the answer, what went wrong. */
class PairingScreen : ListScreen() {
    private var answered = false

    override val title: String get() = ui.view.devices?.words?.title.orEmpty()

    override fun stands(view: ViewModel): Boolean = view.devices == null || view.devices?.pairing != null || !answered

    override fun leaving() {
        if (ui.view.devices?.pairing != null) ui.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Cancel)))
    }

    override fun rows(): List<Row> {
        val view = ui.view.devices ?: return emptyList()
        val pairing = view.pairing ?: return listOf(Row("status", view.status))
        answered = true
        val words = view.words
        return buildList {
            pairing.problem?.let { add(Row("problem", it)) }
            if (pairing.words != null) {
                add(Row("match", listOf(words.matchMessage, pairing.matching.orEmpty()).filter { it.isNotEmpty() }.joinToString(". ")))
                add(Row("yes", words.matchYes, { confirm(true) }))
                add(Row("no", words.matchNo, { confirm(false) }))
            } else if (pairing.mine != null) {
                add(Row("code", listOf(words.waiting, pairing.mineLine ?: pairing.mine.orEmpty()).joinToString(". ")))
            } else {
                add(Row("status", if (pairing.waiting) words.opening else words.connecting))
            }
            add(Row("cancel", appWords.cancel, { ui.pop() }))
        }
    }

    private fun confirm(yes: Boolean) = ui.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Confirm(yes))))
}
