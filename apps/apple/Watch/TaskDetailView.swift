import SwiftUI

/// One task: every field the core knows, each a row read as its name and value, then what
/// can be done to it.
struct TaskDetailView: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var assigning = false

    var body: some View {
        let shown: TaskDetail? = {
            _ = core.generation
            return core.read { try core.lumenna.showTask(id: id).task }
        }()
        List {
            if let task = shown {
                Text(task.title).font(.headline).accessibilityAddTraits(.isHeader)
                field("Due", due(task))
                field("Repeats", task.repetition)
                field("Priority", task.priority < 4 ? "Priority \(task.priority)" : nil)
                field("Project", task.project)
                field("Labels", task.labels.isEmpty ? nil : task.labels.joined(separator: ", "))
                field("Estimate", task.estimateMins.map { Clock.length($0) })
                field("Depends on", task.depends.isEmpty ? nil : task.depends.map(\.title).joined(separator: ", "))
                field("State", task.state.filter { $0 != "ready" }.joined(separator: ", "))
                field("Notes", task.notes.isEmpty ? nil : task.notes)
                Section {
                    if task.state.contains("completed") {
                        Button("Mark Not Done") { core.act { try core.lumenna.uncompleteTask(id: id) } }
                    } else {
                        Button("Mark Done") { core.act { try core.lumenna.completeTask(id: id) } }
                    }
                    Button("Put in a Block") { assigning = true }
                    Button("Delete", role: .destructive) {
                        if core.act({ try core.lumenna.trashTask(id: id) }) { dismiss() }
                    }
                }
            }
        }
        .navigationTitle("Task")
        .sheet(isPresented: $assigning) {
            BlockChoiceView(task: id)
        }
    }

    @ViewBuilder
    private func field(_ name: String, _ value: String?) -> some View {
        if let value, !value.isEmpty {
            VStack(alignment: .leading) {
                Text(name).font(.footnote).foregroundStyle(.secondary)
                Text(value)
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(name)
            .accessibilityValue(value)
        }
    }

    /// The due date as a person says it, and its time in this watch's clock.
    private func due(_ task: TaskDetail) -> String? {
        guard let due = task.due else { return nil }
        let day = Clock.spokenDay(due)
        return task.dueTime.map { "\(day) at \(Clock.time($0))" } ?? day
    }
}

/// The work blocks of the coming week a task could go in: which ones is the core's.
struct BlockChoiceView: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    let task: String

    var body: some View {
        let blocks = core.read { try core.lumenna.workBlocks(from: nil, days: nil).blocks } ?? []
        List {
            if blocks.isEmpty {
                Text("There are no work blocks this week; add one in the day")
            }
            ForEach(blocks, id: \.id) { block in
                Button("\(Clock.spokenDay(block.date)), \(Clock.time(block.start)) to \(Clock.time(block.end)), \(block.title)") {
                    if core.act({ try core.lumenna.assign(task: task, block: block.id, date: block.date, minutes: nil) }) {
                        dismiss()
                    }
                }
            }
        }
        .navigationTitle("Put It In")
    }
}
