import SwiftUI
import UIKit

/// One task's details, editable (§16.1: task detail / edit).
final class TaskDetailViewController: UIHostingController<TaskDetailView> {
    init(core: Core, id: String) {
        let model = TaskDetailModel(core: core, id: id)
        super.init(rootView: TaskDetailView(model: model))
        title = "Task"
        model.host = self
        // A form gets the whole screen. Under the floating tab bar its last rows would be
        // read through glass.
        hidesBottomBarWhenPushed = true
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    private var model: TaskDetailModel { rootView.model }

    override func viewDidLoad() {
        super.viewDidLoad()
        // A UIKit button, always enabled: a disabled one failed contrast and is easy to miss
        // without sight, and saving with nothing changed says so.
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: "Save", primaryAction: UIAction { [weak self] _ in self?.model.save() }
        )
        navigationItem.rightBarButtonItem?.style = .done
    }
}

/// The task as stored, and the fields as edited.
final class TaskDetailModel: ObservableObject {
    private let core: Core
    private let id: String
    /// The screen showing this, for pickers and for leaving when the task goes.
    weak var host: UIViewController?

    @Published private(set) var task: TaskDetail?
    @Published var title = ""
    @Published var due = ""
    @Published var priority: UInt8 = 4
    @Published var estimate = ""
    @Published var project = ""
    @Published var notes = ""
    @Published var labels = ""
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
            labels = shown.task.labels.joined(separator: ", ")
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
            || labels != task.labels.joined(separator: ", ")
    }

    /// Sends only the fields that changed, so a concurrent edit to another field on another
    /// device is not overwritten with what this screen happened to show.
    func save() {
        guard let task else { return }
        guard hasChanges else {
            Announcer.say("Nothing changed")
            return
        }
        let edit = TaskEdit(
            title: title != task.title ? title : nil,
            due: due != Self.dueText(task) ? (due.isEmpty ? "none" : due) : nil,
            priority: priority != task.priority ? priority : nil,
            estimate: estimate != (task.estimateMins.map { "\($0)m" } ?? "")
                ? (estimate.isEmpty ? "none" : estimate) : nil,
            notes: notes != task.notes ? notes : nil,
            project: project != (task.project ?? "") && !project.isEmpty ? project : nil,
            labels: labels != task.labels.joined(separator: ", ")
                ? labels.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                : nil
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

    /// Chooses a task for this one to wait for (§3.3).
    func addDependency() {
        guard let host, let task else { return }
        let waiting = Set(task.depends.map(\.id) + [id])
        TaskPicker.present(from: host, core: core, title: "Waits For", excluding: waiting) { [weak self] other in
            guard let self else { return }
            self.run { try self.core.lumenna.addDependency(id: self.id, on: other.key) }
        }
    }

    func removeDependency(_ other: Dependency) {
        run { try core.lumenna.removeDependency(id: id, on: other.id) }
    }

    /// Puts this task under another, joining that one's project.
    func makeSubtask() {
        guard let host else { return }
        TaskPicker.present(from: host, core: core, title: "Make Subtask Of", excluding: [id]) { [weak self] parent in
            guard let self else { return }
            self.run { try self.core.lumenna.moveTask(id: self.id, to: .parent(id: parent.key)) }
        }
    }

    func moveToTop() {
        run { try core.lumenna.moveTask(id: id, to: .top) }
    }

    func trash() {
        do {
            let change = try core.lumenna.trashTask(id: id)
            Announcer.say(change.announcement, notices: change.notices)
            host?.navigationController?.popViewController(animated: true)
        } catch {
            failure = error.sentence
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
    private func field(
        _ name: String, text: Binding<String>, example placeholder: String, axis: Axis = .horizontal
    ) -> some View {
        NamedRow(name: name) {
            // An empty title, so the name is the label once rather than twice.
            TextField("", text: text, prompt: example(placeholder), axis: axis)
                .multilineTextAlignment(.trailing)
        }
    }

    var body: some View {
        Form {
            Section {
                field("Title", text: $model.title, example: "What to do", axis: .vertical)
                field("Due", text: $model.due, example: "tomorrow")
                    .textInputAutocapitalization(.never)
                    .accessibilityHint("A date, such as tomorrow or next Friday. Empty for none.")
                field("Estimate", text: $model.estimate, example: "45m")
                    .textInputAutocapitalization(.never)
                    .accessibilityHint("Such as 45m or 1h30m. Empty for none.")
                field("Project", text: $model.project, example: "Inbox")
                field("Labels", text: $model.labels, example: "calls, errands")
                    .textInputAutocapitalization(.never)
                    .accessibilityHint("Names separated by commas. A new name becomes a label.")
            }
            Section {
                ChoiceRows(
                    choices: [("Priority 1, highest", UInt8(1)), ("Priority 2", 2), ("Priority 3", 3), ("Priority 4, none", 4)],
                    selection: $model.priority
                )
            } header: {
                FormParts.caption("Priority")
            }
            Section {
                TextField("Notes", text: $model.notes, prompt: example("Anything else"), axis: .vertical)
                    .lineLimit(3...)
                    .accessibilityLabel("Notes")
            } header: {
                FormParts.caption("Notes")
            }
            if let task = model.task {
                Section {
                    ForEach(task.depends, id: \.id) { other in
                        Button("Stop Waiting for \(other.title)") { model.removeDependency(other) }
                    }
                    Button("Add Something It Waits For") { model.addDependency() }
                } header: {
                    FormParts.caption("Waits for")
                }
                Section {
                    if let recurrence = task.recurrence {
                        LabeledContent("Repeats", value: recurrence)
                    }
                    LabeledContent("State", value: task.state.joined(separator: ", "))
                } header: {
                    FormParts.caption("About")
                }
                Section {
                    Button(task.state.contains("completed") ? "Mark Not Done" : "Mark Done") {
                        model.toggleDone()
                    }
                    Button("Make Subtask Of…") { model.makeSubtask() }
                    if task.parent != nil {
                        Button("Move to Top Level") { model.moveToTop() }
                    }
                    WarningButton("Move to Trash") { model.trash() }
                }
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
