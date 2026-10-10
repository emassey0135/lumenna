import AppKit

// Somewhere to go in the main window is the core's `Place`, which every sidebar shares.

/// One row of the sidebar: one of the core's entries (`Lumenna.places`).
final class SidebarNode {
    let entry: SidebarEntry
    let title: String
    let detail: String?
    var children: [SidebarNode] = []

    init(_ entry: SidebarEntry) {
        self.entry = entry
        let title: String = switch entry.kind {
        case let .group(group): switch group {
            case .projects: "Projects"
            case .labels: "Labels"
            case .filters: "Saved Filters"
            }
        case let .place(place): placeTitle(place: place)
        }
        self.title = title
        // The core's line is the title, then what is in it.
        detail = entry.text.hasPrefix(title + ", ") ? String(entry.text.dropFirst(title.count + 2)) : nil
    }

    var place: Place? {
        if case let .place(place) = entry.kind { return place }
        return nil
    }

    var isGroup: Bool {
        if case .group = entry.kind { return true }
        return false
    }
}

/// The places: Today, Tasks, the project tree, labels, saved filters, blocks, the trash.
///
/// A source list, as Mail's mailboxes are, because that is the outline VoiceOver and the
/// keyboard know best on a Mac. The places are the core's (`Lumenna.places`), as every
/// sidebar has them; projects nest as they do in the store. What a project, label or filter
/// can have done to it is its actions, the core's, in its context menu, which VoiceOver
/// opens with VO-Shift-M; a heading's is its New.
final class SidebarViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate, NSMenuDelegate {
    private let core: Core
    private let chose: (Place) -> Void
    let outline = NSOutlineView()
    private var roots: [SidebarNode] = []
    private var selected: Place = .today

    init(core: Core, chose: @escaping (Place) -> Void) {
        self.core = core
        self.chose = chose
        super.init(nibName: nil, bundle: nil)
        title = "Places"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        let column = NSTableColumn(identifier: .init("place"))
        outline.addTableColumn(column)
        outline.outlineTableColumn = column
        outline.headerView = nil
        outline.style = .sourceList
        outline.dataSource = self
        outline.delegate = self
        outline.usesAutomaticRowHeights = true
        outline.floatsGroupRows = false
        outline.setAccessibilityLabel("Places")
        let menu = NSMenu()
        menu.delegate = self
        outline.menu = menu
        let scroll = NSScrollView()
        scroll.documentView = outline
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        view = scroll
        NotificationCenter.default.addObserver(self, selector: #selector(reload), name: Core.changed, object: nil)
        reload()
    }

    override var preferredFirstResponder: NSView? { outline }

    // MARK: - Building

    @objc func reload() {
        roots = Self.tree(core.lumenna.places().entries.map(SidebarNode.init))
        outline.reloadData()
        expandAll(roots)
        reselect()
    }

    /// The flat, depth-first entries the core sends, as a tree: an entry's parent is the
    /// nearest shallower one above it.
    private static func tree(_ nodes: [SidebarNode]) -> [SidebarNode] {
        var top: [SidebarNode] = []
        var chain: [SidebarNode] = []
        for node in nodes {
            while let last = chain.last, last.entry.depth >= node.entry.depth { chain.removeLast() }
            if let parent = chain.last { parent.children.append(node) } else { top.append(node) }
            chain.append(node)
        }
        return top
    }

    private func expandAll(_ nodes: [SidebarNode]) {
        for node in nodes where !node.children.isEmpty {
            outline.expandItem(node)
            expandAll(node.children)
        }
    }

    private func node(for place: Place, in nodes: [SidebarNode]? = nil) -> SidebarNode? {
        for node in nodes ?? roots {
            if node.place == place { return node }
            if let found = self.node(for: place, in: node.children) { return found }
        }
        return nil
    }

    /// Selects a place, and shows it.
    func select(_ place: Place) {
        selected = place
        reselect()
        chose(place)
    }

    /// Keeps the selection on the same place across a reload, without showing it again —
    /// which would throw away whatever the middle pane was doing.
    private func reselect() {
        guard let node = node(for: selected) else { return }
        let row = outline.row(forItem: node)
        if row >= 0, outline.selectedRow != row {
            ignoringSelection = true
            outline.selectRowIndexes([row], byExtendingSelection: false)
            ignoringSelection = false
        }
    }

    private var ignoringSelection = false

    // MARK: - Outline

    func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        (item as? SidebarNode)?.children.count ?? roots.count
    }

    func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        (item as? SidebarNode)?.children[index] ?? roots[index]
    }

    func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
        !((item as? SidebarNode)?.children.isEmpty ?? true)
    }

    func outlineView(_ outlineView: NSOutlineView, shouldSelectItem item: Any) -> Bool {
        !((item as? SidebarNode)?.isGroup ?? true)
    }

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? SidebarNode else { return nil }
        let cell = NSTableCellView()
        let title = NSTextField(labelWithString: node.title)
        title.lineBreakMode = .byTruncatingTail
        if node.isGroup {
            // Ordinary rows, not source-list group rows, which macOS draws dimmed whatever
            // colour is asked for — under contrast on the sidebar's material. The full text
            // colour, and weight to set the heading apart.
            title.font = .systemFont(ofSize: NSFont.preferredFont(forTextStyle: .subheadline).pointSize, weight: .semibold)
            title.textColor = .labelColor
        }
        title.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(title)
        cell.textField = title
        NSLayoutConstraint.activate([
            title.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 2),
            title.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -2),
            title.topAnchor.constraint(equalTo: cell.topAnchor, constant: 3),
            title.bottomAnchor.constraint(equalTo: cell.bottomAnchor, constant: -3),
        ])
        // The counts and states are for VoiceOver and the tooltip; a source list is too
        // narrow for a second line.
        if let detail = node.detail, !detail.isEmpty {
            cell.setAccessibilityValueDescription(detail)
            cell.toolTip = detail
        }
        return cell
    }

    func outlineViewSelectionDidChange(_ notification: Notification) {
        guard !ignoringSelection, let node = outline.item(atRow: outline.selectedRow) as? SidebarNode,
              let place = node.place, place != selected else { return }
        selected = place
        chose(place)
    }

    // MARK: - Context menus

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let row = outline.clickedRow >= 0 ? outline.clickedRow : outline.selectedRow
        guard let node = outline.item(atRow: row) as? SidebarNode else { return }
        menu.add(node.entry.actions) { [weak self] action in self?.perform(action) }
    }

    private var window: NSWindow? { view.window }

    /// Runs one of a place's actions, the core's. When it renamed the place shown, or made a
    /// new one, it goes there; when it removed the place shown, to Tasks.
    private func perform(_ action: Action) {
        guard let window else { return }
        let shown = selected
        window.run(action, core: core, form: { [weak self] _ in self?.newFilter() }) { [weak self] change, answer in
            guard let self else { return }
            self.reload()
            if change.changed {
                let wasShown = placeTitle(place: shown) == action.target
                if let name = ActionRun.name(after: action, answer: answer), let place = self.place(action.subject, named: name) {
                    if action.kind != .mergeInto || wasShown { self.select(place) }
                } else if action.kind == .delete, wasShown {
                    self.select(.tasks)
                } else if wasShown, let place = self.place(action.subject, named: action.target), place != shown {
                    // Changed in place, as a saved filter's query is: shown again as it now is.
                    self.select(place)
                }
            }
            Announcer.say(change.announcement, notices: change.notices)
        }
    }

    /// The place of `subject`'s kind called `name`, once the sidebar holds it again.
    private func place(_ subject: Subject, named name: String, in nodes: [SidebarNode]? = nil) -> Place? {
        for node in nodes ?? roots {
            switch (subject, node.place) {
            case (.project, .project(name)?), (.label, .label(name)?): return node.place
            case let (.filter, .filter(filter, _)?) where filter == name: return node.place
            default: if let found = place(subject, named: name, in: node.children) { return found }
            }
        }
        return nil
    }

    /// The heading's New, from the File menu.
    private func runNew(under group: SidebarGroup) {
        guard let heading = roots.first(where: { $0.entry.kind == .group(group) }),
              let action = heading.entry.actions.first(.new) else { return }
        perform(action)
    }

    @objc func newProject() { runNew(under: .projects) }
    @objc func newLabel() { runNew(under: .labels) }
    @objc func newSavedFilter() { runNew(under: .filters) }

    /// New Saved Filter, the app's own form: a name, then a query.
    private func newFilter() {
        window?.askForText("New Saved Filter", placeholder: "Name", action: "Next") { [weak self] name in
            // A sheet cannot open while the last is still closing.
            DispatchQueue.main.async {
                self?.window?.askForText("Query for \(name)", placeholder: "#Work & overdue", action: "Save") { query in
                    guard let self else { return }
                    do {
                        let change = try self.core.lumenna.addFilter(name: name, query: query)
                        self.reload()
                        self.select(.filter(name: name, query: query))
                        Announcer.say(change.announcement, notices: change.notices)
                    } catch {
                        self.window?.showFailure(error.sentence)
                    }
                }
            }
        }
    }
}

/// A menu item that runs a closure, for menus built from a list of actions.
final class ClosureMenuItem: NSMenuItem {
    private let run: () -> Void

    init(title: String, keyEquivalent: String = "", action run: @escaping () -> Void) {
        self.run = run
        super.init(title: title, action: #selector(runAction), keyEquivalent: keyEquivalent)
        target = self
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("not used") }

    @objc private func runAction() { run() }
}
