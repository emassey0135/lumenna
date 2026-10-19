import LumennaApp
import UIKit

/// One row of a list drawn from the app: what identifies it to the app (the key it is held
/// by), how it reads, and what can be done to it, the app's.
struct AppItem: Hashable {
    var key: String
    var title: String
    var detail: String?
    var depth: UInt32 = 0
    /// What VoiceOver says after the title: the detail, then folding and the level.
    var spoken: String?
    /// A heading over the items under it, as the sidebar's Projects and Labels are.
    var heading = false
    var actions: [LumennaApp.Action] = []
    /// Why it is offered, a line beneath and VoiceOver's hint, never its name.
    var reason: String?
}

/// A plain list drawn from the app, with the task list's focus rules: the app keeps the row in
/// hand after a change — the same row, else whatever holds its place — and when it moved it
/// (its landing count went up) VoiceOver goes there; a redraw for anything else leaves focus
/// alone. What it lists, folded, and what each row offers are the app's.
///
/// Subclasses say which part of the app's view they list and where they are held; this does the
/// rest. The surface's `ItemListViewController` stays for the lists not yet moved.
class AppListViewController: UIViewController, UICollectionViewDelegate {
    let core: Core
    /// The rows shown.
    private(set) var items: [AppItem] = []
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, String>!
    private let header = UILabel()
    private var observer: NSObjectProtocol?
    /// The landing count this list last moved focus for.
    private var landed: UInt32?

    init(core: Core, title: String) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        self.title = title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    // MARK: - What subclasses say

    /// The rows, and the line over them: how many, or the app's words for none. Nil while the
    /// app shows something else, which leaves the list as it was.
    func rows() -> (items: [AppItem], count: String)? { nil }
    /// Where its rows are held: a list beside the main one (the places, a reminder list), or
    /// the main list itself (nil).
    var side: LumennaApp.Side? { nil }
    /// What this list shows, made the app's as it comes to the front.
    func appear() {}
    /// The row in hand, and the count that moves when the app moves it.
    var held: (focus: String?, landing: UInt32) {
        switch side {
        case .sidebar: (core.app.view.sidebarHeld.focus, core.app.view.sidebarHeld.landing)
        case .reminders: (core.app.view.remindersHeld.focus, core.app.view.remindersHeld.landing)
        case .devices: (core.app.view.devicesHeld.focus, core.app.view.devicesHeld.landing)
        case nil: (core.app.view.focus, core.app.view.landing)
        }
    }
    /// What tapping does.
    func open(_ item: AppItem) {}
    /// What the add button does; `nil` hides it.
    var addTitle: String? { nil }
    func add() {}
    /// Whether Undo and Redo are offered: on every screen that changes the store.
    var offersUndo: Bool { true }
    /// The sidebar's look rather than a list's, with no line counting what is listed.
    var isSidebar: Bool { false }
    /// The item to keep selected, as the sidebar keeps the place shown; nil selects nothing.
    var selectedKey: String? { nil }

    // MARK: - Doing it

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        var configuration = UICollectionLayoutListConfiguration(appearance: isSidebar ? .sidebar : .insetGrouped)
        configuration.headerMode = isSidebar ? .none : .supplementary
        configuration.trailingSwipeActionsConfigurationProvider = { [weak self] path in self?.trailingSwipeActions(at: path) }
        collectionView = UICollectionView(
            frame: view.bounds,
            collectionViewLayout: UICollectionViewCompositionalLayout.list(using: configuration)
        )
        collectionView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        collectionView.delegate = self
        view.addSubview(collectionView)

        let sidebar = isSidebar
        let cell = UICollectionView.CellRegistration<UICollectionViewListCell, String> { [weak self] cell, _, key in
            guard let self, let item = self.items.first(where: { $0.key == key }) else { return }
            var content = sidebar
                ? (item.heading ? UIListContentConfiguration.sidebarHeader() : .sidebarSubtitleCell())
                : UIListContentConfiguration.subtitleCell()
            content.text = item.title
            content.textProperties.numberOfLines = 0
            let lines = [item.detail, item.reason].compactMap { $0 }.filter { !$0.isEmpty }
            content.secondaryText = lines.isEmpty ? nil : lines.joined(separator: "\n")
            content.secondaryTextProperties.color = .quietLabel
            content.secondaryTextProperties.numberOfLines = 0
            content.directionalLayoutMargins.leading += CGFloat(item.depth) * 20
            if sidebar { content.textProperties.color = .label }
            cell.contentConfiguration = content
            cell.accessories = sidebar ? [] : [.disclosureIndicator(displayed: .always)]
            // Said explicitly. Left to itself the cell's label is its text and its detail
            // together, so a value of the detail says it twice.
            cell.isAccessibilityElement = true
            cell.accessibilityLabel = item.title
            cell.accessibilityValue = item.spoken ?? item.detail
            cell.accessibilityHint = item.reason
            cell.accessibilityTraits = item.heading ? [.header, .button] : .button
            // Only what is not swiped, which UIKit adds to the swipe actions.
            cell.accessibilityCustomActions = self.rowActions(for: item).customActions
        }
        let headerRegistration = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(
            elementKind: UICollectionView.elementKindSectionHeader
        ) { [weak self] header, _, _ in
            var content = header.defaultContentConfiguration()
            content.text = self?.header.text
            header.contentConfiguration = content
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) { collectionView, path, key in
            collectionView.dequeueConfiguredReusableCell(using: cell, for: path, item: key)
        }
        dataSource.supplementaryViewProvider = { collectionView, _, path in
            collectionView.dequeueConfiguredReusableSupplementary(using: headerRegistration, for: path)
        }
        if offersUndo {
            navigationItem.leftItemsSupplementBackButton = true
            navigationItem.leftBarButtonItems = [
                .undo { [weak self] in self?.undoChange() },
                .redo { [weak self] in self?.redoChange() },
            ]
        }
        if let addTitle {
            let button = UIBarButtonItem(systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.add() })
            button.accessibilityLabel = addTitle
            navigationItem.rightBarButtonItem = button
        }
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if selectedKey == nil, let selected = collectionView.indexPathsForSelectedItems?.first {
            collectionView.deselectItem(at: selected, animated: animated)
        }
        appear()
        // Coming to the front is not the app moving focus: VoiceOver says the screen itself.
        landed = nil
        draw()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        takeKeyboardCommands()
    }

    /// Draws what the app shows of this list, and moves VoiceOver to the row in hand when the
    /// app moved it.
    func draw() {
        guard isViewLoaded, let (rows, count) = rows() else { return }
        items = rows
        header.text = count
        var snapshot = NSDiffableDataSourceSnapshot<Int, String>()
        snapshot.appendSections([0])
        snapshot.appendItems(items.map(\.key))
        snapshot.reconfigureItems(items.map(\.key))
        snapshot.reloadSections([0])
        let (focus, landing) = held
        let land = landed.map { landing > $0 } ?? false
        landed = landing
        dataSource.apply(snapshot, animatingDifferences: false) { [weak self] in
            guard let self else { return }
            self.reselect()
            guard land, let focus, let index = self.items.firstIndex(where: { $0.key == focus }) else { return }
            let path = IndexPath(item: index, section: 0)
            self.collectionView.scrollToItem(at: path, at: .centeredVertically, animated: false)
            self.collectionView.layoutIfNeeded()
            UIAccessibility.post(notification: .layoutChanged, argument: self.collectionView.cellForItem(at: path))
        }
    }

    /// Selects `selectedKey`, if it is shown.
    func reselect() {
        guard let key = selectedKey, let index = items.firstIndex(where: { $0.key == key }) else { return }
        collectionView.selectItem(at: IndexPath(item: index, section: 0), animated: false, scrollPosition: [])
    }

    /// Runs one of a row's actions, the app's: held where this list holds its rows, then run.
    func perform(_ action: LumennaApp.Action, on item: AppItem) {
        core.app.run(action, on: item.key, side: side)
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        guard let key = dataSource.itemIdentifier(for: path), let item = items.first(where: { $0.key == key }) else { return }
        open(item)
    }

    /// Undoes this device's last change to the store, wherever it was made.
    @objc func undoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canUndo { typing.undo(); return }
        core.app.send(.undo)
    }

    @objc func redoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canRedo { typing.redo(); return }
        core.app.send(.redo)
    }

    // Undo and Redo from the keyboard come here (`KeyboardCommands`).
    override var canBecomeFirstResponder: Bool { true }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(undoChange) || action == #selector(redoChange) { return offersUndo }
        return super.canPerformAction(action, withSender: sender)
    }
}

// The row's actions, apart from the layout that asks for them, so a test can ask too.
extension AppListViewController {
    /// The item's actions as the list offers them (`AppRowActions`), Expand or Collapse among them.
    func rowActions(for item: AppItem) -> AppRowActions {
        AppRowActions(actions: item.actions) { [weak self] action in self?.perform(action, on: item) }
    }

    private func item(at path: IndexPath) -> AppItem? {
        guard let key = dataSource.itemIdentifier(for: path) else { return nil }
        return items.first { $0.key == key }
    }

    /// What a trailing swipe on the row at `path` offers: the primary actions, then Expand or
    /// Collapse.
    func trailingSwipeActions(at path: IndexPath) -> UISwipeActionsConfiguration? {
        item(at: path).flatMap { rowActions(for: $0).trailingSwipe() }
    }

    /// The cell's other actions, for VoiceOver, Switch Control and Full Keyboard Access.
    func customActions(at path: IndexPath) -> [UIAccessibilityCustomAction] {
        item(at: path).map { rowActions(for: $0).customActions } ?? []
    }

    /// What a long press on the row at `path` offers: every action.
    func menu(at path: IndexPath) -> UIMenu? {
        item(at: path).flatMap { rowActions(for: $0).menu }
    }

    func collectionView(
        _ collectionView: UICollectionView, contextMenuConfigurationForItemsAt paths: [IndexPath], point: CGPoint
    ) -> UIContextMenuConfiguration? {
        guard paths.count == 1, let item = item(at: paths[0]) else { return nil }
        return rowActions(for: item).contextMenu
    }
}

extension AppModel {
    /// The sidebar's rows as a list shows them, not folded away, each saying its detail, whether
    /// it is folded and, against the row shown before, its level — what a tree would say itself.
    /// `under` limits them to one heading's places, a level up.
    func sidebarItems(under group: LumennaApp.SidebarGroup? = nil) -> [AppItem] {
        // The rows the app has collapsed: what is deeper under each is left out.
        let folded = Set(view.sidebarFolded)
        var items: [AppItem] = []
        var inside = group == nil
        var previous: UInt32 = 0
        var hiddenBelow: UInt32?
        for entry in view.sidebar {
            if let group {
                if entry.depth == 0 {
                    if case let .group(heading) = entry.kind { inside = heading == group } else { inside = false }
                    hiddenBelow = nil
                    continue
                }
                guard inside else { continue }
            }
            if let below = hiddenBelow {
                if entry.depth > below { continue }
                hiddenBelow = nil
            }
            if folded.contains(entry.key) { hiddenBelow = entry.depth }
            let depth = group == nil ? entry.depth : entry.depth - 1
            let expanded: Bool? = entry.actions.contains { $0.kind == .collapse } ? true
                : entry.actions.contains { $0.kind == .expand } ? false : nil
            let level = depth != previous ? "level \(depth + 1)" : nil
            previous = depth
            var item = AppItem(key: entry.key, title: entry.title, detail: entry.detail.isEmpty ? nil : entry.detail, depth: depth, actions: entry.actions)
            item.spoken = AppModel.spoken(detail: entry.detail, expanded: expanded, level: level)
            if case .group = entry.kind { item.heading = true }
            items.append(item)
        }
        return items
    }

    /// The sidebar entry held by `key`.
    func sidebarEntry(_ key: String) -> LumennaApp.SidebarEntry? {
        view.sidebar.first { $0.key == key }
    }
}
