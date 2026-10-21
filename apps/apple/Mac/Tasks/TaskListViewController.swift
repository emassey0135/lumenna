import AppKit
import LumennaApp
import SwiftUI

/// A place's tasks, with the filter above them, the trash, or every block series (Blocks): the
/// app's list (`ViewModel.list`)
/// in SwiftUI, which is AppKit's own outline underneath, so VoiceOver hears each row's level and
/// expansion from it. The first of the Mac's lists in SwiftUI, judged by VoiceOver's own words
/// and every test as the AppKit outline was.
///
/// Rows nest as the app lists them, a parent folding by its own Expand or Collapse; the
/// selection is the app's row in hand, and moving it is the person's move; Space, Delete and
/// Return are the row's own actions of those kinds, and its context menu every one of them. A
/// block's Return and double click are its Edit Block, the form for every occurrence.
struct MacTaskListScreen: View {
    @ObservedObject var model: TaskListModel
    /// Goes to the details of the task in hand: the pane beside the list.
    let details: () -> Void
    @FocusState private var listFocused: Bool
    @FocusState private var filterFocused: Bool

    typealias Node = OutlineNode<TaskListModel.Row>

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if model.blocks {
                HStack {
                    Text(model.readback)
                        .font(.subheadline)
                        .foregroundStyle(Color.quietLabel)
                    Spacer()
                    Button("Add Block…") { model.core.app.send(.block(.new(start: nil, minutes: nil))) }
                }
                .padding(.horizontal, 12)
                .padding(.top, 12)
            }
            if model.filters {
                // The filter language, which completes: a strip beneath, and a list on Down.
                CompletingLine(app: model.core.app, syntax: .filter, text: $model.filter) {
                    TextField("Filter", text: $model.filter, prompt: Text("#Work & overdue, or search: words"))
                        .accessibilityLabel("Filter")
                        .focused($filterFocused)
                        .onSubmit {
                            // Typing is silent; finishing the filter says what it found, then goes to it.
                            Announcer.say(model.readback)
                            listFocused = true
                        }
                }
                .padding(.horizontal, 12)
                .padding(.top, 12)
            }
            if !model.blocks {
                Text(model.readback)
                    .font(.subheadline)
                    .foregroundStyle(Color.quietLabel)
                    .padding(.horizontal, 12)
            }
            List(selection: Binding(get: { model.focus }, set: { if let id = $0 {
                // A row chosen takes the list's focus with it: a click selects a SwiftUI list's
                // row without making the list first responder, and Space went elsewhere.
                listFocused = true
                model.moved(to: id)
            } })) {
                ForEach(Self.tree(model.rows)) { node in
                    NodeView(node: node, model: model)
                }
            }
            .accessibilityLabel(model.title)
            .focused($listFocused)
            .contextMenu(forSelectionType: String.self) { ids in
                if let id = ids.first, let row = model.rows.first(where: { $0.id == id })?.row {
                    ForEach(row.actions, id: \.self) { action in
                        Button(action.asks ? action.title + "…" : action.title) { model.run(action, on: row) }
                    }
                }
            } primaryAction: { _ in
                if !model.trash { open() }
            }
        }

        // Filter Tasks: the field takes focus as soon as it is on screen.
        .onChange(of: model.filterWanted, initial: true) { _, wanted in
            guard wanted else { return }
            model.filterWanted = false
            filterFocused = true
        }
    }

    /// Space, Delete and Return on the row in hand (`ListKeys`): Space marks it done or not, or
    /// restores it from the trash; Delete deletes it; Return opens it. Whether it took the key.
    func key(_ key: ListKey) -> Bool {
        guard let row = model.inHand else { return false }
        switch key {
        case .space:
            guard !model.blocks else { return false }
            model.run([.markDone, .markNotDone, .restore], on: row, saying: .markDone)
        case .returnKey:
            guard !model.trash else { return false }
            open()
        case .delete:
            model.run([.delete, .deleteForGood], on: row, saying: .delete)
        }
        return true
    }

    /// Return or a double click on the row in hand: a task's details beside the list, a block's
    /// Edit Block.
    private func open() {
        guard model.blocks else { return details() }
        if let row = model.inHand { model.run([.edit], on: row, saying: .edit) }
    }

    /// The rows as the outline shows them, in its order.
    var shown: [String] {
        Outline.shown(Self.tree(model.rows)) { $0.row.expanded != false }
    }

    /// The app's rows as a tree, nested by depth.
    static func tree(_ rows: [TaskListModel.Row]) -> [Node] {
        Outline.tree(rows, id: \.id, depth: \.row.depth)
    }

    /// A row, and inside a disclosure group the rows beneath it, folded by its own action.
    private struct NodeView: View {
        let node: Node
        @ObservedObject var model: TaskListModel

        var body: some View {
            if node.children.isEmpty {
                RowLabel(row: node.row, model: model).tag(node.id)
            } else {
                DisclosureGroup(isExpanded: Binding(get: { node.row.row.expanded != false }, set: { open in
                    let kind: LumennaApp.ActionKind = open ? .expand : .collapse
                    if let fold = node.row.row.actions.first(where: { $0.kind == kind }) { model.run(fold, on: node.row.row) }
                })) {
                    ForEach(node.children) { child in
                        AnyView(NodeView(node: child, model: model))
                    }
                } label: {
                    RowLabel(row: node.row, model: model)
                }
                // A disclosure group makes its label a heading, which no other row is ("Test,
                // heading, expanded"): a task with subtasks is a row like any other.
                .accessibilityRemoveTraits(.isHeader)
                .tag(node.id)
            }
        }
    }

    /// One task: a checkbox, the title, and the rest of its line beneath it.
    private struct RowLabel: View {
        let row: TaskListModel.Row
        @ObservedObject var model: TaskListModel

        var body: some View {
            let done = row.row.checked == true
            let detail = row.line?.detail ?? ""
            HStack(alignment: .top, spacing: 6) {
                if model.filters {
                    // For a pointer alone, as the web's is: Space and the row's menu mark it done
                    // for everyone else, and its line says "completed" in words. Left in, the
                    // check box was said before the title, and differently on a row with
                    // subtasks than on one without.
                    Toggle(isOn: Binding(get: { done }, set: { _ in
                        model.run([.markDone, .markNotDone], on: row.row, saying: .markDone)
                    })) { EmptyView() }
                    .toggleStyle(.checkbox)
                    .accessibilityHidden(true)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.row.title).strikethrough(done)
                    if !detail.isEmpty {
                        Text(detail)
                            .font(.subheadline)
                            .foregroundStyle(row.row.state.contains("overdue") ? Color.warningLabel : Color.quietLabel)
                    }
                }
            }
            // A group named by its whole line, as the phone says it: the title, then the rest
            // ("Test2, subtask, level 1"). As static text VoiceOver said its value alone and no
            // level or expansion; named by the title with the rest as its value, a row without
            // subtasks was said as "subtask". The level and whether it is expanded are the
            // outline's own to report. Found by its title (the identifier).
            .accessibilityElement(children: .contain)
            .accessibilityLabel(row.line?.text ?? row.row.title)
            .accessibilityIdentifier(row.row.title)
        }
    }
}

/// The SwiftUI list in the main window's middle pane, hosted by a view that steps out of
/// VoiceOver's tree (`FlatHostingView`): left as a group, it was a nameless one around the list.
final class TaskListViewController: NSViewController {
    private let core: Core
    private let model: TaskListModel
    private let screen: MacTaskListScreen
    var place: LumennaApp.Place { model.place }
    var trash: Bool { model.trash }
    /// Whether the list has a filter to put in hand.
    var filters: Bool { model.filters }

    init(core: Core, window: MainWindowController, place: LumennaApp.Place) {
        self.core = core
        let model = TaskListModel(core: core, place: place)
        self.model = model
        screen = MacTaskListScreen(model: model, details: { [weak window] in window?.showDetails() })
        super.init(nibName: nil, bundle: nil)
        title = model.title
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
                    guard let self else { return false }
                    outline.inHand(shown: self.screen.shown) { self.model.moved(to: $0) }
                    return self.screen.key(key)
                }
            }
            outline.takeFocusIfNowhere()
        }
    }

    func addTask() {
        guard let window = view.window else { return }
        QuickAddSheet.present(on: window, core: core)
    }

    /// Puts the filter in hand: SwiftUI's focus, taken once the field is on screen.
    func focusFilter() {
        guard model.filters else { return }
        model.filterWanted = true
    }

    /// The in-hand task's id, for the Task menu.
    var selectedID: String? { model.focus }
}
