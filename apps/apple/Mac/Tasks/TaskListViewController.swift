import AppKit

/// A task row as the outline holds it: the core's row, and the rows beneath it.
final class TaskNode {
    let row: RowView
    var children: [TaskNode] = []

    init(_ row: RowView) { self.row = row }

    /// The flat, depth-first rows the core sends, as a tree: a row's parent is the nearest
    /// shallower row above it. One whose parent was filtered out sits at the top.
    static func tree(_ rows: [RowView]) -> [TaskNode] {
        var top: [TaskNode] = []
        var chain: [TaskNode] = []
        for row in rows {
            let node = TaskNode(row)
            while let last = chain.last, last.row.depth >= row.depth { chain.removeLast() }
            if let parent = chain.last { parent.children.append(node) } else { top.append(node) }
            chain.append(node)
        }
        return top
    }
}

/// Tasks, with the filter above them, as an outline.
///
/// `NSOutlineView` is the reason this app is AppKit: Finder's and Mail's outline,
/// whose level, position and expansion VoiceOver reports without help. After a change, the
/// selection — which is VoiceOver's focus here — is put on a row chosen deterministically:
/// the same task if it is still listed, else whatever now holds its place.
final class TaskListViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate, NSMenuDelegate {
    enum Mode {
        case tasks
        /// The trash: restore, or delete from the trash.
        case trash
    }

    let core: Core
    private weak var main: MainWindowController?
    let mode: Mode
    private let quickAddPrefix: String
    private let initialQuery: String
    private var nodes: [TaskNode] = []
    private var rows: [RowView] = []
    private var collapsed: Set<String> = []
    let outline = TaskOutline()
    private lazy var filter = CompletingField(core: core, syntax: .filter, name: "Filter")
    private let readback = NSTextField(wrappingLabelWithString: "")

    init(core: Core, window: MainWindowController, title: String, query: String = "", mode: Mode = .tasks, quickAddPrefix: String = "") {
        self.core = core
        main = window
        self.mode = mode
        self.quickAddPrefix = quickAddPrefix
        initialQuery = query
        super.init(nibName: nil, bundle: nil)
        self.title = title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        filter.stringValue = initialQuery
        filter.placeholderString = "#Work & overdue, or search: words"
        filter.changed = { [weak self] in self?.reload() }
        filter.submitted = { [weak self] in
            guard let self else { return }
            // Typing is silent; finishing the filter says what it found, then goes to it.
            Announcer.say(self.readback.stringValue)
            self.view.window?.makeFirstResponder(self.outline)
        }
        filter.isHidden = mode == .trash
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
        NotificationCenter.default.addObserver(self, selector: #selector(storeChanged), name: Core.changed, object: nil)
        reload()
    }

    override var preferredFirstResponder: NSView? { outline }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(outline)
        if outline.selectedRow < 0, outline.numberOfRows > 0 {
            outline.selectRowIndexes([0], byExtendingSelection: false)
        }
    }

    // MARK: - Showing the store

    @objc private func storeChanged() {
        reload(focusing: selectedID, near: outline.selectedRow, saying: nil)
    }

    /// Lists again. A filter still being typed may not read yet; then the old rows stay and
    /// the readback says what is wrong with it.
    func reload() {
        do {
            let listing = try core.lumenna.listTasks(query: filter.stringValue)
            rows = listing.rows
            nodes = TaskNode.tree(listing.rows)
            var said = [listing.announcement] + listing.notices
            if let understood = listing.query?.description, mode == .tasks {
                said.insert(understood, at: 0)
            }
            readback.stringValue = said.joined(separator: ". ")
        } catch {
            readback.stringValue = error.sentence
        }
        outline.reloadData()
        expand(nodes)
    }

    private func expand(_ nodes: [TaskNode]) {
        for node in nodes where !node.children.isEmpty {
            if !collapsed.contains(node.row.id) { outline.expandItem(node) }
            expand(node.children)
        }
    }

    /// Reloads, puts the selection — VoiceOver's focus — somewhere predictable, and says what
    /// happened.
    func reload(focusing id: String?, near index: Int?, saying change: Change?) {
        reload()
        let target = id.flatMap { id in row(of: id) } ?? index.map { min($0, outline.numberOfRows - 1) }
        if let target, target >= 0 {
            outline.selectRowIndexes([target], byExtendingSelection: false)
            outline.scrollRowToVisible(target)
        } else {
            main?.showTask(nil)
        }
        if let change { Announcer.say(change.announcement, notices: change.notices) }
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

    func outlineViewItemDidCollapse(_ notification: Notification) {
        if let node = notification.userInfo?["NSObject"] as? TaskNode { collapsed.insert(node.row.id) }
    }

    func outlineViewItemDidExpand(_ notification: Notification) {
        if let node = notification.userInfo?["NSObject"] as? TaskNode { collapsed.remove(node.row.id) }
    }

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? TaskNode else { return nil }
        let cell = TaskCell()
        cell.show(node.row, trash: mode == .trash) { [weak self] in self?.toggleDone(node.row) }
        return cell
    }

    func outlineViewSelectionDidChange(_ notification: Notification) {
        guard mode == .tasks else { return }
        main?.showTask(selectedID)
    }

    @objc private func openDetail() {
        guard mode == .tasks, selectedID != nil else { return }
        main?.nextPane(nil)
    }

    /// Keys on the outline itself: Space completes, Delete trashes — or, in the trash,
    /// restores and deletes from the trash — as the menus say.
    private func key(_ key: TaskOutline.Key) -> Bool {
        guard let row = selectedNode?.row else { return false }
        switch (key, mode) {
        case (.space, .tasks): toggleDone(row)
        case (.delete, .tasks): trash(row)
        case (.space, .trash): restore(row)
        case (.delete, .trash): erase(row)
        case (.returnKey, _): openDetail()
        }
        return true
    }

    // MARK: - Doing things

    func perform(focusing id: String?, _ operation: () throws -> Change) {
        let index = outline.selectedRow
        do {
            let change = try operation()
            reload(focusing: id, near: index, saying: change)
        } catch {
            view.window?.showFailure(error.sentence)
        }
    }

    func toggleDone(_ row: RowView) {
        perform(focusing: row.id) {
            row.checked == true
                ? try core.lumenna.uncompleteTask(id: row.id)
                : try core.lumenna.completeTask(id: row.id)
        }
    }

    func trash(_ row: RowView) {
        perform(focusing: nil) { try core.lumenna.trashTask(id: row.id) }
    }

    func restore(_ row: RowView) {
        perform(focusing: nil) { try core.lumenna.restoreTask(id: row.id) }
    }

    /// Deletes a task from the trash, asking first.
    func erase(_ row: RowView) {
        view.window?.confirm(
            "Delete \(row.title) from the trash?",
            message: "Undo can bring it back. It also stays in the history every device keeps, and in backups.",
            action: "Delete"
        ) { [weak self] in
            guard let self else { return }
            self.perform(focusing: nil) { try self.core.lumenna.eraseTask(id: row.id) }
        }
    }

    func addTask() {
        guard mode == .tasks, let window = view.window else { return }
        QuickAddSheet.present(on: window, core: core, initial: quickAddPrefix) { [weak self] change in
            self?.reload(focusing: change.task?.id, near: nil, saying: change)
        }
    }

    func focusFilter() {
        guard mode == .tasks else { return }
        view.window?.makeFirstResponder(filter)
    }

    // MARK: - Context menu

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = outline.clickedRow >= 0 ? outline.clickedRow : outline.selectedRow
        guard let node = outline.item(atRow: clicked) as? TaskNode, let window = view.window else { return }
        if outline.selectedRow != clicked {
            outline.selectRowIndexes([clicked], byExtendingSelection: false)
        }
        let actions = mode == .trash
            ? [("Restore", { [weak self] in self?.restore(node.row) }),
               ("Delete from Trash…", { [weak self] in self?.erase(node.row) })]
            : TaskActions(core: core, window: window, list: self).menu(for: node.row)
        for (title, action) in actions {
            if title == "-" { menu.addItem(.separator()) } else { menu.addItem(ClosureMenuItem(title: title, action: action)) }
        }
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

    func show(_ row: RowView, trash: Bool, toggled: @escaping () -> Void) {
        self.toggled = toggled
        let done = row.checked == true
        check.state = done ? .on : .off
        check.isHidden = trash
        check.setAccessibilityLabel(done ? "Done, \(row.title)" : "Mark \(row.title) done")
        var detail = row.value.map { [$0] } ?? []
        detail += row.state.filter { $0 != "ready" }
        lines.show(title: row.title, detail: detail.joined(separator: ", "), warning: row.state.contains("overdue"))
        lines.title.attributedStringValue = NSAttributedString(
            string: row.title,
            attributes: done ? [.strikethroughStyle: NSUnderlineStyle.single.rawValue] : [:]
        )
        // Said as the phone says it: the title, then done, the date and the states. The level
        // is the outline's own to report.
        setAccessibilityLabel(RowSpeech.label(row))
        setAccessibilityValueDescription(RowSpeech.value(row, previousDepth: row.depth))
    }

    @objc private func toggle() {
        toggled?()
    }
}
