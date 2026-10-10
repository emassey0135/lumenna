import UIKit

/// One row of a simple list: a project, a label, a filter, a device.
struct Item: Hashable {
    /// What identifies it to the core — for most things here, its name.
    var key: String
    var title: String
    var detail: String?
    var depth: UInt32 = 0
    /// What VoiceOver says after the title.
    var spoken: String?
    /// A heading over the items under it, as the sidebar's Projects and Labels are.
    var heading = false
    /// What can be done to it, as the core says, offered as `RowActions` offers them.
    var actions: [Action] = []
}

/// A plain list with the same focus rules as the task list: after a change, VoiceOver stays
/// on the item if it is still there, or moves to whatever now holds its place.
///
/// Subclasses say what to list and what can be done; this does the rest.
class ItemListViewController: UIViewController, UICollectionViewDelegate {
    let core: Core
    /// The items shown, once folded, each saying its fold state and level.
    private(set) var items: [Item] = []
    /// Every item loaded, before folding.
    private var listed: [Item] = []
    private var shown: [Folding.Shown<Item>] = []
    private var folding = Folding()
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, Item>!
    private let header = UILabel()

    init(core: Core, title: String) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        self.title = title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    // MARK: - What subclasses say

    /// The items, and a line saying how many there are. Thrown errors are shown.
    func load() throws -> (items: [Item], count: String) { ([], "") }
    /// What to say in place of the count when nothing is listed, the core's (`Rows.empty`),
    /// set by `load`; empty leaves the count.
    var empty = ""
    /// What can be done to an item: its own actions, from the core.
    func actions(for item: Item) -> [Action] { item.actions }
    /// Opens the app's own form an action asks for (`Question.form`).
    func form(_ action: Action, on item: Item) {}
    /// The key of the item a place named `name` is listed under, for focus after a rename.
    func key(forName name: String, subject: Subject) -> String { name }
    /// After an action changed something: reloads, keeping focus on the item, or the one it
    /// became, or whatever holds its place, and says what happened.
    func acted(_ action: Action, on item: Item, answer: Answer, change: Change) {
        let renamed = ActionRun.name(after: action, answer: answer).map { key(forName: $0, subject: action.subject) }
        reload(focusing: renamed ?? item.key, saying: change)
    }
    /// What tapping does.
    func open(_ item: Item) {}
    /// What the add button does; `nil` hides it.
    var addTitle: String? { nil }
    func add() {}
    /// Whether Undo and Redo are offered: on every screen that changes the store, so undoing
    /// a rename never means going to another tab first. Not in a picker sheet.
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
        let cell = UICollectionView.CellRegistration<UICollectionViewListCell, Item> { [weak self] cell, _, item in
            var content = sidebar
                ? (item.heading ? UIListContentConfiguration.sidebarHeader() : .sidebarSubtitleCell())
                : UIListContentConfiguration.subtitleCell()
            content.text = item.title
            content.textProperties.numberOfLines = 0
            content.secondaryText = item.detail
            content.secondaryTextProperties.color = .quietLabel
            content.secondaryTextProperties.numberOfLines = 0
            content.directionalLayoutMargins.leading += CGFloat(item.depth) * 20
            if sidebar { content.textProperties.color = .label }
            cell.contentConfiguration = content
            cell.accessories = sidebar ? [] : [.disclosureIndicator(displayed: .always)]
            // Said explicitly. Left to itself the cell's label is its text and its detail
            // together, so a value of the detail says it twice: "Projects, 1 project 1 project".
            cell.isAccessibilityElement = true
            cell.accessibilityLabel = item.title
            cell.accessibilityValue = item.spoken ?? item.detail
            cell.accessibilityTraits = item.heading ? [.header, .button] : .button
            // Only what is not swiped, which UIKit adds to the swipe actions.
            cell.accessibilityCustomActions = self?.rowActions(for: item).customActions
        }
        let headerRegistration = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(
            elementKind: UICollectionView.elementKindSectionHeader
        ) { [weak self] header, _, _ in
            var content = header.defaultContentConfiguration()
            content.text = self?.header.text
            header.contentConfiguration = content
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) {
            collectionView, path, item in
            collectionView.dequeueConfiguredReusableCell(using: cell, for: path, item: item)
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
            let button = UIBarButtonItem(
                systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.add() }
            )
            button.accessibilityLabel = addTitle
            navigationItem.rightBarButtonItem = button
        }
        NotificationCenter.default.addObserver(
            self, selector: #selector(storeChanged), name: Core.changed, object: nil
        )
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if selectedKey == nil, let selected = collectionView.indexPathsForSelectedItems?.first {
            collectionView.deselectItem(at: selected, animated: animated)
        }
        reload()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        takeKeyboardCommands()
    }

    @objc private func storeChanged() {
        reload()
    }

    /// Lists again. With `focusing`, puts VoiceOver on that item — or, if it is gone, on
    /// whatever now holds the place it had.
    func reload(focusing key: String? = nil, saying change: Change? = nil) {
        let previous = key.flatMap { key in items.firstIndex { $0.key == key } }
        do {
            let loaded = try load()
            listed = loaded.items
            header.text = loaded.count
        } catch {
            header.text = error.sentence
        }
        refold()
        // With nothing listed, what is empty, in the core's words, in place of a count.
        if items.isEmpty, !empty.isEmpty { header.text = empty }
        var snapshot = NSDiffableDataSourceSnapshot<Int, Item>()
        snapshot.appendSections([0])
        snapshot.appendItems(items)
        snapshot.reloadSections([0])
        dataSource.apply(snapshot, animatingDifferences: false) { [weak self] in
            self?.reselect()
            guard let self, let change else { return }
            let index = key.flatMap { key in self.items.firstIndex { $0.key == key } }
                ?? previous.map { min($0, self.items.count - 1) }
            if let index, index >= 0 {
                let path = IndexPath(item: index, section: 0)
                self.collectionView.scrollToItem(at: path, at: .centeredVertically, animated: false)
                self.collectionView.layoutIfNeeded()
                UIAccessibility.post(
                    notification: .layoutChanged, argument: self.collectionView.cellForItem(at: path)
                )
            }
            Announcer.say(change.announcement, notices: change.notices)
        }
    }

    /// Folds the loaded items into those shown, each saying whether it is collapsed and,
    /// against the item shown before it, its level.
    private func refold() {
        shown = folding.shown(listed, depth: { Int($0.depth) }, key: \.key)
        items = shown.indices.map { index in
            var item = shown[index].item
            let parts = [item.spoken ?? item.detail, shown[index].state, Folding.levelChange(shown, at: index)]
            item.spoken = parts.compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: ", ")
            return item
        }
    }

    /// Selects `selectedKey`, if it is shown.
    func reselect() {
        guard let key = selectedKey, let index = items.firstIndex(where: { $0.key == key }) else { return }
        collectionView.selectItem(at: IndexPath(item: index, section: 0), animated: false, scrollPosition: [])
    }

    /// Folds or unfolds the item `key`, as its Expand or Collapse action does.
    func toggleFold(_ key: String) {
        guard let row = shown.first(where: { $0.item.key == key }), row.parent else { return }
        fold(key, saying: row.collapsed ? "Expanded" : "Collapsed")
    }

    /// Folds or unfolds the item `key`, keeping VoiceOver on it.
    private func fold(_ key: String, saying said: String) {
        folding.toggle(key)
        refold()
        var snapshot = NSDiffableDataSourceSnapshot<Int, Item>()
        snapshot.appendSections([0])
        snapshot.appendItems(items)
        dataSource.apply(snapshot, animatingDifferences: false) { [weak self] in
            self?.reselect()
            guard let self, let index = self.items.firstIndex(where: { $0.key == key }) else { return }
            let path = IndexPath(item: index, section: 0)
            UIAccessibility.post(notification: .layoutChanged, argument: self.collectionView.cellForItem(at: path))
            Announcer.say(said)
        }
    }

    /// Runs an operation on an item, then reloads with focus kept near it.
    func perform(on item: Item, renamedTo newKey: String? = nil, _ operation: () throws -> Change) {
        do {
            let change = try operation()
            reload(focusing: newKey ?? item.key, saying: change)
        } catch {
            showFailure(error.sentence)
        }
    }

    /// Runs one of an item's actions: asks its question, then keeps focus by `acted`.
    func perform(_ action: Action, on item: Item) {
        run(action, core: core, form: { [weak self] action in self?.form(action, on: item) }) { [weak self] change, answer in
            self?.acted(action, on: item, answer: answer, change: change)
        }
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        guard let item = dataSource.itemIdentifier(for: path) else { return }
        open(item)
    }

    /// Undoes this device's last change to the store, wherever it was made.
    @objc func undoChange() {
        storeChange { try $0.undo() }
    }

    @objc func redoChange() {
        storeChange { try $0.redo() }
    }

    private func storeChange(_ operation: (Lumenna) throws -> Change) {
        do {
            let change = try operation(core.lumenna)
            // Every screen showing the store reads it again, this one included.
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            showFailure(error.sentence)
        }
    }

    // Undo and Redo from the keyboard come here (`KeyboardCommands`); ⌘N is New Task
    // everywhere, as on the Mac, and New Project and the others are menu items of their own.
    override var canBecomeFirstResponder: Bool { true }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(undoChange) || action == #selector(redoChange) { return offersUndo }
        return super.canPerformAction(action, withSender: sender)
    }
}

// The row's actions, apart from the layout that asks for them, so a test can ask too.
extension ItemListViewController {
    /// The item's actions as the list offers them (`RowActions`), with Expand or Collapse.
    func rowActions(for item: Item) -> RowActions {
        let fold = shown.first(where: { $0.item.key == item.key }).flatMap { row in
            folding.action(for: row, key: item.key) { [weak self] key, said in self?.fold(key, saying: said) }
        }
        return RowActions(actions: actions(for: item), fold: fold) { [weak self] action in self?.perform(action, on: item) }
    }

    /// What a trailing swipe on the row at `path` offers: the primary actions, then Expand or
    /// Collapse.
    func trailingSwipeActions(at path: IndexPath) -> UISwipeActionsConfiguration? {
        dataSource.itemIdentifier(for: path).flatMap { rowActions(for: $0).trailingSwipe() }
    }

    /// The cell's other actions, for VoiceOver, Switch Control and Full Keyboard Access.
    func customActions(at path: IndexPath) -> [UIAccessibilityCustomAction] {
        dataSource.itemIdentifier(for: path).map { rowActions(for: $0).customActions } ?? []
    }

    /// What a long press on the row at `path` offers: every action.
    func menu(at path: IndexPath) -> UIMenu? {
        dataSource.itemIdentifier(for: path).flatMap { rowActions(for: $0).menu }
    }

    func collectionView(
        _ collectionView: UICollectionView, contextMenuConfigurationForItemsAt paths: [IndexPath], point: CGPoint
    ) -> UIContextMenuConfiguration? {
        guard paths.count == 1, let item = dataSource.itemIdentifier(for: paths[0]) else { return nil }
        return rowActions(for: item).contextMenu
    }
}
