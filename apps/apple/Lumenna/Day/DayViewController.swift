import LumennaApp
import UIKit

/// The planner: a day as it is lived, as a list — the app's day (`ViewModel.day`).
///
/// Blocks in time order with their sittings beneath them, free time as rows of its own, and
/// now as a position rather than a highlight. The heading above says what a glance at a
/// timeline would. The day shown, moving between days, what is in hand and folded, and every
/// row's actions and their forms are the app's; opening on today puts VoiceOver on now, not on
/// midnight.
final class DayViewController: UIViewController, UICollectionViewDelegate {
    private let core: Core
    /// The rows shown, not folded away, each with its line.
    private var rows: [(item: LumennaApp.DayItem, line: LumennaApp.Line?)] = []
    /// The app's landing count this screen last moved focus for.
    private var landed: UInt32?
    /// The row in hand when this screen last showed the day, to put back when it opens again.
    private var lastInHand: String?
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, String>!
    private let summary = UILabel()
    private var landedOnNow = false
    private var observer: NSObjectProtocol?
    private weak var dayButtons: UIStackView?
    /// The day's own actions (`DayView.actions`): Plan the Day, Replan from Now, End the Day.
    /// Among the day's buttons, not in the navigation bar: beside Add and Undo there, it
    /// crowded the title until the audit found it clipped in the iPad's narrow column.
    private lazy var dayMenu: UIButton = {
        let button = dayButton("Day Actions") {}
        button.showsMenuAsPrimaryAction = true
        return button
    }()

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Today"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Whether the app is showing the day: only then is its day this screen's.
    private var here: Bool { core.app.view.place == .today }

    // MARK: - Building

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        summary.font = .preferredFont(forTextStyle: .subheadline)
        summary.adjustsFontForContentSizeCategory = true
        summary.textColor = .quietLabel
        summary.numberOfLines = 0
        summary.accessibilityTraits = .header
        // It carries the day's own actions, so it is a target a finger can land on too.
        summary.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        // Moving through days, under the summary rather than in a bottom toolbar, which the
        // floating tab bar would cover.
        let days = UIStackView(arrangedSubviews: [
            dayButton("Previous Day") { [weak self] in self?.move(.previous) },
            dayButton("Now") { [weak self] in self?.move(.now) },
            dayButton("Next Day") { [weak self] in self?.move(.next) },
            dayButton("Go to Day") { [weak self] in self?.chooseDay() },
            dayMenu,
        ])
        days.distribution = .equalSpacing
        dayButtons = days
        let header = UIStackView(arrangedSubviews: [summary, days])
        header.axis = .vertical
        header.spacing = 8
        header.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(header)

        var configuration = UICollectionLayoutListConfiguration(appearance: .plain)
        configuration.trailingSwipeActionsConfigurationProvider = { [weak self] path in self?.trailingSwipeActions(at: path) }
        collectionView = UICollectionView(
            frame: .zero, collectionViewLayout: UICollectionViewCompositionalLayout.list(using: configuration)
        )
        collectionView.delegate = self
        collectionView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(collectionView)
        NSLayoutConstraint.activate([
            header.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 8),
            header.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: view.layoutMarginsGuide.trailingAnchor),
            collectionView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 8),
            collectionView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            collectionView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            collectionView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])

        let cell = UICollectionView.CellRegistration<UICollectionViewListCell, String> { [weak self] cell, _, key in
            guard let self, let row = self.rows.first(where: { $0.item.key == key }) else { return }
            self.configure(cell, row.item, line: row.line)
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) { view, path, key in
            view.dequeueConfiguredReusableCell(using: cell, for: path, item: key)
        }

        let add = UIBarButtonItem(systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.addBlock() })
        add.accessibilityLabel = "Add block"
        navigationItem.rightBarButtonItem = add
        // In the navigation bar, as on the task list: a bottom toolbar would sit under the
        // floating tab bar.
        navigationItem.leftBarButtonItems = [
            .undo { [weak self] in self?.undoChange() },
            .redo { [weak self] in self?.redoChange() },
        ]
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
    }

    /// Side by side while the three fit, one above another once the text is too large for
    /// that — at whatever size that happens, not only the accessibility sizes.
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        guard let days = dayButtons else { return }
        // Each title's width on one line: a wrapping button reports its size at the width it
        // was squeezed to, so once wrapped it would always seem to fit.
        let oneLine = { (view: UIView) -> CGFloat in
            guard let button = view as? UIButton, let label = button.titleLabel, let text = label.text,
                  let font = label.font else { return view.intrinsicContentSize.width }
            let insets = button.configuration?.contentInsets ?? .zero
            return ceil((text as NSString).size(withAttributes: [.font: font]).width) + insets.leading + insets.trailing
        }
        let needed = days.arrangedSubviews.reduce(CGFloat(0)) { $0 + oneLine($1) }
            + days.spacing * CGFloat(days.arrangedSubviews.count - 1)
        let fits = needed <= view.layoutMarginsGuide.layoutFrame.width
        let axis: NSLayoutConstraint.Axis = fits ? .horizontal : .vertical
        if days.axis != axis {
            days.axis = axis
            // Stacked, each spans the width with its title at the start of the line: left to
            // its own width it would keep the narrow one it had side by side, and wrap there.
            days.alignment = .fill
            days.distribution = fits ? .equalSpacing : .fill
            for case let button as UIButton in days.arrangedSubviews {
                button.contentHorizontalAlignment = fits ? .center : .leading
            }
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
        // Opening the day lands on now, not at midnight, which the app puts in hand. Once, so
        // coming back from a detail does not move focus away from where the person was.
        if !landedOnNow {
            landedOnNow = true
            if let focus = core.app.view.focus { land(on: focus) }
        }
    }

    /// Makes the day the app's place, unless it is already: coming back from a task's details
    /// keeps the day shown and the row in hand.
    private func show() {
        if !here {
            core.app.send(.open(.today))
            if let lastInHand { core.app.send(.focus(lastInHand)) }
            landed = nil
        }
        draw()
    }

    private func dayButton(_ title: String, action: @escaping () -> Void) -> UIButton {
        // The configuration's own type scales with Dynamic Type; a font set here would not.
        var configuration = UIButton.Configuration.plain()
        configuration.title = title
        configuration.titleLineBreakMode = .byWordWrapping
        // Narrow sides, so the four fit in a row at the usual text sizes; the tap target
        // stays the full height.
        configuration.contentInsets.leading = 6
        configuration.contentInsets.trailing = 6
        let button = UIButton(configuration: configuration, primaryAction: UIAction { _ in action() })
        return button
    }

    // MARK: - Rows

    /// Draws the app's day while it is shown, each row redrawn in place, and moves VoiceOver to
    /// the row in hand when the app moved it.
    private func draw() {
        guard isViewLoaded, here, let day = core.app.view.day else { return }
        let view = core.app.view
        let lines = core.app.lines
        title = lines.dayTitle
        let worded = day.items.enumerated().map { index, item in (item, index < lines.day.count ? lines.day[index] : nil) }
        // The day's heading, as every app says it: "<day>. <summary>".
        summary.text = worded.first { $0.0.kind == .summary }?.1?.text ?? lines.dayTitle
        offerDayActions(day)
        rows = worded.filter { !$0.0.hidden && $0.0.kind != .summary }
        lastInHand = view.focus
        var snapshot = NSDiffableDataSourceSnapshot<Int, String>()
        snapshot.appendSections([0])
        snapshot.appendItems(rows.map(\.item.key))
        // A block's fold state is in its value, so every row is redrawn.
        snapshot.reconfigureItems(rows.map(\.item.key))
        let landing = view.landing
        let land = landed.map { landing > $0 } ?? false
        landed = landing
        dataSource.apply(snapshot, animatingDifferences: false) { [weak self] in
            guard land, let self, let focus = view.focus else { return }
            self.land(on: focus)
        }
    }

    /// Puts VoiceOver on the row `key`, scrolled into view.
    private func land(on key: String) {
        guard let index = rows.firstIndex(where: { $0.item.key == key }) else { return }
        let path = IndexPath(item: index, section: 0)
        collectionView.scrollToItem(at: path, at: .centeredVertically, animated: false)
        collectionView.layoutIfNeeded()
        UIAccessibility.post(notification: .layoutChanged, argument: collectionView.cellForItem(at: path))
    }

    private func configure(_ cell: UICollectionViewListCell, _ item: LumennaApp.DayItem, line: LumennaApp.Line?) {
        var content = UIListContentConfiguration.subtitleCell()
        content.textProperties.numberOfLines = 0
        content.secondaryTextProperties.numberOfLines = 0
        content.secondaryTextProperties.color = .quietLabel
        content.imageProperties.preferredSymbolConfiguration = UIImage.SymbolConfiguration(textStyle: .body)
        // The line is the core's, in every app's order and this device's clock.
        content.text = line?.title
        content.secondaryText = line.flatMap { $0.detail.isEmpty ? nil : $0.detail }
        cell.accessories = []
        switch item.kind {
        case .block:
            content.image = UIImage(systemName: item.block?.when == "now" ? "clock.fill" : "clock")
            cell.accessories = [.disclosureIndicator(displayed: .always)]
        case .sitting:
            let sitting = item.sitting
            content.image = UIImage(systemName: sitting?.running == true ? "timer" : sitting?.status == "paused" ? "pause.circle" : "circle.dashed")
            content.directionalLayoutMargins.leading += 24
            cell.accessories = [.disclosureIndicator(displayed: .always)]
        case .free:
            content.textProperties.color = .quietLabel
        case .now:
            content.textProperties.font = .preferredFont(forTextStyle: .headline)
            content.image = UIImage(systemName: "arrowtriangle.right.fill")
            content.imageProperties.tintColor = .warningLabel
        case .cancelled:
            content.textProperties.color = .quietLabel
            content.image = UIImage(systemName: "xmark.circle")
            cell.accessories = [.disclosureIndicator(displayed: .always)]
        case .summary:
            break
        }
        cell.contentConfiguration = content
        cell.isAccessibilityElement = true
        cell.accessibilityLabel = line?.title
        cell.accessibilityValue = AppModel.spoken(detail: line?.detail, expanded: item.expanded, level: item.level)
        cell.accessibilityTraits = item.kind == .now ? .staticText : .button
        // Only what is not swiped, which UIKit adds to the swipe actions.
        cell.accessibilityCustomActions = rowActions(for: item).customActions
    }

    // MARK: - Actions

    private func perform(_ action: LumennaApp.Action, on item: LumennaApp.DayItem) {
        core.app.run(action, on: item.key)
    }

    /// The row's action of one of these kinds, if it has one.
    private func run(_ kinds: LumennaApp.ActionKind..., on item: LumennaApp.DayItem) {
        if let action = item.actions.first(where: { kinds.contains($0.kind) }) { perform(action, on: item) }
    }

    // MARK: - The day's own actions

    /// The day's actions, the app's, in its order: a menu among the day's buttons, and the
    /// heading's actions for VoiceOver, where the day is.
    private func offerDayActions(_ day: LumennaApp.DayView) {
        let run = { [weak self] (action: LumennaApp.Action) in self?.core.app.send(.perform(action)) }
        dayMenu.menu = UIMenu(title: core.app.lines.dayTitle, children: day.actions.map { action in
            UIAction(title: action.title, attributes: action.destructive ? .destructive : []) { _ in run(action) }
        })
        dayMenu.isHidden = day.actions.isEmpty
        summary.isAccessibilityElement = true
        summary.accessibilityCustomActions = day.actions.map { action in
            UIAccessibilityCustomAction(name: action.title) { _ in
                run(action)
                return true
            }
        }
    }

    /// The day's actions as offered, for tests: the menu's titles.
    var dayActionTitles: [String] { dayMenu.menu?.children.map(\.title) ?? [] }
    /// The heading's actions, as VoiceOver lists them.
    var headingActions: [UIAccessibilityCustomAction] { summary.accessibilityCustomActions ?? [] }

    /// Replan from Now, as the overrun warning's Replan opens it: today, the screen over it.
    func openReplan() {
        show()
        core.app.send(.day(.now))
        core.app.send(.proposal(.replan))
    }

    // MARK: - Moving through days

    private func move(_ step: LumennaApp.DayMove) {
        show()
        // The app says the day newly shown, "<day>. <summary>", and puts now in hand for Now.
        core.app.send(.day(step))
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        guard let key = dataSource.itemIdentifier(for: path), let item = rows.first(where: { $0.item.key == key })?.item else { return }
        switch item.kind {
        case .block: run(.edit, on: item)
        case .sitting:
            if let task = item.sitting?.task {
                core.app.send(.focus(item.key))
                showBeside(TaskDetailViewController(core: core, id: task))
            }
        case .free: run(.addBlock, on: item)
        case .cancelled:
            collectionView.deselectItem(at: path, animated: true)
            let line = rows.first { $0.item.key == key }?.line?.text ?? ""
            choose(line, actions: item.actions.map { action in
                (action.title, { [weak self] in self?.perform(action, on: item) })
            })
        case .now, .summary: collectionView.deselectItem(at: path, animated: true)
        }
    }

    /// A reminder's block, chosen in Notification Center: its day, with VoiceOver on it.
    func show(day iso: String, series: String) {
        show()
        goTo(iso)
        if let block = core.app.current.view.day?.items.first(where: { $0.kind == .block && $0.block?.series == series }) {
            core.app.send(.focus(block.key))
            DispatchQueue.main.async { [weak self] in self?.land(on: block.key) }
        }
    }

    /// Goes to the day named, as Go to Day's answer: true if the core read it, else the day's
    /// question stays closed and why is `goToRefusal`.
    @discardableResult
    private func goTo(_ named: String) -> Bool {
        core.app.send(.day(.ask))
        core.app.send(.answer(.text(text: named)))
        if let refusal = core.app.current.view.asking?.refusal {
            core.app.send(.cancel)
            goToRefusal = refusal
            return false
        }
        return true
    }

    private var goToRefusal: String?

    // MARK: - Undo, and going to a day

    @objc func undoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canUndo { typing.undo(); return }
        core.app.send(.undo)
    }

    @objc func redoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canRedo { typing.redo(); return }
        core.app.send(.redo)
    }

    /// Go to Day: a day typed in the core's words, or the system's calendar. The app reads it,
    /// as the answer to its own Go to Day; a day it cannot read stays in the sheet with why.
    private func chooseDay() {
        let shown = core.app.view.day.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        let picker = DayPickerViewController(showing: shown) { [weak self] named in
            guard let self else { return {} }
            self.show()
            guard self.goTo(named) else { throw DayRefused(sentence: self.goToRefusal ?? "") }
            return {}
        }
        present(UINavigationController(rootViewController: picker), animated: true)
    }

    // MARK: - Keyboard

    // The Day menu's commands, and New Block on the day shown (`KeyboardCommands`).
    override var canBecomeFirstResponder: Bool { true }

    @objc func previousDay() { move(.previous) }
    @objc func nextDay() { move(.next) }
    @objc func goToNow() { move(.now) }
    @objc func goToDay() { chooseDay() }
    @objc func newBlock() {
        show()
        core.app.send(.block(.new(start: nil, minutes: nil)))
    }
    // The day's own actions, from the Day menu: the day shown's of each kind.
    @objc func planTheDay() { runDay(.planDay) }
    @objc func replanFromNow() { runDay(.replan) }
    @objc func whatNow() { runDay(.whatNow) }
    @objc func endTheDay() { runDay(.endDay) }
    @objc func clearTheDay() { runDay(.clearDay) }

    /// The day's action of `kind`, or why the day has none, in the core's words.
    func runDay(_ kind: LumennaApp.ActionKind) {
        // A screen just chosen by a command has not yet appeared to open the day.
        loadViewIfNeeded()
        show()
        if let action = core.app.current.view.day?.actions.first(where: { $0.kind == kind }) {
            core.app.send(.perform(action))
        } else {
            Announcer.say(LumennaApp.notOffered(kind: kind, subject: .day, thisDevice: false))
        }
    }

    /// Add Block, in the bar.
    private func addBlock() {
        newBlock()
    }
}

/// A day the core could not read, its sentence for the Go to Day sheet to show.
private struct DayRefused: LocalizedError {
    let sentence: String
    var errorDescription: String? { sentence }
}

// The row's actions, apart from the layout that asks for them, so a test can ask too.
extension DayViewController {
    /// The row's actions as the day offers them (`AppRowActions`), Expand or Collapse among them.
    private func rowActions(for item: LumennaApp.DayItem) -> AppRowActions {
        AppRowActions(actions: item.actions) { [weak self] action in self?.perform(action, on: item) }
    }

    private func item(at path: IndexPath) -> LumennaApp.DayItem? {
        guard let key = dataSource.itemIdentifier(for: path) else { return nil }
        return rows.first { $0.item.key == key }?.item
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
