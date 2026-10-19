import AppKit
import LumennaApp

/// A task row as the outline holds it: the app's row and its line, and the rows beneath it.
final class TaskNode {
    let item: LumennaApp.Item
    let line: LumennaApp.Line?
    var children: [TaskNode] = []

    init(_ item: LumennaApp.Item, line: LumennaApp.Line?) {
        self.item = item
        self.line = line
    }

    var row: LumennaApp.RowView { item.row }

    /// The flat, depth-first rows the app sends, as a tree: a row's parent is the nearest
    /// shallower row above it. One whose parent was filtered out sits at the top.
    static func tree(_ rows: [(LumennaApp.Item, LumennaApp.Line?)]) -> [TaskNode] {
        var top: [TaskNode] = []
        var chain: [TaskNode] = []
        for (item, line) in rows {
            let node = TaskNode(item, line: line)
            while let last = chain.last, last.row.depth >= item.row.depth { chain.removeLast() }
            if let parent = chain.last { parent.children.append(node) } else { top.append(node) }
            chain.append(node)
        }
        return top
    }
}

/// A place's tasks, with the filter above them, or the trash, as an outline: the app's list
/// (`ViewModel.list`).
///
/// `NSOutlineView` is the reason this app is AppKit: Finder's and Mail's outline,
/// whose level, position and expansion VoiceOver reports without help. What is listed, what is
/// folded, the row in hand after a change — the same task if it is still listed, else whatever
/// now holds its place — and what each row offers are the app's. The selection is the row in
/// hand, which is VoiceOver's focus here; it is scrolled to only when the app moved it
/// (`ViewModel.landing`), so a sync or a new minute redraws without moving anything.
final class TaskListViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate, NSMenuDelegate {
    let core: Core
    private weak var main: MainWindowController?
    /// The place this list shows.
    let place: LumennaApp.Place
    var trash: Bool { place == .trash }
    private var nodes: [TaskNode] = []
    let outline = TaskOutline()
    private lazy var filter = CompletingField(core: core, syntax: .filter, name: "Filter")
    private let readback = NSTextField(wrappingLabelWithString: "")
    /// Whether the outline is being set from the app's, not moved by the person.
    private var following = false
    /// The landing count this list last moved the selection for.
    private var landed: UInt32?
    private var observer: NSObjectProtocol?

    init(core: Core, window: MainWindowController, place: LumennaApp.Place) {
        self.core = core
        main = window
        self.place = place
        super.init(nibName: nil, bundle: nil)
        title = core.app.view.title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    override func loadView() {
        filter.stringValue = core.app.view.query
        filter.placeholderString = "#Work & overdue, or search: words"
        filter.changed = { [weak self] in
            guard let self else { return }
            self.core.app.send(.filter(self.filter.stringValue))
        }
        filter.submitted = { [weak self] in
            guard let self else { return }
            // Typing is silent; finishing the filter says what it found, then goes to it.
            Announcer.say(self.readback.stringValue)
            self.view.window?.makeFirstResponder(self.outline)
        }
        filter.isHidden = trash
        readback.textColor = .quietLabel
        readback.font = .preferredFont(forTextStyle: .subheadline)

        let column = NSTableColumn(identifier: .init("task"))
        column.title = title ?? "Tasks"
        outline.addTableColumn(column)
        outline.outlineTableColumn = column
        outline.headerView = nil
        outline.dataSource = self
        outline.delegate = self
        outline.usesAutomaticRowHeights = true
        outline.style = .inset
        outline.setAccessibilityLabel(title ?? "Tasks")
        outline.target = self
        outline.doubleAction = #selector(openDetail)
        outline.keys = { [weak self] key in self?.key(key) ?? false }
        let menu = NSMenu()
        menu.delegate = self
        outline.menu = menu
        let scroll = NSScrollView()
        scroll.documentView = outline
        scroll.hasVerticalScroller = true

        let header = NSStackView(views: [filter, readback])
        header.orientation = .vertical
        header.alignment = .leading
        header.spacing = 6
        header.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 6, right: 12)
        for view in [filter, readback] {
            view.widthAnchor.constraint(equalTo: header.widthAnchor, constant: -24).isActive = true
        }
        let stack = NSStackView(views: [header, scroll])
        stack.orientation = .vertical
        stack.spacing = 0
        header.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        scroll.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        view = stack
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
        // Opening the place is not the app moving focus: VoiceOver says the list itself.
        landed = core.app.view.landing
        draw()
    }

    override var preferredFirstResponder: NSView? { outline }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(outline)
    }

    // MARK: - Showing the app's list

    /// Draws the app's list while the app shows this place: every row, those folded away
    /// inside their collapsed parents, the row in hand selected.
    private func draw() {
        let shown = core.app.view
        guard shown.place == place, let list = shown.list else { return }
        let lines = core.app.lines
        readback.stringValue = lines.readback
        let worded = list.items.enumerated().map { index, item in (item, index < lines.list.count ? lines.list[index] : nil) }
        following = true
        defer { following = false }
        if list.items != drawn?.items || lines.list != drawn?.lines {
            drawn = (list.items, lines.list)
            nodes = TaskNode.tree(worded)
            outline.reloadData()
            fold(nodes)
        }
        let land = landed.map { shown.landing > $0 } ?? false
        landed = shown.landing
        guard let focus = shown.focus, let row = row(of: focus) else { return }
        if outline.selectedRow != row { outline.selectRowIndexes([row], byExtendingSelection: false) }
        if land {
            outline.scrollRowToVisible(row)
            // A change made from a sheet comes back to the list, as the app moved the row in
            // hand; focus elsewhere in the window stays where it is.
            if let window = view.window, window.firstResponder == nil || window.firstResponder === window {
                window.makeFirstResponder(outline)
            }
        }
    }

    /// The rows and lines last drawn, so a redraw that changes nothing leaves the outline alone.
    private var drawn: (items: [LumennaApp.Item], lines: [LumennaApp.Line])?

    private func fold(_ nodes: [TaskNode]) {
        for node in nodes where !node.children.isEmpty {
            if node.row.expanded == false {
                outline.collapseItem(node)
            } else {
                outline.expandItem(node)
                fold(node.children)
            }
        }
    }

    private func row(of id: String) -> Int? {
        (0..<outline.numberOfRows).first { (outline.item(atRow: $0) as? TaskNode)?.row.id == id }
    }

    var selectedNode: TaskNode? { outline.item(atRow: outline.selectedRow) as? TaskNode }
    var selectedID: String? { selectedNode?.row.id }

    // MARK: - Outline

    func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        (item as? TaskNode)?.children.count ?? nodes.count
    }

    func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        (item as? TaskNode)?.children[index] ?? nodes[index]
    }

    func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
        !((item as? TaskNode)?.children.isEmpty ?? true)
    }

    /// Folded or unfolded by the person: the row's own Collapse or Expand, the app's.
    func outlineViewItemDidCollapse(_ notification: Notification) { folded(notification, .collapse) }
    func outlineViewItemDidExpand(_ notification: Notification) { folded(notification, .expand) }

    private func folded(_ notification: Notification, _ kind: LumennaApp.ActionKind) {
        guard !following, let node = notification.userInfo?["NSObject"] as? TaskNode,
              let action = node.row.actions.first(where: { $0.kind == kind }) else { return }
        core.app.run(action, on: node.row.id)
    }

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? TaskNode else { return nil }
        let cell = TaskCell()
        cell.show(node.row, line: node.line, trash: trash) { [weak self] in
            if let done = node.row.actions.first(where: { [.markDone, .markNotDone].contains($0.kind) }) {
                self?.perform(done, on: node.row)
            }
        }
        return cell
    }

    /// The person moved to a row: the app's row in hand, whose details show beside it.
    func outlineViewSelectionDidChange(_ notification: Notification) {
        guard !following, let id = selectedID else { return }
        core.app.send(.focus(id))
    }

    @objc private func openDetail() {
        guard !trash, selectedID != nil else { return }
        main?.showDetails()
    }

    /// Keys on the outline itself, each the row's own action of that kind: Space marks it
    /// done or not, or restores it from the trash; Delete trashes it, or deletes it from the
    /// trash.
    private func key(_ key: TaskOutline.Key) -> Bool {
        guard let row = selectedNode?.row else { return false }
        let kinds: [LumennaApp.ActionKind] = switch key {
        case .space: [.markDone, .markNotDone, .restore]
        case .delete: [.delete, .deleteForGood]
        case .returnKey: []
        }
        if key == .returnKey {
            openDetail()
        } else if let action = row.actions.first(where: { kinds.contains($0.kind) }) {
            perform(action, on: row)
        } else {
            // Why not, in the core's words: silence would leave the key a guess.
            Announcer.say(LumennaApp.notOffered(kind: key == .space ? .markDone : .delete, subject: .task, thisDevice: false))
        }
        return true
    }

    // MARK: - Doing things

    /// Runs a row's action, the app's: the row in hand, then the action, whose question the
    /// window's `AppQuestions` asks.
    func perform(_ action: LumennaApp.Action, on row: LumennaApp.RowView) {
        core.app.run(action, on: row.id)
    }

    func addTask() {
        guard !trash, let window = view.window else { return }
        QuickAddSheet.present(on: window, core: core)
    }

    func focusFilter() {
        guard !trash else { return }
        view.window?.makeFirstResponder(filter)
    }

    // MARK: - Context menu

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = outline.clickedRow >= 0 ? outline.clickedRow : outline.selectedRow
        guard let node = outline.item(atRow: clicked) as? TaskNode else { return }
        if outline.selectedRow != clicked {
            outline.selectRowIndexes([clicked], byExtendingSelection: false)
        }
        menu.add(node.row.actions) { [weak self] action in self?.perform(action, on: node.row) }
    }
}

/// The task outline, which answers Space, Delete and Return itself.
final class TaskOutline: NSOutlineView {
    enum Key { case space, delete, returnKey }
    var keys: ((Key) -> Bool)?

    override func keyDown(with event: NSEvent) {
        let key: Key? = switch event.keyCode {
        case 49: .space
        case 51, 117: .delete
        case 36, 76: .returnKey
        default: nil
        }
        if let key, event.modifierFlags.intersection([.command, .option, .control]).isEmpty, keys?(key) == true {
            return
        }
        super.keyDown(with: event)
    }
}

/// One task: a checkbox, the title, and the due date and notable states beneath it.
final class TaskCell: NSTableCellView {
    private let check = NSButton(checkboxWithTitle: "", target: nil, action: nil)
    private let lines = TwoLineCell()
    private var toggled: (() -> Void)?

    override init(frame: NSRect) {
        super.init(frame: frame)
        check.target = self
        check.action = #selector(toggle)
        let stack = NSStackView(views: [check, lines])
        stack.alignment = .top
        stack.spacing = 6
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        textField = lines.title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    func show(_ row: LumennaApp.RowView, line: LumennaApp.Line?, trash: Bool, toggled: @escaping () -> Void) {
        self.toggled = toggled
        let done = row.checked == true
        check.state = done ? .on : .off
        check.isHidden = trash
        check.setAccessibilityLabel(done ? "Done, \(row.title)" : "Mark \(row.title) done")
        // The rest of the app's line beneath the title: when, and the notable states.
        let detail = line?.detail ?? ""
        lines.show(title: row.title, detail: detail, warning: row.state.contains("overdue"))
        lines.title.attributedStringValue = NSAttributedString(
            string: row.title,
            attributes: done ? [.strikethroughStyle: NSUnderlineStyle.single.rawValue] : [:]
        )
        // Said as the phone says it: the title, then the rest of its line. The level and
        // whether it is expanded are the outline's own to report.
        setAccessibilityLabel(row.title)
        setAccessibilityValueDescription(detail)
    }

    @objc private func toggle() {
        toggled?()
    }
}
