import LumennaApp
import SwiftUI
import UIKit

/// The planner: a day as it is lived, as a list — the app's day (`ViewModel.day`), in SwiftUI.
///
/// Blocks in time order with their sittings beneath them, free time as rows of its own, and
/// now as a position rather than a highlight. The heading above says what a glance at a
/// timeline would, and carries the day's own actions. The day shown, moving between days, what
/// is in hand and folded, and every row's actions and their forms are the app's; opening on
/// today puts VoiceOver on now, not on midnight, and VoiceOver moves only when the app moved
/// the row in hand.
struct DayScreen: View {
    @ObservedObject var model: DayModel
    /// Opens a sitting's task beside the day on iPad, pushed on iPhone: UIKit's.
    let openTask: (String) -> Void
    /// Go to Day: UIKit's sheet of a typed day or the calendar.
    let chooseDay: () -> Void
    @AccessibilityFocusState private var spoken: String?
    /// A cancelled day whose actions are offered, tapped.
    @State private var offering: DayModel.Row?

    var body: some View {
        List {
            Section {
                heading
                buttons
            }
            ForEach(model.shown) { row in
                rowView(row)
            }
        }
        .listStyle(.plain)
        .navigationTitle(model.title)
        .confirmationDialog(offering?.line?.text ?? "", isPresented: Binding(get: { offering != nil }, set: { if !$0 { offering = nil } }), presenting: offering) { row in
            ForEach(row.item.actions, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: row.item) }
            }
        }
        .onChange(of: model.landed?.count) { _, _ in
            // The app moved the row in hand: VoiceOver goes there, once the row is drawn.
            guard let key = model.landed?.key else { return }
            DispatchQueue.main.async { spoken = key }
        }
        .onChange(of: spoken) { _, key in
            if let key { model.moved(to: key) }
        }
    }

    /// The day's heading, as every app says it, its actions the day's own.
    private var heading: some View {
        Text(model.heading)
            .font(.subheadline)
            .foregroundStyle(Color.quietLabel)
            // It carries the day's own actions, so it is a target a finger can land on too.
            .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
            .accessibilityAddTraits(.isHeader)
            .accessibilityIdentifier("day.heading")
            .accessibilityActions {
                ForEach(model.actions, id: \.self) { action in
                    Button(action.title) { model.perform(action) }
                }
            }
    }

    /// Moving through days, and the day's own actions: side by side while they fit, then on two
    /// lines, the moves and the rest, and one above another only once the text is too large for
    /// that. One per line on an SE at the ordinary size, the day's rows went under the tab bar.
    private var buttons: some View {
        ViewThatFits(in: .horizontal) {
            HStack { moves; Spacer(minLength: 0); others }
            VStack(alignment: .leading, spacing: 8) {
                HStack { moves }
                HStack { others }
            }
            VStack(alignment: .leading, spacing: 8) { moves; others }
        }
        .buttonStyle(.borderless)
    }

    /// The day before, now, and the day after.
    @ViewBuilder private var moves: some View {
        Button("Previous Day") { model.move(.previous) }
        Spacer(minLength: 0)
        Button("Now") { model.move(.now) }
        Spacer(minLength: 0)
        Button("Next Day") { model.move(.next) }
    }

    /// Going to another day, and the day's own actions.
    @ViewBuilder private var others: some View {
        Button("Go to Day") { chooseDay() }
        if !model.actions.isEmpty {
            Spacer(minLength: 0)
            Menu("Day Actions") {
                ForEach(model.actions, id: \.self) { action in
                    Button(action.title, role: action.destructive ? .destructive : nil) { model.perform(action) }
                }
            }
        }
    }

    private func rowView(_ row: DayModel.Row) -> some View {
        let item = row.item
        let split = DayScreen.actions(of: item) { _ in }
        let folds = item.actions.filter { [.expand, .collapse].contains($0.kind) }
        let detail = row.line?.detail ?? ""
        return Button {
            tapped(row)
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                if let symbol = Self.symbol(for: item) {
                    Image(systemName: symbol.name).foregroundStyle(symbol.colour)
                }
                SwatchMark(swatch: item.block?.swatch)
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.line?.title ?? "")
                        .font(item.kind == .now ? .headline : .body)
                        .foregroundStyle([.free, .cancelled].contains(item.kind) ? Color.quietLabel : Color.primary)
                    if !detail.isEmpty {
                        Text(detail).font(.subheadline).foregroundStyle(Color.quietLabel)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.leading, item.kind == .sitting ? 24 : 0)
            .contentShape(Rectangle())
        }
        .foregroundStyle(Color.primary)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(row.line?.title ?? "")
        .accessibilityValue(AppModel.spoken(detail: detail, expanded: item.expanded, level: item.level))
        .accessibilityAddTraits(item.kind == .now ? .isStaticText : .isButton)
        .accessibilityRemoveTraits(item.kind == .now ? .isButton : [])
        .accessibilityFocused($spoken, equals: row.id)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            ForEach(split.trailing, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: item) }
                    .tint(folds.contains(action) ? Color.lumennaTint : nil)
            }
        }
        .accessibilityActions {
            // Only what is not swiped: SwiftUI offers the swipe actions to VoiceOver already.
            ForEach(split.others, id: \.self) { action in
                Button(action.title) { model.run(action, on: item) }
            }
        }
        .contextMenu {
            ForEach(item.actions, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: item) }
            }
        }
    }

    /// What a tap does: a block's Edit, a sitting's task, free time's Add Block, a cancelled
    /// day's actions offered.
    private func tapped(_ row: DayModel.Row) {
        let item = row.item
        switch item.kind {
        case .block: model.run([.edit], on: item)
        case .sitting:
            if let task = item.sitting?.task {
                model.moved(to: item.key)
                openTask(task)
            }
        case .free: model.run([.addBlock], on: item)
        case .cancelled: offering = row
        case .now, .summary: break
        }
    }

    /// A row's actions as the day offers them (`AppRowActions`): the primary ones and Expand or
    /// Collapse swiped from the trailing edge, the rest as accessibility actions, every one in
    /// the context menu.
    static func actions(of item: LumennaApp.DayItem, run: @escaping (LumennaApp.Action) -> Void) -> AppRowActions {
        AppRowActions(actions: item.actions, run: run)
    }

    private static func symbol(for item: LumennaApp.DayItem) -> (name: String, colour: Color)? {
        switch item.kind {
        case .block: (item.block?.when == "now" ? "clock.fill" : "clock", Color.accentColor)
        case .sitting:
            (item.sitting?.running == true ? "timer" : item.sitting?.status == "paused" ? "pause.circle" : "circle.dashed", Color.accentColor)
        case .now: ("arrowtriangle.right.fill", Color.warningLabel)
        case .cancelled: ("xmark.circle", Color.quietLabel)
        case .free, .summary: nil
        }
    }
}

/// The SwiftUI day in the UIKit shell: the bars, the keyboard's Day commands, a sitting's task
/// and Go to Day's sheet are UIKit's, as for every screen.
final class DayViewController: UIHostingController<DayScreen> {
    private let core: Core
    private let model: DayModel
    private var observer: NSObjectProtocol?
    /// Why the last day typed could not be read, for Go to Day's sheet.
    private var goToRefusal: String?

    init(core: Core) {
        self.core = core
        let model = DayModel(core: core)
        self.model = model
        weak var host: DayViewController?
        super.init(rootView: DayScreen(
            model: model,
            openTask: { task in host?.showBeside(TaskDetailViewController(core: core, id: task)) },
            chooseDay: { host?.chooseDay() }
        ))
        host = self
        title = "Today"
        let add = UIBarButtonItem(systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.newBlock() })
        add.accessibilityLabel = "Add block"
        navigationItem.rightBarButtonItem = add
        // In the navigation bar, as on the task list: a bottom toolbar would sit under the
        // floating tab bar.
        navigationItem.leftBarButtonItems = [
            .undo { [weak self] in self?.undoChange() },
            .redo { [weak self] in self?.redoChange() },
        ]
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            guard let self, self.title != self.model.title else { return }
            self.title = self.model.title
        }
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        model.appear()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        takeKeyboardCommands()
    }

    /// The day's actions as offered, for tests.
    var dayActionTitles: [String] { model.actions.map(\.title) }

    /// Replan from Now, as the overrun warning's Replan opens it: today, the screen over it.
    func openReplan() {
        model.appear()
        core.app.send(.day(.now))
        core.app.send(.proposal(.replan))
    }

    /// A reminder's block, chosen in Notification Center: its day, with that block in hand.
    func show(day iso: String, series: String) {
        guard model.goTo(iso) == nil else { return }
        if let block = core.app.current.view.day?.items.first(where: { $0.kind == .block && $0.block?.series == series }) {
            core.app.send(.focus(block.key))
        }
    }

    /// Go to Day: a day typed in the core's words, or the system's calendar. The app reads it,
    /// as the answer to its own Go to Day; a day it cannot read stays in the sheet with why.
    private func chooseDay() {
        let shown = model.date.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0) } ?? .now
        let picker = DayPickerViewController(showing: shown) { [weak self] named in
            guard let self else { return {} }
            if let refusal = self.model.goTo(named) { throw DayRefused(sentence: refusal) }
            return {}
        }
        present(UINavigationController(rootViewController: picker), animated: true)
    }

    @objc func undoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canUndo { typing.undo(); return }
        core.app.send(.undo)
    }

    @objc func redoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canRedo { typing.redo(); return }
        core.app.send(.redo)
    }

    // MARK: - Keyboard

    // The Day menu's commands, and New Block on the day shown (`KeyboardCommands`).
    override var canBecomeFirstResponder: Bool { true }

    @objc func previousDay() { model.move(.previous) }
    @objc func nextDay() { model.move(.next) }
    @objc func goToNow() { model.move(.now) }
    @objc func goToDay() { chooseDay() }
    @objc func newBlock() {
        model.appear()
        core.app.send(.block(.new(start: nil, minutes: nil)))
    }
    // The day's own actions, from the Day menu: the day shown's of each kind.
    @objc func planTheDay() { runDay(.planDay) }
    @objc func replanFromNow() { runDay(.replan) }
    @objc func whatNow() { runDay(.whatNow) }
    @objc func endTheDay() { runDay(.endDay) }
    @objc func clearTheDay() { runDay(.clearDay) }

    /// The day's action of `kind`, or the app says why the day has none, whatever row is in hand.
    func runDay(_ kind: LumennaApp.ActionKind) {
        model.appear()
        core.app.send(.runDay([kind]))
    }
}

/// A day the core could not read, its sentence for the Go to Day sheet to show.
private struct DayRefused: LocalizedError {
    let sentence: String
    var errorDescription: String? { sentence }
}
