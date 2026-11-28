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
    private let core: Core
    private weak var asker: WatchAsker?
    private var blockForm: AppBlockFormModel?
    private var proposal: AppProposalModel?
    private var reminders: RemindersModel?
    private var observer: NSObjectProtocol?

    init(core: Core, asker: WatchAsker) {
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
            asker.show(RemindersForm(model: model).navigationTitle(view.remindersTitle ?? ""), gone: { app.send(.closeReminders) })
        }
    }
}

/// The core's root list (`Arrangement.watch`, as a watch that syncs through its phone has it):
/// New Task, the places as every app's sidebar has them (Today, Tasks, the projects, labels and
/// saved filters, Blocks and Trash, folded as the app has them), Undo, Redo, What Should I Work
/// On? and Settings. Then syncing with the iPhone, which stands for the Sync Now the list leaves
/// out.
struct RootView: View {
    @EnvironmentObject private var core: Core
    @EnvironmentObject private var phone: PhoneSync
    @State private var adding = false
    @StateObject private var asker: WatchAsker
    @StateObject private var questions = Questions()
    /// What a reminder chosen in its notification is about: its task, or the day.
    @State private var remindedTask: TaskOpened?
    @State private var remindedDay: LumennaApp.Place?
    /// Where a complication tapped took the app: the core's launch, as the app left it.
    @State private var launched: LumennaApp.Place?

    init(core: Core) {
        _asker = StateObject(wrappedValue: WatchAsker(core: core))
    }

    var body: some View {
        NavigationStack {
            List {
                // The core's root list: New Task, the places, then the rest of Home's actions
                // (Undo, Redo, What Should I Work On?, Settings), a section of their own after the
                // last place.
                let rows = self.rows
                let foot = (rows.lastIndex { if case .place = $0 { true } else { false } }).map { $0 + 1 } ?? 0
                ForEach(rows[..<foot], id: \.id) { row($0) }
                Section {
                    ForEach(rows[foot...], id: \.id) { row($0) }
                }
                Section(Wording.phone) {
                    Button(LumennaApp.syncWith(phone: Wording.thePhone)) { phone.syncNow() }
                    Text(phone.status).font(.footnote)
                }
            }
            .navigationTitle(Wording.metadata.name)
            .navigationDestination(for: LumennaApp.Place.self) { place in
                PlaceView(core: core, place: place)
            }
            .navigationDestination(item: $remindedTask) { task in
                TaskView(id: task.id)
            }
            .navigationDestination(item: $remindedDay) { place in
                PlaceView(core: core, place: place)
            }
            .navigationDestination(item: $launched) { place in
                PlaceView(core: core, place: place)
            }
            .onOpenURL { url in
                // A complication tapped, on one of the core's launches.
                guard let id = LaunchURL.id(in: url) else { return }
                Launches.shared.ask(id)
            }
            .onReceive(NotificationCenter.default.publisher(for: Opened.reminded)) { note in
                guard let reminded = note.object as? RemindedOf else { return }
                if reminded.replan {
                    // The overrun warning's Replan: Replan from Now, the phone's screen.
                    core.app.replan(from: reminded)
                } else if reminded.kind == "task" {
                    remindedTask = TaskOpened(id: reminded.target)
                } else {
                    // The block on its day, in hand: the app's, as on every app.
                    core.app.send(.showBlock(series: reminded.target, date: reminded.day))
                    remindedDay = .today
                }
            }
            .sheet(isPresented: $adding) {
                QuickAddView(fresh: true).showsWhatIsSaid()
            }
            .sheet(item: $asker.asked, onDismiss: { asker.dismissed() }) { asked in
                NavigationStack { asked.view }
                    .showsWhatIsSaid()
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
                // A complication's or a control's launch (`NewTaskControlIntent`), and any that
                // came before this screen did: the app's place, or quick add straight away if the
                // app opened its line (New Task).
                Launches.shared.handle { id in
                    core.app.send(.launch(id))
                    core.app.whenStill {
                        if core.app.view.line != nil { adding = true } else { launched = core.app.view.place }
                    }
                }
            }
            // The core's sentence is the whole alert: what could not be done and why.
            .alert(core.failure ?? "", isPresented: failed) {
                Button(Wording.close) { core.failure = nil }
            }
        }
        // What the app says, shown for sighted wearers; the asker's sheets show it over themselves.
        .showsWhatIsSaid()
        .environmentObject(asker)
    }

    /// The core's root list (`Arrangement.watch`): every place shown as the sidebar has it,
    /// folded away left out, each saying whether it is folded and, where it changes, its level;
    /// what can be done from anywhere around them.
    private var rows: [LumennaApp.RootRow] {
        _ = core.generation
        return core.app.arrangement.watch
    }

    private var failed: Binding<Bool> {
        Binding(get: { core.failure != nil }, set: { if !$0 { core.failure = nil } })
    }

    @ViewBuilder
    private func row(_ row: LumennaApp.RootRow) -> some View {
        switch row {
        case let .place(entry): place(AppItem(entry: entry), entry: entry)
        case let .action(action): self.action(action)
        }
    }

    /// One of what can be done from anywhere, as a button named by the core: New Task brings up
    /// quick add, which opens the app's line itself; Settings is the watch's own screen of them;
    /// any other is the core's key. What Should I Work On? and Energy Today take the app to today
    /// with their question asked, as their launches do: the day is pushed under the asker's
    /// sheet, as a launch's is.
    @ViewBuilder
    private func action(_ action: LumennaApp.ScreenAction) -> some View {
        switch action.id {
        case "new-task": Button(action.title) { adding = true }
        case "settings": NavigationLink(action.title) { SettingsView() }
        case "what-now", "energy-today":
            ScreenActionButton(action: action) {
                core.app.perform($0)
                core.app.whenStill { launched = core.app.view.place }
            }
        default: ScreenActionButton(action: action) { core.app.perform($0) }
        }
    }

    @ViewBuilder
    private func place(_ item: AppItem, entry: LumennaApp.SidebarEntry) -> some View {
        let fold = item.actions.first { [.expand, .collapse].contains($0.kind) }
        switch entry.kind {
        case let .place(place):
            // Named with what is in it, "Inbox, no tasks", as the phone's sidebar says it; its
            // value whether it is folded, and its level where that changes.
            NavigationLink(value: place) {
                HStack {
                    SwatchMark(swatch: item.swatch)
                    EmojiMark(emoji: item.emoji)
                    Text(entry.text)
                }
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
        }
    }
}

/// What a place opens to: the day for Today, the block list for Blocks, else its tasks.
struct PlaceView: View {
    /// Handed in, not observed: rebuilt on every turn of the app, the place shown was never
    /// pushed.
    let core: Core
    let place: LumennaApp.Place

    var body: some View {
        switch place {
        case .today: DayView(core: core)
        case .blocks: BlockListView(core: core)
        default: TaskListView(core: core, place: place)
        }
    }
}

extension LumennaApp.RootRow {
    /// What tells it apart in the list: a place's key, an action's id.
    var id: String {
        switch self {
        case let .place(entry): entry.key
        case let .action(action): action.id
        }
    }
}
