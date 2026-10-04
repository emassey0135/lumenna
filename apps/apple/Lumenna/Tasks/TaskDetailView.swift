import SwiftUI
import UIKit

/// One task's details, editable (§16.1: task detail / edit).
final class TaskDetailViewController: UIHostingController<TaskDetailView> {
    init(core: Core, id: String) {
        super.init(rootView: TaskDetailView(model: TaskDetailModel(core: core, id: id)))
        title = "Task"
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        navigationController?.setToolbarHidden(true, animated: animated)
    }
}

/// The task as stored, and the fields as edited.
final class TaskDetailModel: ObservableObject {
    private let core: Core
    private let id: String

    @Published private(set) var task: TaskDetail?
    @Published var title = ""
    @Published var due = ""
    @Published var priority: UInt8 = 4
    @Published var estimate = ""
    @Published var project = ""
    @Published var notes = ""
    @Published var failure: String?

    init(core: Core, id: String) {
        self.core = core
        self.id = id
        load()
    }

    /// Reads the task again and resets the fields to it.
    func load() {
        do {
            let shown = try core.lumenna.showTask(id: id)
            task = shown.task
            title = shown.task.title
            due = Self.dueText(shown.task)
            priority = shown.task.priority
            estimate = shown.task.estimateMins.map { "\($0)m" } ?? ""
            project = shown.task.project ?? ""
            notes = shown.task.notes
        } catch {
            failure = error.sentence
        }
    }

    /// The due date as a phrase the core reads back in: `2026-10-09 14:00`.
    private static func dueText(_ task: TaskDetail) -> String {
        [task.due, task.dueTime].compactMap { $0 }.joined(separator: " ")
    }

    var hasChanges: Bool {
        guard let task else { return false }
        return title != task.title || due != Self.dueText(task) || priority != task.priority
            || estimate != (task.estimateMins.map { "\($0)m" } ?? "")
            || project != (task.project ?? "") || notes != task.notes
    }

    /// Sends only the fields that changed, so a concurrent edit to another field on another
    /// device is not overwritten with what this screen happened to show.
    func save() {
        guard let task else { return }
        let edit = TaskEdit(
            title: title != task.title ? title : nil,
            due: due != Self.dueText(task) ? (due.isEmpty ? "none" : due) : nil,
            priority: priority != task.priority ? priority : nil,
            estimate: estimate != (task.estimateMins.map { "\($0)m" } ?? "")
                ? (estimate.isEmpty ? "none" : estimate) : nil,
            notes: notes != task.notes ? notes : nil,
            project: project != (task.project ?? "") && !project.isEmpty ? project : nil
        )
        run { try self.core.lumenna.editTask(id: self.id, edit: edit) }
    }

    func toggleDone() {
        let done = task?.state.contains("completed") == true
        run {
            done
                ? try self.core.lumenna.uncompleteTask(id: self.id)
                : try self.core.lumenna.completeTask(id: self.id)
        }
    }

    private func run(_ operation: () throws -> Change) {
        do {
            let change = try operation()
            load()
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            failure = error.sentence
        }
    }
}

struct TaskDetailView: View {
    @ObservedObject var model: TaskDetailModel

    /// A text field that keeps its name once it has text in it.
    ///
    /// A SwiftUI `TextField`'s title is only a placeholder: once the field holds something,
    /// VoiceOver reads the value with nothing to say which field it is. The name is shown
    /// beside the field for sight, and given to the field as its label for VoiceOver — once:
    /// the visible name is hidden from it, or it would be a second stop saying the same word.
    private func field(_ name: String, text: Binding<String>, axis: Axis = .horizontal) -> some View {
        LabeledContent {
            TextField(name, text: text, axis: axis)
                .multilineTextAlignment(.trailing)
                .accessibilityLabel(name)
        } label: {
            Text(name).accessibilityHidden(true)
        }
    }

    var body: some View {
        Form {
            Section {
                field("Title", text: $model.title, axis: .vertical)
                field("Due", text: $model.due)
                    .textInputAutocapitalization(.never)
                    .accessibilityHint("A date, such as tomorrow or next Friday. Empty for none.")
                Picker("Priority", selection: $model.priority) {
                    Text("Priority 1, highest").tag(UInt8(1))
                    Text("Priority 2").tag(UInt8(2))
                    Text("Priority 3").tag(UInt8(3))
                    Text("Priority 4, none").tag(UInt8(4))
                }
                field("Estimate", text: $model.estimate)
                    .textInputAutocapitalization(.never)
                    .accessibilityHint("Such as 45m or 1h30m. Empty for none.")
                field("Project", text: $model.project)
            }
            Section("Notes") {
                TextField("Notes", text: $model.notes, axis: .vertical)
                    .lineLimit(3...)
                    .accessibilityLabel("Notes")
            }
            if let task = model.task {
                Section("About") {
                    if !task.labels.isEmpty {
                        LabeledContent("Labels", value: task.labels.joined(separator: ", "))
                    }
                    if !task.depends.isEmpty {
                        LabeledContent(
                            "Waits for", value: task.depends.map(\.title).joined(separator: ", ")
                        )
                    }
                    if let recurrence = task.recurrence {
                        LabeledContent("Repeats", value: recurrence)
                    }
                    LabeledContent("State", value: task.state.joined(separator: ", "))
                }
                Section {
                    Button(task.state.contains("completed") ? "Mark Not Done" : "Mark Done") {
                        model.toggleDone()
                    }
                }
            }
        }
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") { model.save() }
                    .disabled(!model.hasChanges)
            }
        }
        .alert(
            "Could not do that",
            isPresented: Binding(
                get: { model.failure != nil },
                set: { if !$0 { model.failure = nil } }
            ),
            presenting: model.failure
        ) { _ in
            Button("OK") {}
        } message: { failure in
            Text(failure)
        }
    }
}
