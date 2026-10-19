import AppKit
import LumennaApp

/// One row of the day as an outline item: the app's row and its line, and a block's sittings.
final class DayNode {
    let item: LumennaApp.DayItem
    let line: LumennaApp.Line?
    var children: [DayNode] = []

    init(_ item: LumennaApp.DayItem, line: LumennaApp.Line?) {
        self.item = item
        self.line = line
    }

    var key: String { item.key }
}

/// The planner: a day as it is lived, as an outline — the app's day (`ViewModel.day`).
///
/// Blocks in time order with their sittings beneath them, free time as rows of its own, and
/// now as a position rather than a highlight. The heading above says what a glance at a
/// timeline would. The day shown, moving between days, the row in hand and folding, and every
/// row's actions and their forms are the app's; opening on today puts the selection —
/// VoiceOver's focus — on now.
final class DayViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate, NSMenuDelegate {
    private let core: Core
    private weak var main: MainWindowController?
    private var nodes: [DayNode] = []
    let outline = TaskOutline()
    private let summary = HeadingLabel(wrappingLabelWithString: "")
    /// Whether the outline is being set from the app's, not moved by the person.
    private var following = false
    /// The landing count this day last moved the selection for.
    private var landed: UInt32?
    private var observer: NSObjectProtocol?
    /// The day's own actions (`DayView.actions`), the app's: a pull-down beside the day's
    /// buttons, and the Day menu's.
    private let dayActionsButton: NSPopUpButton = {
        let button = NSPopUpButton(frame: .zero, pullsDown: true)
        button.setAccessibilityLabel("Day Actions")
        return button
    }()

    init(core: Core, window: MainWindowController) {
        self.core = core
        main = window
        super.init(nibName: nil, bundle: nil)
        title = "Today"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

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
            dayActionsButton,
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
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
        // Opening the day is not the app moving focus, though now is in hand: the selection
        // goes there without a landing.
        landed = core.app.view.landing
        draw()
    }

    override var preferredFirstResponder: NSView? { outline }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(outline)
    }

    // MARK: - Rows

    /// Draws the app's day while it is shown: every row, sittings folded away inside their
    /// collapsed block, the row in hand selected.
    private func draw() {
        let shown = core.app.view
        guard shown.place == .today, let day = shown.day else { return }
        let lines = core.app.lines
        let worded = day.items.enumerated().map { index, item in (item, index < lines.day.count ? lines.day[index] : nil) }
        // The day's heading, as every app says it: "<day>. <summary>".
        summary.stringValue = worded.first { $0.0.kind == .summary }?.1?.text ?? lines.dayTitle
        title = lines.dayTitle
        offerDayActions(day)
        following = true
        defer { following = false }
        if day.items != drawn?.items || lines.day != drawn?.lines {
            drawn = (day.items, lines.day)
            var nodes: [DayNode] = []
            for (item, line) in worded where item.kind != .summary {
                let node = DayNode(item, line: line)
                if item.depth > 0, let block = nodes.last {
                    block.children.append(node)
                } else {
                    nodes.append(node)
                }
            }
            self.nodes = nodes
            outline.reloadData()
            for node in nodes where !node.children.isEmpty {
                if node.item.expanded == false { outline.collapseItem(node) } else { outline.expandItem(node) }
            }
        }
        let land = landed.map { shown.landing > $0 } ?? false
        landed = shown.landing
        guard let focus = shown.focus, let row = row(of: focus) else { return }
        if outline.selectedRow != row { outline.selectRowIndexes([row], byExtendingSelection: false) }
        if land {
            outline.scrollRowToVisible(row)
            if let window = view.window, window.firstResponder == nil || window.firstResponder === window {
                window.makeFirstResponder(outline)
            }
        }
    }

    /// The rows and lines last drawn, so a redraw that changes nothing leaves the outline alone.
    private var drawn: (items: [LumennaApp.DayItem], lines: [LumennaApp.Line])?

    private func row(of key: String) -> Int? {
        (0..<outline.numberOfRows).first { (outline.item(atRow: $0) as? DayNode)?.key == key }
    }

    var selected: DayNode? { outline.item(atRow: outline.selectedRow) as? DayNode }

    /// The task a selected sitting is for: what the Task menu acts on from the day.
    var selectedTaskID: String? { selected?.item.sitting?.task }

    func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        (item as? DayNode)?.children.count ?? nodes.count
    }

    func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        (item as? DayNode)?.children[index] ?? nodes[index]
    }

    func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
        !((item as? DayNode)?.children.isEmpty ?? true)
    }

    /// Folded or unfolded by the person: the block's own Collapse or Expand, the app's.
    func outlineViewItemDidCollapse(_ notification: Notification) { folded(notification, .collapse) }
    func outlineViewItemDidExpand(_ notification: Notification) { folded(notification, .expand) }

    private func folded(_ notification: Notification, _ kind: LumennaApp.ActionKind) {
        guard !following, let node = notification.userInfo?["NSObject"] as? DayNode,
              let action = node.item.actions.first(where: { $0.kind == kind }) else { return }
        perform(action, on: node)
    }

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? DayNode else { return nil }
        let cell = TwoLineCell()
        // The line is the core's, in every app's order and this Mac's clock.
        let title = node.line?.title ?? ""
        let detail = node.line?.detail ?? ""
        cell.show(title: title, detail: detail)
        if node.item.kind == .now { cell.title.font = .preferredFont(forTextStyle: .headline) }
        cell.setAccessibilityLabel(title)
        cell.setAccessibilityValueDescription(detail)
        return cell
    }

    /// The person moved to a row: the app's row in hand, a sitting's task shown beside it.
    func outlineViewSelectionDidChange(_ notification: Notification) {
        guard !following, let key = selected?.key else { return }
        core.app.send(.focus(key))
    }

    // MARK: - Keys and activation

    /// Return, or a double click: a block's Edit, a sitting's task's details, free time's Add
    /// Block, a cancelled day's actions.
    @objc private func activate() {
        guard let node = selected else { return }
        switch node.item.kind {
        case .block: run([.edit], on: node)
        case .sitting: main?.showDetails()
        case .free: run([.addBlock], on: node)
        case .cancelled:
            view.window?.choose(node.line?.text ?? "", actions: node.item.actions.map { action in
                (action.title, { [weak self] in self?.perform(action, on: node) })
            })
        case .now, .summary: break
        }
    }

    /// Return activates a row; Space and Delete are the row's own actions of those kinds:
    /// Space starts, pauses or resumes a sitting's timer, Delete takes a sitting out or
    /// deletes a block — each also in the row's menu.
    private func key(_ key: TaskOutline.Key) -> Bool {
        guard let node = selected else { return false }
        let kinds: [LumennaApp.ActionKind] = switch key {
        case .returnKey: []
        case .space: [.startTimer, .resumeTimer, .pauseTimer]
        case .delete: [.delete, .unassign]
        }
        if key == .returnKey {
            activate()
        } else if !run(kinds, on: node) {
            // Why not, in the core's words: silence would leave the key a guess.
            let subject: LumennaApp.Subject = node.item.kind == .sitting ? .sitting : .block
            Announcer.say(LumennaApp.notOffered(kind: key == .space ? .startTimer : .delete, subject: subject, thisDevice: false))
        }
        return true
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = outline.clickedRow >= 0 ? outline.clickedRow : outline.selectedRow
        guard let node = outline.item(atRow: clicked) as? DayNode else { return }
        if outline.selectedRow != clicked { outline.selectRowIndexes([clicked], byExtendingSelection: false) }
        menu.add(node.item.actions) { [weak self] action in self?.perform(action, on: node) }
    }

    // MARK: - Doing things

    /// Runs a row's action, the app's: the row in hand, then the action, whose question and
    /// form the window asks and opens.
    private func perform(_ action: LumennaApp.Action, on node: DayNode) {
        core.app.run(action, on: node.key)
    }

    /// The row's action of one of these kinds, if it has one.
    @discardableResult
    private func run(_ kinds: [LumennaApp.ActionKind], on node: DayNode) -> Bool {
        guard let action = node.item.actions.first(where: { kinds.contains($0.kind) }) else { return false }
        perform(action, on: node)
        return true
    }

    // MARK: - The day's own actions

    /// Replan from Now, as the overrun warning's Replan opens it: today, the sheet over it.
    func openReplan() {
        core.app.send(.day(.now))
        core.app.send(.proposal(.replan))
    }

    private func offerDayActions(_ day: LumennaApp.DayView) {
        let menu = NSMenu()
        // A pull-down shows its first item as its title.
        menu.addItem(NSMenuItem(title: "Day Actions", action: nil, keyEquivalent: ""))
        menu.add(day.actions) { [weak self] action in self?.core.app.send(.perform(action)) }
        dayActionsButton.menu = menu
        dayActionsButton.isHidden = day.actions.isEmpty
    }

    /// The day's actions as the pull-down offers them, for tests.
    var dayActionTitles: [String] { dayActionsButton.itemArray.dropFirst().map(\.title) }

    // MARK: - Moving through days

    @objc func previousDay(_ sender: Any?) { core.app.send(.day(.previous)) }
    @objc func nextDay(_ sender: Any?) { core.app.send(.day(.next)) }
    /// Today, on now: the app puts now in hand and says the day.
    @objc func goToToday(_ sender: Any?) { core.app.send(.day(.now)) }
    @objc func newBlock(_ sender: Any?) { core.app.send(.block(.new(start: nil, minutes: nil))) }

    /// A reminder's block, chosen in Notification Center: its day, with the block in hand.
    func show(day iso: String, series: String) {
        guard goTo(iso) == nil else { return }
        if let block = core.app.current.view.day?.items.first(where: { $0.kind == .block && $0.block?.series == series }) {
            core.app.send(.focus(block.key))
            view.window?.makeFirstResponder(outline)
        }
    }

    /// Goes to the day named, as Go to Day's answer: nil if the app read it, else why not, the
    /// question put away again.
    private func goTo(_ named: String) -> String? {
        core.app.send(.day(.ask))
        core.app.send(.answer(.text(text: named)))
        guard let refusal = core.app.current.view.asking?.refusal else { return nil }
        core.app.send(.cancel)
        return refusal
    }

    @objc func goToDay(_ sender: Any?) { askDay(typed: "", problem: nil) }

    /// The app's Go to Day, answered by typing a day ("next friday", "12 October") or with
    /// the system's calendar. A day the app cannot read asks again, saying why, with what was
    /// typed still there.
    private func askDay(typed: String, problem: String?) {
        guard let window = view.window,
              case let .text(title, label, _, hint, _, yes, _) = LumennaApp.goToDayQuestion() else { return }
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = [problem, hint].compactMap { $0 }.joined(separator: "\n\n")
        let field = NSTextField(string: typed)
        field.placeholderString = label
        field.setAccessibilityLabel(label)
        field.setAccessibilityHelp(hint)
        let picker = NSDatePicker()
        picker.datePickerStyle = .clockAndCalendar
        picker.datePickerElements = .yearMonthDay
        picker.dateValue = core.app.view.day.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        picker.sizeToFit()
        picker.setAccessibilityLabel(label)
        let stack = NSStackView(views: [field, picker])
        stack.orientation = .vertical
        stack.alignment = .leading
        field.widthAnchor.constraint(equalToConstant: max(picker.frame.width, 240)).isActive = true
        stack.frame = NSRect(x: 0, y: 0, width: max(picker.frame.width, 240), height: picker.frame.height + 32)
        alert.accessoryView = stack
        alert.addButton(withTitle: yes)
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] response in
            guard let self, response == .alertFirstButtonReturn else { return }
            let text = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            let named = text.isEmpty ? Clock.isoDay(picker.dateValue) : text
            if let refusal = self.goTo(named) {
                DispatchQueue.main.async { self.askDay(typed: text, problem: refusal) }
            }
        }
    }
}
