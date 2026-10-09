import SwiftUI

/// The places, as every app's sidebar has them, from the core: Today, Tasks, the projects,
/// labels and saved filters, Blocks and Trash. Then Undo, Redo, and syncing with the iPhone.
struct RootView: View {
    @EnvironmentObject private var core: WatchCore
    @EnvironmentObject private var phone: PhoneSync
    @State private var adding = false

    var body: some View {
        NavigationStack {
            List {
                Button("New Task") { adding = true }
                ForEach(Array(entries.enumerated()), id: \.offset) { index, entry in
                    row(entry, after: index == 0 ? nil : entries[index - 1])
                }
                Section {
                    Button("Undo") { core.act { try core.lumenna.undo() } }
                    Button("Redo") { core.act { try core.lumenna.redo() } }
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
            .alert("Could not do that", isPresented: failed) {
                Button("OK") { core.failure = nil }
            } message: {
                Text(core.failure ?? "")
            }
        }
    }

    private var entries: [SidebarEntry] {
        _ = core.generation
        return core.lumenna.places().entries.filter { !$0.archived }
    }

    private var failed: Binding<Bool> {
        Binding(get: { core.failure != nil }, set: { if !$0 { core.failure = nil } })
    }

    @ViewBuilder
    private func row(_ entry: SidebarEntry, after previous: SidebarEntry?) -> some View {
        switch entry.kind {
        case let .place(place):
            NavigationLink(value: place) {
                Text(entry.text)
            }
            // Depth is said where it changes, as every app says it, never by indentation.
            .accessibilityValue(entry.depth == previous?.depth ?? 0 ? "" : "level \(entry.depth + 1)")
        case .group:
            Text(entry.text)
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
        }
    }
}

/// What a place opens to: the day for Today, the block list for Blocks, else its tasks.
struct PlaceView: View {
    let place: Place

    var body: some View {
        switch place {
        case .today: DayView()
        case .blocks: BlockListView()
        default: TaskListView(place: place)
        }
    }
}
