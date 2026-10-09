import UIKit

/// The iPad's top level, as the Mac's window is: the places in a sidebar, the place chosen
/// beside it, and beside that whatever was opened from it — a task, a settings page — or a
/// line saying nothing is. The iPhone has tabs instead (`RootTabs`).
final class IPadRoot: UISplitViewController, UISplitViewControllerDelegate, CommandActions {
    private let core: Core
    private lazy var sidebar = SidebarViewController(core: core) { [weak self] in self?.show($0) }
    /// The place shown: the middle column.
    let list = UINavigationController()
    // Kept, so coming back to them finds them as they were left.
    private lazy var day = DayViewController(core: core)
    private lazy var tasks = TaskListViewController(core: core)
    private var empty = "No task open"

    init(core: Core) {
        self.core = core
        super.init(style: .tripleColumn)
        delegate = self
        preferredDisplayMode = .twoBesideSecondary
        preferredSplitBehavior = .tile
        // The place at an iPhone's width where there is room: narrower, its rows wrap after a
        // word or two, and a swipe across one runs its first action.
        preferredSupplementaryColumnWidth = 375
        setViewController(UINavigationController(rootViewController: sidebar), for: .primary)
        setViewController(list, for: .supplementary)
        show(.place(.today))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    /// Whether something is open beside the place.
    private(set) var hasOpen = false

    /// Shows `destination`, then `then` on its screen once it is in the window: a screen not
    /// yet shown can neither present nor take focus.
    func show(_ destination: Destination, then: ((UIViewController) -> Void)? = nil) {
        if let presented = presentedViewController { presented.dismiss(animated: false) }
        let screen = screen(for: destination)
        empty = destination == .settings ? "No settings page open" : "No task open"
        list.setViewControllers([screen], animated: false)
        sidebar.current = destination
        close()
        show(.supplementary)
        // Lying over the place, as in portrait, the sidebar goes once a place is chosen, as
        // Mail's does; otherwise it stays over what was chosen, and over what is tapped next.
        if !isCollapsed && displayMode != .twoBesideSecondary { hide(.primary) }
        if let then { DispatchQueue.main.async { then(screen) } }
    }

    private func screen(for destination: Destination) -> UIViewController {
        guard case let .place(place) = destination else { return SettingsViewController(core: core) }
        switch place {
        case .today: return day
        case .tasks: return tasks
        case .blocks: return BlocksViewController(core: core)
        case .trash: return TaskListViewController(core: core, title: "Trash", query: placeQuery(place: place), mode: .trash)
        case .project, .label, .filter:
            return TaskListViewController(
                core: core, title: placeTitle(place: place), query: placeQuery(place: place),
                quickAddPrefix: placeQuickAddPrefix(place: place)
            )
        }
    }

    /// Shows `detail` beside the place, and puts VoiceOver on it, as a push would.
    func open(_ detail: UIViewController) {
        hasOpen = true
        setViewController(UINavigationController(rootViewController: detail), for: .secondary)
        DispatchQueue.main.async { UIAccessibility.post(notification: .screenChanged, argument: nil) }
    }

    /// Nothing open beside the place.
    func close() {
        hasOpen = false
        setViewController(UINavigationController(rootViewController: Unopened(empty)), for: .secondary)
    }

    // Narrowed to one column — Slide Over, a narrow Split View — it shows the place unless
    // something was opened from it.
    func splitViewController(
        _ svc: UISplitViewController, topColumnForCollapsingToProposedTopColumn proposedTopColumn: UISplitViewController.Column
    ) -> UISplitViewController.Column {
        hasOpen ? .secondary : .supplementary
    }

    // MARK: - Keyboard commands (`KeyboardCommands`); the screen in front answers first.

    override var canBecomeFirstResponder: Bool { true }

    /// The Home Screen's New Task, and ⌘N.
    @objc func newTask() { show(.place(.tasks)) { ($0 as? TaskListViewController)?.addTask() } }
    @objc func newBlock() { show(.place(.today)) { ($0 as? DayViewController)?.newBlock() } }
    @objc func newProject() { sidebar.newProject() }
    @objc func newLabel() { sidebar.newLabel() }
    @objc func newFilter() { sidebar.newFilter() }
    @objc func filterTasks() { show(.place(.tasks)) { ($0 as? TaskListViewController)?.focusFilter() } }
    @objc func goToToday() { show(.place(.today)) }
    @objc func goToTasks() { show(.place(.tasks)) }
    @objc func goToBlocks() { show(.place(.blocks)) }
    @objc func goToTrash() { show(.place(.trash)) }
    @objc func showSettings() { show(.settings) }
    @objc func undoChange() { undoInStore(core) }
    @objc func redoChange() { redoInStore(core) }
    @objc func syncNow() { syncAndSay(core) }
}

/// The column beside a list, before anything is opened from it.
final class Unopened: UIViewController {
    init(_ text: String) {
        super.init(nibName: nil, bundle: nil)
        var content = UIContentUnavailableConfiguration.empty()
        content.text = text
        content.textProperties.font = .preferredFont(forTextStyle: .title3)
        content.textProperties.color = .quietLabel
        contentUnavailableConfiguration = content
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }
}

extension UIViewController {
    /// Makes this screen where keyboard commands start, as it appears: with nothing first
    /// responder, UIKit has no chain to send the Task and Day menus' commands down, and they
    /// reach nothing. Text being edited keeps its place.
    func takeKeyboardCommands() {
        // Only text still on screen: a closing sheet's field is first responder until it goes.
        let first = UIResponder.first
        let editing = first is UITextInput && (first as? UIView)?.window != nil
        if !editing { becomeFirstResponder() }
    }

    /// The iPad's root, if this list is the place it shows beside its details.
    private var besideRoot: IPadRoot? {
        guard let root = splitViewController as? IPadRoot, !root.isCollapsed, navigationController === root.list
        else { return nil }
        return root
    }

    /// Opens `detail`: beside this list on iPad, over it on iPhone.
    func showBeside(_ detail: UIViewController) {
        if let root = besideRoot {
            root.open(detail)
        } else {
            navigationController?.pushViewController(detail, animated: true)
        }
    }

    /// Leaves `self`, opened by `showBeside`: back to the list either way.
    func closeBeside() {
        if let root = splitViewController as? IPadRoot, !root.isCollapsed, navigationController !== root.list {
            root.close()
        } else {
            navigationController?.popViewController(animated: true)
        }
    }

    /// ⌘Z where no screen of its own answers. In a text field with typing to undo, the
    /// typing is what goes; otherwise the last change to the store.
    func undoInStore(_ core: Core) {
        if let typing = UIResponder.editingWithUndo, typing.canUndo { typing.undo(); return }
        storeChange(core) { try $0.undo() }
    }

    func redoInStore(_ core: Core) {
        if let typing = UIResponder.editingWithUndo, typing.canRedo { typing.redo(); return }
        storeChange(core) { try $0.redo() }
    }

    private func storeChange(_ core: Core, _ operation: (Lumenna) throws -> Change) {
        do {
            let change = try operation(core.lumenna)
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            showFailure(error.sentence)
        }
    }

    /// A round with every paired device now, saying how it went.
    func syncAndSay(_ core: Core) {
        Announcer.say("Syncing")
        core.syncNow { [weak self] result in
            switch result {
            case let .success(report):
                NotificationCenter.default.post(name: Core.changed, object: nil)
                Announcer.say(report.announcement, notices: report.peers.compactMap { peer in
                    peer.error.map { "\(peer.name): \($0)" }
                })
            case let .failure(error):
                self?.showFailure(error.sentence)
            }
        }
    }
}

