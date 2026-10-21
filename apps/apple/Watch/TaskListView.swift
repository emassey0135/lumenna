import LumennaApp
import SwiftUI

/// A place's tasks, the app's list (`ViewModel.list`) while the app shows this place, which it
/// opens as it appears. Each row is said as the phone says it: the title, then the rest of its
/// line. Its primary actions, the app's, are its swipe actions — Mark Done from the leading
/// edge, the rest and Expand or Collapse from the trailing — which VoiceOver offers as the
/// row's actions; every one is on the task's own screen. Tasks can be filtered, with the
/// filter read back; a project, label or filter's own actions are at the foot, as Browse
/// offers them.
struct TaskListView: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    @StateObject private var model: TaskListModel
    @State private var adding = false
    /// Whether one of the place's own actions ran here: when the app then goes on to another
    /// place — renamed, merged, deleted — this list goes back to the places.
    @State private var acted = false
    @AccessibilityFocusState private var spoken: String?

    init(core: WatchCore, place: LumennaApp.Place) {
        _model = StateObject(wrappedValue: TaskListModel(core: core, place: place))
    }

    private var place: LumennaApp.Place { model.place }

    var body: some View {
        List {
            // On Tasks alone: on a watch's screen a place's own list is short enough to read.
            if place == .tasks {
                TextField("Filter", text: $model.filter, prompt: example("#Work & overdue"))
                    .textInputAutocapitalization(.never)
                    .accessibilityLabel("Filter")
                CompletionOffer(text: $model.filter, syntax: .filter)
            }
            // The readback: how the query was understood and how many it found, or what is
            // empty, in the core's words. A misread filter shows wrong results silently.
            Text(model.readback).font(.footnote)
            if !model.trash {
                Button("New Task") { adding = true }
            }
            ForEach(model.shown) { row in
                taskRow(row)
            }
            if model.here { PlaceActions(place: place, acted: $acted) }
        }
        .navigationTitle(model.title)
        .navigationDestination(for: TaskOpened.self) { opened in
            TaskView(id: opened.id)
        }
        .sheet(isPresented: $adding) {
            QuickAddView().showsWhatIsSaid()
        }
        // Coming back from a task's details keeps the place, its filter and the row in hand;
        // from anywhere else it opens again with them.
        .onAppear { model.appear() }
        .onChange(of: core.app.view.place) { _, shown in
            if acted, shown != place {
                acted = false
                dismiss()
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

    private func taskRow(_ shown: TaskListModel.Row) -> some View {
        let row = shown.row
        // Split as the phone splits them (`AppRowActions`): Mark Done from the leading edge,
        // the rest and Expand or Collapse from the trailing; every one is on the task's screen.
        let split = AppRowActions(actions: row.actions, doneLeads: true) { _ in }
        return NavigationLink(value: TaskOpened(id: row.id)) {
            // The row owns what VoiceOver says: the title as its name, then the rest of its
            // line, whether it is folded and its level as its value, as on the phone.
            HStack {
                SwatchMark(swatch: row.swatch)
                VStack(alignment: .leading) {
                    Text(row.title)
                    if let detail = shown.line?.detail, !detail.isEmpty {
                        Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                    }
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(row.title)
            .accessibilityValue(AppModel.spoken(detail: shown.line?.detail, expanded: row.expanded, level: shown.item.level))
        }
        .accessibilityFocused($spoken, equals: row.id)
        .swipeActions(edge: .leading) {
            ForEach(split.leading, id: \.self) { action in
                Button(action.title) { model.run(action, on: row) }
            }
        }
        .swipeActions(edge: .trailing) {
            ForEach(split.trailing, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: row) }
            }
        }
    }
}

/// A task to open, by identifier.
struct TaskOpened: Hashable {
    let id: String
}

/// What can be done to a project, a label or a saved filter: its actions, the app's, as
/// Browse and the sidebar offer them on the phone, held in the sidebar's hand. At the foot of
/// its list, as buttons, since a watch's row has room for few swipes.
private struct PlaceActions: View {
    @EnvironmentObject private var core: WatchCore
    let place: LumennaApp.Place
    @Binding var acted: Bool

    var body: some View {
        if let entry {
            Section {
                ForEach(entry.actions, id: \.self) { action in
                    Button(action.title, role: action.destructive ? .destructive : nil) {
                        acted = true
                        core.app.run(action, on: entry.key, side: .sidebar)
                    }
                }
            } header: {
                FormParts.heading(heading)
            }
        }
    }

    private var entry: LumennaApp.SidebarEntry? {
        core.app.view.sidebar.first { $0.kind == .place(place) && !$0.actions.isEmpty }
    }

    private var heading: String {
        switch place {
        case .project: "Project"
        case .label: "Label"
        default: "Saved Filter"
        }
    }
}

/// Every block series, each with when it happens, as Browse lists them: the app's Blocks
/// place. Each opens the form for every occurrence (its Edit Block); its other actions, the
/// app's, are its swipes.
struct BlockListView: View {
    @StateObject private var model: TaskListModel
    @AccessibilityFocusState private var spoken: String?

    init(core: WatchCore) {
        _model = StateObject(wrappedValue: TaskListModel(core: core, place: .blocks))
    }

    var body: some View {
        List {
            if model.shown.isEmpty {
                Text(model.readback)
            }
            ForEach(model.shown) { shown in
                let row = shown.row
                // Said as a task row is: when, in this watch's clock, then how long.
                let detail = shown.line?.detail ?? ""
                Button {
                    model.run([.edit], on: row, saying: .edit)
                } label: {
                    HStack {
                        SwatchMark(swatch: row.swatch)
                        VStack(alignment: .leading) {
                            Text(row.title)
                            if !detail.isEmpty {
                                Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                            }
                        }
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(row.title)
                    .accessibilityValue(detail)
                }
                .accessibilityFocused($spoken, equals: row.id)
                .swipeActions(edge: .trailing) {
                    // The primary ones and Reminders; Edit Block is the row's tap.
                    ForEach(row.actions.filter { ($0.primary && $0.kind != .edit) || $0.kind == .reminders }, id: \.self) { action in
                        Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: row) }
                    }
                }
            }
            Section {
                Button("Add Block") { model.core.app.send(.block(.new(start: nil, minutes: nil))) }
            }
        }
        .navigationTitle("Blocks")
        .onAppear { model.appear() }
        .onChange(of: model.landed?.count) { _, _ in
            guard let key = model.landed?.key else { return }
            DispatchQueue.main.async { spoken = key }
        }
        .onChange(of: spoken) { _, key in
            if let key { model.moved(to: key) }
        }
    }
}
