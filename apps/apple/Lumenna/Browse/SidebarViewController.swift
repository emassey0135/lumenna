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
/// is its swipe actions, the core's, as in Browse on the iPhone.
final class SidebarViewController: ItemListViewController {
    private let chose: (Destination) -> Void
    private var entries: [String: SidebarEntry] = [:]
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
            var item = Item(key: Self.key(entry.kind), title: title, detail: detail, depth: entry.depth, actions: entry.actions)
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

    /// A renamed or new project, label or filter is found again by its place.
    override func key(forName name: String, subject: Subject) -> String {
        switch subject {
        case .project: "project:\(name)"
        case .label: "label:\(name)"
        case .filter: "filter:\(name)"
        default: name
        }
    }

    /// New Saved Filter is the app's own form: a name, then a query.
    override func form(_ action: Action, on item: Item) {
        if action.kind == .new, action.subject == .filter { addFilter { "filter:\($0)" } }
    }

    // The File menu's New Project, New Label and New Saved Filter (`KeyboardCommands`): each
    // heading's New action.
    @objc func newProject() { runNew(under: .projects) }
    @objc func newLabel() { runNew(under: .labels) }
    @objc func newFilter() { runNew(under: .filters) }

    private func runNew(under group: SidebarGroup) {
        let key = Self.key(.group(group))
        guard let item = items.first(where: { $0.key == key }), let action = item.actions.first(.new) else { return }
        perform(action, on: item)
    }
}
