import AppKit
import LumennaApp

/// One row of the sidebar: one of the app's entries (`ViewModel.sidebar`).
final class SidebarNode {
    let entry: LumennaApp.SidebarEntry
    var children: [SidebarNode] = []

    init(_ entry: LumennaApp.SidebarEntry) {
        self.entry = entry
    }

    var place: LumennaApp.Place? {
        if case let .place(place) = entry.kind { return place }
        return nil
    }

    var isGroup: Bool {
        if case .group = entry.kind { return true }
        return false
    }

    /// Whether it folds: Expand or Collapse is among its actions, the app's.
    var folds: Bool { entry.actions.contains { [.expand, .collapse].contains($0.kind) } }
}

/// The places: Today, Tasks, the project tree, labels, saved filters, blocks, the trash.
///
/// A source list, as Mail's mailboxes are, because that is the outline VoiceOver and the
/// keyboard know best on a Mac. The places, which of them are folded, and what a project,
/// label or filter can have done to it are the app's (`ViewModel.sidebar`), as every sidebar
/// has them; its actions are in its context menu, which VoiceOver opens with VO-Shift-M, and a
/// heading's is its New. Its rows are held in the sidebar's hand, so a change made here lands
/// here; the place selected is the one the window shows.
final class SidebarViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate, NSMenuDelegate {
    private let core: Core
    let outline = NSOutlineView()
    private var roots: [SidebarNode] = []
    /// Whether the outline is being set from the app's, not moved by the person.
    private var following = false
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Places"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

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
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
        draw()
    }

    override var preferredFirstResponder: NSView? { outline }

    // MARK: - Drawing

    /// The app's places, folded as the app has them, the place shown selected. Drawn again
    /// only when they changed, so a minute or a sync does not make VoiceOver read the row again.
    private func draw() {
        let view = core.app.view
        let entries = view.sidebar
        let folded = view.sidebarFolded
        if entries != drawn?.entries || folded != drawn?.folded {
            drawn = (entries, folded)
            roots = Self.tree(entries.map(SidebarNode.init))
            following = true
            outline.reloadData()
            fold(roots, folded: Set(folded))
            following = false
        }
        reselect()
    }

    /// What was drawn last.
    private var drawn: (entries: [LumennaApp.SidebarEntry], folded: [String])?

    /// The flat, depth-first entries the app sends, as a tree: an entry's parent is the
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

    private func fold(_ nodes: [SidebarNode], folded: Set<String>) {
        for node in nodes where !node.children.isEmpty {
            if folded.contains(node.entry.key) {
                outline.collapseItem(node)
            } else {
                outline.expandItem(node)
                fold(node.children, folded: folded)
            }
        }
    }

    private func node(where matches: (SidebarNode) -> Bool, in nodes: [SidebarNode]? = nil) -> SidebarNode? {
        for node in nodes ?? roots {
            if matches(node) { return node }
            if let found = self.node(where: matches, in: node.children) { return found }
        }
        return nil
    }

    /// Keeps the selection on the place the window shows.
    private func reselect() {
        guard let place = core.app.view.place, let node = node(where: { $0.place == place }) else { return }
        let row = outline.row(forItem: node)
        if row >= 0, outline.selectedRow != row {
            following = true
            outline.selectRowIndexes([row], byExtendingSelection: false)
            following = false
        }
    }

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

    /// Folded or unfolded by the person, with the arrow keys or the disclosure triangle: the
    /// row's own Collapse or Expand, the app's, which remembers it.
    func outlineViewItemDidCollapse(_ notification: Notification) { folded(notification, .collapse) }
    func outlineViewItemDidExpand(_ notification: Notification) { folded(notification, .expand) }

    private func folded(_ notification: Notification, _ kind: LumennaApp.ActionKind) {
        guard !following, let node = notification.userInfo?["NSObject"] as? SidebarNode,
              let action = node.entry.actions.first(where: { $0.kind == kind }) else { return }
        core.app.run(action, on: node.entry.key, side: .sidebar)
    }

    func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? SidebarNode else { return nil }
        let cell = NSTableCellView()
        let title = NSTextField(labelWithString: node.entry.title)
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
        let detail = node.entry.detail
        if !detail.isEmpty {
            cell.setAccessibilityValueDescription(detail)
            cell.toolTip = detail
        }
        return cell
    }

    /// A place chosen: held in the sidebar's hand, and shown.
    func outlineViewSelectionDidChange(_ notification: Notification) {
        guard !following, let node = outline.item(atRow: outline.selectedRow) as? SidebarNode,
              let place = node.place, place != core.app.view.place else { return }
        core.app.send(.hold(field0: .sidebar, field1: node.entry.key))
        core.app.send(.open(place))
    }

    // MARK: - Context menus

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let row = outline.clickedRow >= 0 ? outline.clickedRow : outline.selectedRow
        guard let node = outline.item(atRow: row) as? SidebarNode else { return }
        menu.add(node.entry.actions) { [weak self] action in
            self?.core.app.run(action, on: node.entry.key, side: .sidebar)
        }
    }

    /// The heading's New, from the File menu.
    private func runNew(under group: LumennaApp.SidebarGroup) {
        guard let heading = node(where: { if case let .group(found) = $0.entry.kind { found == group } else { false } }),
              let action = heading.entry.actions.first(where: { $0.kind == .new }) else { return }
        core.app.run(action, on: heading.entry.key, side: .sidebar)
    }

    @objc func newProject() { runNew(under: .projects) }
    @objc func newLabel() { runNew(under: .labels) }
    @objc func newSavedFilter() { runNew(under: .filters) }
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
