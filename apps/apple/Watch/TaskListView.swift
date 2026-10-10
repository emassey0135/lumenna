import SwiftUI

/// A place's tasks, each row as the phone says it: the title, then what the core says of it.
/// Its primary actions, the core's, are its swipe actions — Mark Done from the leading edge,
/// the rest from the trailing — which VoiceOver offers as the row's actions; every one is on
/// the task's own screen. Tasks can be filtered, with the filter read
/// back; a project, label or filter's own actions are at the foot, as Browse offers them.
struct TaskListView: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    let place: Place
    @State private var adding = false
    @State private var filter = ""
    @State private var folding = Folding()
    @StateObject private var asker: WatchAsker

    init(place: Place, core: WatchCore) {
        self.place = place
        _asker = StateObject(wrappedValue: WatchAsker(core: core))
    }

    var body: some View {
        let listing = rows
        List {
            if place == .tasks {
                TextField("Filter", text: $filter, prompt: example("#Work & overdue"))
                    .textInputAutocapitalization(.never)
                    .accessibilityLabel("Filter")
                CompletionOffer(text: $filter, syntax: .filter)
                // The readback: a misread filter shows wrong results silently.
                if let query = listing?.query {
                    Text(query.description).font(.footnote)
                }
            }
            if !trash {
                Button("New Task") { adding = true }
            }
            if let listing, listing.rows.isEmpty {
                // What is empty, in the core's words: "The trash is empty."
                Text(listing.empty.isEmpty ? listing.announcement : listing.empty)
            }
            // Folded as on the phone, the level said against the row shown before.
            let shown = folding.shown(listing?.rows ?? [], depth: { Int($0.depth) }, key: \.id)
            ForEach(Array(shown.enumerated()), id: \.element.item.id) { index, row in
                taskRow(row, after: index == 0 ? nil : shown[index - 1].item.depth)
            }
            PlaceActions(place: place, asker: asker, left: { dismiss() })
        }
        .navigationTitle(placeTitle(place: place))
        .navigationDestination(for: TaskOpened.self) { opened in
            TaskView(id: opened.id)
        }
        .sheet(isPresented: $adding) {
            QuickAddView(prefix: placeQuickAddPrefix(place: place))
        }
        .sheet(item: $asker.asked) { asked in
            NavigationStack { asked.view }
        }
    }

    private func taskRow(_ shown: Folding.Shown<RowView>, after previous: UInt32?) -> some View {
        let row = shown.item
        return NavigationLink(value: TaskOpened(id: row.id)) {
            // The row owns what VoiceOver says: the title as its name, then what the core
            // says of it as its value, as on the phone.
            VStack(alignment: .leading) {
                Text(row.title)
                if let detail = RowSpeech.details(row) {
                    Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(RowSpeech.label(row))
            .accessibilityValue(RowSpeech.value(row, previousDepth: previous, fold: shown.state))
        }
        .swipeActions(edge: .leading) {
            ForEach(leading(row.actions), id: \.self) { action in
                Button(action.title) { asker.run(action) }
            }
        }
        .swipeActions(edge: .trailing) {
            ForEach(row.actions.filter { $0.primary && !leading(row.actions).contains($0) }, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { asker.run(action) }
            }
            if let fold = Folding.action(for: shown) {
                Button(fold.title) {
                    folding.toggle(row.id)
                    Announcer.say(fold.said)
                }
            }
        }
    }

    /// What swipes from the leading edge: Mark Done or Mark Not Done, when primary.
    private func leading(_ actions: [Action]) -> [Action] {
        actions.filter { $0.primary && [.markDone, .markNotDone].contains($0.kind) }
    }

    private var trash: Bool { place == .trash }

    private var rows: Rows? {
        _ = core.generation
        let typed = filter.trimmingCharacters(in: .whitespaces)
        let query = place == .tasks && !typed.isEmpty ? typed : placeQuery(place: place)
        return core.read { try core.lumenna.listTasks(query: query) }
    }
}

/// A task to open, by identifier.
struct TaskOpened: Hashable {
    let id: String
}

/// What can be done to a project, a label or a saved filter: its actions, the core's, as
/// Browse and the sidebar offer them on the phone. At the foot of its list, as buttons, since
/// a watch's row has room for few swipes.
private struct PlaceActions: View {
    @EnvironmentObject private var core: WatchCore
    let place: Place
    @ObservedObject var asker: WatchAsker
    /// Leaves the place's list, once the place is gone or renamed.
    let left: () -> Void

    var body: some View {
        if let entry {
            Section {
                ForEach(entry.actions, id: \.self) { action in
                    Button(action.title, role: action.destructive ? .destructive : nil) {
                        asker.run(action, done: { change in
                            // This list stands for something gone or renamed: back to the places.
                            if change.changed, [.rename, .delete, .mergeInto].contains(action.kind) { left() }
                        })
                    }
                }
            } header: {
                FormParts.heading(heading)
            }
        }
    }

    private var entry: SidebarEntry? {
        _ = core.generation
        return core.lumenna.places().entries.first { $0.kind == .place(place) && !$0.actions.isEmpty }
    }

    private var heading: String {
        switch place {
        case .project: "Project"
        case .label: "Label"
        default: "Saved Filter"
        }
    }
}

/// Every block series, each with when it happens, as Browse lists them, each opening the
/// form for every occurrence (its Edit Block); its other actions, the core's, are its swipes.
struct BlockListView: View {
    @EnvironmentObject private var core: WatchCore
    @State private var editing: BlockFormModel?
    @StateObject private var asker: WatchAsker

    init(core: WatchCore) {
        _asker = StateObject(wrappedValue: WatchAsker(core: core))
    }

    var body: some View {
        let listing: Rows? = {
            _ = core.generation
            return core.read { try core.lumenna.listBlocks() }
        }()
        List {
            if let listing, listing.rows.isEmpty {
                Text(listing.empty)
            }
            ForEach(listing?.rows ?? [], id: \.id) { row in
                Button {
                    editing = core.read { try BlockFormModel.series(core: core, id: row.id) { change in
                        core.changed()
                        Announcer.say(change.announcement, notices: change.notices)
                    } }
                } label: {
                    VStack(alignment: .leading) {
                        Text(row.title)
                        // Said as a task row is: when, in this watch's clock, then how long.
                        if let detail = RowSpeech.details(row) {
                            Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                        }
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(row.title)
                    .accessibilityValue(RowSpeech.details(row) ?? "")
                }
                .swipeActions(edge: .trailing) {
                    // The primary ones; Edit Block is the row's tap.
                    ForEach(row.actions.filter { $0.primary && !$0.isForm }, id: \.self) { action in
                        Button(action.title, role: action.destructive ? .destructive : nil) { asker.run(action) }
                    }
                }
            }
        }
        .navigationTitle("Blocks")
        .sheet(item: $asker.asked) { asked in
            NavigationStack { asked.view }
        }
        .sheet(item: $editing) { model in
            BlockSheet(model: model)
        }
    }
}

extension BlockFormModel: Identifiable {}

/// The block form (`Shared/Forms/BlockForm.swift`) as a watch sheet, closing itself on save.
struct BlockSheet: View {
    @ObservedObject var model: BlockFormModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            BlockForm(model: model)
                .navigationTitle(model.heading)
        }
        .onAppear { model.close = { dismiss() } }
    }
}
