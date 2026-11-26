package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.ControlSends
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

/**
 * The details of a task, as the core's task form (`App::task_form_on`): its fields, the lines of
 * its details, its own actions, Save and Cancel; the task's other actions a menu away. Opened from
 * a reminder's notification, where no question asks for it; a task's Edit (Enter on a task) is the
 * question's own dialog ([QuestionScreen]), the app showing the task as it asks.
 */
class DetailsScreen(private val id: String) : CoreDialog() {
    override fun shown(): DialogShown? = host.app.taskForm()?.takeIf { host.view.details?.task?.id == id }

    override val toldAsDialog: Boolean get() = false

    override fun resumed() {
        host.send(ShellEvent.Details(DetailsEvent.Show(id)))
    }

    override val offered: List<Offered> get() = host.view.details?.task?.offered.orEmpty()
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
        if (details is DetailsEvent.Discard) host.later(0) { host.remove(this) }
    }

    override fun cancelDialog() {
        dialog?.cancel()
        host.remove(this)
    }
}

/** The block form, the core's while it is open: its fields, Save (or Add) and Cancel. */
class BlockFormScreen : CoreDialog() {
    override fun shown(): DialogShown? = host.screenView.dialog?.takeIf { it.id == "block" }

    override val toldAsDialog: Boolean get() = false

    override fun stands(view: ViewModel): Boolean = view.blockForm != null

    override fun leaving() {
        if (host.view.blockForm != null) host.send(ShellEvent.Block(BlockEvent.Cancel))
    }

    override fun cancelDialog() {
        dialog?.cancel()
    }
}

/** The core's settings pages. */
class SettingsScreen : AppList() {
    override val title: String get() = titleOf("settings")

    override fun loaded(view: ViewModel): Boolean = view.settings?.settings?.isNotEmpty() == true

    // Opened by the core (Settings on Home, its letter) or here: opened once either way.
    override fun resumed() {
        if (host.view.settings == null) host.send(ShellEvent.Settings(SettingsEvent.Open))
    }

    override fun leaving() {
        host.settingsClosed = true
        host.send(ShellEvent.Settings(SettingsEvent.Close))
    }

    // The core lists only the pages with something on them here.
    override fun rows(): List<Row> {
        val view = host.view.settings ?: return emptyList()
        return view.pages.map { page ->
                Row(page.id, page.line, {
                    host.push(if (page.devices) DevicesScreen() else PageScreen(page))
                }, letter = pageLetter(page.id))
            }
    }
}

/**
 * One of the core's settings pages (`App::settings_page_on`): each setting as its control, applied
 * as it changes, the page's caption, its buttons and the store's own actions. A page has nothing to
 * cancel: Escape leaves it.
 */
class PageScreen(private val page: SettingsPage) : CoreDialog() {
    override fun shown(): DialogShown? = host.app.settingsPage(page.id)

    override val title: String get() = shown()?.title?.ifEmpty { null } ?: page.title

    override val toldAsDialog: Boolean get() = false

    override fun cancelDialog() {
        dialog?.commit()
        host.remove(this)
    }

    override fun beforeBack(): Boolean {
        dialog?.commit()
        return false
    }

    /** What the core leaves to this app: a file to pick, a backup into the folder, pairing, a folder. */
    override fun shell(control: ControlShown) {
        when (control.key) {
            "back-up" -> host.core.backUpNow()
            "restore-backup", "import" -> host.pickImport()
            "pair" -> host.push(DevicesScreen())
            // A folder setting (`backup-dir`): chosen in Orbit's File Manager, starting where it is.
            else -> (control.sends as? ControlSends.Shell)?.takeIf { control.key.startsWith("setting:") }
                ?.let { host.chooseFolder(it.field0, control.value) }
                ?: host.refuse()
        }
    }
}

/** The paired devices, how syncing goes, and pairing another. */
class DevicesScreen : AppList() {
    override val lands: Lands get() = Lands.SIDE

    override val title: String get() = SettingsPages.all.firstOrNull { it.devices }?.title.orEmpty()

    override fun loaded(view: ViewModel): Boolean = view.devices != null

    override fun resumed() {
        host.send(ShellEvent.Devices(DevicesEvent.Show))
    }

    override fun inHand(): String? = host.view.devicesHeld.focus

    override fun moved(row: Row) {
        if (row.on != null) host.send(ShellEvent.Hold(Side.DEVICES, row.key))
    }

    override fun rows(): List<Row> {
        val view = host.view.devices ?: return emptyList()
        val lines = host.lines?.devices.orEmpty()
        return buildList {
            view.status.takeIf { it.isNotEmpty() }?.let { add(Row("status", it)) }
            view.list?.devices?.forEachIndexed { index, device ->
                add(Row(device.nodeId, lines.getOrNull(index)?.text ?: device.name, null, device.offered, ActingOn.Held(Side.DEVICES, device.nodeId)))
            }
            add(Row("sync", titleOf("sync-now"), { host.send(ShellEvent.Key("sync-now")) }, letter = buttonLetter("sync-now")))
            add(Row("wait", view.words.wait, { pair(null) }, letter = buttonLetter("pair")))
            add(Row("join", view.words.join, {
                host.stack.askLine(view.words.theirCode, "", view.words.intro) { code ->
                    // As typed, spaces and all: the core reads the code.
                    if (code != null) pair(code)
                }
            }))
        }
    }

    private fun pair(code: String?) {
        if (code != null && code.isEmpty()) {
            host.say(host.view.devices?.words?.needCode.orEmpty())
            return
        }
        host.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Open(deviceName()))))
        if (code != null) {
            host.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Code(code))))
            host.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Join)))
        } else {
            host.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Wait)))
        }
        host.push(PairingScreen())
    }

    private fun deviceName(): String = android.os.Build.MODEL.orEmpty()
}

/** Pairing under way: the code to give, the words to compare, the answer, what went wrong. */
class PairingScreen : AppList() {
    private var answered = false

    override val title: String get() = host.view.devices?.words?.title.orEmpty()

    override fun stands(view: ViewModel): Boolean = view.devices == null || view.devices?.pairing != null || !answered

    override fun leaving() {
        if (host.view.devices?.pairing != null) host.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Cancel)))
    }

    override fun rows(): List<Row> {
        val view = host.view.devices ?: return emptyList()
        val pairing = view.pairing ?: return listOf(Row("status", view.pairingStatus))
        answered = true
        val words = view.words
        // Where pairing stands and the question about the words are the core's lines.
        return buildList {
            pairing.status.takeIf { it.isNotEmpty() }?.let { add(Row("status", it)) }
            val question = pairing.question
            if (question != null) {
                add(Row("match", question))
                add(Row("yes", words.matchYes, { confirm(true) }))
                add(Row("no", words.matchNo, { confirm(false) }))
            } else {
                pairing.mineLine?.let { add(Row("code", it)) }
            }
            add(Row("cancel", appWords.cancel, { host.pop() }))
        }
    }

    private fun confirm(yes: Boolean) = host.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Confirm(yes))))
}
