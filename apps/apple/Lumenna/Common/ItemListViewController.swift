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
}

/// What can be done to an item: a swipe action, which UIKit also offers to VoiceOver, Switch
/// Control and Full Keyboard Access as an action.
struct ItemAction {
    var title: String
    var destructive = false
    var run: (Item) -> Void
}

/// A plain list with the same focus rules as the task list: after a change, VoiceOver stays
/// on the item if it is still there, or moves to whatever now holds its place.
///
/// Subclasses say what to list and what can be done; this does the rest.
class ItemListViewController: UIViewController, UICollectionViewDelegate {
    let core: Core
    private(set) var items: [Item] = []
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
    /// What a trailing swipe offers, most often used first.
    func actions(for item: Item) -> [ItemAction] { [] }
    /// What tapping does.
    func open(_ item: Item) {}
    /// What the add button does; `nil` hides it.
    var addTitle: String? { nil }
    func add() {}
    /// Whether Undo and Redo are offered: on every screen that changes the store, so undoing
    /// a rename never means going to another tab first. Not in a picker sheet.
    var offersUndo: Bool { true }

    // MARK: - Doing it

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        var configuration = UICollectionLayoutListConfiguration(appearance: .insetGrouped)
        configuration.headerMode = .supplementary
        configuration.trailingSwipeActionsConfigurationProvider = { [weak self] path in
            guard let self, let item = self.dataSource.itemIdentifier(for: path) else { return nil }
            let actions = self.actions(for: item).map { action in
                UIContextualAction(
                    style: action.destructive ? .destructive : .normal, title: action.title
                ) { _, _, finished in
                    action.run(item)
                    finished(true)
                }
            }
            return actions.isEmpty ? nil : UISwipeActionsConfiguration(actions: actions)
        }
        collectionView = UICollectionView(
            frame: view.bounds,
            collectionViewLayout: UICollectionViewCompositionalLayout.list(using: configuration)
        )
        collectionView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        collectionView.delegate = self
        view.addSubview(collectionView)

        let cell = UICollectionView.CellRegistration<UICollectionViewListCell, Item> { cell, _, item in
            var content = UIListContentConfiguration.subtitleCell()
            content.text = item.title
            content.textProperties.numberOfLines = 0
            content.secondaryText = item.detail
            content.secondaryTextProperties.color = .quietLabel
            content.secondaryTextProperties.numberOfLines = 0
            content.directionalLayoutMargins.leading += CGFloat(item.depth) * 20
            cell.contentConfiguration = content
            cell.accessories = [.disclosureIndicator(displayed: .always)]
            // Said explicitly. Left to itself the cell's label is its text and its detail
            // together, so a value of the detail says it twice: "Projects, 1 project 1 project".
            cell.isAccessibilityElement = true
            cell.accessibilityLabel = item.title
            cell.accessibilityValue = item.spoken ?? item.detail
            cell.accessibilityTraits = .button
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
        if let selected = collectionView.indexPathsForSelectedItems?.first {
            collectionView.deselectItem(at: selected, animated: animated)
        }
        reload()
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
            items = loaded.items
            header.text = loaded.count
        } catch {
            header.text = error.sentence
        }
        var snapshot = NSDiffableDataSourceSnapshot<Int, Item>()
        snapshot.appendSections([0])
        snapshot.appendItems(items)
        snapshot.reloadSections([0])
        dataSource.apply(snapshot, animatingDifferences: false) { [weak self] in
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

    /// Runs an operation on an item, then reloads with focus kept near it.
    func perform(on item: Item, renamedTo newKey: String? = nil, _ operation: () throws -> Change) {
        do {
            let change = try operation()
            reload(focusing: newKey ?? item.key, saying: change)
        } catch {
            showFailure(error.sentence)
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

    override var canBecomeFirstResponder: Bool { true }

    override var keyCommands: [UIKeyCommand]? {
        var commands: [UIKeyCommand] = []
        if let addTitle {
            commands.append(UIKeyCommand(title: addTitle, action: #selector(addFromKeyboard), input: "n", modifierFlags: .command))
        }
        if offersUndo {
            commands.append(UIKeyCommand(title: "Undo", action: #selector(undoChange), input: "z", modifierFlags: .command))
            commands.append(UIKeyCommand(title: "Redo", action: #selector(redoChange), input: "z", modifierFlags: [.command, .shift]))
        }
        return commands
    }

    @objc private func addFromKeyboard() {
        add()
    }
}
