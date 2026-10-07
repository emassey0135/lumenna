import UIKit

/// The planner: a day as it is lived, as a list.
///
/// Blocks in time order with their sittings beneath them, free time as rows of its own, and
/// now as a position rather than a highlight. The summary above says what a glance at a
/// timeline would. Opening on today puts VoiceOver on now, not on midnight.
final class DayViewController: UIViewController, UICollectionViewDelegate {
    private enum Row: Hashable {
        case block(PlanBlock)
        case sitting(PlanAssignment, in: PlanBlock)
        case free(start: String, end: String, minutes: UInt32)
        case now(String)
        /// A repeating block cancelled for this day alone, so the day can be put back.
        case cancelled(CancelledBlock)
    }

    private let core: Core
    /// The day shown, as an ISO date; `nil` follows today.
    private var day: String?
    private var plan: Plan?
    /// Every row of the day, and those shown once folded.
    private var listed: [Row] = []
    private var shown: [Folding.Shown<Row>] = []
    private var rows: [Row] = []
    private var folding = Folding()
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, Row>!
    private let summary = UILabel()
    private var landedOnNow = false
    private weak var dayButtons: UIStackView?

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Today"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    // MARK: - Building

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        summary.font = .preferredFont(forTextStyle: .subheadline)
        summary.adjustsFontForContentSizeCategory = true
        summary.textColor = .quietLabel
        summary.numberOfLines = 0
        summary.accessibilityTraits = .header
        // Moving through days, under the summary rather than in a bottom toolbar, which the
        // floating tab bar would cover.
        let days = UIStackView(arrangedSubviews: [
            dayButton("Previous Day") { [weak self] in self?.step(-1) },
            dayButton("Now") { [weak self] in self?.goToNow() },
            dayButton("Next Day") { [weak self] in self?.step(1) },
            dayButton("Go to Day") { [weak self] in self?.chooseDay() },
        ])
        days.distribution = .equalSpacing
        dayButtons = days
        let header = UIStackView(arrangedSubviews: [summary, days])
        header.axis = .vertical
        header.spacing = 8
        header.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(header)

        var configuration = UICollectionLayoutListConfiguration(appearance: .plain)
        configuration.trailingSwipeActionsConfigurationProvider = { [weak self] path in
            guard let self, let row = self.dataSource.itemIdentifier(for: path) else { return nil }
            var actions = self.actions(for: row).map { title, destructive, run in
                UIContextualAction(style: destructive ? .destructive : .normal, title: title) { _, _, done in
                    run()
                    done(true)
                }
            }
            if let index = self.rows.firstIndex(of: row),
               let fold = self.folding.action(for: self.shown[index], key: self.foldKey(row), changed: { [weak self] key, said in
                   self?.fold(key, saying: said)
               }) {
                actions.append(fold)
            }
            return actions.isEmpty ? nil : UISwipeActionsConfiguration(actions: actions)
        }
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

        let cell = UICollectionView.CellRegistration<UICollectionViewListCell, Row> { [weak self] cell, path, row in
            self?.configure(cell, row, at: path.item)
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) { view, path, row in
            view.dequeueConfiguredReusableCell(using: cell, for: path, item: row)
        }

        let add = UIBarButtonItem(systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.addBlock() })
        add.accessibilityLabel = "Add block"
        navigationItem.rightBarButtonItem = add
        // In the navigation bar, as on the task list: a bottom toolbar would sit under the
        // floating tab bar.
        navigationItem.leftBarButtonItems = [
            .undo { [weak self] in self?.undo() },
            .redo { [weak self] in self?.redo() },
        ]
        NotificationCenter.default.addObserver(self, selector: #selector(storeChanged), name: Core.changed, object: nil)
    }

    /// Side by side while the three fit, one above another once the text is too large for
    /// that — at whatever size that happens, not only the accessibility sizes.
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        guard let days = dayButtons else { return }
        let needed = days.arrangedSubviews.reduce(CGFloat(0)) { $0 + $1.intrinsicContentSize.width }
            + days.spacing * CGFloat(days.arrangedSubviews.count - 1)
        let fits = needed <= view.layoutMarginsGuide.layoutFrame.width
        let axis: NSLayoutConstraint.Axis = fits ? .horizontal : .vertical
        if days.axis != axis {
            days.axis = axis
            days.alignment = fits ? .fill : .leading
        }
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if let selected = collectionView.indexPathsForSelectedItems?.first {
            collectionView.deselectItem(at: selected, animated: animated)
        }
        reload()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Opening the day lands on now, not at midnight. Once, so coming back from a
        // detail does not move focus away from where the person was.
        if !landedOnNow {
            landedOnNow = true
            goToNow(announcing: false)
        }
    }

    private func dayButton(_ title: String, action: @escaping () -> Void) -> UIButton {
        // The configuration's own type scales with Dynamic Type; a font set here would not.
        var configuration = UIButton.Configuration.plain()
        configuration.title = title
        configuration.titleLineBreakMode = .byWordWrapping
        let button = UIButton(configuration: configuration, primaryAction: UIAction { _ in action() })
        return button
    }

    // MARK: - Rows

    @objc private func storeChanged() {
        reload()
    }

    private func reload(then finished: (() -> Void)? = nil) {
        do {
            let plan = try core.lumenna.plan(date: day)
            self.plan = plan
            title = Clock.spokenDay(plan.date)
            summary.text = plan.summary
            listed = plan.timeline.flatMap { item -> [Row] in
                switch item {
                case let .block(row):
                    guard let block = plan.blocks.first(where: { $0.row == row }) else { return [] }
                    return [.block(block)] + block.assignments.map { .sitting($0, in: block) }
                case let .free(start, end, minutes):
                    return [.free(start: start, end: end, minutes: minutes)]
                case let .now(time):
                    return [.now(time)]
                }
            } + plan.cancelled.map { .cancelled($0) }
        } catch {
            summary.text = error.sentence
        }
        apply(then: finished)
    }

    /// Shows the day's rows as folded.
    private func apply(then finished: (() -> Void)? = nil) {
        shown = folding.shown(listed, depth: depth, key: foldKey)
        rows = shown.map(\.item)
        var snapshot = NSDiffableDataSourceSnapshot<Int, Row>()
        snapshot.appendSections([0])
        snapshot.appendItems(rows)
        // A block's fold state is in its value, so every row is redrawn.
        snapshot.reconfigureItems(rows)
        dataSource.apply(snapshot, animatingDifferences: false, completion: finished)
    }

    /// What identifies a row for folding; only a block has anything under it.
    private func foldKey(_ row: Row) -> String {
        if case let .block(block) = row { return "block:\(block.id)" }
        return String(describing: row)
    }

    /// Folds or unfolds the block `key`, keeping VoiceOver on it.
    private func fold(_ key: String, saying said: String) {
        folding.toggle(key)
        apply { [weak self] in
            guard let self, let index = self.rows.firstIndex(where: { self.foldKey($0) == key }) else { return }
            let path = IndexPath(item: index, section: 0)
            UIAccessibility.post(notification: .layoutChanged, argument: self.collectionView.cellForItem(at: path))
            Announcer.say(said)
        }
    }

    private func depth(_ row: Row) -> Int {
        if case .sitting = row { return 1 }
        return 0
    }

    private func configure(_ cell: UICollectionViewListCell, _ row: Row, at index: Int) {
        var content = UIListContentConfiguration.subtitleCell()
        content.textProperties.numberOfLines = 0
        content.secondaryTextProperties.numberOfLines = 0
        content.secondaryTextProperties.color = .quietLabel
        content.imageProperties.preferredSymbolConfiguration = UIImage.SymbolConfiguration(textStyle: .body)
        var label: String
        var value: [String] = []
        switch row {
        case let .block(block):
            label = "\(Clock.time(block.start)) to \(Clock.time(block.end)), \(block.title)"
            // The core words the details for every app.
            value = block.details
            content.text = label
            content.secondaryText = value.joined(separator: ", ")
            content.image = UIImage(systemName: block.when == "now" ? "clock.fill" : "clock")
            cell.accessories = [.disclosureIndicator(displayed: .always)]
        case let .sitting(sitting, _):
            label = sitting.title
            value = sitting.details
            content.text = label
            content.secondaryText = value.joined(separator: ", ")
            content.image = UIImage(systemName: sitting.running ? "timer" : sitting.status == "paused" ? "pause.circle" : "circle.dashed")
            content.directionalLayoutMargins.leading += 24
            cell.accessories = [.disclosureIndicator(displayed: .always)]
        case let .free(start, end, minutes):
            label = "Free, \(Clock.length(minutes))"
            value = ["\(Clock.time(start)) to \(Clock.time(end))"]
            content.text = label
            content.secondaryText = value[0]
            content.textProperties.color = .quietLabel
            cell.accessories = []
        case let .now(time):
            label = "Now, \(Clock.time(time))"
            content.text = label
            content.textProperties.font = .preferredFont(forTextStyle: .headline)
            content.image = UIImage(systemName: "arrowtriangle.right.fill")
            content.imageProperties.tintColor = .warningLabel
            cell.accessories = []
        case let .cancelled(block):
            label = "\(Clock.time(block.start)), \(block.title)"
            value = ["cancelled for this day"]
            content.text = label
            content.secondaryText = value[0]
            content.textProperties.color = .quietLabel
            content.image = UIImage(systemName: "xmark.circle")
            cell.accessories = [.disclosureIndicator(displayed: .always)]
        }
        if index < shown.count, let fold = shown[index].state {
            value.append(fold)
        }
        // Depth is said where it changes: indentation alone says nothing in speech.
        let previous = index > 0 && index - 1 < rows.count ? depth(rows[index - 1]) : 0
        if depth(row) != previous {
            value.append("level \(depth(row) + 1)")
        }
        cell.contentConfiguration = content
        cell.isAccessibilityElement = true
        cell.accessibilityLabel = label
        cell.accessibilityValue = value.joined(separator: ", ")
        cell.accessibilityTraits = {
            if case .now = row { return .staticText }
            return .button
        }()
    }

    // MARK: - Actions

    /// What can be done to a row: its swipe actions, which VoiceOver lists as actions.
    private func actions(for row: Row) -> [(String, Bool, () -> Void)] {
        switch row {
        case let .block(block):
            var actions: [(String, Bool, () -> Void)] = []
            if block.acceptsTasks {
                actions.append(("Assign Task", false, { [weak self] in self?.assign(to: block) }))
            }
            actions.append(("Edit", false, { [weak self] in self?.edit(block) }))
            if block.repeats {
                actions.append(("Cancel This Day", false, { [weak self] in
                    self?.change(focusing: row) { try self!.core.lumenna.cancelOccurrence(id: block.series, date: self!.plan!.date) }
                }))
            }
            if block.changedForThisDay {
                actions.append(("Restore This Day", false, { [weak self] in
                    self?.change(focusing: row) { try self!.core.lumenna.restoreOccurrence(id: block.series, date: self!.plan!.date) }
                }))
            }
            actions.append(("Delete Block", true, { [weak self] in self?.delete(block) }))
            return actions
        case let .sitting(sitting, _):
            // Start, pause and stop: a paused sitting is still in progress, and stopping
            // either a running or a paused one ends it.
            var timer: [(String, Bool, () -> Void)] = []
            let start = { [weak self] in
                guard let self else { return }
                self.change(focusing: row) { try self.core.lumenna.startTimer(assignment: sitting.id) }
            }
            if sitting.running {
                timer.append(("Pause Timer", false, { [weak self] in self?.pauseTimer(sitting, row: row) }))
            } else {
                timer.append((sitting.status == "paused" ? "Resume Timer" : "Start Timer", false, start))
            }
            if sitting.running || sitting.status == "paused" {
                timer.append(("Stop Timer", false, { [weak self] in self?.stopTimer(sitting, row: row) }))
            }
            return timer + [
                ("Planned Length", false, { [weak self] in self?.planLength(sitting, row: row) }),
                ("Log Minutes", false, { [weak self] in self?.logMinutes(sitting, row: row) }),
                ("Unassign", true, { [weak self] in
                    self?.change(focusing: row) { try self!.core.lumenna.unassign(assignment: sitting.id) }
                }),
            ]
        case let .free(start, _, minutes):
            return [("Add Block Here", false, { [weak self] in self?.addBlock(at: start, minutes: minutes) })]
        case let .cancelled(block):
            return [("Restore This Day", false, { [weak self] in
                guard let self, let date = self.plan?.date else { return }
                self.change(focusing: row) { try self.core.lumenna.restoreOccurrence(id: block.series, date: date) }
            })]
        case .now:
            return []
        }
    }

    /// Runs a change, then reloads with focus on the same row if it is still there, or on
    /// whatever now holds its place, and says what happened.
    private func change(focusing row: Row?, _ operation: () throws -> Change) {
        let index = row.flatMap { rows.firstIndex(of: $0) }
        do {
            let change = try operation()
            reload { [weak self] in
                guard let self else { return }
                let target = index.map { min($0, self.rows.count - 1) }
                if let target, target >= 0 {
                    self.focus(IndexPath(item: target, section: 0))
                }
                Announcer.say(change.announcement, notices: change.notices)
            }
        } catch {
            showFailure(error.sentence)
        }
    }

    private func focus(_ path: IndexPath) {
        collectionView.scrollToItem(at: path, at: .centeredVertically, animated: false)
        collectionView.layoutIfNeeded()
        UIAccessibility.post(notification: .layoutChanged, argument: collectionView.cellForItem(at: path))
    }

    private func stopTimer(_ sitting: PlanAssignment, row: Row) {
        do {
            let timer = try core.lumenna.stopTimer(assignment: sitting.id, minutes: nil)
            reload { [weak self] in
                if let index = self?.rows.firstIndex(where: {
                    if case let .sitting(s, _) = $0 { return s.id == sitting.id }
                    return false
                }) {
                    self?.focus(IndexPath(item: index, section: 0))
                }
                Announcer.say(timer.announcement, notices: timer.notices)
            }
        } catch {
            showFailure(error.sentence)
        }
    }

    /// Pauses a running timer, keeping the time so far; the sitting stays in progress.
    private func pauseTimer(_ sitting: PlanAssignment, row: Row) {
        do {
            let timer = try core.lumenna.pauseTimer(assignment: sitting.id)
            reload { [weak self] in
                if let index = self?.rows.firstIndex(where: {
                    if case let .sitting(s, _) = $0 { return s.id == sitting.id }
                    return false
                }) {
                    self?.focus(IndexPath(item: index, section: 0))
                }
                Announcer.say(timer.announcement, notices: timer.notices)
            }
        } catch {
            showFailure(error.sentence)
        }
    }

    /// Records a sitting's whole time by hand — without a timer, or to replace a capped one.
    private func logMinutes(_ sitting: PlanAssignment, row: Row) {
        askForText(
            "Minutes on \(sitting.title)",
            message: "The whole of this sitting, replacing what is logged.",
            placeholder: "45",
            action: "Log"
        ) { [weak self] text in
            guard let self, let minutes = UInt32(text) else {
                self?.showFailure("That is not a number of minutes.")
                return
            }
            do {
                let timer = try self.core.lumenna.stopTimer(assignment: sitting.id, minutes: minutes)
                self.reload { Announcer.say(timer.announcement, notices: timer.notices) }
            } catch {
                self.showFailure(error.sentence)
            }
        }
    }

    private func assign(to block: PlanBlock) {
        TaskPicker.present(from: self, core: core, title: "Assign to \(block.title)") { [weak self] task in
            guard let self, let date = self.plan?.date else { return }
            self.askForLength("How long is \(task.title) meant to take?", without: "Skip") { minutes in
                self.change(focusing: .block(block)) {
                    try self.core.lumenna.assign(task: task.key, block: block.series, date: date, minutes: minutes)
                }
            }
        }
    }

    /// Sets or clears how long a sitting is meant to take; what was logged stays.
    private func planLength(_ sitting: PlanAssignment, row: Row) {
        askForLength(
            "Planned length of \(sitting.title)", current: sitting.plannedMins, without: "No Planned Length"
        ) { [weak self] minutes in
            guard let self else { return }
            self.change(focusing: row) { try self.core.lumenna.planMinutes(assignment: sitting.id, minutes: minutes) }
        }
    }

    /// Asks "this day, or every day?" of a repeating block — never guessed.
    private func edit(_ block: PlanBlock) {
        guard block.repeats, let date = plan?.date else {
            presentForm(series: block)
            return
        }
        choose("Change \(block.title)", message: "Which occurrences?", actions: [
            ("\(Clock.spokenDay(date)) Only", { [weak self] in self?.presentForm(occurrence: block, day: date) }),
            ("Every Occurrence", { [weak self] in self?.presentForm(series: block) }),
        ])
    }

    private func presentForm(series block: PlanBlock) {
        do {
            presentBlockForm(try .series(core: core, id: block.series, saved: saved(focusing: .block(block))))
        } catch {
            showFailure(error.sentence)
        }
    }

    private func presentForm(occurrence block: PlanBlock, day: String) {
        presentBlockForm(.occurrence(core: core, block: block, day: day, saved: saved(focusing: .block(block))))
    }

    /// After a form saves: reload, keep focus on the row edited, and say what changed.
    private func saved(focusing row: Row) -> (Change) -> Void {
        { [weak self] change in
            guard let self else { return }
            let index = self.rows.firstIndex(of: row)
            self.reload {
                if let index, !self.rows.isEmpty {
                    self.focus(IndexPath(item: min(index, self.rows.count - 1), section: 0))
                }
                Announcer.say(change.announcement, notices: change.notices)
            }
        }
    }

    private func addBlock(at start: String? = nil, minutes: UInt32? = nil) {
        let day = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        let model = BlockFormModel(
            core: core,
            purpose: .add,
            start: start ?? "09:00",
            minutes: Int(min(minutes ?? 60, 720)),
            day: day
        ) { [weak self] change in
            self?.reload { Announcer.say(change.announcement, notices: change.notices) }
        }
        presentBlockForm(model)
    }

    private func delete(_ block: PlanBlock) {
        let message = block.repeats
            ? "Every occurrence goes, not only this day. To skip one day, cancel it instead."
            : "It goes to the trash with its assignments."
        confirm("Delete \(block.title)?", message: message, action: "Delete") { [weak self] in
            self?.change(focusing: .block(block)) { try self!.core.lumenna.deleteBlock(id: block.series) }
        }
    }

    // MARK: - Moving through days

    private func step(_ days: Int) {
        let current = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        guard let next = Calendar.current.date(byAdding: .day, value: days, to: current) else { return }
        day = Calendar.current.isDateInToday(next) ? nil : Clock.isoDay(next)
        reload { [weak self] in
            guard let self else { return }
            // A new day is a new screen's worth: say it, and start at its top.
            UIAccessibility.post(notification: .screenChanged, argument: self.summary)
        }
    }

    /// "Go to now": today, on the now row or the block happening now.
    private func goToNow(announcing: Bool = true) {
        day = nil
        reload { [weak self] in
            guard let self else { return }
            let index = self.rows.firstIndex {
                switch $0 {
                case .now: return true
                case let .block(block): return block.when == "now"
                default: return false
                }
            }
            if let index {
                self.focus(IndexPath(item: index, section: 0))
            } else if announcing {
                UIAccessibility.post(notification: .screenChanged, argument: self.summary)
            }
        }
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        guard let row = dataSource.itemIdentifier(for: path) else { return }
        switch row {
        case let .block(block): edit(block)
        case let .sitting(sitting, _):
            navigationController?.pushViewController(TaskDetailViewController(core: core, id: sitting.task), animated: true)
        case let .free(start, _, minutes): addBlock(at: start, minutes: minutes)
        case let .cancelled(block):
            collectionView.deselectItem(at: path, animated: true)
            choose("\(block.title) is cancelled for this day", actions: actions(for: row).map { ($0.0, $0.2) })
        case .now: collectionView.deselectItem(at: path, animated: true)
        }
    }

    // MARK: - Undo, and going to a day

    @objc private func undo() {
        change(focusing: nil) { try core.lumenna.undo() }
    }

    @objc private func redo() {
        change(focusing: nil) { try core.lumenna.redo() }
    }

    private func chooseDay() {
        let current = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        let picker = DayPickerViewController(showing: current) { [weak self] day in
            guard let self else { return }
            self.day = Calendar.current.isDateInToday(day) ? nil : Clock.isoDay(day)
            self.reload {
                UIAccessibility.post(notification: .screenChanged, argument: self.summary)
            }
        }
        present(UINavigationController(rootViewController: picker), animated: true)
    }

    // MARK: - Keyboard

    override var canBecomeFirstResponder: Bool { true }

    override var keyCommands: [UIKeyCommand]? {
        [
            UIKeyCommand(title: "Previous Day", action: #selector(previousDay), input: UIKeyCommand.inputLeftArrow, modifierFlags: .command),
            UIKeyCommand(title: "Next Day", action: #selector(nextDay), input: UIKeyCommand.inputRightArrow, modifierFlags: .command),
            UIKeyCommand(title: "Go to Now", action: #selector(now), input: "j", modifierFlags: .command),
            UIKeyCommand(title: "New Block", action: #selector(newBlock), input: "n", modifierFlags: .command),
            UIKeyCommand(title: "Undo", action: #selector(undo), input: "z", modifierFlags: .command),
            UIKeyCommand(title: "Redo", action: #selector(redo), input: "z", modifierFlags: [.command, .shift]),
        ]
    }

    @objc private func previousDay() { step(-1) }
    @objc private func nextDay() { step(1) }
    @objc private func now() { goToNow() }
    @objc private func newBlock() { addBlock() }
}
