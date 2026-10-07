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
        configuration.leadingSwipeActionsConfigurationProvider = { [weak self] path in
            guard let self, self.mode == .tasks, let row = self.row(at: path) else { return nil }
            let action = UIContextualAction(
                // The title is also the action's name to VoiceOver, so it says what it does.
                style: .normal, title: row.checked == true ? "Mark Not Done" : "Mark Done"
            ) { [weak self] _, _, finished in
                self?.toggleDone(row)
                finished(true)
            }
            action.backgroundColor = .systemGreen
            return UISwipeActionsConfiguration(actions: [action])
        }
        configuration.trailingSwipeActionsConfigurationProvider = { [weak self] path in
            guard let self, let row = self.row(at: path) else { return nil }
            if self.mode == .trash {
                let restore = UIContextualAction(style: .normal, title: "Restore") {
                    [weak self] _, _, finished in
                    self?.restore(row)
                    finished(true)
                }
                let erase = UIContextualAction(style: .destructive, title: "Delete from Trash") {
                    [weak self] _, _, finished in
                    self?.erase(row)
                    finished(true)
                }
                return UISwipeActionsConfiguration(actions: [restore, erase])
            }
            let action = UIContextualAction(style: .destructive, title: "Delete") {
                [weak self] _, _, finished in
                self?.trash(row)
                finished(true)
            }
            let fold = self.shown.first { $0.item.id == row.id }.flatMap { shown in
                self.folding.action(for: shown, key: row.id) { [weak self] key, said in self?.fold(key, saying: said) }
            }
            return UISwipeActionsConfiguration(actions: [action] + [fold].compactMap { $0 })
        }
        collectionView = UICollectionView(
            frame: .zero,
            collectionViewLayout: UICollectionViewCompositionalLayout.list(using: configuration)
        )
        collectionView.delegate = self
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
        let undo = UIBarButtonItem.undo { [weak self] in self?.undo() }
        let redo = UIBarButtonItem.redo { [weak self] in self?.redo() }
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
        var detail = row.value.map { [$0] } ?? []
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

    private func toggleDone(_ row: RowView) {
        let index = rows.firstIndex { $0.id == row.id }
        perform(focusing: row.id, near: index) {
            row.checked == true
                ? try core.lumenna.uncompleteTask(id: row.id)
                : try core.lumenna.completeTask(id: row.id)
        }
    }

    private func trash(_ row: RowView) {
        let index = rows.firstIndex { $0.id == row.id }
        perform(focusing: nil, near: index) { try core.lumenna.trashTask(id: row.id) }
    }

    private func restore(_ row: RowView) {
        let index = rows.firstIndex { $0.id == row.id }
        perform(focusing: nil, near: index) { try core.lumenna.restoreTask(id: row.id) }
    }

    /// Deletes a task from the trash, asking first.
    private func erase(_ row: RowView) {
        confirm(
            "Delete \(row.title) from the trash?",
            message: "Undo can bring it back. It also stays in the history every device keeps, and in backups.",
            action: "Delete"
        ) { [weak self] in
            guard let self else { return }
            let index = self.rows.firstIndex { $0.id == row.id }
            self.perform(focusing: nil, near: index) { try self.core.lumenna.eraseTask(id: row.id) }
        }
    }

    @objc private func undo() {
        perform(focusing: nil, near: nil) {
            let change = try core.lumenna.undo()
            return change
        }
    }

    @objc private func redo() {
        perform(focusing: nil, near: nil) { try core.lumenna.redo() }
    }

    @objc func addTask() {
        guard mode == .tasks else { return }
        let adding = QuickAddViewController(core: core, initial: quickAddPrefix) { [weak self] change in
            self?.reload(focusing: change.task?.id, near: nil, saying: change)
        }
        present(UINavigationController(rootViewController: adding), animated: true)
    }

    @objc private func focusFilter() {
        filterField.becomeFirstResponder()
    }

    // MARK: - Keyboard

    override var canBecomeFirstResponder: Bool { true }

    override var keyCommands: [UIKeyCommand]? {
        [
            UIKeyCommand(title: "New Task", action: #selector(addTask), input: "n", modifierFlags: .command),
            UIKeyCommand(title: "Filter", action: #selector(focusFilter), input: "f", modifierFlags: .command),
            UIKeyCommand(title: "Undo", action: #selector(undo), input: "z", modifierFlags: .command),
            UIKeyCommand(title: "Redo", action: #selector(redo), input: "z", modifierFlags: [.command, .shift]),
        ]
    }
}

extension TaskListViewController: UICollectionViewDelegate {
    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        guard mode == .tasks, let row = row(at: path) else { return }
        navigationController?.pushViewController(
            TaskDetailViewController(core: core, id: row.id), animated: true
        )
    }
}
