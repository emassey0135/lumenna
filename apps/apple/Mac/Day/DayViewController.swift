import AppKit

/// One row of the day as an outline item.
final class DayNode {
    enum Kind {
        case block(PlanBlock)
        case sitting(PlanAssignment, in: PlanBlock)
        case free(start: String, end: String, minutes: UInt32)
        case now(String)
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
        case let .free(start, _, _): "free:\(start)"
        case .now: "now"
        case let .cancelled(block): "cancelled:\(block.series)"
        }
    }
}

/// The planner (§16.1, §13's worked example): a day as it is lived, as an outline.
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
    private let summary = NSTextField(wrappingLabelWithString: "")
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
        summary.setAccessibilityRole(NSAccessibility.Role(rawValue: "AXHeading"))

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
        // §13: opening the day lands on now, not at midnight. Once, so coming back from
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
                case let .free(start, end, minutes):
                    nodes.append(DayNode(.free(start: start, end: end, minutes: minutes)))
                case let .now(time):
                    nodes.append(DayNode(.now(time)))
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
            value = [Clock.length(block.durationMins), "\(block.kind) block"]
            if !block.when.isEmpty { value.append(block.when) }
            if block.changedForThisDay { value.append("changed for this day") }
            if block.kind == "work" {
                let n = block.assignments.count
                value.append(n == 0 ? "nothing assigned" : n == 1 ? "1 task assigned" : "\(n) tasks assigned")
            }
        case let .sitting(sitting, _):
            label = sitting.title
            value = sittingStatus(sitting)
            if sitting.minutes > 0 { value.append("\(Clock.length(sitting.minutes)) logged") }
            if sitting.capped { value.append("capped, the timer looks forgotten") }
        case let .free(start, end, minutes):
            label = "Free, \(Clock.length(minutes))"
            value = ["\(Clock.time(start)) to \(Clock.time(end))"]
        case let .now(time):
            label = "Now, \(Clock.time(time))"
            cell.title.font = .preferredFont(forTextStyle: .headline)
        case let .cancelled(block):
            label = "\(Clock.time(block.start)), \(block.title)"
            value = ["cancelled for this day"]
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
        case let .free(start, _, minutes): addBlock(at: start, minutes: minutes)
        case let .cancelled(block): restore(block)
        case .now: break
        }
    }

    /// Return activates a row, Space starts or stops a sitting's timer, Delete takes a sitting
    /// out or deletes a block — each also in the row's menu.
    private func key(_ key: TaskOutline.Key) -> Bool {
        guard let node = selected else { return false }
        switch (key, node.kind) {
        case (.returnKey, _): activate()
        case let (.space, .sitting(sitting, _)): toggleTimer(sitting)
        case let (.delete, .sitting(sitting, _)):
            change(keeping: nil) { try self.core.lumenna.unassign(assignment: sitting.id) }
        case let (.delete, .block(block)): delete(block)
        default: return false
        }
        return true
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = outline.clickedRow >= 0 ? outline.clickedRow : outline.selectedRow
        guard let node = outline.item(atRow: clicked) as? DayNode else { return }
        if outline.selectedRow != clicked { outline.selectRowIndexes([clicked], byExtendingSelection: false) }
        for (title, action) in actions(for: node) {
            if title == "-" { menu.addItem(.separator()) } else { menu.addItem(ClosureMenuItem(title: title, action: action)) }
        }
    }

    private func actions(for node: DayNode) -> [(String, () -> Void)] {
        let lumenna = core.lumenna
        let date = plan?.date ?? ""
        switch node.kind {
        case let .block(block):
            var actions: [(String, () -> Void)] = []
            if block.kind == "work" { actions.append(("Assign a Task…", { [weak self] in self?.assign(to: block) })) }
            actions.append(("Edit…", { [weak self] in self?.edit(block) }))
            if block.repeats {
                actions.append(("Cancel This Day", { [weak self] in
                    self?.change(keeping: node.key) { try lumenna.cancelOccurrence(id: block.series, date: date) }
                }))
            }
            if block.changedForThisDay {
                actions.append(("Restore This Day", { [weak self] in
                    self?.change(keeping: node.key) { try lumenna.restoreOccurrence(id: block.series, date: date) }
                }))
            }
            actions += [("-", {}), ("Delete Block…", { [weak self] in self?.delete(block) })]
            return actions
        case let .sitting(sitting, _):
            return [
                (sitting.status == "in progress" ? "Stop Timer" : "Start Timer", { [weak self] in self?.toggleTimer(sitting) }),
                ("Planned Length…", { [weak self] in self?.planLength(sitting, key: node.key) }),
                ("Log Minutes…", { [weak self] in self?.logMinutes(sitting) }),
                ("-", {}),
                ("Unassign", { [weak self] in self?.change(keeping: nil) { try lumenna.unassign(assignment: sitting.id) } }),
            ]
        case let .free(start, _, minutes):
            return [("Add Block Here…", { [weak self] in self?.addBlock(at: start, minutes: minutes) })]
        case let .cancelled(block):
            return [("Restore This Day", { [weak self] in self?.restore(block) })]
        case .now:
            return []
        }
    }

    // MARK: - Doing things

    private func change(keeping key: String?, _ operation: () throws -> Change) {
        let index = outline.selectedRow
        do {
            let change = try operation()
            reload(keeping: key, near: index, saying: change)
        } catch {
            view.window?.showFailure(error.sentence)
        }
    }

    private func toggleTimer(_ sitting: PlanAssignment) {
        let key = "sitting:\(sitting.id)"
        if sitting.status == "in progress" {
            do {
                let timer = try core.lumenna.stopTimer(assignment: sitting.id, minutes: nil)
                reload(keeping: key, near: nil)
                Announcer.say(timer.announcement, notices: timer.notices)
            } catch {
                view.window?.showFailure(error.sentence)
            }
        } else {
            change(keeping: key) { try core.lumenna.startTimer(assignment: sitting.id) }
        }
    }

    private func planLength(_ sitting: PlanAssignment, key: String) {
        view.window?.askForLength("Planned length of \(sitting.title)", current: sitting.plannedMins, without: "No Planned Length") { [weak self] minutes in
            self?.change(keeping: key) { try self!.core.lumenna.planMinutes(assignment: sitting.id, minutes: minutes) }
        }
    }

    /// Records a sitting's whole time by hand — without a timer, or to replace a capped one.
    private func logMinutes(_ sitting: PlanAssignment) {
        view.window?.askForText(
            "Minutes on \(sitting.title)",
            message: "The whole of this sitting, replacing what is logged.",
            placeholder: "45",
            action: "Log"
        ) { [weak self] text in
            guard let self, let minutes = UInt32(text) else {
                self?.view.window?.showFailure("That is not a number of minutes.")
                return
            }
            do {
                let timer = try self.core.lumenna.stopTimer(assignment: sitting.id, minutes: minutes)
                self.reload(keeping: "sitting:\(sitting.id)", near: nil)
                Announcer.say(timer.announcement, notices: timer.notices)
            } catch {
                self.view.window?.showFailure(error.sentence)
            }
        }
    }

    private func assign(to block: PlanBlock) {
        guard let window = view.window, let date = plan?.date else { return }
        let tasks = ((try? core.lumenna.listTasks(query: "").rows) ?? [])
            .map { PickerItem(key: $0.id, title: $0.title, detail: $0.value) }
        PickerSheet.present(on: window, title: "Assign to \(block.title)", items: tasks) { [weak self] task in
            window.askForLength("How long is \(task.title) meant to take?", without: "Skip") { minutes in
                self?.change(keeping: "block:\(block.id)") {
                    try self!.core.lumenna.assign(task: task.key, block: block.series, date: date, minutes: minutes)
                }
            }
        }
    }

    private func restore(_ block: CancelledBlock) {
        guard let date = plan?.date else { return }
        change(keeping: nil) { try core.lumenna.restoreOccurrence(id: block.series, date: date) }
    }

    /// Asks "this day, or every day?" of a repeating block — never guessed (§4.3).
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
                BlockFormModel(
                    core: self.core, purpose: .occurrence(block.series, day: date), name: block.title,
                    start: block.start, minutes: Int(block.durationMins), kind: block.kind
                ).present(on: window, saved: saved)
            }),
            ("Every Occurrence", { DispatchQueue.main.async(execute: series) }),
        ])
    }

    func addBlock(at start: String? = nil, minutes: UInt32? = nil) {
        guard let window = view.window else { return }
        let day = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        BlockFormModel(
            core: core, purpose: .add, start: start ?? "09:00", minutes: Int(min(minutes ?? 60, 720)), day: day
        ).present(on: window) { [weak self] change in
            self?.reload(keeping: nil, near: nil, saying: change)
        }
    }

    private func delete(_ block: PlanBlock) {
        let message = block.repeats
            ? "Every occurrence goes, not only this day. To skip one day, cancel it instead."
            : "It goes to the trash with its assignments."
        view.window?.confirm("Delete \(block.title)?", message: message, action: "Delete") { [weak self] in
            self?.change(keeping: nil) { try self!.core.lumenna.deleteBlock(id: block.series) }
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
        day = Calendar.current.isDateInToday(date) ? nil : Clock.isoDay(date)
        reload()
        // A new day is a new screen's worth: say it, and start at its top.
        if outline.numberOfRows > 0 { outline.selectRowIndexes([0], byExtendingSelection: false) }
        Announcer.say(summary.stringValue)
    }

    /// §13's "go to now": today, on the now row or the block happening now.
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

    @objc func goToDay(_ sender: Any?) {
        guard let window = view.window else { return }
        let alert = NSAlert()
        alert.messageText = "Go to Day"
        let picker = NSDatePicker()
        picker.datePickerStyle = .clockAndCalendar
        picker.datePickerElements = .yearMonthDay
        picker.dateValue = shownDate
        picker.sizeToFit()
        picker.setAccessibilityLabel("Day")
        alert.accessoryView = picker
        alert.addButton(withTitle: "Go")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = picker
        alert.beginSheetModal(for: window) { [weak self] response in
            if response == .alertFirstButtonReturn { self?.show(picker.dateValue) }
        }
    }
}
