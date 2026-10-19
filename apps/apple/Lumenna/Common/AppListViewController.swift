import LumennaApp
import SwiftUI
import UIKit

/// A plain list drawn from the app, with the task list's focus rules: the app keeps the row in
/// hand after a change — the same row, else whatever holds its place — and when it moved it
/// (its landing count went up) VoiceOver goes there; a redraw for anything else leaves focus
/// alone. What it lists, folded, and what each row offers are the app's.
///
/// Subclasses say which part of the app's view they list and where they are held; this does the
/// rest, in SwiftUI (`AppListScreen`) inside a UIKit controller, which keeps the bars, the
/// keyboard's commands and what is pushed from it.
class AppListViewController: UIViewController {
    let core: Core
    /// The rows shown.
    private(set) var items: [AppItem] = []
    private let state = AppListState()
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
        let screen = AppListScreen(
            state: state, sidebar: isSidebar,
            actions: { [weak self] item in self?.rowActions(for: item) ?? AppRowActions(actions: [], run: { _ in }) },
            open: { [weak self] item in self?.open(item) }
        )
        let host = UIHostingController(rootView: screen)
        addChild(host)
        host.view.frame = view.bounds
        host.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(host.view)
        host.didMove(toParent: self)
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
        if state.items != rows { state.items = rows }
        if state.header != count { state.header = count }
        reselect()
        let (focus, landing) = held
        let land = landed.map { landing > $0 } ?? false
        landed = landing
        if land, let focus, rows.contains(where: { $0.key == focus }) {
            state.landed = (focus, (state.landed?.count ?? 0) + 1)
        }
    }

    /// Selects `selectedKey`, if it is shown.
    func reselect() {
        if state.selected != selectedKey { state.selected = selectedKey }
    }

    /// Runs one of a row's actions, the app's: held where this list holds its rows, then run.
    func perform(_ action: LumennaApp.Action, on item: AppItem) {
        core.app.run(action, on: item.key, side: side)
    }

    /// The item's actions as the list offers them (`AppRowActions`), Expand or Collapse among them.
    func rowActions(for item: AppItem) -> AppRowActions {
        AppRowActions(actions: item.actions) { [weak self] action in self?.perform(action, on: item) }
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

/// What an `AppListViewController` shows, for its SwiftUI list.
final class AppListState: ObservableObject {
    @Published var items: [AppItem] = []
    /// The line over the rows: how many, or the app's words for none.
    @Published var header = ""
    /// The row kept selected, as the sidebar keeps the place shown.
    @Published var selected: String?
    /// The row the app moved focus to, each time it moves it: a new `count` is a new move.
    @Published var landed: (key: String, count: Int)?
}

/// The list itself: each row says its title, then the rest of its line, its folding and its
/// level, and why it is offered as its hint; its primary actions and folding swipe from the
/// trailing edge, the rest are accessibility actions, and all of them are in its context menu.
struct AppListScreen: View {
    @ObservedObject var state: AppListState
    let sidebar: Bool
    let actions: (AppItem) -> AppRowActions
    let open: (AppItem) -> Void
    @AccessibilityFocusState private var spoken: String?

    var body: some View {
        ScrollViewReader { scroller in
            List {
                // A row of its own, as the task list's readback is: as a section's header, in
                // any colour, the audit measured it against what lies behind the list and
                // failed its contrast.
                if !state.header.isEmpty {
                    Section {
                        FormParts.caption(state.header)
                    }
                }
                Section {
                    ForEach(state.items, id: \.key) { item in
                        row(item).id(item.key)
                    }
                }
            }
            .styled(sidebar ? .sidebar : .insetGrouped)
            .onChange(of: state.landed?.count) { _, _ in
                // The app moved the row in hand: VoiceOver goes there, once it is on screen.
                guard let key = state.landed?.key else { return }
                scroller.scrollTo(key, anchor: .center)
                DispatchQueue.main.async { spoken = key }
            }
        }
    }

    /// What VoiceOver says of a row: its title as its name; the rest of its line, its folding
    /// and its level as its value; why it is offered as its hint, never part of its name.
    static func speech(of item: AppItem) -> (label: String, value: String, hint: String) {
        (item.title, item.spoken ?? item.detail ?? "", item.reason ?? "")
    }

    /// The lines beneath the title: the detail, then why it is offered.
    static func lines(of item: AppItem) -> [String] {
        [item.detail, item.reason].compactMap { $0 }.filter { !$0.isEmpty }
    }

    private func row(_ item: AppItem) -> some View {
        let split = actions(item)
        let speech = Self.speech(of: item)
        let selected = state.selected == item.key
        return Button { open(item) } label: {
            HStack(spacing: 8) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title)
                        .font(sidebar && item.heading ? .headline : .body)
                    ForEach(Self.lines(of: item), id: \.self) { line in
                        Text(line)
                            .font(.subheadline)
                            .foregroundStyle(Color.quietLabel)
                    }
                }
                Spacer(minLength: 0)
                if !sidebar {
                    Image(systemName: "chevron.forward")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(Color.quietLabel)
                        .accessibilityHidden(true)
                }
            }
            // Depth shown by indentation, for sight only; VoiceOver hears it in words, where it
            // changes.
            .padding(.leading, CGFloat(item.depth) * 20)
            .contentShape(Rectangle())
        }
        .foregroundStyle(Color.primary)
        .listRowBackground(selected ? Color.lumennaTint.opacity(0.2) : nil)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(speech.label)
        .accessibilityValue(speech.value)
        .accessibilityHint(speech.hint)
        .accessibilityAddTraits(item.heading ? [.isHeader, .isButton] : .isButton)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityFocused($spoken, equals: item.key)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            ForEach(split.trailing, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { split.run(action) }
                    .tint([.expand, .collapse].contains(action.kind) ? Color.lumennaTint : nil)
            }
        }
        .accessibilityActions {
            // Only what is not swiped: SwiftUI offers the swipe actions to VoiceOver already.
            ForEach(split.others, id: \.self) { action in
                Button(action.title) { split.run(action) }
            }
        }
        .contextMenu {
            ForEach(split.actions, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { split.run(action) }
            }
        }
    }
}

/// A list style chosen at run time: the sidebar's look or a list's.
private enum AnyListStyle {
    case sidebar, insetGrouped
}

private extension View {
    @ViewBuilder
    func styled(_ style: AnyListStyle) -> some View {
        switch style {
        case .sidebar: listStyle(.sidebar)
        case .insetGrouped: listStyle(.insetGrouped)
        }
    }
}
