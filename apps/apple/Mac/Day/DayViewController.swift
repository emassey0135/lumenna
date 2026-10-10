import AppKit

/// One row of the day as an outline item.
final class DayNode {
    enum Kind {
        case block(PlanBlock)
        case sitting(PlanAssignment, in: PlanBlock)
        case free(start: String, end: String, minutes: UInt32, title: String, details: [String], actions: [Action])
        case now(time: String, title: String)
        /// A repeating block cancelled for this day alone, so the day can be put back.
        case cancelled(CancelledBlock)
    }

    let kind: Kind
    var children: [DayNode] = []

    init(_ kind: Kind) { self.kind = kind }

    /// What identifies it across a reload, for keeping the selection.
    var key: String {
        switch kind {
        case let .block(block): "block:\(block.id)"
        case let .sitting(sitting, _): "sitting:\(sitting.id)"
        case let .free(start, _, _, _, _, _): "free:\(start)"
        case .now: "now"
        case let .cancelled(block): "cancelled:\(block.series)"
        }
    }
}

/// The planner: a day as it is lived, as an outline.
///
/// Blocks in time order with their sittings beneath them, free time as rows of its own, and
/// now as a position rather than a highlight. The summary above says what a glance at a
/// timeline would. Opening on today puts the selection — VoiceOver's focus — on now.
final class DayViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate, NSMenuDelegate {
    private let core: Core
    private weak var main: MainWindowController?
    /// The day shown, as an ISO date; `nil` follows today.
    private var day: String?
    private(set) var plan: Plan?
    private var nodes: [DayNode] = []
    let outline = TaskOutline()
    private let summary = HeadingLabel(wrappingLabelWithString: "")
    private var landedOnNow = false

    init(core: Core, window: MainWindowController) {
        self.core = core
        main = window
        super.init(nibName: nil, bundle: nil)
        title = "Today"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        summary.font = .preferredFont(forTextStyle: .subheadline)
        summary.textColor = .quietLabel
        // The day's heading, as on the phone: VoiceOver's heading commands land on it.

        let buttons = NSStackView(views: [
            NSButton(title: "Previous Day", target: self, action: #selector(previousDay(_:))),
            NSButton(title: "Today", target: self, action: #selector(goToToday(_:))),
            NSButton(title: "Next Day", target: self, action: #selector(nextDay(_:))),
            NSButton(title: "Go to Day…", target: self, action: #selector(goToDay(_:))),
            NSButton(title: "Add Block…", target: self, action: #selector(newBlock(_:))),
        ])
        buttons.spacing = 8

        let column = NSTableColumn(identifier: .init("day"))
        outline.addTableColumn(column)
        outline.outlineTableColumn = column
        outline.headerView = nil
        outline.dataSource = self
        outline.delegate = self
        outline.usesAutomaticRowHeights = true
        outline.style = .inset
        outline.setAccessibilityLabel("The day")
        outline.target = self
        outline.doubleAction = #selector(activate)
        outline.keys = { [weak self] key in self?.key(key) ?? false }
        let menu = NSMenu()
        menu.delegate = self
        outline.menu = menu
        let scroll = NSScrollView()
        scroll.documentView = outline
        scroll.hasVerticalScroller = true

        let header = NSStackView(views: [summary, buttons])
        header.orientation = .vertical
        header.alignment = .leading
        header.spacing = 8
        header.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 6, right: 12)
        summary.widthAnchor.constraint(equalTo: header.widthAnchor, constant: -24).isActive = true
        let stack = NSStackView(views: [header, scroll])
        stack.orientation = .vertical
        stack.spacing = 0
        header.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        scroll.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        view = stack
        NotificationCenter.default.addObserver(self, selector: #selector(storeChanged), name: Core.changed, object: nil)
        reload()
    }

    override var preferredFirstResponder: NSView? { outline }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(outline)
        // Opening the day lands on now, not at midnight. Once, so coming back from
        // elsewhere does not move the selection from where the person left it.
        if !landedOnNow {
            landedOnNow = true
            goToNow(announcing: false)
        }
    }

    // MARK: - Rows

    @objc private func storeChanged() {
        reload(keeping: selected?.key, near: outline.selectedRow)
    }

    private func reload() {
        do {
            let plan = try core.lumenna.plan(date: day)
            self.plan = plan
            title = Clock.spokenDay(plan.date)
            main?.window?.title = title ?? "Today"
            summary.stringValue = "\(Clock.spokenDay(plan.date)). \(plan.summary)"
            var nodes: [DayNode] = []
            for item in plan.timeline {
                switch item {
                case let .block(row):
                    guard let block = plan.blocks.first(where: { $0.row == row }) else { continue }
                    let node = DayNode(.block(block))
                    node.children = block.assignments.map { DayNode(.sitting($0, in: block)) }
                    nodes.append(node)
                case let .free(start, end, minutes, title, details, actions):
                    nodes.append(DayNode(.free(start: start, end: end, minutes: minutes, title: title, details: details, actions: actions)))
                case let .now(time, title):
                    nodes.append(DayNode(.now(time: time, title: title)))
                }
            }
            nodes += plan.cancelled.map { DayNode(.cancelled($0)) }
            self.nodes = nodes
        } catch {
            summary.stringValue = error.sentence
        }
        outline.reloadData()
        outline.expandItem(nil, expandChildren: true)
    }

    private func reload(keeping key: String?, near index: Int?, saying change: Change? = nil) {
        reload()
        let target = key.flatMap { key in
            (0..<outline.numberOfRows).first { (outline.item(atRow: $0) as? DayNode)?.key == key }
        } ?? index.map { min($0, outline.numberOfRows - 1) }
        if let target, target >= 0 {
            outline.selectRowIndexes([target], byExtendingSelection: false)
            outline.scrollRowToVisible(target)
        }
        if let change { Announcer.say(change.announcement, notices: change.notices) }
    }

    var selected: DayNode? { outline.item(atRow: outline.selectedRow) as? DayNode }

    /// The task a selected sitting is for: what the Task menu acts on from the day.
    var selectedTaskID: String? {
        if case let .sitting(sitting, _) = selected?.kind { return sitting.task }
        return nil
    }

    func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        (item as? DayNode)?.children.count ?? nodes.count
    }

    func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        (item as? DayNode)?.children[index] ?? nodes[index]
    }

    func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
        !((item as? DayNode)?.children.isEmpty ?? true)
    }

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? DayNode else { return nil }
        let cell = TwoLineCell()
        var label: String
        var value: [String] = []
        switch node.kind {
        case let .block(block):
            label = "\(Clock.time(block.start)) to \(Clock.time(block.end)), \(block.title)"
            // The core words the details for every app.
            value = block.details
        case let .sitting(sitting, _):
            label = sitting.title
            value = sitting.details
        case let .free(start, end, _, title, details, _):
            // "<title>, <details>, <start> to <end>", the core's words in every app's order.
            label = ([title] + details).joined(separator: ", ")
            value = ["\(Clock.time(start)) to \(Clock.time(end))"]
        case let .now(time, title):
            label = "\(title), \(Clock.time(time))"
            cell.title.font = .preferredFont(forTextStyle: .headline)
        case let .cancelled(block):
            label = "\(Clock.time(block.start)), \(block.title)"
            value = block.details
        }
        cell.show(title: label, detail: value.joined(separator: ", "))
        cell.setAccessibilityLabel(label)
        cell.setAccessibilityValueDescription(value.joined(separator: ", "))
        return cell
    }

    func outlineViewSelectionDidChange(_ notification: Notification) {
        if let task = selectedTaskID { main?.showTask(task) } else { main?.showTask(nil) }
    }

    // MARK: - Keys and activation

    @objc private func activate() {
        guard let node = selected else { return }
        switch node.kind {
        case let .block(block): edit(block)
        case .sitting: main?.nextPane(nil)
        case let .free(start, _, minutes, _, _, _): addBlock(at: start, minutes: minutes)
        case let .cancelled(block): restore(block)
        case .now: break
        }
    }

    /// Return activates a row; Space and Delete are the row's own actions of those kinds:
    /// Space starts, pauses or resumes a sitting's timer, Delete takes a sitting out or
    /// deletes a block — each also in the row's menu.
    private func key(_ key: TaskOutline.Key) -> Bool {
        guard let node = selected else { return false }
        let actions = actions(for: node)
        let action: Action? = switch key {
        case .returnKey: nil
        case .space: actions.first(.startTimer, .resumeTimer, .pauseTimer)
        case .delete: actions.first(.delete, .unassign)
        }
        if key == .returnKey {
            activate()
        } else if let action {
            perform(action, on: node)
        } else {
            // Why not, in the core's words: silence would leave the key a guess.
            let subject: Subject = if case .sitting = node.kind { .sitting } else { .block }
            Announcer.say(notOffered(kind: key == .space ? .startTimer : .delete, subject: subject, thisDevice: false))
        }
        return true
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = outline.clickedRow >= 0 ? outline.clickedRow : outline.selectedRow
        guard let node = outline.item(atRow: clicked) as? DayNode else { return }
        if outline.selectedRow != clicked { outline.selectRowIndexes([clicked], byExtendingSelection: false) }
        menu.add(actions(for: node)) { [weak self] action in self?.perform(action, on: node) }
    }

    /// What can be done to a row, the core's.
    private func actions(for node: DayNode) -> [Action] {
        switch node.kind {
        case let .block(block): block.actions
        case let .sitting(sitting, _): sitting.actions
        case let .free(_, _, _, _, _, actions): actions
        case let .cancelled(block): block.actions
        case .now: []
        }
    }

    // MARK: - Doing things

    /// Runs a row's action: asks its question, then keeps the selection on the row, or near
    /// where it was, and says what happened. The forms are this screen's.
    private func perform(_ action: Action, on node: DayNode) {
        guard let window = view.window else { return }
        let key = node.key
        let index = outline.selectedRow
        window.run(action, core: core, form: { [weak self] action in self?.form(action, on: node) }) { [weak self] change, _ in
            self?.reload(keeping: key, near: index, saying: change)
        }
    }

    private func form(_ action: Action, on node: DayNode) {
        switch (action.kind, node.kind) {
        case let (.edit, .block(block)): edit(block)
        case let (.addBlock, .free(start, _, minutes, _, _, _)): addBlock(at: action.other ?? start, minutes: minutes)
        case (.editTask, _): main?.showTask(action.target); main?.nextPane(nil)
        default: break
        }
    }

    /// Return on a cancelled day: its Restore This Day.
    private func restore(_ block: CancelledBlock) {
        guard let node = selected, let action = block.actions.first(.restoreDay) else { return }
        perform(action, on: node)
    }

    /// Asks "this day, or every day?" of a repeating block — never guessed.
    private func edit(_ block: PlanBlock) {
        guard let window = view.window else { return }
        let key = "block:\(block.id)"
        let saved: (Change) -> Void = { [weak self] change in self?.reload(keeping: key, near: nil, saying: change) }
        let series = { [weak self] in
            guard let self else { return }
            do {
                try BlockFormModel.series(core: self.core, id: block.series).present(on: window, saved: saved)
            } catch {
                window.showFailure(error.sentence)
            }
        }
        guard block.repeats, let date = plan?.date else {
            series()
            return
        }
        window.choose("Change \(block.title)", message: "Which occurrences?", actions: [
            ("\(Clock.spokenDay(date)) Only", { [weak self] in
                guard let self else { return }
                BlockFormModel.occurrence(core: self.core, block: block, day: date).present(on: window, saved: saved)
            }),
            ("Every Occurrence", { DispatchQueue.main.async(execute: series) }),
        ])
    }

    func addBlock(at start: String? = nil, minutes: UInt32? = nil) {
        guard let window = view.window else { return }
        let day = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        BlockFormModel(
            core: core, purpose: .add, start: start, minutes: Int(min(minutes ?? 60, 720)), day: day
        ).present(on: window) { [weak self] change in
            self?.reload(keeping: nil, near: nil, saying: change)
        }
    }

    // MARK: - Moving through days

    @objc func previousDay(_ sender: Any?) { step(-1) }
    @objc func nextDay(_ sender: Any?) { step(1) }
    @objc func goToToday(_ sender: Any?) { goToNow(announcing: true) }
    @objc func newBlock(_ sender: Any?) { addBlock() }

    private var shownDate: Date {
        plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
    }

    private func step(_ days: Int) {
        guard let next = Calendar.current.date(byAdding: .day, value: days, to: shownDate) else { return }
        show(next)
    }

    private func show(_ date: Date) {
        showDay(iso: Clock.isoDay(date))
    }

    private func showDay(iso: String) {
        day = iso == Clock.isoDay(.now) ? nil : iso
        reload()
        // A new day is a new screen's worth: say it, and start at its top.
        if outline.numberOfRows > 0 { outline.selectRowIndexes([0], byExtendingSelection: false) }
        Announcer.say(summary.stringValue)
    }

    /// "Go to now": today, on the now row or the block happening now.
    private func goToNow(announcing: Bool) {
        day = nil
        reload()
        let index = (0..<outline.numberOfRows).first { row in
            switch (outline.item(atRow: row) as? DayNode)?.kind {
            case .now: true
            case let .block(block): block.when == "now"
            default: false
            }
        }
        if let index {
            outline.selectRowIndexes([index], byExtendingSelection: false)
            outline.scrollRowToVisible(index)
        } else if outline.numberOfRows > 0 {
            outline.selectRowIndexes([0], byExtendingSelection: false)
        }
        if announcing { Announcer.say(summary.stringValue) }
    }

    @objc func goToDay(_ sender: Any?) { askDay(typed: "", problem: nil) }

    /// The core's question, answered by typing a day ("next friday", "12 October") or with
    /// the system's calendar. A day the core cannot read asks again, saying why, with what was
    /// typed still there.
    private func askDay(typed: String, problem: String?) {
        guard let window = view.window else { return }
        let question = TextQuestion.goToDay
        let alert = NSAlert()
        alert.messageText = question.title
        alert.informativeText = [problem, question.hint].compactMap { $0 }.joined(separator: "\n\n")
        let field = NSTextField(string: typed)
        field.placeholderString = question.label
        field.setAccessibilityLabel(question.label)
        field.setAccessibilityHelp(question.hint)
        let picker = NSDatePicker()
        picker.datePickerStyle = .clockAndCalendar
        picker.datePickerElements = .yearMonthDay
        picker.dateValue = shownDate
        picker.sizeToFit()
        picker.setAccessibilityLabel(question.label)
        let stack = NSStackView(views: [field, picker])
        stack.orientation = .vertical
        stack.alignment = .leading
        field.widthAnchor.constraint(equalToConstant: max(picker.frame.width, 240)).isActive = true
        stack.frame = NSRect(x: 0, y: 0, width: max(picker.frame.width, 240), height: picker.frame.height + 32)
        alert.accessoryView = stack
        alert.addButton(withTitle: question.yes)
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] response in
            guard let self, response == .alertFirstButtonReturn else { return }
            let text = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            let named = text.isEmpty ? Clock.isoDay(picker.dateValue) : text
            do {
                // The core reads the day; the one it found is kept, so the days step from it.
                let found = try self.core.lumenna.plan(date: named).date
                self.showDay(iso: found)
            } catch {
                DispatchQueue.main.async { self.askDay(typed: text, problem: error.sentence) }
            }
        }
    }

}
