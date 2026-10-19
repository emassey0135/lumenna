import LumennaApp
import SwiftUI

/// The app's questions and the screens it opens by itself, kept for as long as the root is
/// shown: each a sheet of the root's one asker.
final class Questions: ObservableObject {
    var asking: AppQuestions?
    var screens: WatchScreens?
}

/// The screens the app opens by itself — the block form, a proposal, a reminder list — shown
/// as sheets when the app opens one and closed when it lets it go; one closed by the person is
/// put away in the app.
final class WatchScreens {
    private let core: WatchCore
    private weak var asker: WatchAsker?
    private var blockForm: AppBlockFormModel?
    private var proposal: AppProposalModel?
    private var reminders: RemindersModel?
    private var observer: NSObjectProtocol?

    init(core: WatchCore, asker: WatchAsker) {
        self.core = core
        self.asker = asker
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.update()
        }
        update()
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    private func update() {
        guard let asker else { return }
        let view = core.app.view
        let app = core.app
        if view.blockForm != nil, blockForm == nil {
            let model = AppBlockFormModel(core: core)
            model.close = { [weak self, weak asker] in
                self?.blockForm = nil
                asker?.close()
            }
            blockForm = model
            asker.show(AppBlockForm(model: model).navigationTitle(model.heading), gone: { app.send(.block(.cancel)) })
        }
        if view.proposal != nil, proposal == nil {
            let model = AppProposalModel(core: core)
            model.close = { [weak self, weak asker] in
                self?.proposal = nil
                asker?.close()
            }
            proposal = model
            asker.show(AppProposalForm(model: model).navigationTitle(model.title), gone: { app.send(.proposal(.close)) })
        }
        if view.reminders != nil, reminders == nil {
            let model = RemindersModel(core: core)
            model.close = { [weak self, weak asker] in
                self?.reminders = nil
                asker?.close()
            }
            reminders = model
            asker.show(RemindersForm(model: model).navigationTitle("Reminders"), gone: { app.send(.closeReminders) })
        }
    }
}

/// The places, as every app's sidebar has them, from the app: Today, Tasks, the projects,
/// labels and saved filters, Blocks and Trash, folded as the app has them. Then Undo, Redo,
/// and syncing with the iPhone.
struct RootView: View {
    @EnvironmentObject private var core: WatchCore
    @EnvironmentObject private var phone: PhoneSync
    @State private var adding = false
    @StateObject private var asker: WatchAsker
    @StateObject private var questions = Questions()
    /// What a reminder chosen in its notification is about: its task, or the day.
    @State private var remindedTask: TaskOpened?
    @State private var remindedDay: LumennaApp.Place?

    init(core: WatchCore) {
        _asker = StateObject(wrappedValue: WatchAsker(core: core))
    }

    var body: some View {
        NavigationStack {
            List {
                Button("New Task") { adding = true }
                let shown = entries
                ForEach(Array(shown.enumerated()), id: \.element.key) { _, item in
                    row(item)
                }
                Section {
                    Button("Undo") { core.app.send(.undo) }
                    Button("Redo") { core.app.send(.redo) }
                    NavigationLink("Settings") { SettingsView() }
                }
                Section("iPhone") {
                    Button("Sync with iPhone") { phone.syncNow() }
                    Text(phone.status).font(.footnote)
                }
            }
            .navigationTitle("Lumenna")
            .navigationDestination(for: LumennaApp.Place.self) { place in
                PlaceView(core: core, place: place)
            }
            .navigationDestination(item: $remindedTask) { task in
                TaskView(id: task.id)
            }
            .navigationDestination(item: $remindedDay) { place in
                PlaceView(core: core, place: place)
            }
            .onReceive(NotificationCenter.default.publisher(for: Opened.reminded)) { note in
                guard let reminded = note.object as? RemindedOf else { return }
                if reminded.replan {
                    // The overrun warning's Replan: Replan from Now, the phone's screen.
                    core.app.send(.open(.today))
                    core.app.send(.day(.now))
                    core.app.send(.proposal(.replan))
                } else if reminded.kind == "task" {
                    remindedTask = TaskOpened(id: reminded.target)
                } else {
                    remindedDay = .today
                }
            }
            .sheet(isPresented: $adding) {
                QuickAddView(fresh: true)
            }
            .sheet(item: $asker.asked, onDismiss: { asker.dismissed() }) { asked in
                NavigationStack { asked.view }
                    .environmentObject(core)
                    .environmentObject(asker)
            }
            .onAppear {
                // The app's questions, asked here as sheets, and the forms they open: a task's
                // details are its own screen.
                if questions.asking == nil {
                    questions.asking = AppQuestions(app: core.app, asker: asker) { action in
                        if action.subject == .task || action.kind == .editTask {
                            remindedTask = TaskOpened(id: action.target)
                        }
                    }
                    questions.asking?.answeredElsewhere = RemindersModel.answersItself
                    questions.screens = WatchScreens(core: core, asker: asker)
                }
            }
            .alert("Could not do that", isPresented: failed) {
                Button("OK") { core.failure = nil }
            } message: {
                Text(core.failure ?? "")
            }
        }
        .environmentObject(asker)
    }

    /// The sidebar's rows, those folded away left out, each saying whether it is folded and,
    /// where it changes, its level.
    private var entries: [AppItem] {
        _ = core.generation
        return core.app.sidebarItems()
    }

    private var failed: Binding<Bool> {
        Binding(get: { core.failure != nil }, set: { if !$0 { core.failure = nil } })
    }

    @ViewBuilder
    private func row(_ item: AppItem) -> some View {
        let entry = core.app.sidebarEntry(item.key)
        let fold = item.actions.first { [.expand, .collapse].contains($0.kind) }
        switch entry?.kind {
        case let .place(place):
            // Named with what is in it, "Inbox, no tasks", as the phone's sidebar says it; its
            // value whether it is folded, and its level where that changes.
            NavigationLink(value: place) {
                Text(item.detail.map { "\(item.title), \($0)" } ?? item.title)
            }
            .accessibilityValue(AppModel.spoken(detail: nil, expanded: item.expanded, level: item.level))
            .swipeActions(edge: .trailing) {
                if let fold {
                    Button(fold.title) { core.app.run(fold, on: item.key, side: .sidebar) }
                }
            }
        case .group:
            // A heading folds what is under it when pressed: its own Expand or Collapse.
            Button {
                if let fold { core.app.run(fold, on: item.key, side: .sidebar) }
            } label: {
                Text(item.title).font(.headline)
            }
            .accessibilityAddTraits(.isHeader)
            .accessibilityValue(AppModel.spoken(detail: nil, expanded: item.expanded, level: item.level))
            if fold?.kind != .expand {
                // What Browse adds under each heading: its New, the app's.
                ForEach(item.actions.filter { $0.kind == .new }, id: \.self) { action in
                    Button(action.title) { core.app.run(action, on: item.key, side: .sidebar) }
                }
            }
        case nil:
            EmptyView()
        }
    }
}

/// What a place opens to: the day for Today, the block list for Blocks, else its tasks.
struct PlaceView: View {
    /// Handed in, not observed: rebuilt on every turn of the app, the place shown was never
    /// pushed.
    let core: WatchCore
    let place: LumennaApp.Place

    var body: some View {
        switch place {
        case .today: DayView(core: core)
        case .blocks: BlockListView(core: core)
        default: TaskListView(core: core, place: place)
        }
    }
}
