import SwiftUI

/// The places, as every app's sidebar has them, from the core: Today, Tasks, the projects,
/// labels and saved filters, Blocks and Trash. Then Undo, Redo, and syncing with the iPhone.
struct RootView: View {
    @EnvironmentObject private var core: WatchCore
    @EnvironmentObject private var phone: PhoneSync
    @State private var adding = false
    @StateObject private var asker: WatchAsker
    @State private var folding = Folding()

    init(core: WatchCore) {
        _asker = StateObject(wrappedValue: WatchAsker(core: core))
    }

    var body: some View {
        NavigationStack {
            List {
                Button("New Task") { adding = true }
                // Headings and projects with subprojects fold, as in the phone's sidebar.
                let shown = folding.shown(entries, depth: { Int($0.depth) }, key: Self.key)
                ForEach(Array(shown.enumerated()), id: \.offset) { index, entry in
                    row(entry, level: Folding.levelChange(shown, at: index))
                }
                Section {
                    Button("Undo") { core.act { try core.lumenna.undo() } }
                    Button("Redo") { core.act { try core.lumenna.redo() } }
                    NavigationLink("Settings") { SettingsView() }
                }
                Section("iPhone") {
                    Button("Sync with iPhone") { phone.syncNow() }
                    Text(phone.status).font(.footnote)
                }
            }
            .navigationTitle("Lumenna")
            .navigationDestination(for: Place.self) { place in
                PlaceView(place: place)
            }
            .sheet(isPresented: $adding) {
                QuickAddView(prefix: "")
            }
            .sheet(item: $asker.asked) { asked in
                NavigationStack { asked.view }
            }
            .alert("Could not do that", isPresented: failed) {
                Button("OK") { core.failure = nil }
            } message: {
                Text(core.failure ?? "")
            }
        }
    }

    private var entries: [SidebarEntry] {
        _ = core.generation
        return core.lumenna.places().entries
    }

    private var failed: Binding<Bool> {
        Binding(get: { core.failure != nil }, set: { if !$0 { core.failure = nil } })
    }

    /// What identifies a place for folding and for its row: a project and a label can share
    /// a name.
    private static func key(_ entry: SidebarEntry) -> String {
        switch entry.kind {
        case let .place(place): "place:\(place)"
        case let .group(group): "group:\(group)"
        }
    }

    private func toggle(_ entry: SidebarEntry, saying said: String) {
        folding.toggle(Self.key(entry))
        Announcer.say(said)
    }

    @ViewBuilder
    private func row(_ shown: Folding.Shown<SidebarEntry>, level: String?) -> some View {
        let entry = shown.item
        // Depth is said where it changes, as every app says it, never by indentation.
        let value = [shown.state, level].compactMap { $0 }.joined(separator: ", ")
        switch entry.kind {
        case let .place(place):
            NavigationLink(value: place) {
                Text(entry.text)
            }
            .accessibilityValue(value)
            .swipeActions(edge: .trailing) {
                if let fold = Folding.action(for: shown) {
                    Button(fold.title) { toggle(entry, saying: fold.said) }
                }
            }
        case .group:
            // A heading folds what is under it when pressed.
            Button {
                if let fold = Folding.action(for: shown) { toggle(entry, saying: fold.said) }
            } label: {
                Text(entry.text).font(.headline)
            }
            .accessibilityAddTraits(.isHeader)
            .accessibilityValue(value)
            if !shown.collapsed {
                // What Browse adds under each heading: its New, the core's.
                ForEach(entry.actions, id: \.self) { action in
                    Button(action.title) { asker.run(action, form: { _ in newFilter() }) }
                }
            }
        }
    }

    /// New Saved Filter, the app's own form: a name, then a query.
    private func newFilter() {
        // The core's two questions: the name, then the query.
        let steps = TextQuestion.newFilter
        guard steps.count == 2 else { return }
        let (named, queried) = (steps[0], steps[1])
        asker.show(TextPrompt(named.title, placeholder: named.label, action: named.yes) { name in
            asker.show(TextPrompt(queried.title, message: queried.hint, placeholder: queried.label, action: queried.yes, syntax: .filter) { query in
                core.act { try core.lumenna.addFilter(name: name, query: query) }
            })
        })
    }
}

/// What a place opens to: the day for Today, the block list for Blocks, else its tasks.
struct PlaceView: View {
    @EnvironmentObject private var core: WatchCore
    let place: Place

    var body: some View {
        switch place {
        case .today: DayView(core: core)
        case .blocks: BlockListView(core: core)
        default: TaskListView(place: place, core: core)
        }
    }
}
