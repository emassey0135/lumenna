package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.Choice
import dev.emassey0135.lumenna.app.DetailsEvent
import dev.emassey0135.lumenna.app.DevicesEvent
import dev.emassey0135.lumenna.app.FieldKind
import dev.emassey0135.lumenna.app.FormField
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.SettingKind
import dev.emassey0135.lumenna.app.SettingView
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.SettingsPage
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.TaskFields
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.buttonLetter
import dev.emassey0135.lumenna.app.pageLetter
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.flag
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.text

/** A choice of a setting's or field's options, the current one first selected. */
class ChoiceScreen(
    private val name: String,
    private val options: List<Choice>,
    private val current: String,
    private val chosen: (String) -> Unit,
) : Screen() {
    override val title: String get() = name
    override fun resumed() {
        index = options.indexOfFirst { it.id == current }.coerceAtLeast(0)
    }
    override fun rows(): List<Row> = options.map { option ->
        Row(option.id, option.title, {
            ui.pop()
            chosen(option.id)
        })
    }
}

/** A field typed in a form: the task's or block's, or a setting's. */
class FieldEditScreen(
    private val fieldName: String,
    private val value: String,
    private val fieldHint: String,
    private val done: (String) -> Unit,
) : EditScreen() {
    override val title: String get() = fieldName
    override val hint: String get() = fieldHint
    override fun initial(): String = value
    override fun confirmed(text: String) {
        ui.pop()
        done(text)
    }
    override fun cancelled() = ui.pop()
}

/** The value of a [FormField] shown: the option's title where it has options. */
private fun shownValue(field: FormField, value: String): String =
    (field.options.firstOrNull { it.id == value }?.title ?: value).ifEmpty { appWords.noneValue }

/** The details of a task: its form's fields, its lines, its save button, its actions a menu away. */
class DetailsScreen(private val id: String) : ListScreen() {
    override val title: String get() = ui.view.details?.task?.title.orEmpty()

    override fun resumed() {
        ui.send(ShellEvent.Details(DetailsEvent.Show(id)))
    }

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

    /** Back saves what changed first; refused, it stays here to be put right. */
    override fun beforeBack(): Boolean {
        if (ui.view.details?.dirty != true) return false
        saving = true
        ui.send(ShellEvent.Details(DetailsEvent.Save))
        return true
    }

    override fun rows(): List<Row> {
        val details = ui.view.details ?: return emptyList()
        val on = ActingOn.Details
        val offered = details.task.offered
        return buildList {
            for (field in details.form) {
                val value = value(details.fields, field)
                add(Row("field-${field.key}", listOf(field.label, shownValue(field, value)).joinToString(", "), {
                    edit(field, value, details.projects)
                }, offered, on))
            }
            ui.lines?.details.orEmpty().forEachIndexed { index, line ->
                add(Row("line-$index", line.text, null, offered, on))
            }
            if (details.dirty) add(Row("save", titleOf("save-task"), { ui.send(ShellEvent.Details(DetailsEvent.Save)) }))
        }
    }

    private fun value(fields: TaskFields, field: FormField): String =
        if (field.key == "priority") fields.priority.toString() else fields.text(field.key).orEmpty()

    private fun edit(field: FormField, value: String, projects: List<Choice>) {
        val options = if (field.key == "project") projects else field.options
        if (options.isNotEmpty()) {
            ui.push(ChoiceScreen(field.label, options, value) { chosen ->
                ui.send(ShellEvent.Details(DetailsEvent.Field(field.key, chosen)))
            })
        } else {
            ui.push(FieldEditScreen(field.label, value, field.hint) { typed ->
                ui.send(ShellEvent.Details(DetailsEvent.Field(field.key, typed)))
            })
        }
    }
}

/** The block form: the app's, its fields by their keys, Save and Cancel at its end. */
class BlockFormScreen : ListScreen() {
    private val form get() = ui.view.blockForm
    private var refusal: String? = null

    override val title: String get() = form?.heading.orEmpty()

    override fun stands(view: ViewModel): Boolean = view.blockForm != null

    override fun leaving() {
        if (ui.view.blockForm != null) ui.send(ShellEvent.Block(BlockEvent.Cancel))
    }

    override fun rows(): List<Row> {
        val form = form ?: return emptyList()
        form.refusal.takeIf { it != refusal }?.let {
            refusal = it
            ui.refuse()
            ui.say(it)
        }
        return buildList {
            for (field in form.fields) {
                val value = if (field.key == "date") form.date else form.values.text(field.key)
                when (field.kind) {
                    FieldKind.TOGGLE -> {
                        val on = form.values.flag(field.key)
                        val state = appWords.yesNo.firstOrNull { it.id == if (on) "yes" else "no" }?.title.orEmpty()
                        add(Row("field-${field.key}", listOf(field.label, state).joinToString(", "), {
                            ui.send(ShellEvent.Block(BlockEvent.Field(field.key, (!on).toString())))
                        }))
                    }
                    else -> add(Row("field-${field.key}", listOf(field.label, shownValue(field, value)).joinToString(", "), {
                        if (field.options.isNotEmpty()) {
                            ui.push(ChoiceScreen(field.label, field.options, value) { chosen ->
                                ui.send(ShellEvent.Block(BlockEvent.Field(field.key, chosen)))
                            })
                        } else {
                            ui.push(FieldEditScreen(field.label, value, field.hint) { typed ->
                                ui.send(ShellEvent.Block(BlockEvent.Field(field.key, typed)))
                            })
                        }
                    }))
                }
            }
            add(Row("save", form.yes, { ui.send(ShellEvent.Block(BlockEvent.Save)) }))
            add(Row("cancel", appWords.cancel, { ui.send(ShellEvent.Block(BlockEvent.Cancel)) }))
        }
    }
}

/** The core's settings pages. */
class SettingsScreen : ListScreen() {
    override val title: String get() = titleOf("settings")

    override fun loaded(view: ViewModel): Boolean = view.settings?.settings?.isNotEmpty() == true

    override fun resumed() {
        ui.send(ShellEvent.Settings(SettingsEvent.Open))
    }

    override fun leaving() {
        ui.send(ShellEvent.Settings(SettingsEvent.Close))
    }

    override fun rows(): List<Row> {
        val view = ui.view.settings ?: return emptyList()
        val keys = view.settings.map { it.setting.key }.toSet()
        return view.pages.filter { page -> page.buttons.isNotEmpty() || page.id == "export" || page.id == "devices" || page.keys.any { it in keys } }
            .map { page ->
                Row(page.id, listOf(page.title, page.description).filter { it.isNotEmpty() }.joinToString(". "), {
                    ui.push(if (page.id == "devices") DevicesScreen() else PageScreen(page))
                }, letter = pageLetter(page.id))
            }
    }
}

/** One of the core's settings pages: its settings, its buttons, the store's own actions. */
class PageScreen(private val page: SettingsPage) : ListScreen() {
    override fun loaded(view: ViewModel): Boolean = view.settings?.settings?.isNotEmpty() == true

    override val title: String get() = page.title

    private fun shown(): List<SettingView> =
        page.keys.mapNotNull { key -> ui.view.settings?.settings?.firstOrNull { it.setting.key == key } }

    private var problems = mutableMapOf<String, String?>()

    override fun rows(): List<Row> {
        val view = ui.view.settings ?: return emptyList()
        return buildList {
            for (item in shown()) {
                val setting = item.setting
                if (item.problem != problems[setting.key]) {
                    problems[setting.key] = item.problem
                    item.problem?.let {
                        ui.refuse()
                        ui.say(it)
                    }
                }
                val value = setting.options.firstOrNull { it.id == setting.value }?.title ?: setting.value
                add(Row(setting.key, listOf(setting.title, value).filter { it.isNotEmpty() }.joinToString(", "), { change(item) }))
            }
            page.caption.takeIf { it.isNotEmpty() }?.let { add(Row("caption", it)) }
            if (page.id == "export") {
                view.exports.forEach { export ->
                    add(Row("export-${export.format}", export.label, { ui.send(ShellEvent.Settings(SettingsEvent.Export(export.format))) }))
                }
            }
            page.buttons.forEach { button ->
                add(Row(button.id, button.title, { pressed(button.id) }, letter = buttonLetter(button.id)))
            }
            if (page.id == "export") {
                view.offered.forEach { action ->
                    add(Row("offered-${action.kind}", listOf(action.name, view.offeredCaption).filter { it.isNotEmpty() }.joinToString(". "), {
                        ui.send(ShellEvent.ActOn(ActingOn.Settings, action))
                    }))
                }
            }
        }
    }

    private fun change(item: SettingView) {
        val setting = item.setting
        when {
            setting.kind == SettingKind.TOGGLE -> set(setting.key, if (setting.value == "true") "false" else "true")
            setting.options.isNotEmpty() -> ui.push(ChoiceScreen(setting.title, setting.options, setting.value) { set(setting.key, it) })
            setting.kind == SettingKind.FOLDER -> ui.refuse()
            else -> ui.push(FieldEditScreen(setting.title, setting.value, hintOf(item)) { set(setting.key, it) })
        }
    }

    private fun hintOf(item: SettingView): String {
        val words = ui.view.settings?.words
        return item.setting.hint.ifEmpty {
            when (item.setting.kind) {
                SettingKind.TIME -> words?.timeHint.orEmpty()
                SettingKind.NUMBER -> words?.numberHint.orEmpty()
                else -> ""
            }
        }
    }

    private fun set(key: String, value: String) = ui.send(ShellEvent.Settings(SettingsEvent.Set(key, value)))

    private fun pressed(id: String) {
        when (id) {
            "sync-now" -> ui.send(ShellEvent.Key("sync-now"))
            "back-up" -> ui.core.backUpNow()
            "restore-backup", "import" -> ui.pickImport()
            "pair" -> ui.push(DevicesScreen())
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
            add(Row("sync", titleOf("sync-now"), { ui.send(ShellEvent.Devices(DevicesEvent.SyncNow)) }, letter = buttonLetter("sync-now")))
            add(Row("wait", view.words.wait, { pair(null) }, letter = buttonLetter("pair")))
            add(Row("join", view.words.join, {
                ui.push(FieldEditScreen(view.words.theirCode, "", view.words.intro) { code ->
                    pair(code.replace(" ", ""))
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
