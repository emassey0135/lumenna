import LumennaApp
import SwiftUI
import UIKit

/// A place's tasks, with a filter field above them, the trash, or every block series (Blocks):
/// the app's list
/// (`ViewModel.list`) while this screen shows its place, which it opens as it comes to the front.
///
/// The first of the iPhone's lists in SwiftUI, which VoiceOver, the audits and every test judged
/// as the UIKit list was: each row says its title, then the rest of its line, its
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
    /// The list of what fits, open: the field's Show Completions action, its button, or Down.
    @State private var listing = false

    var body: some View {
        ScrollViewReader { scroller in
            List {
                Section {
                    if model.filters {
                        TextField(QuickAddText.filter, text: $model.filter, prompt: Text(QuickAddText.filterHint), axis: .vertical)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .focused($typing)
                            .accessibilityLabel(QuickAddText.filter)
                            .onSubmit {
                                typing = false
                                // Typing is silent; finishing the filter says what it found.
                                Announcer.say(model.readback)
                            }
                            .onChange(of: model.filter) { _, text in complete(text) }
                            .accessibilityAction(named: CompletionWords.show) { showCompletions() }
                            .onKeyPress(.downArrow) {
                                showCompletions()
                                return .handled
                            }
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
                                Button(CompletionWords.show) { showCompletions() }
                            }
                        }
                    }
                }
            }
            .sheet(isPresented: $listing) {
                CompletionList(offered: offered) { candidate in
                    listing = false
                    if let candidate, let offered { insert(candidate, at: offered) }
                }
            }
            .onChange(of: model.landed?.count) { _, _ in
                // The app moved the row in hand: VoiceOver goes there, once the row is drawn.
                guard let key = model.landed?.key else { return }
                scroller.scrollTo(key, anchor: .center)
                DispatchQueue.main.async { spoken = key }
            }
            .onChange(of: spoken) { _, key in
                if let key { model.moved(to: key) }
            }
            // Filter Tasks: the field takes focus as soon as it is on screen.
            .onChange(of: model.filterWanted, initial: true) { _, wanted in
                guard wanted else { return }
                model.filterWanted = false
                typing = true
            }
        }
    }

    private func rowView(_ row: TaskListModel.Row) -> some View {
        let actions = row.actions
        // Split as every iPhone list splits them (`AppRowActions`), each listed once.
        let split = TaskListScreen.actions(of: row, blocks: model.blocks) { _ in }
        let leading = split.leading
        let folds = actions.filter { [.expand, .collapse].contains($0.kind) }
        let trailing = split.trailing
        let others = split.others
        let done = row.row.checked == true
        let detail = row.line?.detail ?? ""
        return Button {
            guard !model.trash else { return }
            model.moved(to: row.id)
            // A block's own screen is the block form, for every occurrence: its Edit Block.
            if model.blocks {
                model.run([.edit], on: row.row, saying: .edit)
            } else {
                open(row.row)
            }
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                if !model.blocks {
                    Image(systemName: done ? "checkmark.circle.fill" : "circle")
                        .foregroundStyle(done ? Color.accentColor : Color.quietLabel)
                }
                SwatchMark(swatch: row.row.swatch)
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
        .accessibilityHint(model.trash ? "" : model.blocks ? "Edits every occurrence" : "Shows details")
        .accessibilityAddTraits(.isButton)
        .accessibilityFocused($spoken, equals: row.id)
        .swipeActions(edge: .leading) {
            ForEach(leading, id: \.self) { action in
                Button(action.title) { model.run(action, on: row.row) }.tint(.green)
            }
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
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

    /// A row's actions as the list offers them (`AppRowActions`): a task's Mark Done swiped from
    /// the leading edge, the other primary ones and Expand or Collapse from the trailing, the
    /// rest as accessibility actions, every one in the context menu.
    static func actions(
        of row: TaskListModel.Row, blocks: Bool = false, run: @escaping (LumennaApp.Offered) -> Void
    ) -> AppRowActions<LumennaApp.Offered> {
        AppRowActions(actions: row.actions, doneLeads: !blocks, run: run)
    }

    private func showCompletions() {
        complete(model.filter)
        listing = true
    }

    private func complete(_ text: String) {
        offered = try? model.core.app.complete(text, cursor: UInt32(text.utf8.count), syntax: .filter)
    }

    private func insert(_ candidate: LumennaApp.Candidate, at offered: LumennaApp.Completions) {
        model.filter = CompletionText.insert(candidate.text, into: model.filter, start: offered.start, end: offered.end).text
    }
}

/// The SwiftUI list in the UIKit shell: the bars, the keyboard's commands and a task's own
/// screen are UIKit's, as for every screen.
final class TaskListViewController: UIHostingController<TaskListScreen> {
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
            add.accessibilityLabel = place == .blocks ? "Add block" : "Add task"
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
        offerRotors()
    }

    /// VoiceOver's Overdue rotor, from one overdue task to the next (`RowRotor`), on the list
    /// and the view holding it: VoiceOver collects a rotor from any container above its row.
    private func offerRotors() {
        guard place != .trash, place != .blocks else { return }
        let model = model
        let overdue = RowRotor.make(
            "Overdue", in: self,
            rows: { model.shown.map(\.id) },
            wanted: { index in model.shown.indices.contains(index) && model.shown[index].row.state.contains("overdue") },
            current: { model.focus },
            landed: { model.moved(to: $0) }
        )
        view.accessibilityCustomRotors = [overdue]
        RowRotor.list(in: view)?.accessibilityCustomRotors = [overdue]
    }

    @objc func undoChange() { undoInStore(core) }
    @objc func redoChange() { redoInStore(core) }

    /// Quick add, or in Blocks the block form for a new one.
    @objc func addTask() {
        guard place != .trash else { return }
        guard place != .blocks else { return core.app.send(.block(.new(start: nil, minutes: nil))) }
        let adding = QuickAddViewController(core: core)
        adding.closed = { [weak self] in self?.takeKeyboardCommands() }
        present(UINavigationController(rootViewController: adding), animated: true)
    }

    /// Puts the filter in hand: SwiftUI's focus, taken once the field is on screen.
    func focusFilter() {
        guard model.filters else { return }
        model.filterWanted = true
    }

    // The Task menu's commands act on the row in hand, the app's (`KeyboardCommands`).
    override var canBecomeFirstResponder: Bool { true }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        switch action {
        case #selector(toggleDone as () -> Void), #selector(moveToTrash), #selector(putInBlock), #selector(moveToProject),
             #selector(makeSubtask), #selector(waitFor):
            model.inHand != nil
        case #selector(filterTasks): model.filters
        default: super.canPerformAction(action, withSender: sender)
        }
    }

    @objc func toggleDone() {
        guard let row = model.inHand else { return }
        model.run([.markDone, .markNotDone], on: row, saying: .markDone)
    }

    // The row's own action of each kind, or the app says why it has none.
    @objc func putInBlock() { runInHand(.putInBlock) }
    @objc func moveToProject() { runInHand(.moveToProject) }
    @objc func makeSubtask() { runInHand(.makeSubtaskOf) }
    @objc func waitFor() { runInHand(.waitFor) }

    private func runInHand(_ kind: LumennaApp.ActionKind) {
        guard let row = model.inHand else { return }
        model.run([kind], on: row, saying: kind)
    }

    @objc func moveToTrash() {
        guard let row = model.inHand else { return }
        // The core's Delete: Move to Trash, or Delete from Trash in the trash, never Erase
        // Permanently.
        model.core.app.key("delete", on: row.id)
    }

    @objc func filterTasks() {
        focusFilter()
    }
}
