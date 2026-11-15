import Combine
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
    /// Runs one of the day's screen actions: UIKit's, which holds the bar's too.
    let run: (LumennaApp.ScreenAction) -> Void
    @AccessibilityFocusState private var spoken: String?

    var body: some View {
        ScrollViewReader { scroller in
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
            .onChange(of: model.landed?.count) { _, _ in
                // The app moved the row in hand: VoiceOver goes there, once the row is drawn.
                guard let key = model.landed?.key else { return }
                scroller.scrollTo(key, anchor: .center)
                DispatchQueue.main.async { spoken = key }
            }
            .onChange(of: spoken) { _, key in
                if let key { model.moved(to: key) }
            }
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

    /// The screen's own actions the bar holds no icon for (`ScreenActionBar`), the core's, in its
    /// order: What Should I Work On?, the moves through days, Go to Day; then the day's own
    /// actions. Side by side while they fit, then on two lines, and one above another only once the
    /// text is too large for that. One per line on an SE at the ordinary size, the day's rows went
    /// under the tab bar.
    private var buttons: some View {
        ScreenActionRow(actions: model.screenActions.filter { !ScreenActionBar.holds($0) }, spread: true, run: run) {
            if !model.actions.isEmpty {
                Menu(Wording.dayActions) {
                    ForEach(model.actions, id: \.self) { action in
                        Button(action.title, role: action.destructive ? .destructive : nil) { model.perform(action) }
                    }
                }
            }
        }
        .buttonStyle(.borderless)
    }

    private func rowView(_ row: DayModel.Row) -> some View {
        let item = row.item
        let split = DayScreen.actions(of: item) { _ in }
        let folds = item.offered.filter { [.expand, .collapse].contains($0.kind) }
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
                    .listedOnce()
            }
        }
        .accessibilityActions {
            // Only what is not swiped: the row's cell offers the swipe actions to VoiceOver.
            ForEach(split.others, id: \.self) { action in
                Button(action.title) { model.run(action, on: item) }
            }
        }
        .contextMenu {
            ForEach(item.offered, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: item) }
            }
        }
    }

    /// What a tap does: the core's Open, as Return is on every app — a block's Edit, a sitting's
    /// task's details (its form, which the window opens beside the day), free time's Add Block,
    /// a cancelled day restored — or the app says why the row has none.
    private func tapped(_ row: DayModel.Row) {
        model.key("open", on: row.item)
    }

    /// A row's actions as the day offers them (`AppRowActions`): the primary ones and Expand or
    /// Collapse swiped from the trailing edge, the rest as accessibility actions, every one in
    /// the context menu.
    static func actions(of item: LumennaApp.DayItem, run: @escaping (LumennaApp.Offered) -> Void) -> AppRowActions<LumennaApp.Offered> {
        AppRowActions(actions: item.offered, run: run)
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

    init(core: Core) {
        self.core = core
        let model = DayModel(core: core)
        self.model = model
        weak var host: DayViewController?
        super.init(rootView: DayScreen(model: model, run: { action in host?.perform(action) }))
        host = self
        title = LumennaApp.placeTitle(place: .today)
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            guard let self, self.title != self.model.title else { return }
            self.title = self.model.title
        }
        // The day's screen actions, the core's: Undo, Redo and New Block in the navigation bar,
        // as on the task list (a bottom toolbar would sit under the floating tab bar), the rest
        // among the day's buttons.
        actions = model.$screenActions.removeDuplicates().sink { [weak self] actions in
            guard let self else { return }
            ScreenActionBar.place(actions, in: self.navigationItem, menu: false) { [weak self] action in self?.perform(action) }
        }
    }

    private var actions: AnyCancellable?

    /// One of the day's screen actions: Undo Typing where a field has typing to undo, else the
    /// core's, on the day shown.
    func perform(_ action: LumennaApp.ScreenAction) {
        guard !ScreenActionBar.undoesTyping(action) else { return }
        model.perform(action)
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
        offerRotors()
    }

    /// VoiceOver's rotors over the day (`RowRotor`): Blocks, from one block to the next, and
    /// Running, the sitting whose timer runs.
    private func offerRotors() {
        let model = model
        let rotor = { (name: String, wanted: @escaping (LumennaApp.DayItem) -> Bool) in
            RowRotor.make(
                name, in: self,
                rows: { model.shown.map(\.id) },
                wanted: { index in model.shown.indices.contains(index) && wanted(model.shown[index].item) },
                current: { model.focus },
                landed: { model.moved(to: $0) }
            )
        }
        let rotors = [
            rotor(LumennaApp.placeTitle(place: .blocks)) { $0.kind == .block },
            rotor(Wording.running) { $0.kind == .sitting && $0.sitting?.running == true },
        ]
        view.accessibilityCustomRotors = rotors
        RowRotor.list(in: view)?.accessibilityCustomRotors = rotors
    }

    /// The day's actions as offered, for tests.
    var dayActionTitles: [String] { model.actions.map(\.title) }

    /// Go to Day: the app's own question, which the window's `AppQuestions` asks with the
    /// system's calendar beside the line (`DayPickerViewController`).
    private func chooseDay() {
        model.move(.ask)
    }

    @objc func undoChange() { undoInStore(core) }
    @objc func redoChange() { redoInStore(core) }

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
    // The day's own actions, from the Day menu: the core's commands on the day shown.
    @objc func planTheDay() { runDay("plan-day") }
    @objc func replanFromNow() { runDay("replan") }
    @objc func whatNow() { runDay("what-now") }
    @objc func endTheDay() { runDay("end-day") }
    @objc func clearTheDay() { runDay("clear-day") }

    /// The day's command `id`, or the app says why the day has none, whatever row is in hand.
    func runDay(_ id: String) {
        model.key(id)
    }
}
