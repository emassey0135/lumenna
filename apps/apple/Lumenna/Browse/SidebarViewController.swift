import UIKit

/// Somewhere the iPad's sidebar goes: one of the core's places, or Settings, which the Mac
/// keeps in a window of its own.
enum Destination: Equatable {
    case place(Place)
    case settings
}

/// The iPad's sidebar: Today, Tasks, the project tree, labels, saved filters, blocks, the
/// trash — the core's places (`Lumenna.places`), as the Mac, Windows, GTK and the web list
/// them — then Settings.
///
/// Headings and projects with subprojects fold, saying "expanded" or "collapsed", with
/// Expand and Collapse among their actions; what can be done to a project, label or filter
/// is its swipe actions, as in Browse on the iPhone (`PlaceActions`).
final class SidebarViewController: ItemListViewController {
    private let chose: (Destination) -> Void
    private var entries: [String: SidebarEntry] = [:]
    private var projects: [String] = []
    private var labels: [String] = []
    /// The row of what is shown, kept selected.
    var current: Destination = .place(.today) {
        didSet { reselect() }
    }

    init(core: Core, chose: @escaping (Destination) -> Void) {
        self.chose = chose
        super.init(core: core, title: "Places")
    }

    override var isSidebar: Bool { true }
    // Each place's own list has Undo and Redo; the sidebar is somewhere to go.
    override var offersUndo: Bool { false }
    override var selectedKey: String? {
        switch current {
        case let .place(place): Self.key(.place(place))
        case .settings: "settings"
        }
    }

    /// What identifies a row across a reload: a project and a label can share a name.
    static func key(_ kind: SidebarKind) -> String {
        switch kind {
        case let .group(group): "group:\(group)"
        case let .place(place):
            switch place {
            case .today: "today"
            case .tasks: "tasks"
            case let .project(name): "project:\(name)"
            case let .label(name): "label:\(name)"
            case let .filter(name, _): "filter:\(name)"
            case .blocks: "blocks"
            case .trash: "trash"
            }
        }
    }

    override func load() throws -> (items: [Item], count: String) {
        let places = core.lumenna.places().entries
        entries = Dictionary(places.map { (Self.key($0.kind), $0) }, uniquingKeysWith: { first, _ in first })
        projects = places.compactMap { if case let .place(.project(name)) = $0.kind { name } else { nil } }
        labels = places.compactMap { if case let .place(.label(name)) = $0.kind { name } else { nil } }
        var items = places.map { entry -> Item in
            let title: String = switch entry.kind {
            case let .group(group): switch group {
                case .projects: "Projects"
                case .labels: "Labels"
                case .filters: "Saved Filters"
                }
            case let .place(place): placeTitle(place: place)
            }
            // The core's line is the title, then what is in it: shown on two lines here.
            let detail = entry.text.hasPrefix(title + ", ") ? String(entry.text.dropFirst(title.count + 2)) : nil
            var item = Item(key: Self.key(entry.kind), title: title, detail: detail, depth: entry.depth)
            if case .group = entry.kind { item.heading = true }
            return item
        }
        items.append(Item(key: "settings", title: "Settings"))
        return (items, "")
    }

    override func open(_ item: Item) {
        if item.key == "settings" {
            chose(.settings)
        } else if let entry = entries[item.key] {
            switch entry.kind {
            case .group: toggleFold(item.key)
            case let .place(place): chose(.place(place))
            }
        }
    }

    override func actions(for item: Item) -> [ItemAction] {
        guard let entry = entries[item.key] else { return [] }
        switch entry.kind {
        case .group(.projects):
            return [ItemAction(title: "New Project") { [weak self] _ in self?.newProject() }]
        case .group(.labels):
            return [ItemAction(title: "New Label") { [weak self] _ in self?.newLabel() }]
        case .group(.filters):
            return [ItemAction(title: "New Saved Filter") { [weak self] _ in self?.newFilter() }]
        case let .place(.project(name)):
            return projectActions(name, archived: entry.archived, others: { [weak self] in self?.projects ?? [] }) { "project:\($0)" }
        case let .place(.label(name)):
            return labelActions(name, others: { [weak self] in self?.labels ?? [] }) { "label:\($0)" }
        case let .place(.filter(name, query)):
            return filterActions(name, query: query) { "filter:\($0)" }
        case .place:
            return []
        }
    }

    // The File menu's New Project, New Label and New Saved Filter (`KeyboardCommands`).
    @objc func newProject() { addProject { "project:\($0)" } }
    @objc func newLabel() { addLabel { "label:\($0)" } }
    @objc func newFilter() { addFilter { "filter:\($0)" } }
}
