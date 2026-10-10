import SwiftUI

/// One task, in the form the iPhone and the Mac edit it with (`Shared/Forms/TaskForm.swift`):
/// every field, saved as only what changed, then what can be done to it. What only the watch
/// does is choosing a task or a block, as sheets.
struct TaskView: View {
    @EnvironmentObject private var core: WatchCore
    let id: String

    var body: some View {
        Hosted(core: core, id: id)
    }

    /// The model is made once the store is in hand, which a view's initialiser does not have.
    private struct Hosted: View {
        @StateObject private var model: TaskDetailModel
        @StateObject private var host = TaskSheets()
        @Environment(\.dismiss) private var dismiss

        init(core: WatchCore, id: String) {
            _model = StateObject(wrappedValue: TaskDetailModel(core: core, id: id))
        }

        var body: some View {
            TaskDetailView(model: model)
                .navigationTitle("Task")
                .onAppear {
                    model.host = host
                    host.left = { dismiss() }
                }
                .sheet(item: $host.asked) { asked in
                    NavigationStack { asked.view }
                }
        }
    }
}

/// The watch's answers to what the task form asks of its platform.
final class TaskSheets: ObservableObject, TaskFormHost {
    @Published var asked: Asked?
    var left: () -> Void = {}

    func chooseTask(_ title: String, excluding: Set<String>, chosen: @escaping (String) -> Void) {
        asked = Asked(TaskChoice(title: title, excluding: excluding, chosen: chosen))
    }

    func chooseBlock(for task: TaskDetail, chosen: @escaping (String, String, UInt32?) -> Void) {
        asked = Asked(WorkBlockChoice(chosen: { [weak self] block, date in
            // Then how long the sitting is meant to take, as the phone asks.
            self?.asked = Asked(LengthChoice(title: "Planned length", without: "No Planned Length") { minutes in
                chosen(block, date, minutes)
            })
        }))
    }

    func trashed() {
        left()
    }
}

/// The open tasks, less some, to choose one from.
private struct TaskChoice: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    let title: String
    let excluding: Set<String>
    let chosen: (String) -> Void

    var body: some View {
        let rows = (core.read { try core.lumenna.listTasks(query: "").rows } ?? []).filter { !excluding.contains($0.id) }
        List {
            if rows.isEmpty { Text("No other open tasks") }
            ForEach(rows, id: \.id) { row in
                Button {
                    dismiss()
                    chosen(row.id)
                } label: {
                    Text(row.title)
                }
                .accessibilityValue(RowSpeech.details(row) ?? "")
            }
        }
        .navigationTitle(title)
    }
}

/// The work blocks of the coming week a task could go in: which ones is the core's.
struct WorkBlockChoice: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    let chosen: (_ block: String, _ date: String) -> Void

    var body: some View {
        let blocks = core.workBlocksThisWeek()
        List {
            if blocks.isEmpty {
                Text("There are no work blocks this week; add one in the day")
            }
            ForEach(blocks, id: \.id) { block in
                Button(block.title) {
                    dismiss()
                    chosen(block.id, block.date)
                }
                .accessibilityValue(block.detail)
            }
        }
        .navigationTitle("Put It In")
    }
}

/// A length in minutes, from the lengths a sitting usually takes, or none.
struct LengthChoice: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let without: String?
    let chosen: (UInt32?) -> Void

    var body: some View {
        List {
            ForEach([15, 25, 30, 45, 60, 90, 120], id: \.self) { minutes in
                Button(Clock.length(UInt32(minutes))) {
                    dismiss()
                    chosen(UInt32(minutes))
                }
            }
            if let without {
                Button(without) {
                    dismiss()
                    chosen(nil)
                }
            }
        }
        .navigationTitle(title)
    }
}
