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
    let place: LumennaApp.Place
    @State private var adding = false
    @State private var filter = ""
    /// Whether one of the place's own actions ran here: when the app then goes on to another
    /// place — renamed, merged, deleted — this list goes back to the places.
    @State private var acted = false

    var body: some View {
        _ = core.generation
        let view = core.app.view
        let here = view.place == place
        let lines = core.app.lines
        let rows: [(item: LumennaApp.Item, line: LumennaApp.Line?)] = here
            ? (view.list?.items ?? []).enumerated()
                .map { index, item in (item, index < lines.list.count ? lines.list[index] : nil) }
                .filter { !$0.item.hidden }
            : []
        return List {
            if place == .tasks {
                TextField("Filter", text: $filter, prompt: example("#Work & overdue"))
                    .textInputAutocapitalization(.never)
                    .accessibilityLabel("Filter")
                    .onChange(of: filter) { _, filter in core.app.send(.filter(filter)) }
                CompletionOffer(text: $filter, syntax: .filter)
            }
            if here {
                // The readback: how the query was understood and how many it found, or what
                // is empty, in the core's words. A misread filter shows wrong results silently.
                Text(lines.readback).font(.footnote)
            }
            if place != .trash {
                Button("New Task") { adding = true }
            }
            ForEach(rows, id: \.item.row.id) { row in
                taskRow(row.item, line: row.line)
            }
            if here { PlaceActions(place: place, acted: $acted) }
        }
        .navigationTitle(here ? view.title : LumennaApp.placeTitle(place: place))
        .navigationDestination(for: TaskOpened.self) { opened in
            TaskView(id: opened.id)
        }
        .sheet(isPresented: $adding) {
            QuickAddView()
        }
        .onAppear {
            // Coming back from a task's details keeps the place, its filter and the row in
            // hand; from anywhere else it opens again.
            guard core.app.view.place != place else { return }
            core.app.send(.open(place))
            if place == .tasks, !filter.isEmpty { core.app.send(.filter(filter)) }
        }
        .onChange(of: view.place) { _, shown in
            if acted, shown != place {
                acted = false
                dismiss()
            }
        }
    }

    private func taskRow(_ item: LumennaApp.Item, line: LumennaApp.Line?) -> some View {
        let row = item.row
        let leading = row.actions.filter { $0.primary && [.markDone, .markNotDone].contains($0.kind) }
        let trailing = row.actions.filter { $0.primary && !leading.contains($0) && ![.expand, .collapse].contains($0.kind) }
            + row.actions.filter { [.expand, .collapse].contains($0.kind) }
        return NavigationLink(value: TaskOpened(id: row.id)) {
            // The row owns what VoiceOver says: the title as its name, then the rest of its
            // line, whether it is folded and its level as its value, as on the phone.
            VStack(alignment: .leading) {
                Text(row.title)
                if let detail = line?.detail, !detail.isEmpty {
                    Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(row.title)
            .accessibilityValue(AppModel.spoken(detail: line?.detail, expanded: row.expanded, level: item.level))
        }
        .swipeActions(edge: .leading) {
            ForEach(leading, id: \.self) { action in
                Button(action.title) { core.app.run(action, on: row.id) }
            }
        }
        .swipeActions(edge: .trailing) {
            ForEach(trailing, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { core.app.run(action, on: row.id) }
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
    @EnvironmentObject private var core: WatchCore

    var body: some View {
        _ = core.generation
        let view = core.app.view
        let lines = core.app.lines
        let rows: [(item: LumennaApp.Item, line: LumennaApp.Line?)] = view.place == .blocks
            ? (view.list?.items ?? []).enumerated().map { index, item in (item, index < lines.list.count ? lines.list[index] : nil) }
            : []
        return List {
            if view.place == .blocks, rows.isEmpty {
                Text(lines.readback)
            }
            ForEach(rows, id: \.item.row.id) { shown in
                let row = shown.item.row
                // Said as a task row is: when, in this watch's clock, then how long.
                let detail = shown.line?.detail ?? ""
                Button {
                    if let edit = row.actions.first(where: { $0.kind == .edit }) { core.app.run(edit, on: row.id) }
                } label: {
                    VStack(alignment: .leading) {
                        Text(row.title)
                        if !detail.isEmpty {
                            Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                        }
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(row.title)
                    .accessibilityValue(detail)
                }
                .swipeActions(edge: .trailing) {
                    // The primary ones and Reminders; Edit Block is the row's tap.
                    ForEach(row.actions.filter { ($0.primary && $0.kind != .edit) || $0.kind == .reminders }, id: \.self) { action in
                        Button(action.title, role: action.destructive ? .destructive : nil) { core.app.run(action, on: row.id) }
                    }
                }
            }
            Section {
                Button("Add Block") { core.app.send(.block(.new(start: nil, minutes: nil))) }
            }
        }
        .navigationTitle("Blocks")
        .onAppear {
            if core.app.view.place != .blocks { core.app.send(.open(.blocks)) }
        }
    }
}
