import UIKit

/// Tasks, with a filter field above them.
///
/// UIKit owns the structure — the collection view, focus, and the keyboard. The reason
/// is focus: after a mutation re-sorts the list, VoiceOver is put on a row chosen here,
/// deterministically, rather than wherever a re-render leaves it.
///
/// Rows are UIKit's own list content too, not SwiftUI in a `UIHostingConfiguration`. Hosted
/// text is invisible to the accessibility audit's Dynamic Type checks and is reported clipped
/// when the size changes at run time; the stock configuration passes both.
final class TaskListViewController: UIViewController {
    /// What the list is for.
    enum Mode {
        /// Tasks to do.
        case tasks
        /// The trash: restore, or delete from the trash.
        case trash
    }

    private let core: Core
    private let mode: Mode
    /// What quick add starts with — `#Work ` in a project's list, so a task added there
    /// lands there.
    private let quickAddPrefix: String
    /// Every row listed, and those shown once folded.
    private var listed: [RowView] = []
    private var shown: [Folding.Shown<RowView>] = []
    private var rows: [RowView] = []
    private var folding = Folding()

    private let filterField = LineEntry(name: "Filter")
    private let readback = UILabel()
    private lazy var completions = CompletionBar(core: core, syntax: .filter, field: filterField)
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, String>!

    init(core: Core, title: String = "Tasks", query: String = "", mode: Mode = .tasks, quickAddPrefix: String = "") {
        self.core = core
        self.mode = mode
        self.quickAddPrefix = quickAddPrefix
        super.init(nibName: nil, bundle: nil)
        self.title = title
        filterField.text = query
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    // MARK: - Building the view

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        buildFilter()
        buildList()
        buildBars()
        NotificationCenter.default.addObserver(
            self, selector: #selector(storeChanged), name: Core.changed, object: nil
        )
        reload()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        // Back from a task's details, which may have changed it.
        if let selected = collectionView.indexPathsForSelectedItems?.first {
            collectionView.deselectItem(at: selected, animated: animated)
        }
        reload()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        takeKeyboardCommands()
        if filterAsked {
            filterAsked = false
            filterField.becomeFirstResponder()
        }
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
            guard let self, let row = self.rows.first(where: { $0.id == id }) else { return }
            self.configure(cell, with: row, at: path.item)
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) {
            collectionView, path, id in
            collectionView.dequeueConfiguredReusableCell(using: registration, for: path, item: id)
        }

        let header = UIStackView(arrangedSubviews: [filterField, readback])
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
        navigationItem.leftBarButtonItems = mode == .tasks ? [undo, redo] : [undo]
        guard mode == .tasks else { return }
        let add = UIBarButtonItem(
            systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.addTask() }
        )
        add.accessibilityLabel = "Add task"
        navigationItem.rightBarButtonItem = add
    }

    /// What a row says, and what can be done to it without seeing it.
    private func configure(_ cell: UICollectionViewListCell, with row: RowView, at index: Int) {
        cell.contentConfiguration = Self.content(for: row)
        cell.accessories = [.disclosureIndicator(displayed: .always)]

        let previous = index > 0 && index - 1 < rows.count ? rows[index - 1].depth : nil
        cell.isAccessibilityElement = true
        cell.accessibilityLabel = RowSpeech.label(row)
        let fold = index < shown.count ? shown[index].state : nil
        cell.accessibilityValue = RowSpeech.value(row, previousDepth: previous, fold: fold)
        cell.accessibilityHint = mode == .trash ? nil : "Shows details"
        cell.accessibilityTraits = .button
        // No custom actions here: UIKit already offers the swipe actions to VoiceOver, Switch
        // Control and Full Keyboard Access, and actions set on the cell are added to those
        // rather than replacing them, so each would be listed twice.
    }

    /// What a row looks like: the title, then the due date and notable states beneath it.
    private static func content(for row: RowView) -> UIListContentConfiguration {
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
        var detail = RowSpeech.details(row).map { [$0] } ?? []
        detail += row.state.filter { $0 != "ready" }
        if !detail.isEmpty {
            content.secondaryText = detail.joined(separator: ", ")
            content.secondaryTextProperties.font = .preferredFont(forTextStyle: .subheadline)
            // Overdue is said in words as well as colour, never colour alone.
            content.secondaryTextProperties.color =
                row.state.contains("overdue") ? .warningLabel : .quietLabel
            content.secondaryTextProperties.numberOfLines = 0
        }
        // Depth shown by indentation, for sight only; VoiceOver hears it in words (`RowSpeech`).
        content.directionalLayoutMargins.leading += CGFloat(row.depth) * 20
        return content
    }

    // MARK: - Showing the store

    private func row(at path: IndexPath) -> RowView? {
        guard let id = dataSource.itemIdentifier(for: path) else { return nil }
        return rows.first { $0.id == id }
    }

    /// Lists again and redraws. A filter still being typed may not read yet; then the old rows
    /// stay and the readback says what is wrong with it.
    private func reload(then finished: (() -> Void)? = nil) {
        let query = filterField.text ?? ""
        do {
            let listing = try core.lumenna.listTasks(query: query)
            listed = listing.rows
            var said = [listing.announcement] + listing.notices
            if let understood = listing.query?.description {
                said.insert(understood, at: 0)
            }
            readback.text = said.joined(separator: ". ")
        } catch {
            readback.text = error.sentence
        }
        apply(then: finished)
    }

    /// Shows the listed rows as folded, redrawing each in place.
    private func apply(then finished: (() -> Void)? = nil) {
        shown = folding.shown(listed, depth: { Int($0.depth) }, key: \.id)
        rows = shown.map(\.item)
        var snapshot = NSDiffableDataSourceSnapshot<Int, String>()
        snapshot.appendSections([0])
        snapshot.appendItems(rows.map(\.id))
        // A row that changed keeps its identity and is redrawn in place, which is what lets
        // VoiceOver stay on it.
        snapshot.reconfigureItems(rows.map(\.id))
        dataSource.apply(
            snapshot,
            animatingDifferences: !UIAccessibility.isReduceMotionEnabled,
            completion: finished
        )
    }

    /// Reloads, puts VoiceOver focus somewhere predictable, and says what happened.
    ///
    /// Focus goes to `id` if it is still listed; otherwise to whatever now occupies the row
    /// it was in, so completing a task that leaves the list lands on the next one.
    private func reload(focusing id: String?, near index: Int?, saying change: Change) {
        reload { [weak self] in
            guard let self else { return }
            let target = id.flatMap { id in self.rows.firstIndex { $0.id == id } }
                ?? index.map { min($0, self.rows.count - 1) }
            if let target, target >= 0 {
                let path = IndexPath(item: target, section: 0)
                self.collectionView.scrollToItem(at: path, at: .centeredVertically, animated: false)
                self.collectionView.layoutIfNeeded()
                UIAccessibility.post(
                    notification: .layoutChanged,
                    argument: self.collectionView.cellForItem(at: path)
                )
            }
            Announcer.say(change.announcement, notices: change.notices)
        }
    }

    /// Folds or unfolds the row `key`, keeping VoiceOver on it.
    private func fold(_ key: String, saying said: String) {
        folding.toggle(key)
        apply { [weak self] in
            guard let self, let index = self.rows.firstIndex(where: { $0.id == key }) else { return }
            let path = IndexPath(item: index, section: 0)
            UIAccessibility.post(notification: .layoutChanged, argument: self.collectionView.cellForItem(at: path))
            Announcer.say(said)
        }
    }

    @objc private func storeChanged() {
        reload()
    }

    private func filterChanged() {
        reload()
        completions.update()
    }

    private func filterSubmitted() {
        filterField.resignFirstResponder()
        // Typing is silent; finishing the filter says what it found.
        Announcer.say(readback.text ?? "")
    }

    // MARK: - Doing things

    /// Runs an operation, then reloads with focus on `id` or near `index`.
    private func perform(focusing id: String?, near index: Int?, _ operation: () throws -> Change) {
        do {
            let change = try operation()
            reload(focusing: id, near: index, saying: change)
        } catch {
            showFailure(error.sentence)
        }
    }

    /// Runs one of a row's actions, the core's, then keeps focus on the task, or — when it
    /// left the list, to the trash or back from it — on whatever now holds its place.
    private func perform(_ action: Action, on row: RowView) {
        let index = rows.firstIndex { $0.id == row.id }
        let leaves = [.delete, .restore, .deleteForGood].contains(action.kind)
        run(action, core: core, form: { [weak self] _ in self?.open(row) }) { [weak self] change, _ in
            self?.reload(focusing: leaves ? nil : row.id, near: index, saying: change)
        }
    }

    /// Edit Details: the task's own screen, beside the list on iPad.
    private func open(_ row: RowView) {
        showBeside(TaskDetailViewController(core: core, id: row.id))
    }

    @objc func undoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canUndo { typing.undo(); return }
        perform(focusing: nil, near: nil) { try core.lumenna.undo() }
    }

    @objc func redoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canRedo { typing.redo(); return }
        perform(focusing: nil, near: nil) { try core.lumenna.redo() }
    }

    @objc func addTask() {
        guard mode == .tasks else { return }
        let adding = QuickAddViewController(core: core, initial: quickAddPrefix) { [weak self] change in
            self?.reload(focusing: change.task?.id, near: nil, saying: change)
        }
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
    private var rowInHand: RowView? {
        let focused = UIFocusSystem.focusSystem(for: view)?.focusedItem as? UIView
        let spoken = UIAccessibility.focusedElement(using: .notificationVoiceOver) as? UIView
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
            mode == .tasks
        default:
            super.canPerformAction(action, withSender: sender)
        }
    }

    // Each command is the row's own action of that kind, so a key never does what the row's
    // actions do not.
    // A row without one (a trashed task has no Mark Done) says why, in the core's words.
    @objc func toggleDone() {
        guard let row = rowInHand else { return }
        if let action = row.actions.first(.markDone, .markNotDone) {
            perform(action, on: row)
        } else {
            Announcer.say(notOffered(kind: .markDone, subject: .task, thisDevice: false))
        }
    }

    @objc func moveToTrash() {
        guard let row = rowInHand else { return }
        if let action = row.actions.first(.delete) {
            perform(action, on: row)
        } else {
            Announcer.say(notOffered(kind: .delete, subject: .task, thisDevice: false))
        }
    }

    @objc func filterTasks() {
        focusFilter()
    }
}

extension TaskListViewController: UICollectionViewDelegate {
    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        guard mode == .tasks, let row = row(at: path) else { return }
        open(row)
    }
}

// The swipe actions, apart from the layout that asks for them, so a test can ask too.
extension TaskListViewController {
    /// What a leading swipe on the row at `path` offers: Mark Done or Mark Not Done, the
    /// row's first action. VoiceOver lists it first among the row's actions.
    func leadingSwipeActions(at path: IndexPath) -> UISwipeActionsConfiguration? {
        guard let row = self.row(at: path), let done = row.actions.first, [.markDone, .markNotDone].contains(done.kind) else { return nil }
        return UISwipeActionsConfiguration(actions: [swipeAction(done) { [weak self] in self?.perform(done, on: row) }])
    }

    /// What a trailing swipe on the row at `path` offers: the rest of the row's actions, in
    /// the core's order, then Expand or Collapse.
    func trailingSwipeActions(at path: IndexPath) -> UISwipeActionsConfiguration? {
        guard let row = self.row(at: path) else { return nil }
        let leading = leadingSwipeActions(at: path) == nil ? 0 : 1
        var actions = row.actions.dropFirst(leading).map { action in
            swipeAction(action) { [weak self] in self?.perform(action, on: row) }
        }
        if let fold = self.shown.first(where: { $0.item.id == row.id }).flatMap({ shown in
            self.folding.action(for: shown, key: row.id) { [weak self] key, said in self?.fold(key, saying: said) }
        }) {
            actions.append(fold)
        }
        return actions.isEmpty ? nil : UISwipeActionsConfiguration(actions: actions)
    }
}
