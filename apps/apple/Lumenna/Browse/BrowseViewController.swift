import LumennaApp
import UIKit

/// Everything that is not a tab of its own: projects, labels, saved filters, every block
/// series, the history and the trash — the core's Browse (`Arrangement.browse`), each place with
/// its detail, swatch and emoji, each group with how many places it holds, in the core's words.
final class BrowseViewController: AppListViewController {
    init(core: Core) {
        super.init(core: core, title: Wording.browse)
    }

    // Each place's own list has Undo and Redo; Browse is somewhere to go.
    override var offersUndo: Bool { false }

    override func rows() -> (items: [AppItem], count: String)? {
        // Somewhere to go, not to change: a group's folding and New are its own screen's, and
        // every row is at the top, so none has a level to say.
        let items = core.app.arrangement.browse.map { entry in
            AppItem(key: entry.key, title: entry.title, detail: entry.detail.isEmpty ? nil : entry.detail, swatch: entry.swatch, emoji: entry.emoji)
        }
        return (items, "")
    }

    override func open(_ item: AppItem) {
        guard let entry = core.app.arrangement.browse.first(where: { $0.key == item.key }) else { return }
        _ = show(entry.kind)
    }

    /// Opens one of Browse's rows, as choosing it does, and gives back its screen: a place's
    /// tasks inside Browse, a group's own screen. The keyboard's New Project, Blocks and the
    /// others go there too.
    func show(_ kind: LumennaApp.SidebarKind) -> UIViewController {
        let next: UIViewController
        switch kind {
        case let .group(group): next = PlacesViewController(core: core, group: group)
        case let .place(place): next = TaskListViewController(core: core, place: place)
        }
        navigationController?.pushViewController(next, animated: true)
        return next
    }
}

/// One group's own screen, the core's (`GroupScreen`): the project tree, with weights; the
/// labels; the saved filters — each opening its tasks, with what can be done to it, the app's:
/// renamed, made, reordered and removed. Its rows fold as the sidebar's do, and it is titled and
/// adds as the sidebar's heading over them. Held in the sidebar's hand, so a change lands back
/// here.
final class PlacesViewController: AppListViewController {
    private let group: LumennaApp.SidebarGroup

    init(core: Core, group: LumennaApp.SidebarGroup) {
        self.group = group
        super.init(core: core, title: core.app.groupScreen(group)?.heading.title ?? "")
    }

    override var side: LumennaApp.Side? { .sidebar }

    /// The core's screen for this group, as the app shows the sidebar now.
    private var screen: LumennaApp.GroupScreen? { core.app.groupScreen(group) }

    override func rows() -> (items: [AppItem], count: String)? {
        guard let screen else { return nil }
        if !screen.heading.title.isEmpty, title != screen.heading.title { title = screen.heading.title }
        let items = screen.shown
        // How many, as Browse's row says it; with none, what is empty, in the core's words.
        let count = items.isEmpty
            ? screen.empty
            : core.app.arrangement.browse.first { $0.key == screen.heading.key }?.detail ?? ""
        return (items, count)
    }

    /// The heading's New: what makes one, the core's.
    private var new: LumennaApp.Offered? {
        screen?.heading.offered.first { $0.kind == .new }
    }

    override var addTitle: String? { new?.title }

    /// Adds one as the sidebar's heading over them does: its New action, the app's, so the add
    /// button asks what every app asks.
    override func add() {
        guard let screen, let new else { return }
        core.app.run(new, on: screen.heading.key, side: .sidebar)
    }

    override func open(_ item: AppItem) {
        guard let entry = screen?.entries.first(where: { $0.key == item.key }), case let .place(place) = entry.kind else { return }
        core.app.send(.hold(field0: .sidebar, field1: item.key))
        navigationController?.pushViewController(TaskListViewController(core: core, place: place), animated: true)
    }
}
