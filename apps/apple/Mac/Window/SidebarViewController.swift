import AppKit

/// Somewhere to go in the main window.
enum Place: Hashable {
    case today, tasks, blocks, trash
    case project(String)
    case label(String)
    case filter(String, query: String)
}

/// One row of the sidebar.
final class SidebarNode {
    enum Kind {
        case place(Place)
        /// "Projects", "Labels", "Saved Filters": a group heading, not somewhere to go.
        case group(String)
    }

    let kind: Kind
    let title: String
    let detail: String?
    var children: [SidebarNode] = []
    /// For a project: its row, which carries depth, state and the counts.
    let row: RowView?

    init(_ kind: Kind, title: String, detail: String? = nil, row: RowView? = nil) {
        self.kind = kind
        self.title = title
        self.detail = detail
        self.row = row
    }

    var place: Place? {
        if case let .place(place) = kind { return place }
        return nil
    }

    var isGroup: Bool {
        if case .group = kind { return true }
        return false
    }
}

/// The places: Today, Tasks, the project tree, labels, saved filters, blocks, the trash.
///
/// A source list, as Mail's mailboxes are, because that is the outline VoiceOver and the
/// keyboard know best on a Mac. Projects nest as they do in the store; what a project, label
/// or filter can have done to it is in its context menu, which VoiceOver opens with VO-Shift-M.
final class SidebarViewController: NSViewController, NSOutlineViewDataSource, NSOutlineViewDelegate, NSMenuDelegate {
    private let core: Core
    private let chose: (Place) -> Void
    let outline = NSOutlineView()
    private var roots: [SidebarNode] = []
    private var selected: Place = .today
    private var archived: Set<String> = []
    private var labels: [String] = []
    private var projects: [String] = []

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
        var places = [
            SidebarNode(.place(.today), title: "Today"),
            SidebarNode(.place(.tasks), title: "Tasks"),
        ]
        let projectsGroup = SidebarNode(.group("Projects"), title: "Projects")
        if let rows = try? core.lumenna.listProjects().rows {
            projectsGroup.children = Self.tree(rows.map { row in
                SidebarNode(
                    .place(.project(row.title)), title: row.title,
                    detail: ([row.value].compactMap { $0 } + row.state).joined(separator: ", "), row: row
                )
            })
            archived = Set(rows.filter { $0.state.contains("archived") }.map(\.title))
            projects = rows.map(\.title)
        }
        let labelsGroup = SidebarNode(.group("Labels"), title: "Labels")
        if let rows = try? core.lumenna.listLabels().rows {
            labelsGroup.children = rows.map { SidebarNode(.place(.label($0.title)), title: $0.title, detail: $0.value) }
            labels = rows.map(\.title)
        }
        let filtersGroup = SidebarNode(.group("Saved Filters"), title: "Saved Filters")
        if let filters = try? core.lumenna.listFilters().filters {
            filtersGroup.children = filters.map {
                SidebarNode(.place(.filter($0.name, query: $0.query)), title: $0.name, detail: $0.query)
            }
        }
        let trash = (try? core.lumenna.listTasks(query: "deleted").count).map { $0 == 1 ? "1 task" : "\($0) tasks" }
        places += [
            projectsGroup, labelsGroup, filtersGroup,
            SidebarNode(.place(.blocks), title: "Blocks"),
            SidebarNode(.place(.trash), title: "Trash", detail: trash),
        ]
        roots = places
        outline.reloadData()
        expandAll(roots)
        reselect()
    }

    /// The flat, depth-first project rows the core sends, as a tree.
    private static func tree(_ nodes: [SidebarNode]) -> [SidebarNode] {
        var top: [SidebarNode] = []
        var chain: [SidebarNode] = []
        for node in nodes {
            let depth = Int(node.row?.depth ?? 0)
            while chain.count > depth { chain.removeLast() }
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

    func outlineView(_ outlineView: NSOutlineView, isGroupItem item: Any) -> Bool {
        (item as? SidebarNode)?.isGroup ?? false
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
            title.font = .preferredFont(forTextStyle: .subheadline)
            title.textColor = .quietLabel
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
        for (title, action) in actions(for: node) {
            if title == "-" {
                menu.addItem(.separator())
            } else {
                menu.addItem(ClosureMenuItem(title: title, action: action))
            }
        }
    }

    private var window: NSWindow? { view.window }

    func actions(for node: SidebarNode) -> [(String, () -> Void)] {
        let lumenna = core.lumenna
        switch node.kind {
        case let .group(name):
            switch name {
            case "Projects": return [("New Project…", { [weak self] in self?.newProject(inside: nil) })]
            case "Labels": return [("New Label…", { [weak self] in self?.newLabel() })]
            default: return [("New Saved Filter…", { [weak self] in self?.newFilter() })]
            }
        case .place(.project(let name)):
            return [
                ("Rename…", { [weak self] in
                    self?.window?.askForText("Rename \(name)", initial: name) { to in
                        self?.change(then: .project(to)) { try lumenna.renameProject(name: name, to: to) }
                    }
                }),
                ("New Project Inside…", { [weak self] in self?.newProject(inside: name) }),
                ("Move Under…", { [weak self] in self?.moveProject(name) }),
                ("Move Up", { [weak self] in self?.change { try lumenna.reorderProject(name: name, direction: .up) } }),
                ("Move Down", { [weak self] in self?.change { try lumenna.reorderProject(name: name, direction: .down) } }),
                ("Weight…", { [weak self] in
                    self?.window?.askForText(
                        "Weight of \(name)",
                        message: "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.",
                        placeholder: "1.0"
                    ) { text in
                        let weight: Weight = Float(text).map { .value(value: $0) } ?? .inherit
                        self?.change { try lumenna.weighProject(name: name, weight: weight) }
                    }
                }),
                (archived.contains(name) ? "Unarchive" : "Archive", { [weak self] in
                    self?.change { try lumenna.archiveProject(name: name) }
                }),
                ("-", {}),
                ("Delete…", { [weak self] in
                    self?.window?.choose(
                        "Delete \(name)?", message: "Its tasks can go to the trash with it, or move to the Inbox.",
                        actions: [
                            ("Delete and Trash Its Tasks", { self?.change(then: .tasks) { try lumenna.deleteProject(name: name, keepTasks: false) } }),
                            ("Delete and Keep Its Tasks", { self?.change(then: .tasks) { try lumenna.deleteProject(name: name, keepTasks: true) } }),
                        ]
                    )
                }),
            ]
        case .place(.label(let name)):
            return [
                ("Rename…", { [weak self] in
                    self?.window?.askForText("Rename \(name)", initial: name) { to in
                        self?.change(then: .label(to)) { try lumenna.renameLabel(name: name, to: to) }
                    }
                }),
                ("Merge Into…", { [weak self] in
                    guard let self, let window = self.window else { return }
                    // For when a typo made a near-duplicate: this one's tasks move to the other.
                    let others = self.labels.filter { $0 != name }.map { PickerItem(key: $0, title: $0) }
                    PickerSheet.present(on: window, title: "Merge \(name) Into", items: others) { other in
                        self.change(then: .label(other.key)) { try lumenna.mergeLabels(from: name, into: other.key) }
                    }
                }),
                ("Colour…", { [weak self] in
                    self?.window?.askForText(
                        "Colour for \(name)",
                        message: "A colour name, such as red or teal, or none. The name always shows too.",
                        placeholder: "teal"
                    ) { colour in
                        self?.change { try lumenna.recolourLabel(name: name, colour: colour.lowercased() == "none" ? nil : colour) }
                    }
                }),
                ("Move Up", { [weak self] in self?.change { try lumenna.reorderLabel(name: name, direction: .up) } }),
                ("Move Down", { [weak self] in self?.change { try lumenna.reorderLabel(name: name, direction: .down) } }),
                ("-", {}),
                ("Delete…", { [weak self] in
                    self?.window?.confirm("Delete \(name)?", message: "Tasks wearing it stay; they just stop showing it.", action: "Delete") {
                        self?.change(then: .tasks) { try lumenna.deleteLabel(name: name) }
                    }
                }),
            ]
        case .place(.filter(let name, let query)):
            return [
                ("Rename…", { [weak self] in
                    self?.window?.askForText("Rename \(name)", initial: name) { to in
                        self?.change(then: .filter(to, query: query)) { try lumenna.editFilter(name: name, rename: to, query: nil) }
                    }
                }),
                ("Change Query…", { [weak self] in
                    self?.window?.askForText("Query for \(name)", initial: query) { new in
                        self?.change(then: .filter(name, query: new)) { try lumenna.editFilter(name: name, rename: nil, query: new) }
                    }
                }),
                ("Move Up", { [weak self] in self?.change { try lumenna.reorderFilter(name: name, direction: .up) } }),
                ("Move Down", { [weak self] in self?.change { try lumenna.reorderFilter(name: name, direction: .down) } }),
                ("-", {}),
                ("Delete…", { [weak self] in
                    self?.window?.confirm("Delete \(name)?", message: "The tasks it shows are not touched.", action: "Delete") {
                        self?.change(then: .tasks) { try lumenna.deleteFilter(name: name) }
                    }
                }),
            ]
        default:
            return []
        }
    }

    /// Runs a change, says what it did, and — when it renamed or removed the place shown —
    /// goes to `then`.
    private func change(then place: Place? = nil, _ operation: () throws -> Change) {
        do {
            let change = try operation()
            reload()
            if let place, change.changed { select(place) }
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            window?.showFailure(error.sentence)
        }
    }

    @objc func newProject(inside parent: String?) {
        window?.askForText(parent.map { "New Project in \($0)" } ?? "New Project", placeholder: "Name", action: "Add") { [weak self] name in
            self?.change(then: .project(name)) { try self!.core.lumenna.addProject(name: name, parent: parent) }
        }
    }

    @objc func newLabel() {
        window?.askForText("New Label", placeholder: "Name", action: "Add") { [weak self] name in
            self?.change(then: .label(name)) { try self!.core.lumenna.addLabel(name: name) }
        }
    }

    @objc func newFilter() {
        window?.askForText("New Saved Filter", placeholder: "Name", action: "Next") { [weak self] name in
            // A sheet cannot open while the last is still closing.
            DispatchQueue.main.async {
                self?.window?.askForText("Query for \(name)", placeholder: "#Work & overdue", action: "Save") { query in
                    self?.change(then: .filter(name, query: query)) { try self!.core.lumenna.addFilter(name: name, query: query) }
                }
            }
        }
    }

    private func moveProject(_ name: String) {
        guard let window else { return }
        let choices = [PickerItem(key: "", title: "The top level")]
            + projects.filter { $0 != name }.map { PickerItem(key: $0, title: $0) }
        PickerSheet.present(on: window, title: "Move \(name) Under", items: choices) { [weak self] choice in
            self?.change { try self!.core.lumenna.moveProject(name: name, parent: choice.key.isEmpty ? nil : choice.key) }
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
