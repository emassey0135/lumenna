import LumennaApp
import UIKit

/// A place's tasks, with a filter field above them, or the trash: the app's list
/// (`ViewModel.list`) while this screen shows its place, which it opens as it comes to the
/// front.
///
/// UIKit owns the structure — the collection view, focus, and the keyboard — and the app owns
/// what it holds: the rows, their order and folding, the row in hand after a change, what each
/// row offers and what to say. After a change the app has moved the row in hand
/// (`ViewModel.landing`), VoiceOver is put on it; a sync or a new minute redraws in place and
/// leaves focus alone.
///
/// Rows are UIKit's own list content too, not SwiftUI in a `UIHostingConfiguration`. Hosted
/// text is invisible to the accessibility audit's Dynamic Type checks and is reported clipped
/// when the size changes at run time; the stock configuration passes both.
final class TaskListViewController: UIViewController {
    private let core: Core
    /// The place this screen shows.
    let place: LumennaApp.Place
    private var trash: Bool { place == .trash }
    /// The rows shown, not folded away, each with its line.
    private var rows: [(item: LumennaApp.Item, line: LumennaApp.Line?)] = []
    /// The app's landing count this screen last moved focus for.
    private var landed: UInt32?
    /// The row in hand when this screen last showed its place, to put back when it opens again.
    private var lastInHand: String?

    private let filterField = LineEntry(name: "Filter")
    private let readback = UILabel()
    private lazy var completions = CompletionBar(core: core, syntax: .filter, field: filterField)
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, String>!
    private var observer: NSObjectProtocol?

    init(core: Core, place: LumennaApp.Place = .tasks) {
        self.core = core
        self.place = place
        super.init(nibName: nil, bundle: nil)
        title = LumennaApp.placeTitle(place: place)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Whether the app is showing this screen's place: only then is its list this screen's.
    private var here: Bool { core.app.view.place == place }

    // MARK: - Building the view

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        buildFilter()
        buildList()
        buildBars()
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if let selected = collectionView.indexPathsForSelectedItems?.first {
            collectionView.deselectItem(at: selected, animated: animated)
        }
        show()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        takeKeyboardCommands()
        if filterAsked {
            filterAsked = false
            filterField.becomeFirstResponder()
        }
    }

    /// Makes this screen's place the app's, unless it is already: coming back from a task's
    /// details keeps the row in hand and the filter, and coming back from another place opens
    /// this one again with them.
    private func show() {
        if !here {
            core.app.send(.open(place))
            let query = filterField.text ?? ""
            if !query.isEmpty { core.app.send(.filter(query)) }
            if let lastInHand { core.app.send(.focus(lastInHand)) }
            // Opening is the screen coming to the front, which VoiceOver says itself: no landing.
            landed = nil
        }
        draw()
    }

    private func buildFilter() {
        filterField.placeholder = "#Work & overdue"
        filterField.autocapitalizationType = .none
        filterField.autocorrectionType = .no
        filterField.spellCheckingType = .no
        filterField.inputAccessoryView = completions
        filterField.changed = { [weak self] in self?.filterChanged() }
        filterField.selectionChanged = { [weak self] in self?.completions.update() }
        filterField.submitted = { [weak self] in self?.filterSubmitted() }

        // The readback: how the query was understood, and how many it found. A mis-read
        // filter shows wrong results silently, and wrong results are invisible.
        readback.font = .preferredFont(forTextStyle: .footnote)
        readback.adjustsFontForContentSizeCategory = true
        readback.textColor = .quietLabel
        readback.numberOfLines = 0
    }

    private func buildList() {
        var configuration = UICollectionLayoutListConfiguration(appearance: .plain)
        configuration.leadingSwipeActionsConfigurationProvider = { [weak self] path in self?.leadingSwipeActions(at: path) }
        configuration.trailingSwipeActionsConfigurationProvider = { [weak self] path in self?.trailingSwipeActions(at: path) }
        collectionView = UICollectionView(
            frame: .zero,
            collectionViewLayout: UICollectionViewCompositionalLayout.list(using: configuration)
        )
        collectionView.delegate = self
        // Rows take keyboard focus on iPad, for the Task menu's commands to act on.
        collectionView.allowsFocus = true
        collectionView.keyboardDismissMode = .onDrag
        collectionView.accessibilityLabel = "Tasks"

        let registration = UICollectionView.CellRegistration<UICollectionViewListCell, String> {
            [weak self] cell, path, id in
            guard let self, let shown = self.rows.first(where: { $0.item.row.id == id }) else { return }
            self.configure(cell, with: shown.item, line: shown.line)
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) {
            collectionView, path, id in
            collectionView.dequeueConfiguredReusableCell(using: registration, for: path, item: id)
        }

        let header = UIStackView(arrangedSubviews: trash ? [readback] : [filterField, readback])
        header.axis = .vertical
        header.spacing = 6
        header.translatesAutoresizingMaskIntoConstraints = false
        collectionView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(header)
        view.addSubview(collectionView)
        let margins = view.layoutMarginsGuide
        NSLayoutConstraint.activate([
            header.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 8),
            header.leadingAnchor.constraint(equalTo: margins.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: margins.trailingAnchor),
            collectionView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 8),
            collectionView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            collectionView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            collectionView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    /// Everything in the navigation bar. A bottom toolbar inside the tab bar sits where the
    /// tab bar floats, so a tap on Undo would land on a tab — for VoiceOver too, which
    /// activates the middle of an element's frame.
    private func buildBars() {
        let undo = UIBarButtonItem.undo { [weak self] in self?.undoChange() }
        let redo = UIBarButtonItem.redo { [weak self] in self?.redoChange() }
        navigationItem.leftItemsSupplementBackButton = true
        navigationItem.leftBarButtonItems = trash ? [undo] : [undo, redo]
        guard !trash else { return }
        let add = UIBarButtonItem(
            systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.addTask() }
        )
        add.accessibilityLabel = "Add task"
        navigationItem.rightBarButtonItem = add
    }

    /// What a row says, and what can be done to it without seeing it.
    private func configure(_ cell: UICollectionViewListCell, with item: LumennaApp.Item, line: LumennaApp.Line?) {
        let row = item.row
        cell.contentConfiguration = Self.content(for: row, detail: line?.detail)
        cell.accessories = trash ? [] : [.disclosureIndicator(displayed: .always)]
        cell.isAccessibilityElement = true
        cell.accessibilityLabel = row.title
        cell.accessibilityValue = AppModel.spoken(detail: line?.detail, expanded: row.expanded, level: item.level)
        cell.accessibilityHint = trash ? nil : "Shows details"
        cell.accessibilityTraits = .button
        // Only what is not swiped: UIKit offers the swipe actions to VoiceOver, Switch Control
        // and Full Keyboard Access already, and adds these to them.
        cell.accessibilityCustomActions = rowActions(for: row).customActions
    }

    /// What a row looks like: the title, then the rest of its line beneath it.
    private static func content(for row: LumennaApp.RowView, detail: String?) -> UIListContentConfiguration {
        var content = UIListContentConfiguration.subtitleCell()
        let done = row.checked == true
        content.image = UIImage(systemName: done ? "checkmark.circle.fill" : "circle")
        content.imageProperties.tintColor = done ? .tintColor : .quietLabel
        content.imageProperties.preferredSymbolConfiguration =
            UIImage.SymbolConfiguration(textStyle: .body)
        content.attributedText = NSAttributedString(
            string: row.title,
            attributes: [
                .font: UIFont.preferredFont(forTextStyle: .body),
                .strikethroughStyle: done ? NSUnderlineStyle.single.rawValue : 0,
            ]
        )
        content.textProperties.numberOfLines = 0
        if let detail, !detail.isEmpty {
            content.secondaryText = detail
            content.secondaryTextProperties.font = .preferredFont(forTextStyle: .subheadline)
            // Overdue is said in words as well as colour, never colour alone.
            content.secondaryTextProperties.color =
                row.state.contains("overdue") ? .warningLabel : .quietLabel
            content.secondaryTextProperties.numberOfLines = 0
        }
        // Depth shown by indentation, for sight only; VoiceOver hears it in words, where it changes.
        content.directionalLayoutMargins.leading += CGFloat(row.depth) * 20
        return content
    }

    // MARK: - Showing the app's list

    private func row(at path: IndexPath) -> LumennaApp.RowView? {
        guard let id = dataSource.itemIdentifier(for: path) else { return nil }
        return rows.first { $0.item.row.id == id }?.item.row
    }

    /// Draws the app's list while the app shows this screen's place, each row redrawn in place,
    /// and moves VoiceOver to the row in hand when the app moved it.
    private func draw() {
        guard isViewLoaded, here, let list = core.app.view.list else { return }
        let view = core.app.view
        let lines = core.app.lines
        title = view.title
        readback.text = lines.readback
        rows = list.items.enumerated()
            .map { index, item in (item, index < lines.list.count ? lines.list[index] : nil) }
            .filter { !$0.item.hidden }
        lastInHand = view.focus
        var snapshot = NSDiffableDataSourceSnapshot<Int, String>()
        snapshot.appendSections([0])
        snapshot.appendItems(rows.map(\.item.row.id))
        // A row that changed keeps its identity and is redrawn in place, which is what lets
        // VoiceOver stay on it.
        snapshot.reconfigureItems(rows.map(\.item.row.id))
        let landing = view.landing
        let land = landed.map { landing > $0 } ?? false
        landed = landing
        dataSource.apply(snapshot, animatingDifferences: !UIAccessibility.isReduceMotionEnabled) { [weak self] in
            guard land, let self, let focus = view.focus else { return }
            self.land(on: focus)
        }
    }

    /// Puts VoiceOver on the row `key`, scrolled into view.
    private func land(on key: String) {
        guard let index = rows.firstIndex(where: { $0.item.row.id == key }) else { return }
        let path = IndexPath(item: index, section: 0)
        collectionView.scrollToItem(at: path, at: .centeredVertically, animated: false)
        collectionView.layoutIfNeeded()
        UIAccessibility.post(notification: .layoutChanged, argument: collectionView.cellForItem(at: path))
    }

    private func filterChanged() {
        core.app.send(.filter(filterField.text ?? ""))
        completions.update()
    }

    private func filterSubmitted() {
        filterField.resignFirstResponder()
        // Typing is silent; finishing the filter says what it found.
        Announcer.say(readback.text ?? "")
    }

    // MARK: - Doing things

    /// Runs one of a row's actions, the app's: the row in hand, then the action, whose question
    /// the window's `AppQuestions` asks.
    private func perform(_ action: LumennaApp.Action, on row: LumennaApp.RowView) {
        core.app.run(action, on: row.id)
    }

    /// A task's own screen, beside the list on iPad.
    private func open(_ row: LumennaApp.RowView) {
        core.app.send(.focus(row.id))
        showBeside(TaskDetailViewController(core: core, id: row.id))
    }

    @objc func undoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canUndo { typing.undo(); return }
        core.app.send(.undo)
    }

    @objc func redoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canRedo { typing.redo(); return }
        core.app.send(.redo)
    }

    @objc func addTask() {
        guard !trash else { return }
        let adding = QuickAddViewController(core: core)
        adding.closed = { [weak self] in self?.takeKeyboardCommands() }
        present(UINavigationController(rootViewController: adding), animated: true)
    }

    /// ⌘F from another tab, before this one is in the window: done once it appears.
    private var filterAsked = false

    func focusFilter(attempt: Int = 0) {
        if view.window != nil, filterField.becomeFirstResponder() {
            filterAsked = false
            return
        }
        // Not in the window yet, or the tab still changing: once it appears, or shortly.
        filterAsked = true
        // Five seconds at most: on a slow machine a tab can take that to show.
        guard attempt < 100 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in
            guard let self, self.filterAsked else { return }
            self.focusFilter(attempt: attempt + 1)
        }
    }

    // MARK: - Keyboard

    // The Task menu's commands act on the row in hand (`KeyboardCommands`).
    override var canBecomeFirstResponder: Bool { true }

    /// The row a command acts on: the one with keyboard focus, else VoiceOver's, else the one
    /// open beside the list.
    private var rowInHand: LumennaApp.RowView? {
        let focused = UIFocusSystem.focusSystem(for: view)?.focusedItem as? UIView
        let spoken = UIAccessibility.voiceOverView
        for view in [focused, spoken].compactMap({ $0 }) {
            let cell = sequence(first: view, next: \.superview).first { $0 is UICollectionViewCell } as? UICollectionViewCell
            if let cell, let path = collectionView.indexPath(for: cell) { return row(at: path) }
        }
        return collectionView.indexPathsForSelectedItems?.first.flatMap(row(at:))
    }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        switch action {
        case #selector(toggleDone as () -> Void), #selector(moveToTrash):
            rowInHand != nil
        case #selector(filterTasks):
            !trash
        default:
            super.canPerformAction(action, withSender: sender)
        }
    }

    // Each command is the row's own action of that kind, so a key never does what the row's
    // actions do not.
    // A row without one (a trashed task has no Mark Done) says why, in the core's words.
    @objc func toggleDone() {
        guard let row = rowInHand else { return }
        if let action = rowActions(for: row).first(.markDone, .markNotDone) {
            perform(action, on: row)
        } else {
            Announcer.say(LumennaApp.notOffered(kind: .markDone, subject: .task, thisDevice: false))
        }
    }

    @objc func moveToTrash() {
        guard let row = rowInHand else { return }
        if let action = rowActions(for: row).first(.delete) {
            perform(action, on: row)
        } else {
            Announcer.say(LumennaApp.notOffered(kind: .delete, subject: .task, thisDevice: false))
        }
    }

    @objc func filterTasks() {
        focusFilter()
    }
}

extension TaskListViewController: UICollectionViewDelegate {
    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        guard !trash, let row = row(at: path) else { return }
        open(row)
    }

    func collectionView(
        _ collectionView: UICollectionView, contextMenuConfigurationForItemsAt paths: [IndexPath], point: CGPoint
    ) -> UIContextMenuConfiguration? {
        guard paths.count == 1, let row = row(at: paths[0]) else { return nil }
        return rowActions(for: row).contextMenu
    }
}

// The row's actions, apart from the layout that asks for them, so a test can ask too.
extension TaskListViewController {
    /// The row's actions, the app's, as the list offers them (`AppRowActions`).
    func rowActions(for row: LumennaApp.RowView) -> AppRowActions {
        AppRowActions(actions: row.actions, doneLeads: true) { [weak self] action in
            self?.perform(action, on: row)
        }
    }

    /// What a leading swipe on the row at `path` offers: Mark Done or Mark Not Done.
    func leadingSwipeActions(at path: IndexPath) -> UISwipeActionsConfiguration? {
        row(at: path).flatMap { rowActions(for: $0).leadingSwipe() }
    }

    /// What a trailing swipe offers: the row's other primary actions, then Expand or Collapse.
    func trailingSwipeActions(at path: IndexPath) -> UISwipeActionsConfiguration? {
        row(at: path).flatMap { rowActions(for: $0).trailingSwipe() }
    }

    /// The cell's other actions, for VoiceOver, Switch Control and Full Keyboard Access.
    func customActions(at path: IndexPath) -> [UIAccessibilityCustomAction] {
        row(at: path).map { rowActions(for: $0).customActions } ?? []
    }

    /// What a long press on the row at `path` offers: every action.
    func menu(at path: IndexPath) -> UIMenu? {
        row(at: path).flatMap { rowActions(for: $0).menu }
    }
}
