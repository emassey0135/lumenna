import AppKit
import LumennaApp
import SwiftUI

/// A place's tasks in SwiftUI, the trial of the Mac's lists in SwiftUI beside the AppKit
/// `TaskListViewController`, chosen by `LUMENNA_SWIFTUI` until it is judged.
///
/// The same rules as the outline: rows nest as the app lists them, a parent folding by its own
/// Expand or Collapse; the selection is the app's row in hand, and moving it is the person's
/// move; Space, Delete and Return are the row's own actions of those kinds, and its context
/// menu every one of them.
struct MacTaskListScreen: View {
    @ObservedObject var model: TaskListModel
    /// Goes to the details of the task in hand: the pane beside the list.
    let details: () -> Void
    @FocusState private var listFocused: Bool

    /// A row and the rows beneath it.
    struct Node: Identifiable {
        let row: TaskListModel.Row
        var children: [Node] = []
        var id: String { row.id }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !model.trash {
                TextField("Filter", text: $model.filter, prompt: Text("#Work & overdue, or search: words"))
                    .accessibilityLabel("Filter")
                    .onSubmit {
                        // Typing is silent; finishing the filter says what it found, then goes to it.
                        Announcer.say(model.readback)
                        listFocused = true
                    }
                    .padding(.horizontal, 12)
                    .padding(.top, 12)
            }
            Text(model.readback)
                .font(.subheadline)
                .foregroundStyle(Color.quietLabel)
                .padding(.horizontal, 12)
            List(selection: Binding(get: { model.focus }, set: { if let id = $0 { model.moved(to: id) } })) {
                ForEach(Self.tree(model.rows)) { node in
                    NodeView(node: node, model: model)
                }
            }
            .accessibilityLabel(model.title)
            .focused($listFocused)
            .onKeyPress(.space) {
                guard let row = model.inHand else { return .ignored }
                model.run([.markDone, .markNotDone, .restore], on: row, saying: .markDone)
                return .handled
            }
            .onKeyPress(.return) {
                guard !model.trash, model.inHand != nil else { return .ignored }
                details()
                return .handled
            }
            .onDeleteCommand {
                guard let row = model.inHand else { return }
                model.run([.delete, .deleteForGood], on: row, saying: .delete)
            }
            .contextMenu(forSelectionType: String.self) { ids in
                if let id = ids.first, let row = model.rows.first(where: { $0.id == id })?.row {
                    ForEach(row.actions, id: \.self) { action in
                        Button(action.asks ? action.title + "…" : action.title) { model.run(action, on: row) }
                    }
                }
            } primaryAction: { _ in
                if !model.trash { details() }
            }
        }
        .onAppear { listFocused = true }
    }

    /// The flat, depth-first rows the app sends, as a tree: a row's parent is the nearest
    /// shallower row above it. One whose parent was filtered out sits at the top.
    static func tree(_ rows: [TaskListModel.Row]) -> [Node] {
        var top: [Node] = []
        // The chain of open parents, as paths into `top`.
        var chain: [(depth: UInt32, path: [Int])] = []
        for row in rows {
            while let last = chain.last, last.depth >= row.row.depth { chain.removeLast() }
            let node = Node(row: row)
            if let parent = chain.last {
                var path = parent.path
                let index = Self.append(node, at: path, in: &top)
                path.append(index)
                chain.append((row.row.depth, path))
            } else {
                top.append(node)
                chain.append((row.row.depth, [top.count - 1]))
            }
        }
        return top
    }

    private static func append(_ node: Node, at path: [Int], in nodes: inout [Node]) -> Int {
        guard let first = path.first else { return 0 }
        if path.count == 1 {
            nodes[first].children.append(node)
            return nodes[first].children.count - 1
        }
        return append(node, at: Array(path.dropFirst()), in: &nodes[first].children)
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
                if !model.trash {
                    Toggle(isOn: Binding(get: { done }, set: { _ in
                        model.run([.markDone, .markNotDone], on: row.row, saying: .markDone)
                    })) { EmptyView() }
                    .toggleStyle(.checkbox)
                    .accessibilityLabel(done ? "Done, \(row.row.title)" : "Mark \(row.row.title) done")
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
            // Said as the phone says it: the title, then the rest of its line, the check box a
            // control inside it as in the AppKit cell; combined, the row became a check box named
            // by its title and the line was lost. The level and whether it is expanded are the
            // outline's own to report.
            .accessibilityElement(children: .contain)
            .accessibilityLabel(row.row.title)
            .accessibilityValue(detail)
        }
    }
}

/// What the window and the menus ask of a task list, AppKit's or SwiftUI's.
protocol MacTaskListing: NSViewController {
    var place: LumennaApp.Place { get }
    var trash: Bool { get }
    func addTask()
    func focusFilter()
}

extension TaskListViewController: MacTaskListing {}

/// A place's task list, AppKit's or, under `LUMENNA_SWIFTUI`, the SwiftUI trial's.
func taskList(core: Core, window: MainWindowController, place: LumennaApp.Place) -> NSViewController & MacTaskListing {
    ProcessInfo.processInfo.environment["LUMENNA_SWIFTUI"] != nil
        ? TaskListHostingController(core: core, window: window, place: place)
        : TaskListViewController(core: core, window: window, place: place)
}

/// The SwiftUI list in the main window's middle pane, hosted by a view that steps out of
/// VoiceOver's tree (`FlatHostingView`): left as a group, it was a nameless one around the list.
final class TaskListHostingController: NSViewController, MacTaskListing {
    private let core: Core
    private let model: TaskListModel
    private let screen: MacTaskListScreen
    var place: LumennaApp.Place { model.place }
    var trash: Bool { model.trash }

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

    func addTask() {
        guard !trash, let window = view.window else { return }
        QuickAddSheet.present(on: window, core: core)
    }

    func focusFilter() {
        guard !trash, let window = view.window else { return }
        window.makeFirstResponder(nil)
        // The field is SwiftUI's: the first text field in the pane.
        func field(in view: NSView) -> NSView? {
            if view is NSTextField, (view as? NSTextField)?.isEditable == true { return view }
            return view.subviews.lazy.compactMap(field).first
        }
        if let found = field(in: view) { window.makeFirstResponder(found) }
    }

    /// The in-hand task's id, for the Task menu.
    var selectedID: String? { model.focus }
}
