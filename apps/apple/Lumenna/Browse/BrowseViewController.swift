import LumennaApp
import UIKit

/// Everything that is not the day or the task list: projects, labels, saved filters, every
/// block series, and the trash — the app's places (`ViewModel.sidebar`), each with what is in
/// it, in the core's words.
final class BrowseViewController: AppListViewController {
    init(core: Core) {
        super.init(core: core, title: LumennaApp.browse(sentence: false))
    }

    // Each place's own list has Undo and Redo; Browse is somewhere to go.
    override var offersUndo: Bool { false }

    override func rows() -> (items: [AppItem], count: String)? {
        let entries = core.app.view.sidebar
        func under(_ group: LumennaApp.SidebarGroup) -> UInt32 {
            var inside = false
            var count: UInt32 = 0
            for entry in entries {
                if entry.depth == 0 {
                    if case let .group(heading) = entry.kind { inside = heading == group } else { inside = false }
                } else if inside, entry.depth == 1 {
                    count += 1
                }
            }
            return count
        }
        var items: [AppItem] = []
        for entry in entries where entry.depth == 0 {
            switch entry.kind {
            case let .group(group):
                let count = LumennaApp.groupCount(group: group, count: under(group))
                items.append(AppItem(key: entry.key, title: entry.title, detail: count, swatch: entry.swatch))
            case .place(.today), .place(.tasks):
                // Tabs of their own.
                continue
            case .place:
                // Every other place the app lists at the top: Blocks, Trash, History.
                items.append(AppItem(key: entry.key, title: entry.title, detail: entry.detail.isEmpty ? nil : entry.detail, swatch: entry.swatch))
            }
        }
        return (items, "")
    }

    override func open(_ item: AppItem) {
        guard let entry = core.app.sidebarEntry(item.key) else { return }
        _ = show(entry.kind, titled: entry.title)
    }

    /// Opens one of Browse's places, as choosing its row does, and gives back its screen: the
    /// keyboard's New Project, Blocks and the others go there too.
    func show(_ kind: LumennaApp.SidebarKind, titled title: String? = nil) -> UIViewController {
        let title = title ?? core.app.sidebarEntry(LumennaApp.sidebarKey(kind: kind))?.title ?? ""
        let next: UIViewController
        switch kind {
        case let .group(group): next = PlacesViewController(core: core, group: group, title: title)
        case let .place(place): next = TaskListViewController(core: core, place: place)
        }
        navigationController?.pushViewController(next, animated: true)
        return next
    }
}

/// The places under one of the sidebar's headings — the project tree, with weights; the labels;
/// the saved filters — each opening its tasks, with what can be done to it, the app's: renamed,
/// made, reordered and removed. Held in the sidebar's hand, so a change lands back here.
final class PlacesViewController: AppListViewController {
    private let group: LumennaApp.SidebarGroup

    init(core: Core, group: LumennaApp.SidebarGroup, title: String) {
        self.group = group
        // Short: a bar title is one line between its buttons, and "Saved Filters" overflows a
        // small phone at the largest text sizes. Browse lists it by its full name.
        super.init(core: core, title: group == .filters ? "Filters" : title)
    }

    override var side: LumennaApp.Side? { .sidebar }

    override func rows() -> (items: [AppItem], count: String)? {
        let items = core.app.sidebarItems(under: group)
        // With nothing listed, what is empty, in the core's words, in place of a count.
        let count = items.isEmpty ? LumennaApp.groupEmpty(group: group) : LumennaApp.groupCount(group: group, count: UInt32(items.count))
        return (items, count)
    }

    /// The heading over these places, whose New makes one.
    private var heading: LumennaApp.SidebarEntry? {
        core.app.view.sidebar.first { if case let .group(heading) = $0.kind { return heading == group } else { return false } }
    }

    override var addTitle: String? {
        switch group {
        case .projects: "Add project"
        case .labels: "Add label"
        case .filters: "Add filter"
        }
    }

    /// Adds one as the sidebar's heading over them does: its New action, the app's, so the add
    /// button asks what every app asks.
    override func add() {
        guard let heading, let action = heading.offered.first(where: { $0.kind == .new }) else { return }
        core.app.run(action, on: heading.key, side: .sidebar)
    }

    override func open(_ item: AppItem) {
        guard case let .place(place) = core.app.sidebarEntry(item.key)?.kind else { return }
        core.app.send(.hold(field0: .sidebar, field1: item.key))
        navigationController?.pushViewController(TaskListViewController(core: core, place: place), animated: true)
    }
}
