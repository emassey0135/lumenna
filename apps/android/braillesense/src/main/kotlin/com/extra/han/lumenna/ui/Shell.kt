package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Key
import com.extra.han.lumenna.device.Translator
import dev.emassey0135.lumenna.AppModel
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.commandTitle
import dev.emassey0135.lumenna.flag
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.offeredOf
import dev.emassey0135.lumenna.on
import dev.emassey0135.lumenna.shownRows
import dev.emassey0135.lumenna.text
import dev.emassey0135.lumenna.TextOffsets
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.ActionKind
import dev.emassey0135.lumenna.app.Answer
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.DayKind
import dev.emassey0135.lumenna.app.DayMove
import dev.emassey0135.lumenna.app.DetailsEvent
import dev.emassey0135.lumenna.app.DevicesEvent
import dev.emassey0135.lumenna.app.FieldKind
import dev.emassey0135.lumenna.app.Lines
import dev.emassey0135.lumenna.app.Offered
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ProposalEvent
import dev.emassey0135.lumenna.app.Question
import dev.emassey0135.lumenna.app.SettingKind
import dev.emassey0135.lumenna.app.SettingView
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.ThisDevice
import dev.emassey0135.lumenna.app.Typing
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.foldState
import dev.emassey0135.lumenna.app.menus

/** What only the activity can do. */
interface Host {
    fun exit()
    fun importFile()
    fun requestNotifications()
    fun openNotificationSettings()
    fun deviceName(): String
}

/** A screen the person went to, on the way back from which Escape walks. */
private sealed interface Page {
    data object Places : Page
    data object Listing : Page
    data class Task(val id: String) : Page
    data object Settings : Page
    data class SettingsPage(val id: String) : Page
    data object Devices : Page
    data object Reminders : Page
    data object Due : Page
    data object Help : Page
}

/**
 * The screens, over the Crux app's view model: what is shown is the app's, this decides only how
 * it is laid out on a line at a time, and what the keys do to it. All on the main thread.
 *
 * Each turn the view is laid out afresh as frames; the frame in front is said and shown if it is
 * new or the app moved what is in hand (`landing`), and only put on the cells otherwise, so a
 * sync or a new minute never cuts off what is being read.
 */
class Shell(
    private val core: Core,
    private val out: Speaker,
    private val translator: Translator,
    private val host: Host,
    private val letters: Letters = BrailleLetters.letters,
) {
    private val app: AppModel get() = core.app
    private val pages = ArrayDeque<Page>(listOf(Page.Places))
    private var menu: MenuFrame? = null
    private var frame: Frame? = null

    /** The row in hand in each list, by the frame it is in. */
    private val selected = HashMap<String, String>()

    /** A line being typed, kept across redraws, by what it is for. */
    private val editors = HashMap<String, LineEditor>()

    private var lastLanding = 0u
    private var landedNow = false
    private var lastId = ""
    private var lastSpeech = ""
    private val announcements = ArrayList<String>()
    private var pendingRow: String? = null
    private val handler = android.os.Handler(android.os.Looper.getMainLooper())
    private val saying = Runnable { flush() }

    private fun send(event: ShellEvent) = app.send(event)

    private val view: ViewModel? get() = app.view.value
    private val lines: Lines? get() = app.lines.value

    private fun editor(key: String, initial: String): LineEditor =
        editors.getOrPut(key) { LineEditor(initial, translator) }

    /** Something the app said: said with the next change, or at once if nothing moves. */
    fun announce(text: String) {
        announcements += text
        schedule()
    }

    private fun schedule() {
        handler.removeCallbacks(saying)
        handler.postDelayed(saying, SPEECH_DELAY)
    }

    /** Says what the app said and then the line it left, as one thing. */
    private fun flush() {
        handler.removeCallbacks(saying)
        val parts = announcements.filter { it.isNotBlank() }.map { if (it.last() in ".?!") it else "$it." } + listOfNotNull(pendingRow?.takeIf { it.isNotBlank() })
        announcements.clear()
        pendingRow = null
        if (parts.isNotEmpty()) out.say(parts.joinToString(" "))
    }

    /** The app left a turn. */
    fun turned() = refresh(false)

    /** What is on the screen for a sighted helper: the line in front. */
    fun currentText(): String = frame?.rendered?.speech.orEmpty()

    /** A reminder's notification opened on a task. */
    fun openTask(id: String) = openDetails(id)

    /** Comes to the front: says where the person is. */
    fun front() {
        lastId = ""
        refresh(true)
    }

    private fun build(): Frame {
        menu?.let { return it }
        val model = view ?: return Blank
        model.asking?.let { asked -> question(model, asked)?.let { return it } }
        model.line?.let { return quickAdd(model) }
        model.blockForm?.let { return blockForm(model) }
        model.proposal?.let { return proposal(model) }
        model.reminders?.let { return reminders(model) }
        return when (val page = pages.last()) {
            Page.Places -> places(model)
            Page.Listing -> listing(model)
            is Page.Task -> task(model, page.id)
            Page.Settings -> settings(model)
            is Page.SettingsPage -> settingsPage(model, page.id)
            Page.Devices -> devices(model)
            Page.Reminders -> reminders(model)
            Page.Due -> due(model)
            Page.Help -> help()
        }
    }

    private object Blank : Frame("blank") {
        override fun key(key: Key) = false
        override val rendered = Rendered("")
    }

    private fun refresh(moved: Boolean, fromKey: Boolean = false) {
        val model = view
        val landed = model != null && model.landing != lastLanding
        landedNow = landed
        if (model != null) lastLanding = model.landing
        val built = build()
        frame = built
        val rendered = built.rendered
        out.show(rendered)
        val fresh = built.id != lastId
        lastId = built.id
        val firstContent = lastSpeech.isEmpty() && rendered.speech.isNotEmpty()
        if (moved || fresh || landed || firstContent) {
            val title = if (fresh && built is ListFrame) built.title else ""
            pendingRow = listOf(title, rendered.speech).filter { it.isNotBlank() }.joinToString(". ")
        }
        lastSpeech = rendered.speech
        if (fromKey) flush() else if (pendingRow != null) schedule()
    }

    /** A key, from the braille keys or the QWERTY keyboard. */
    fun press(key: Key) {
        when (key) {
            Key.Silence -> return out.silence()
            Key.Repeat -> return run {
                frame?.rendered?.let { out.say(it.speech); out.show(it) }
            }
            Key.ScrollOn -> {
                if (!out.scrollOn()) step(Key.Down)
                return
            }
            Key.ScrollBack -> {
                if (!out.scrollBack()) step(Key.Up)
                return
            }
            is Key.Route -> {
                val at = out.routed(key.cell)
                if (at >= 0) {
                    frame?.routed(at)
                    refresh(true)
                }
                return
            }
            else -> {}
        }
        out.silence()
        val current = frame ?: build()
        if (current.key(key)) {
            if (typing(current, key)) {
                refresh(false)
                echo(current, key)
            } else {
                refresh(true, fromKey = true)
            }
            return
        }
        // What a frame leaves is the program's.
        when (key) {
            Key.Menu -> openMenu()
            Key.Escape -> back()
            Key.Exit -> host.exit()
            Key.Help -> pushPage(Page.Help)
            Key.Options -> pushPage(Page.Settings)
            Key.Find -> startFiltering()
            Key.Context -> {}
            is Key.SpaceDots -> commandLetter(BrailleLetters.of(key.dots), spaced = true)
            is Key.EnterDots -> commandLetter(BrailleLetters.of(key.dots), spaced = false)
            is Key.SpaceLetter -> commandLetter(key.letter, spaced = true)
            is Key.EnterLetter -> commandLetter(key.letter, spaced = false)
            else -> {}
        }
    }

    private fun step(key: Key) {
        out.silence()
        frame?.key(key)
        refresh(true, fromKey = true)
    }

    private fun typing(frame: Frame, key: Key): Boolean {
        val editing = frame is LineFrame || (frame as? FormFrame)?.current is Control.Edit
        return editing && (key is Key.Cell || key is Key.Typed || key == Key.Backspace || key == Key.Space)
    }

    /** What typing says: the word just finished, or the character typed. */
    private fun echo(frame: Frame, key: Key) {
        val editor = (frame as? LineFrame)?.editor ?: ((frame as? FormFrame)?.current as? Control.Edit)?.editor ?: return
        val word = editor.takeFinished()
        when {
            word != null -> out.say(word)
            key is Key.Typed -> out.say(key.text)
        }
    }

    private fun pushPage(page: Page) {
        pages.addLast(page)
        when (page) {
            Page.Settings, is Page.SettingsPage -> send(ShellEvent.Settings(SettingsEvent.Open))
            Page.Devices -> send(ShellEvent.Devices(DevicesEvent.Show))
            else -> {}
        }
        refresh(true)
    }

    private fun back() {
        val model = view
        when {
            model?.asking != null -> send(ShellEvent.Cancel)
            model?.line != null -> send(ShellEvent.Typing(Typing.Close))
            model?.blockForm != null -> send(ShellEvent.Block(BlockEvent.Cancel))
            model?.proposal != null -> send(ShellEvent.Proposal(ProposalEvent.Close))
            model?.reminders != null -> send(ShellEvent.CloseReminders)
            pages.size > 1 -> {
                val left = pages.removeLast()
                if (left is Page.Settings || left is Page.SettingsPage) {
                    if (pages.last() !is Page.Settings && pages.last() !is Page.SettingsPage) send(ShellEvent.Settings(SettingsEvent.Close))
                }
                refresh(true)
            }
        }
    }

    /** Space or Enter with a letter: the row's command, or the app's. */
    private fun commandLetter(letter: Char?, spaced: Boolean) {
        if (letter == null) return
        if (spaced && letter in BrailleLetters.RESERVED) return
        val row = (frame as? ListFrame)?.current
        if (row?.command?.invoke(letter) == true) return
        val kinds = BrailleLetters.COMMANDS[letter]
        when {
            !spaced && letter == 'n' -> runCommand("new-task")
            !spaced && letter == 'z' -> send(ShellEvent.Undo)
            !spaced && letter == 'y' -> send(ShellEvent.Redo)
            !spaced && letter == 't' -> goTo(Place.Today)
            kinds != null -> send(ShellEvent.Run(kinds))
        }
    }

    private fun startFiltering() {
        val model = view ?: return
        if (model.list == null) return
        val key = "filter"
        val line = editor(key, model.query)
        frame = LineFrame("filter", appWords.quickAdd.filter, line, appWords.quickAdd.filterHint, done = {
            editors.remove(key)
            filtering = false
            refresh(true)
        }, changed = { send(ShellEvent.Filter(it)) })
        filtering = true
        out.show(frame!!.rendered)
        out.say(frame!!.rendered.speech)
    }

    private var filtering = false

    /** Where a place chosen is going, until the app shows it: its view is the last place's till then. */
    private var goingTo: Place? = null

    private fun goTo(place: Place) {
        goingTo = place
        send(ShellEvent.Open(place))
        while (pages.size > 1) pages.removeLast()
        pages.addLast(Page.Listing)
        refresh(true)
    }

    private fun openMenu() {
        val model = view ?: return
        val states = model.commands.associateBy { it.id }
        val groups = menus(true).map { group ->
            val children = group.items.filter { it.id.isNotEmpty() && it.id !in NOT_HERE }.map { item ->
                val title = states[item.id]?.title?.ifEmpty { null } ?: item.title
                MenuEntry(title) { runCommand(item.id) }
            }
            MenuEntry(group.title, children = children)
        }
        menu = MenuFrame(groups, letters, appWords.pullDown, closed = {
            menu = null
            refresh(true)
        })
        refresh(true)
    }

    private fun runCommand(id: String) {
        when (id) {
            "settings" -> pushPage(Page.Settings)
            "devices" -> pushPage(Page.Devices)
            "export-import" -> pushPage(Page.SettingsPage("export"))
            "restore-backup" -> pushPage(Page.SettingsPage("backups"))
            "back-up" -> core.backUpNow()
            "close-window", "quit" -> host.exit()
            "go-today" -> goTo(Place.Today)
            "go-tasks" -> goTo(Place.Tasks)
            "go-blocks" -> goTo(Place.Blocks)
            "go-trash" -> goTo(Place.Trash)
            "go-history" -> goTo(Place.History)
            "filter" -> startFiltering()
            else -> send(ShellEvent.Key(id))
        }
    }

    private fun actionsMenu(row: Row) {
        menu = MenuFrame(row.actions, letters, appWords.pullDown, closed = {
            menu = null
            refresh(true)
        })
        refresh(true)
    }

    private fun entries(on: ActingOn, offered: List<Offered>): List<MenuEntry> =
        offered.sortedByDescending { it.primary }.map { action ->
            MenuEntry(action.name) { send(ShellEvent.ActOn(on, action)) }
        }

    private fun command(on: ActingOn, offered: List<Offered>): (Char) -> Boolean = { letter ->
        val kinds = BrailleLetters.COMMANDS[letter].orEmpty()
        val action = offered.firstOrNull { it.kind in kinds }
        if (action != null) send(ShellEvent.ActOn(on, action))
        action != null
    }

    private fun held(id: String, side: Side?) {
        send(if (side != null) ShellEvent.Hold(side, id) else ShellEvent.Focus(id))
    }

    private fun list(
        id: String,
        title: String,
        rows: List<Row>,
        fallback: String?,
        empty: String,
        side: Side? = null,
        hold: Boolean = true,
    ): ListFrame {
        val appChoice = fallback
        val chosen = if (landedNow && appChoice != null) appChoice else selected[id] ?: appChoice
        return ListFrame(id, title, rows, chosen, empty, letters, moved = { row ->
            selected[id] = row.key
            if (hold) held(row.key, side)
        }, openActions = ::actionsMenu).also { frame ->
            frame.current?.let { selected[id] = it.key }
        }
    }

    // ── Places ─────────────────────────────────────────────────────────────────────────────

    private fun places(model: ViewModel): Frame {
        val rows = model.sidebar.filter { !it.hidden }.map { entry ->
            val fold = entry.offered.firstOrNull { it.kind == ActionKind.EXPAND || it.kind == ActionKind.COLLAPSE }
            val state = listOfNotNull(entry.expanded?.let(::foldState), entry.level)
            val on = ActingOn.Held(Side.SIDEBAR, entry.key)
            val other = entry.offered.filter { it.kind != ActionKind.EXPAND && it.kind != ActionKind.COLLAPSE }
            Row(
                entry.key,
                (listOf(entry.text) + state).joinToString(", "),
                finding = entry.title,
                open = when (val kind = entry.kind) {
                    is SidebarKind.Place -> ({ goTo(kind.field0) })
                    is SidebarKind.Group -> ({ fold?.let { send(ShellEvent.ActOn(on, it)) } })
                },
                actions = entries(on, entry.offered),
                command = command(on, other),
            )
        }
        return list("places", appWords.places, rows, model.sidebarHeld.focus, "", side = Side.SIDEBAR)
    }

    private fun listing(model: ViewModel): Frame {
        if (filtering) frame?.takeIf { it.id == "filter" }?.let { return it }
        // Until the app shows the place chosen, its view is the place left.
        goingTo?.let { if (model.place != it) return Rendering("") else goingTo = null }
        model.day?.let { return day(model, it) }
        val list = model.list ?: return Rendering(model.title)
        val worded = lines?.list.orEmpty()
        val rows = shownRows(list.items, worded).map { (item, line) ->
            val task = item.row
            val offered = list.offeredOf(item)
            val on = ActingOn.Row(task.id)
            val state = listOfNotNull(item.row.expanded?.let(::foldState), item.level, line?.detail?.ifEmpty { null })
            Row(
                task.id,
                (listOf(line?.title ?: task.title) + state).joinToString(", "),
                braille = line?.braille?.ifEmpty { null } ?: line?.text ?: task.title,
                finding = task.title,
                open = { openDetails(task.id) },
                space = offered.firstOrNull { it.kind == ActionKind.MARK_DONE || it.kind == ActionKind.MARK_NOT_DONE }?.let { action ->
                    { send(ShellEvent.ActOn(on, action)) }
                },
                actions = entries(on, offered),
                command = command(on, offered),
                delete = { held(task.id, null); send(ShellEvent.Key("delete")) },
            )
        }
        val empty = (list.refusal ?: list.empty)
        return list("list:${model.title}", model.title, rows, model.focus, empty)
    }

    private fun Rendering(text: String): Frame = object : Frame(if (text.isEmpty()) "going" else "rendering:$text") {
        override fun key(key: Key) = false
        override val rendered = Rendered(text)
    }

    private fun openDetails(id: String) {
        send(ShellEvent.Focus(id))
        send(ShellEvent.Details(DetailsEvent.Show(id)))
        pushPage(Page.Task(id))
    }

    private fun day(model: ViewModel, day: dev.emassey0135.lumenna.app.DayView): Frame {
        val worded = lines?.day.orEmpty()
        val rows = day.items.mapIndexedNotNull { index, item ->
            if (item.hidden) return@mapIndexedNotNull null
            val line = worded.getOrNull(index)
            val heading = item.kind == DayKind.SUMMARY
            val on = if (heading) ActingOn.Day else ActingOn.Row(item.key)
            val offered = if (heading) day.offered else item.offered
            val state = listOfNotNull(item.expanded?.let(::foldState), item.level)
            val edits = offered.any { it.kind == ActionKind.EDIT || it.kind == ActionKind.EDIT_TASK || it.kind == ActionKind.RESTORE_DAY || it.kind == ActionKind.ADD_BLOCK }
            val actions = entries(on, offered)
            Row(
                item.key,
                (listOf(line?.text.orEmpty()) + state).joinToString(", "),
                braille = line?.braille?.ifEmpty { null } ?: line?.text.orEmpty(),
                finding = line?.title.orEmpty(),
                open = { if (edits && !heading) { held(item.key, null); send(ShellEvent.Key("open")) } else if (actions.isNotEmpty()) openDayActions(item.key, actions) },
                actions = actions,
                command = command(on, offered),
                delete = { held(item.key, null); send(ShellEvent.Key("delete")) },
            )
        }
        return list("day:${day.date}", lines?.dayTitle.orEmpty(), rows, model.focus, "")
    }

    private fun openDayActions(key: String, actions: List<MenuEntry>) {
        menu = MenuFrame(actions, letters, appWords.pullDown, closed = { menu = null; refresh(true) })
        refresh(true)
    }

    // ── Details ────────────────────────────────────────────────────────────────────────────

    private fun task(model: ViewModel, id: String): Frame {
        val details = model.details?.takeIf { it.task.id == id } ?: return Rendering(appWords.details.gone)
        val form = details.fields
        val set = { key: String, value: String -> send(ShellEvent.Details(DetailsEvent.Field(key, value))) }
        val controls = mutableListOf<Control>()
        controls += Control.Static("title-heading", details.task.title)
        details.form.forEach { field ->
            when {
                field.key == "project" -> {
                    val options = details.projects.map { it.id to it.title }
                    val at = details.projects.indexOfFirst { it.title == form.project }.coerceAtLeast(0)
                    controls += Control.Choice(field.key, field.label, options, at, { set("project", it) })
                }
                form.text(field.key) != null -> {
                    val edit = editor("task:$id:${field.key}", form.text(field.key).orEmpty())
                    controls += Control.Edit(field.key, field.label, edit, field.hint, changed = { set(field.key, it) })
                }
                else -> {
                    val options = field.options.map { it.id to it.title }
                    val at = field.options.indexOfFirst { it.id == form.priority.toString() }.coerceAtLeast(0)
                    controls += Control.Choice(field.key, field.label, options, at, { set(field.key, it) })
                }
            }
        }
        controls += Control.Button("save", appWords.details.save) { send(ShellEvent.Details(DetailsEvent.Save)) }
        details.lines.forEachIndexed { index, line -> controls += Control.Static("line$index", "${line.label}: ${line.value}") }
        details.task.offered.forEach { action ->
            controls += Control.Button("act:${action.kind}", action.name) { send(ShellEvent.ActOn(ActingOn.Details, action)) }
        }
        return form("task:$id", controls)
    }

    private fun form(id: String, controls: List<Control>): Frame =
        FormFrame(id, controls, selected[id], letters, moved = { selected[id] = it.key })

    private fun blockForm(model: ViewModel): Frame {
        val form = model.blockForm!!
        val fields = form.values
        val set = { key: String, value: String -> send(ShellEvent.Block(BlockEvent.Field(key, value))) }
        val controls = mutableListOf<Control>()
        controls += Control.Static("heading", form.heading)
        form.refusal?.let { controls += Control.Static("refusal", it) }
        form.fields.forEach { field ->
            when (field.kind) {
                FieldKind.CHOICE -> {
                    val options = field.options.map { it.id to it.title }
                    val at = field.options.indexOfFirst { it.id == fields.text(field.key) }.coerceAtLeast(0)
                    controls += Control.Choice(field.key, field.label, options, at, { set(field.key, it) })
                }
                FieldKind.TOGGLE -> {
                    val on = fields.flag(field.key)
                    controls += Control.Toggle(field.key, field.label, on, onOff(on)) { set(field.key, it.toString()) }
                }
                else -> {
                    val initial = if (field.key == "date") form.date else fields.text(field.key)
                    val edit = editor("block:${form.heading}:${field.key}", initial)
                    controls += Control.Edit(field.key, field.label, edit, field.hint, changed = { set(field.key, it.trim()) })
                }
            }
        }
        controls += Control.Button("save", form.yes) { send(ShellEvent.Block(BlockEvent.Save)) }
        return form("blockform", controls)
    }

    private fun onOff(on: Boolean): String = if (on) appWords.on else appWords.off

    // ── Questions ──────────────────────────────────────────────────────────────────────────

    private fun question(model: ViewModel, asked: dev.emassey0135.lumenna.app.Asking): Frame? {
        val id = "question:${asked.number}"
        val prefix = asked.refusal?.let { "$it " }.orEmpty()
        val cancelled = { send(ShellEvent.Cancel) }
        val answer = { given: Answer -> send(ShellEvent.Answer(given)) }
        return when (val question = asked.question) {
            Question.Immediate -> null
            Question.Form -> {
                val action = asked.action
                if (action != null && action.kind == ActionKind.EDIT_TASK) {
                    send(ShellEvent.Details(DetailsEvent.Show(action.target)))
                    pages.addLast(Page.Task(action.target))
                }
                cancelled()
                null
            }
            is Question.Confirm -> {
                val rows = listOf(
                    Row("yes", question.yes, open = { answer(Answer.Yes) }),
                    Row("no", appWords.cancel, open = { cancelled() }),
                )
                val title = listOf(prefix + question.title, question.message).filter { it.isNotBlank() }.joinToString(". ")
                list(id, title, rows, null, "", hold = false)
            }
            is Question.Text -> {
                val prompt = prefix + listOf(question.sentence.ifEmpty { question.title }, question.label).filter { it.isNotBlank() }.joinToString(", ")
                val line = editor(id, asked.typed ?: question.initial)
                LineFrame(id, prompt, line, question.hint, done = {
                    answer(Answer.Text(it))
                    editors.remove(id)
                })
            }
            is Question.Pick -> {
                val offered = asked.choices ?: return Rendering(question.title)
                val worded = lines?.choices.orEmpty()
                val rows = offered.choices.mapIndexed { index, choice ->
                    val line = worded.getOrNull(index)
                    Row(choice.id, listOfNotNull(line?.text ?: choice.title, choice.reason, line?.level).joinToString(", "),
                        braille = line?.braille?.ifEmpty { null } ?: line?.text ?: choice.title, finding = choice.title,
                        open = { answer(Answer.Picked(choice.id, null)) })
                }
                list(id, prefix + question.title, rows, null, "", hold = false)
            }
            is Question.Choose -> {
                val rows = question.answers.map { choice -> Row(choice.id, choice.title, open = { answer(Answer.Picked(choice.id, null)) }) }
                list(id, listOf(prefix + question.title, question.message).filter { it.isNotBlank() }.joinToString(". "), rows, null, "", hold = false)
            }
        }
    }

    // ── Quick add ──────────────────────────────────────────────────────────────────────────

    private fun quickAdd(model: ViewModel): Frame {
        val line = model.line!!
        val key = "quickadd"
        val edit = editor(key, line.text)
        val words = appWords.quickAdd
        val controls = mutableListOf<Control>()
        val typing = { text: String -> send(ShellEvent.Typing(Typing.Text(text, TextOffsets.bytes(text.length, text)))) }
        controls += Control.Edit(key, words.label, edit, words.example, typing, submit = { send(ShellEvent.QuickAdd(edit.text)) })
        val candidates = line.offer.candidates
        candidates.take(appWords.completionsShown.toInt()).forEachIndexed { index, candidate ->
            controls += Control.Button("complete$index", dev.emassey0135.lumenna.app.completeWith(candidate)) {
                send(ShellEvent.Typing(Typing.Choose(index.toUInt())))
                editors.remove(key)
            }
        }
        line.readback.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("readback", it.replaceFirstChar { c -> c.uppercase() }) }
        model.addRefused?.let { controls += Control.Static("refused", it) }
        controls += Control.Button("add", words.add) { send(ShellEvent.QuickAdd(edit.text)) }
        return form("quickadd", controls)
    }

    // ── Proposals ──────────────────────────────────────────────────────────────────────────

    private fun proposal(model: ViewModel): Frame {
        val proposal = model.proposal!!
        val words = proposal.words
        val rows = mutableListOf<Row>()
        proposal.refusal?.let { rows += Row("refusal", it) }
        if (proposal.ready && proposal.changes.isEmpty()) rows += Row("nothing", words.nothing)
        proposal.changes.forEachIndexed { index, change ->
            val text = listOfNotNull(change.state, change.sentence, change.reason).joinToString(", ")
            rows += Row("change$index", text, finding = change.sentence, open = { send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked))) },
                space = { send(ShellEvent.Proposal(ProposalEvent.Check(index.toUInt(), !change.checked))) })
        }
        proposal.unfit.forEachIndexed { index, line -> rows += Row("unfit$index", "${words.unfit}: $line") }
        proposal.others.forEachIndexed { index, other ->
            rows += Row("other$index", other, open = { send(ShellEvent.Proposal(ProposalEvent.Take(index.toUInt()))) })
        }
        if (proposal.changes.isNotEmpty()) rows += Row("apply", words.apply, open = { send(ShellEvent.Proposal(ProposalEvent.Apply)) })
        rows += Row("close", words.cancel, open = { send(ShellEvent.Proposal(ProposalEvent.Close)) })
        return list("proposal", words.title, rows, null, "", hold = false)
    }

    // ── Reminders ─────────────────────────────────────────────────────────────────────────

    private fun reminders(model: ViewModel): Frame {
        val list = model.reminders ?: return Rendering(model.remindersTitle.orEmpty())
        val worded = lines?.reminders.orEmpty()
        val rows = list.reminders.mapIndexed { index, reminder ->
            val line = worded.getOrNull(index)
            val words = line?.text ?: reminder.words
            val on = ActingOn.Held(Side.REMINDERS, reminder.id)
            val actions = if (reminder.own) entries(on, reminder.offered) else emptyList()
            Row(reminder.id, words, open = { if (actions.isNotEmpty()) openDayActions(reminder.id, actions) }, actions = actions)
        } + list.offered.map { action -> Row("add:${action.kind}", action.name, open = { send(ShellEvent.ActOn(ActingOn.Reminders, action)) }) }
        val title = list.announcement.replaceFirstChar { it.uppercase() }
        return list("reminders", title, rows, model.remindersHeld.focus, "", side = Side.REMINDERS)
    }

    private fun due(model: ViewModel): Frame {
        val rows = model.due.map { due ->
            val buttons = due.buttons.map { button -> MenuEntry(button.title) { pressDue(due.id, button) } }
            Row(due.id, due.text, open = { if (buttons.isNotEmpty()) openDayActions(due.id, buttons) }, actions = buttons)
        }
        return list("due", appWords.remindersDue, rows, model.dueHeld.focus, "", side = Side.DUE)
    }

    private fun pressDue(id: String, button: dev.emassey0135.lumenna.app.DueButton) {
        val event = when (button.id) {
            "dismiss" -> dev.emassey0135.lumenna.app.DueEvent.Dismiss(id)
            "snooze" -> dev.emassey0135.lumenna.app.DueEvent.Snooze(id)
            "carry" -> dev.emassey0135.lumenna.app.DueEvent.Carry(id)
            "replan" -> dev.emassey0135.lumenna.app.DueEvent.Replan(id)
            else -> return
        }
        send(ShellEvent.Due(event))
    }

    // ── Settings ──────────────────────────────────────────────────────────────────────────

    private fun settings(model: ViewModel): Frame {
        val listed = model.settings?.settings.orEmpty()
        val rows = SettingsPages.shown(listed).map { page ->
            Row(page.id, listOf(page.title, page.description).filter { it.isNotEmpty() }.joinToString(": "), finding = page.title, open = {
                when (page.id) {
                    "devices" -> pushPage(Page.Devices)
                    else -> pushPage(Page.SettingsPage(page.id))
                }
            })
        }
        return list("settings", commandTitle("settings"), rows, null, "", hold = false)
    }

    private fun settingsPage(model: ViewModel, id: String): Frame {
        val page = SettingsPages.of(id)
        val controls = mutableListOf<Control>()
        controls += Control.Static("heading", page?.title.orEmpty())
        val all = model.settings?.settings.orEmpty().on(page)
        all.filter { it.setting.kind != SettingKind.FOLDER }.forEach { addSetting(controls, it) }
        if (id == "reminders") reminderControls(controls)
        page?.buttons.orEmpty().forEach { button ->
            controls += Control.Button(button.id, button.title) {
                when (button.id) {
                    "back-up" -> core.backUpNow()
                    "sync-now" -> app.syncNow()
                    else -> host.importFile()
                }
            }
        }
        if (id == "export") model.settings?.exports.orEmpty().forEach { export ->
            controls += Control.Button("export:${export.format}", export.label) { send(ShellEvent.Settings(SettingsEvent.Export(export.format))) }
        }
        page?.caption?.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("caption", it) }
        if (id == "backups" || id == "export") {
            model.settings?.offered.orEmpty().forEach { action ->
                controls += Control.Button("act:${action.kind}", action.name) { send(ShellEvent.ActOn(ActingOn.Settings, action)) }
            }
            model.settings?.offeredCaption?.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("offeredCaption", it) }
        }
        return form("settings:$id", controls)
    }

    private fun addSetting(controls: MutableList<Control>, shown: SettingView) {
        val setting = shown.setting
        val set = { value: String -> send(ShellEvent.Settings(SettingsEvent.Set(setting.key, value))) }
        when (setting.kind) {
            SettingKind.TOGGLE -> {
                val on = setting.value == "true"
                controls += Control.Toggle(setting.key, setting.title, on, onOff(on)) { set(if (it) "true" else "false") }
            }
            SettingKind.CHOICE -> {
                val options = setting.options.map { it.id to it.title }
                val at = setting.options.indexOfFirst { it.id == setting.value }.coerceAtLeast(0)
                controls += Control.Choice(setting.key, setting.title, options, at, set)
            }
            else -> {
                val edit = editor("setting:${setting.key}", shown.typed.ifEmpty { setting.value })
                controls += Control.Edit(setting.key, setting.title, edit, setting.hint, changed = {
                    send(ShellEvent.Settings(SettingsEvent.Type(setting.key, it)))
                })
            }
        }
        shown.problem?.let { controls += Control.Static("problem:${setting.key}", it) }
        if (setting.hint.isNotEmpty() && setting.kind != SettingKind.LINE && setting.kind != SettingKind.TIME && setting.kind != SettingKind.NUMBER) {
            controls += Control.Static("hint:${setting.key}", setting.hint)
        }
    }

    private fun reminderControls(controls: MutableList<Control>) {
        val words = appWords.notifications
        val access = reminderAccess()
        if (!access.notifications) {
            controls += Control.Static("notifications-off", words.off)
            controls += Control.Button("allow-notifications", words.allow) { host.requestNotifications() }
        }
        if (!access.exact) controls += Control.Static("exact-late", words.late)
    }

    /** What Android allows reminders, asked of the system each time it is shown. */
    var reminderAccess: () -> dev.emassey0135.lumenna.ReminderAccess = { dev.emassey0135.lumenna.ReminderAccess(true, true, null) }

    // ── Devices ───────────────────────────────────────────────────────────────────────────

    private fun devices(model: ViewModel): Frame {
        val devices = model.devices ?: return Rendering(SettingsPages.of("devices")?.title.orEmpty())
        val page = SettingsPages.of("devices")
        val worded = lines?.devices.orEmpty()
        val controls = mutableListOf<Control>()
        controls += Control.Static("heading", page?.title.orEmpty())
        devices.status.takeIf { it.isNotEmpty() }?.let { controls += Control.Static("status", it) }
        val syncing = page?.buttons?.firstOrNull { it.id == "sync-now" }?.title ?: commandTitle("sync-now")
        controls += Control.Button("sync", syncing) { app.syncNow() }
        devices.list?.devices.orEmpty().forEachIndexed { index, device ->
            val line = worded.getOrNull(index)
            val text = line?.text ?: device.name
            val on = ActingOn.Held(Side.DEVICES, device.nodeId)
            device.offered.forEach { action ->
                controls += Control.Button("${device.nodeId}:${action.kind}", "$text: ${action.name}") { send(ShellEvent.ActOn(on, action)) }
            }
            if (device.offered.isEmpty()) controls += Control.Static(device.nodeId, text)
        }
        addPairing(controls, devices)
        return form("devices", controls)
    }

    private fun addPairing(controls: MutableList<Control>, devices: dev.emassey0135.lumenna.app.DevicesView) {
        val words = devices.words
        val pairing = devices.pairing
        val pair = { event: PairEvent -> send(ShellEvent.Devices(DevicesEvent.Pair(event))) }
        controls += Control.Static("pairing", words.title)
        val status = pairing?.problem ?: when {
            pairing?.mine != null -> words.waiting
            pairing?.running == true -> words.connecting
            else -> words.introSentence.ifEmpty { words.intro }
        }
        controls += Control.Static("pairing-status", status)
        pairing?.mineLine?.let { controls += Control.Static("mine", it) }
        if (pairing?.running != true) controls += Control.Button("wait", words.wait) { pair(PairEvent.Wait) }
        if (pairing?.running != true || pairing.waiting) {
            val edit = editor("pairing-code", pairing?.theirs.orEmpty())
            controls += Control.Edit("code", words.theirCode, edit, changed = { pair(PairEvent.Code(it)) }, submit = {
                pair(PairEvent.Code(edit.text))
                pair(PairEvent.Join)
            })
            controls += Control.Button("join", words.join) {
                pair(PairEvent.Code(edit.text))
                pair(PairEvent.Join)
            }
        }
        pairing?.words?.let { three ->
            controls += Control.Static("match", "${words.matchTitle} ${pairing.matching ?: three.joinToString(", ")}")
            controls += Control.Button("match-yes", words.matchYes) { pair(PairEvent.Confirm(true)) }
            controls += Control.Button("match-no", words.matchNo) { pair(PairEvent.Confirm(false)) }
        }
    }

    // ── Help ─────────────────────────────────────────────────────────────────────────────

    private fun help(): Frame {
        val rows = dev.emassey0135.lumenna.app.braillesenseKeys().map { row ->
            Row(row.keys, "${row.keys}: ${row.what}", finding = row.what)
        }
        return list("help", appWords.keys, rows, null, "", hold = false)
    }

    private companion object {
        /** How long after a turn its speech waits, for what the app said to arrive with it. */
        const val SPEECH_DELAY = 60L

        /** A command of the shared table that means nothing on a device with one screen. */
        val NOT_HERE = setOf("show", "next-pane", "previous-pane", "new-window")
    }
}
