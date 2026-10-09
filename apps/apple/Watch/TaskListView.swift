import SwiftUI

/// A place's tasks, each row as the phone says it: the title, then what the core says of it.
/// Mark Done and Delete are its swipe actions, which VoiceOver offers as the row's actions;
/// in the trash, Restore and Delete from Trash.
struct TaskListView: View {
    @EnvironmentObject private var core: WatchCore
    let place: Place
    @State private var adding = false
    @State private var erasing: RowView?

    var body: some View {
        let listing = rows
        List {
            if !trash {
                Button("New Task") { adding = true }
            }
            if let listing, listing.rows.isEmpty {
                Text(listing.announcement.prefix(1).uppercased() + listing.announcement.dropFirst())
            }
            ForEach(Array((listing?.rows ?? []).enumerated()), id: \.element.id) { index, row in
                let previous = index == 0 ? nil : listing?.rows[index - 1].depth
                NavigationLink(value: TaskOpened(id: row.id)) {
                    // The row owns what VoiceOver says: the title as its name, then what the
                    // core says of it as its value, as on the phone.
                    VStack(alignment: .leading) {
                        Text(row.title)
                        if let detail = RowSpeech.details(row) {
                            Text(detail).font(.footnote).foregroundStyle(.secondary)
                        }
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(RowSpeech.label(row))
                    .accessibilityValue(RowSpeech.value(row, previousDepth: previous))
                }
                .swipeActions(edge: .leading) {
                    if !trash {
                        Button(row.checked == true ? "Mark Not Done" : "Mark Done") { toggle(row) }
                    }
                }
                .swipeActions(edge: .trailing) {
                    if trash {
                        Button("Restore") { core.act { try core.lumenna.restoreTask(id: row.id) } }
                        Button("Delete from Trash", role: .destructive) { erasing = row }
                    } else {
                        Button("Delete", role: .destructive) { core.act { try core.lumenna.trashTask(id: row.id) } }
                    }
                }
            }
        }
        .navigationTitle(placeTitle(place: place))
        .navigationDestination(for: TaskOpened.self) { opened in
            TaskDetailView(id: opened.id)
        }
        .sheet(isPresented: $adding) {
            QuickAddView(prefix: placeQuickAddPrefix(place: place))
        }
        // What cannot be undone asks first, and the answer that keeps it comes first.
        .confirmationDialog(
            "Delete \(erasing?.title ?? "") for good? This cannot be undone.",
            isPresented: Binding(get: { erasing != nil }, set: { if !$0 { erasing = nil } }),
            titleVisibility: .visible
        ) {
            Button("Cancel", role: .cancel) { erasing = nil }
            Button("Delete for Good", role: .destructive) {
                if let row = erasing { core.act { try core.lumenna.eraseTask(id: row.id) } }
                erasing = nil
            }
        }
    }

    private var trash: Bool { place == .trash }

    private var rows: Rows? {
        _ = core.generation
        return core.read { try core.lumenna.listTasks(query: placeQuery(place: place)) }
    }

    private func toggle(_ row: RowView) {
        if row.checked == true {
            core.act { try core.lumenna.uncompleteTask(id: row.id) }
        } else {
            core.act { try core.lumenna.completeTask(id: row.id) }
        }
    }
}

/// A task to open, by identifier.
struct TaskOpened: Hashable {
    let id: String
}

/// Every block series, each with when it happens, as Browse lists them.
struct BlockListView: View {
    @EnvironmentObject private var core: WatchCore

    var body: some View {
        let listing: Rows? = {
            _ = core.generation
            return core.read { try core.lumenna.listBlocks() }
        }()
        List {
            if let listing, listing.rows.isEmpty {
                Text("No blocks")
            }
            ForEach(listing?.rows ?? [], id: \.id) { row in
                VStack(alignment: .leading) {
                    Text(row.title)
                    if let value = row.value {
                        Text(value).font(.footnote).foregroundStyle(.secondary)
                    }
                }
                .accessibilityElement(children: .combine)
            }
        }
        .navigationTitle("Blocks")
    }
}
