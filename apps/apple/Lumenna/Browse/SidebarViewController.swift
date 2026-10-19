import LumennaApp
import UIKit

/// Somewhere the iPad's sidebar goes: one of the app's places, or Settings, which the Mac
/// keeps in a window of its own.
enum Destination: Equatable {
    case place(LumennaApp.Place)
    case settings
}

/// The iPad's sidebar: Today, Tasks, the project tree, labels, saved filters, blocks, the
/// trash — the app's places (`ViewModel.sidebar`), as the Mac, Windows, GTK and the web list
/// them — then Settings.
///
/// Headings and projects with subprojects fold, saying "expanded" or "collapsed", with
/// Expand and Collapse among their actions, the app's; what can be done to a project, label or
/// filter is its swipe actions, as in Browse on the iPhone. Held in the sidebar's hand, so a
/// change made here lands back here.
final class SidebarViewController: AppListViewController {
    private let chose: (Destination) -> Void
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
    override var side: LumennaApp.Side? { .sidebar }
    override var selectedKey: String? {
        switch current {
        case let .place(place): LumennaApp.sidebarKey(kind: .place(place))
        case .settings: "settings"
        }
    }

    override func rows() -> (items: [AppItem], count: String)? {
        (core.app.sidebarItems() + [AppItem(key: "settings", title: "Settings")], "")
    }

    override func open(_ item: AppItem) {
        guard item.key != "settings" else { return chose(.settings) }
        guard let entry = core.app.sidebarEntry(item.key) else { return }
        core.app.send(.hold(field0: .sidebar, field1: item.key))
        switch entry.kind {
        case .group:
            // A heading folds what is under it when chosen: its own Expand or Collapse.
            if let fold = entry.actions.first(where: { [.expand, .collapse].contains($0.kind) }) { perform(fold, on: item) }
        case let .place(place):
            chose(.place(place))
        }
    }

    // The File menu's New Project, New Label and New Saved Filter (`KeyboardCommands`): each
    // heading's New action.
    @objc func newProject() { runNew(under: .projects) }
    @objc func newLabel() { runNew(under: .labels) }
    @objc func newFilter() { runNew(under: .filters) }

    private func runNew(under group: LumennaApp.SidebarGroup) {
        let key = LumennaApp.sidebarKey(kind: .group(group))
        guard let item = items.first(where: { $0.key == key }), let action = item.actions.first(where: { $0.kind == .new }) else { return }
        perform(action, on: item)
    }
}
