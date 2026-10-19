import LumennaApp
import SwiftUI
import UIKit

/// A place's tasks in SwiftUI, the trial of the iPhone's lists in SwiftUI beside the UIKit
/// `TaskListViewController`, chosen by `LUMENNA_SWIFTUI` until it is judged.
///
/// The same rules as the UIKit list: each row says its title, then the rest of its line, its
/// folding and its level; its primary actions are its swipes, Mark Done from the leading edge,
/// the rest and Expand or Collapse from the trailing, every other one an accessibility action,
/// and all of them in its context menu. VoiceOver goes to the row in hand only when the app
/// moved it, and where VoiceOver goes is the app's row in hand.
struct TaskListScreen: View {
    @ObservedObject var model: TaskListModel
    /// Opens a task's own screen, beside the list on iPad: UIKit's.
    let open: (LumennaApp.RowView) -> Void
    @AccessibilityFocusState private var spoken: String?
    @FocusState private var typing: Bool
    /// What could finish the filter's last word, the app's.
    @State private var offered: LumennaApp.Completions?

    var body: some View {
        List {
            Section {
                if !model.trash {
                    TextField("Filter", text: $model.filter, prompt: Text("#Work & overdue"), axis: .vertical)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .focused($typing)
                        .accessibilityLabel("Filter")
                        .onSubmit {
                            typing = false
                            // Typing is silent; finishing the filter says what it found.
                            Announcer.say(model.readback)
                        }
                        .onChange(of: model.filter) { _, text in complete(text) }
                }
                // The readback: how the query was understood, and how many it found. A
                // misread filter shows wrong results silently, and wrong results are invisible.
                Text(model.readback)
                    .font(.footnote)
                    .foregroundStyle(Color.quietLabel)
            }
            ForEach(model.shown) { row in
                rowView(row)
            }
        }
        .listStyle(.plain)
        .navigationTitle(model.title)
        .toolbar {
            ToolbarItemGroup(placement: .keyboard) {
                // What could finish the last word, the app's, above the keyboard.
                if let offered, !offered.candidates.isEmpty {
                    ScrollView(.horizontal) {
                        HStack {
                            ForEach(offered.candidates, id: \.text) { candidate in
                                Button(candidate.text) { insert(candidate, at: offered) }
                                    .accessibilityLabel(LumennaApp.candidateLine(candidate: candidate))
                            }
                        }
                    }
                }
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

    private func rowView(_ row: TaskListModel.Row) -> some View {
        let actions = row.row.actions
        let leading = actions.first.map { $0.primary && [.markDone, .markNotDone].contains($0.kind) ? [$0] : [] } ?? []
        let folds = actions.filter { [.expand, .collapse].contains($0.kind) }
        let trailing = actions.filter { $0.primary && !leading.contains($0) && !folds.contains($0) } + folds
        let others = actions.filter { !leading.contains($0) && !trailing.contains($0) }
        let done = row.row.checked == true
        let detail = row.line?.detail ?? ""
        return Button {
            guard !model.trash else { return }
            model.moved(to: row.id)
            open(row.row)
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                Image(systemName: done ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(done ? Color.accentColor : Color.quietLabel)
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.row.title).strikethrough(done)
                    if !detail.isEmpty {
                        // Overdue is said in words as well as colour, never colour alone.
                        Text(detail)
                            .font(.subheadline)
                            .foregroundStyle(row.row.state.contains("overdue") ? Color.warningLabel : Color.quietLabel)
                    }
                }
                Spacer(minLength: 0)
            }
            // Depth shown by indentation, for sight only; VoiceOver hears it in words, where it
            // changes.
            .padding(.leading, CGFloat(row.row.depth) * 20)
            .contentShape(Rectangle())
        }
        .foregroundStyle(Color.primary)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(row.row.title)
        .accessibilityValue(AppModel.spoken(detail: detail, expanded: row.row.expanded, level: row.item.level))
        .accessibilityHint(model.trash ? "" : "Shows details")
        .accessibilityAddTraits(.isButton)
        .accessibilityFocused($spoken, equals: row.id)
        .swipeActions(edge: .leading) {
            ForEach(leading, id: \.self) { action in
                Button(action.title) { model.run(action, on: row.row) }.tint(.green)
            }
        }
        .swipeActions(edge: .trailing) {
            ForEach(trailing, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: row.row) }
                    .tint(folds.contains(action) ? Color.lumennaTint : nil)
            }
        }
        .accessibilityActions {
            // Only what is not swiped: SwiftUI offers the swipe actions to VoiceOver already.
            ForEach(others, id: \.self) { action in
                Button(action.title) { model.run(action, on: row.row) }
            }
        }
        .contextMenu {
            ForEach(actions, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: row.row) }
            }
        }
    }

    private func complete(_ text: String) {
        offered = try? model.core.app.complete(text, cursor: UInt32(text.utf8.count), syntax: .filter)
    }

    private func insert(_ candidate: LumennaApp.Candidate, at offered: LumennaApp.Completions) {
        model.filter = CompletionText.insert(candidate.text, into: model.filter, start: offered.start, end: offered.end).text
    }
}

/// What the screens and commands ask of a task list, UIKit's or SwiftUI's.
protocol TaskListing: UIViewController {
    var place: LumennaApp.Place { get }
    func addTask()
    func focusFilter(attempt: Int)
}

extension TaskListing {
    /// Puts the filter in hand, once the list is in the window.
    func focusFilter() { focusFilter(attempt: 0) }
}

extension TaskListViewController: TaskListing {}

/// A place's task list, UIKit's or, under `LUMENNA_SWIFTUI`, the SwiftUI trial's.
func taskList(core: Core, place: LumennaApp.Place = .tasks) -> UIViewController & TaskListing {
    ProcessInfo.processInfo.environment["LUMENNA_SWIFTUI"] != nil
        ? TaskListHostingController(core: core, place: place)
        : TaskListViewController(core: core, place: place)
}

/// The SwiftUI list in the UIKit shell: the bars, the keyboard's commands and a task's own
/// screen are UIKit's, as for every screen.
final class TaskListHostingController: UIHostingController<TaskListScreen>, TaskListing {
    private let core: Core
    private let model: TaskListModel
    var place: LumennaApp.Place { model.place }
    private var observer: NSObjectProtocol?

    init(core: Core, place: LumennaApp.Place) {
        self.core = core
        let model = TaskListModel(core: core, place: place)
        self.model = model
        weak var opener: UIViewController?
        super.init(rootView: TaskListScreen(model: model, open: { row in
            opener?.showBeside(TaskDetailViewController(core: core, id: row.id))
        }))
        opener = self
        title = model.title
        let undo = UIBarButtonItem.undo { [weak self] in self?.undoChange() }
        let redo = UIBarButtonItem.redo { [weak self] in self?.redoChange() }
        navigationItem.leftItemsSupplementBackButton = true
        navigationItem.leftBarButtonItems = place == .trash ? [undo] : [undo, redo]
        if place != .trash {
            let add = UIBarButtonItem(systemItem: .add, primaryAction: UIAction { [weak self] _ in self?.addTask() })
            add.accessibilityLabel = "Add task"
            navigationItem.rightBarButtonItem = add
        }
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            guard let self else { return }
            if self.title != self.model.title { self.title = self.model.title }
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

    @objc func undoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canUndo { typing.undo(); return }
        core.app.send(.undo)
    }

    @objc func redoChange() {
        if let typing = UIResponder.editingWithUndo, typing.canRedo { typing.redo(); return }
        core.app.send(.redo)
    }

    @objc func addTask() {
        guard place != .trash else { return }
        let adding = QuickAddViewController(core: core)
        adding.closed = { [weak self] in self?.takeKeyboardCommands() }
        present(UINavigationController(rootViewController: adding), animated: true)
    }

    func focusFilter(attempt: Int = 0) {
        // The field is SwiftUI's: first responder through the hierarchy once it is drawn.
        guard attempt < 100 else { return }
        if let field = view.window.flatMap({ Self.firstTextField(in: $0) }), field.becomeFirstResponder() { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.focusFilter(attempt: attempt + 1) }
    }

    private static func firstTextField(in view: UIView) -> UIView? {
        if view is UITextField || view is UITextView { return view }
        return view.subviews.lazy.compactMap(firstTextField).first
    }

    // The Task menu's commands act on the row in hand, the app's (`KeyboardCommands`).
    override var canBecomeFirstResponder: Bool { true }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        switch action {
        case #selector(toggleDone as () -> Void), #selector(moveToTrash): model.inHand != nil
        case #selector(filterTasks): place != .trash
        default: super.canPerformAction(action, withSender: sender)
        }
    }

    @objc func toggleDone() {
        guard let row = model.inHand else { return }
        model.run([.markDone, .markNotDone], on: row, saying: .markDone)
    }

    @objc func moveToTrash() {
        guard let row = model.inHand else { return }
        model.run([.delete], on: row, saying: .delete)
    }

    @objc func filterTasks() {
        focusFilter()
    }
}
