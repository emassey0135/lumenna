import LumennaApp
import SwiftUI

/// A place's tasks, the app's list (`ViewModel.list`) while the app shows this place, which it
/// opens as it appears. Each row is said as the phone says it: the title, then the rest of its
/// line. Its primary actions, the app's, are its swipe actions — Mark Done from the leading
/// edge, the rest and Expand or Collapse from the trailing — which VoiceOver offers as the
/// row's actions; every one is on the task's own screen. Tasks can be filtered, with the
/// filter read back; the place's own actions, the core's, are at the foot, then a project,
/// label or filter's own, as Browse offers them.
struct TaskListView: View {
    @EnvironmentObject private var core: Core
    @Environment(\.dismiss) private var dismiss
    @StateObject private var model: TaskListModel
    @State private var adding = false
    /// Whether one of the place's own actions ran here: when the app then goes on to another
    /// place — renamed, merged, deleted — this list goes back to the places.
    @State private var acted = false
    /// Where a row's action took the app (Upcoming's Show the Day: the day), shown over this list.
    @State private var elsewhere: LumennaApp.Place?
    @AccessibilityFocusState private var spoken: String?

    init(core: Core, place: LumennaApp.Place) {
        _model = StateObject(wrappedValue: TaskListModel(core: core, place: place))
    }

    private var place: LumennaApp.Place { model.place }

    var body: some View {
        ScrollViewReader { scroller in
            List {
                // Wherever the phone has one: every place of tasks but the trash.
                if model.filters {
                    TextField(QuickAddText.filter, text: $model.filter, prompt: example(QuickAddText.filterHint))
                        .textInputAutocapitalization(.never)
                        .accessibilityLabel(QuickAddText.filter)
                    CompletionOffer(text: $model.filter, syntax: .filter)
                }
                // The readback: how the query was understood and how many it found, or what is
                // empty, in the core's words. A misread filter shows wrong results silently.
                Text(model.readback).font(.footnote)
                ForEach(model.shown) { row in
                    taskRow(row)
                }
                // The place's own actions, the core's, in its order, at the foot: New Task, New
                // Project Inside, Undo, Redo. Filter Tasks is the field above, where a watch has
                // no way to put a field in hand.
                Section {
                    ForEach(model.screenActions.filter { !$0.shell }, id: \.id) { action in
                        ScreenActionButton(action: action) { action in
                            core.app.perform(action, lineOpened: { adding = true })
                        }
                    }
                }
                if model.here { PlaceActions(place: place, acted: $acted) }
            }
            .navigationTitle(model.title)
            .navigationDestination(for: TaskOpened.self) { opened in
                TaskView(id: opened.id)
            }
            .navigationDestination(item: $elsewhere) { place in
                PlaceView(core: core, place: place)
            }
            .sheet(isPresented: $adding) {
                QuickAddView().showsWhatIsSaid()
            }
            // Coming back from a task's details keeps the place, its filter and the row in hand;
            // from anywhere else it opens again with them.
            .onAppear {
                model.left = { elsewhere = $0 }
                model.appear()
            }
            .onChange(of: core.app.view.place) { _, shown in
                if acted, shown != place {
                    acted = false
                    dismiss()
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
        }
    }

    private func taskRow(_ shown: TaskListModel.Row) -> some View {
        let row = shown.row
        // Split as the phone splits them (`AppRowActions`): Mark Done from the leading edge,
        // the rest and Expand or Collapse from the trailing; every one is on the task's screen.
        let split = AppRowActions(actions: shown.actions, doneLeads: true) { _ in }
        // The row owns what VoiceOver says: the title as its name, then the rest of its line,
        // whether it is folded and its level as its value, as on the phone.
        let label = HStack {
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
        return Group {
            if row.role == "task" {
                NavigationLink(value: TaskOpened(id: row.id)) { label }
            } else {
                // Upcoming's day or block: the core's Open, which shows it on its day; the list
                // follows the app there (`left`).
                Button { model.key("open", on: row) } label: { label }
            }
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
    @EnvironmentObject private var core: Core
    let place: LumennaApp.Place
    @Binding var acted: Bool

    var body: some View {
        if let entry {
            Section {
                ForEach(entry.offered, id: \.self) { action in
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
        core.app.view.sidebar.first { $0.kind == .place(place) && !$0.offered.isEmpty }
    }

    /// What the place is, the core's, over its own actions.
    private var heading: String {
        LumennaApp.placeHeading(place: place, sentence: false)
    }
}

/// Every block series, each with when it happens, as Browse lists them: the app's Blocks
/// place. Tapped, a row offers every one of its actions, the app's — Edit Block, its time zone,
/// its reminders — as the day's rows do, since a watch's row has room for few swipes; its
/// primary ones are its swipes too.
struct BlockListView: View {
    @EnvironmentObject private var asker: WatchAsker
    @StateObject private var model: TaskListModel
    @AccessibilityFocusState private var spoken: String?

    init(core: Core) {
        _model = StateObject(wrappedValue: TaskListModel(core: core, place: .blocks))
    }

    var body: some View {
        ScrollViewReader { scroller in
            List {
                if model.shown.isEmpty {
                    Text(model.readback)
                }
                ForEach(model.shown) { shown in
                    let row = shown.row
                    // Said as a task row is: when, in this watch's clock, then how long.
                    let detail = shown.line?.detail ?? ""
                    Button {
                        asker.offer(row.title, shown.actions, on: row.id)
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
                        // The primary ones; every one is the row's tap.
                        ForEach(shown.actions.filter(\.primary), id: \.self) { action in
                            Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: row) }
                        }
                    }
                }
                // The place's own actions, the core's: New Block, Undo, Redo.
                Section {
                    ForEach(model.screenActions, id: \.id) { action in
                        ScreenActionButton(action: action) { model.core.app.perform($0) }
                    }
                }
            }
            .navigationTitle(LumennaApp.placeTitle(place: .blocks))
            .onAppear { model.appear() }
            .onChange(of: model.landed?.count) { _, _ in
                guard let key = model.landed?.key else { return }
                scroller.scrollTo(key, anchor: .center)
                DispatchQueue.main.async { spoken = key }
            }
            .onChange(of: spoken) { _, key in
                if let key { model.moved(to: key) }
            }
        }
    }
}
