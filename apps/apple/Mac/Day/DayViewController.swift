import AppKit
import LumennaApp
import SwiftUI

/// The planner: a day as it is lived, as an outline — the app's day (`ViewModel.day`), in
/// SwiftUI, which is AppKit's own outline underneath, so VoiceOver hears each row's level and
/// whether a block's sittings are shown.
///
/// Blocks in time order with their sittings beneath them, free time as rows of its own, and
/// now as a position rather than a highlight. The heading above says what a glance at a
/// timeline would. The day shown, moving between days, the row in hand and folding, and every
/// row's actions and their forms are the app's; opening on today puts the selection —
/// VoiceOver's focus — on now.
struct MacDayScreen: View {
    @ObservedObject var model: DayModel
    @FocusState private var listFocused: Bool

    /// A row and the sittings beneath it.
    typealias Node = OutlineNode<DayModel.Row>

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // The day's heading, as on the phone: VoiceOver's heading commands land on it.
            Text(model.heading)
                .font(.subheadline)
                .foregroundStyle(Color.quietLabel)
                .accessibilityAddTraits(.isHeader)
                .padding(.horizontal, 12)
                .padding(.top, 12)
            // The day's screen actions, the core's, in its order (New Block, What Should I Work
            // On?, the moves through days, Undo, Redo), then the day's own.
            ScreenActionRow(actions: model.screenActions, run: { model.perform($0) }) {
                if !model.actions.isEmpty {
                    Menu(Wording.dayActions) {
                        ForEach(model.actions, id: \.self) { action in
                            Button(action.asksFirst ? action.title + "…" : action.title) { model.perform(action) }
                        }
                    }
                    .fixedSize()
                }
            }
            .padding(.horizontal, 12)
            ScrollViewReader { scroller in
                List(selection: Binding(get: { model.focus }, set: { if let key = $0 {
                    // A row chosen takes the list's focus with it: a click selects a SwiftUI list's
                    // row without making the list first responder, and Space went elsewhere.
                    listFocused = true
                    model.moved(to: key)
                } })) {
                    ForEach(Self.tree(model.rows)) { node in
                        nodeView(node)
                    }
                }
                .accessibilityLabel(Wording.theDay)
                .focused($listFocused)
                .contextMenu(forSelectionType: String.self) { keys in
                    if let key = keys.first, let item = model.rows.first(where: { $0.id == key })?.item {
                        ForEach(item.offered, id: \.self) { action in
                            Button(action.asksFirst ? action.title + "…" : action.title) { model.run(action, on: item) }
                        }
                    }
                } primaryAction: { _ in
                    activate()
                }
                .onChange(of: model.landed?.count) { _, _ in
                    // The app moved the row in hand: the outline scrolls to it, which the
                    // selection alone does not, so VoiceOver finds it on screen.
                    if let key = model.landed?.key { scroller.scrollTo(key) }
                }
            }
        }

    }

    /// Blocks with the sittings beneath them; every other row stands alone.
    static func tree(_ rows: [DayModel.Row]) -> [Node] {
        Outline.tree(rows, id: \.id) { $0.item.kind == .sitting ? 1 : 0 }
    }

    /// The rows as the outline shows them, in its order: each block, then its sittings while it
    /// is expanded.
    var shown: [String] {
        Outline.shown(Self.tree(model.rows)) { $0.item.expanded != false }
    }

    @ViewBuilder private func nodeView(_ node: Node) -> some View {
        if node.children.isEmpty {
            label(node.row).tag(node.id)
        } else {
            DisclosureGroup(isExpanded: Binding(get: { node.row.item.expanded != false }, set: { open in
                model.key(open ? "expand" : "collapse", on: node.row.item)
            })) {
                ForEach(node.children) { child in
                    label(child.row).tag(child.id)
                }
            } label: {
                label(node.row)
            }
            // A disclosure group makes its label a heading, which no other row is.
            .accessibilityRemoveTraits(.isHeader)
            .tag(node.id)
        }
    }

    /// A row: its line, the title then the rest, in the core's words and every app's order.
    private func label(_ row: DayModel.Row) -> some View {
        let detail = row.line?.detail ?? ""
        return HStack(alignment: .firstTextBaseline, spacing: 6) {
            SwatchMark(swatch: row.item.block?.swatch)
            EmojiMark(emoji: row.item.emoji)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.line?.title ?? "").font(row.item.kind == .now ? .headline : .body)
                if !detail.isEmpty {
                    Text(detail).font(.subheadline).foregroundStyle(Color.quietLabel)
                }
            }
        }
        // A group named by its whole line, as the task list's rows: as static text VoiceOver
        // said its value alone, without the level or whether a block is expanded.
        .accessibilityElement(children: .contain)
        .accessibilityLabel(row.line?.text ?? "")
        .accessibilityIdentifier(row.line?.title ?? "")
    }

    /// Return and Space and Delete are the core's keys: Return is Open (a block's Edit, a
    /// sitting's task's details, free time's Add Block, a cancelled day restored), Space starts,
    /// pauses or resumes a sitting's timer, Delete takes a sitting out or deletes a block — each
    /// also in the row's menu. A row without one says why, in the core's words.
    /// Whether it took the key (`ListKeys`).
    func key(_ key: ListKey) -> Bool {
        guard let item = model.inHand else { return false }
        switch key {
        case .returnKey: activate()
        case .space: model.core.app.key("toggle", on: item.key)
        case .delete: model.core.app.key("delete", on: item.key)
        }
        return true
    }

    /// Return, or a double click: the core's Open on the row in hand.
    private func activate() {
        guard let item = model.inHand else { return }
        model.key("open", on: item)
    }
}

/// The SwiftUI day in the main window's middle pane, hosted by a view that steps out of
/// VoiceOver's tree (`FlatHostingView`), with what is AppKit's: the Day menu's commands.
final class DayViewController: NSViewController {
    private let core: Core
    private let model: DayModel
    private weak var main: MainWindowController?
    private var screen: MacDayScreen!

    init(core: Core, window: MainWindowController) {
        self.core = core
        model = DayModel(core: core)
        main = window
        super.init(nibName: nil, bundle: nil)
        title = LumennaApp.placeTitle(place: .today)
        screen = MacDayScreen(model: model)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        view = FlatHostingView(rootView: screen)
    }

    override func viewWillAppear() {
        super.viewWillAppear()
        model.appear()
    }

    /// The list's own outline, AppKit's under SwiftUI's `List`: where focus goes as the pane
    /// appears and when F6 reaches it.
    override var preferredFirstResponder: NSView? { outline ?? view }

    private var outline: NSOutlineView? {
        func find(_ view: NSView) -> NSOutlineView? {
            (view as? NSOutlineView) ?? view.subviews.lazy.compactMap(find).first
        }
        return find(view)
    }

    /// Focus for the list at any click on it, and its keys.
    private var clicks: FocusOnClick?
    private var keys: ListKeys?

    override func viewDidAppear() {
        super.viewDidAppear()
        // Once SwiftUI has laid the list out: SwiftUI's own focus, asked for as the app
        // launches, went nowhere.
        view.whenOutline { [weak self] outline in
            guard let self else { return }
            if self.clicks == nil { self.clicks = FocusOnClick(outline) }
            if self.keys == nil {
                self.keys = ListKeys(outline) { [weak self] key in
                    guard let self, let screen = self.screen else { return false }
                    outline.inHand(shown: screen.shown) { self.model.moved(to: $0) }
                    return screen.key(key)
                }
            }
            outline.takeFocusIfNowhere()
        }
    }

    /// The day's actions as the pull-down offers them, for tests.
    var dayActionTitles: [String] { model.actions.map(\.title) }

    // MARK: - Moving through days, from the Day menu

    @objc func previousDay(_ sender: Any?) { model.move(.previous) }
    @objc func nextDay(_ sender: Any?) { model.move(.next) }
    /// Today, on now: the app puts now in hand and says the day.
    @objc func goToToday(_ sender: Any?) { model.move(.now) }
    @objc func newBlock(_ sender: Any?) { core.app.send(.block(.new(start: nil, minutes: nil))) }
    /// Go to Day: the app's own question, asked by the window with the system's calendar beside
    /// the line.
    @objc func goToDay(_ sender: Any?) { model.move(.ask) }
}
