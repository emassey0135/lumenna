import AppKit
import SwiftUI

/// One task's details, editable (§16.1), in the right-hand pane.
final class TaskDetailViewController: NSHostingController<TaskDetailView> {
    private let model: TaskDetailModel

    init(core: Core, id: String, window: MainWindowController) {
        let model = TaskDetailModel(core: core, id: id)
        self.model = model
        super.init(rootView: TaskDetailView(model: model))
        model.list = window.taskList
        title = "Task"
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidAppear() {
        super.viewDidAppear()
        model.window = view.window
    }

    override var preferredFirstResponder: NSView? { view }

    /// The Save item in the menu bar, ⌘S.
    @objc func saveTask(_ sender: Any?) { model.save() }
}

/// The task as stored, and the fields as edited.
final class TaskDetailModel: NSObject, ObservableObject {
    let core: Core
    let id: String
    weak var window: NSWindow?
    weak var list: TaskListViewController?

    @Published private(set) var task: TaskDetail?
    @Published var title = ""
    @Published var due = ""
    @Published var repetition = ""
    @Published var priority: UInt8 = 4
    @Published var estimate = ""
    @Published var project = ""
    @Published var notes = ""
    @Published var labels = ""
    @Published private(set) var projects: [String] = []
    @Published var failure: String?

    init(core: Core, id: String) {
        self.core = core
        self.id = id
        super.init()
        load()
        NotificationCenter.default.addObserver(self, selector: #selector(storeChanged), name: Core.changed, object: nil)
    }

    /// Another device, or `lum`, changed the store. The fields follow unless the person is
    /// part way through editing them, which would be losing their typing.
    @objc private func storeChanged() {
        if !hasChanges { load() }
    }

    func load() {
        do {
            let shown = try core.lumenna.showTask(id: id)
            task = shown.task
            title = shown.task.title
            due = Self.dueText(shown.task)
            repetition = shown.task.repetition ?? ""
            priority = shown.task.priority
            estimate = shown.task.estimateMins.map { "\($0)m" } ?? ""
            project = shown.task.project ?? ""
            notes = shown.task.notes
            labels = shown.task.labels.joined(separator: ", ")
            projects = ((try? core.lumenna.listProjects().rows) ?? []).map(\.title)
        } catch {
            task = nil
        }
    }

    /// The due date as a phrase the core reads back in: `2026-10-09 14:00`.
    private static func dueText(_ task: TaskDetail) -> String {
        [task.due, task.dueTime].compactMap { $0 }.joined(separator: " ")
    }

    var hasChanges: Bool {
        guard let task else { return false }
        return title != task.title || due != Self.dueText(task) || repetition != (task.repetition ?? "")
            || priority != task.priority || estimate != (task.estimateMins.map { "\($0)m" } ?? "")
            || project != (task.project ?? "") || notes != task.notes
            || labels != task.labels.joined(separator: ", ")
    }

    /// Sends only the fields that changed, so a concurrent edit to another field on another
    /// device is not overwritten with what this pane happened to show.
    func save() {
        guard let task else { return }
        guard hasChanges else {
            Announcer.say("Nothing changed")
            return
        }
        let edit = TaskEdit(
            title: title != task.title ? title : nil,
            due: due != Self.dueText(task) ? (due.isEmpty ? "none" : due) : nil,
            repeat: repetition != (task.repetition ?? "") ? (repetition.isEmpty ? "none" : repetition) : nil,
            priority: priority != task.priority ? priority : nil,
            estimate: estimate != (task.estimateMins.map { "\($0)m" } ?? "") ? (estimate.isEmpty ? "none" : estimate) : nil,
            notes: notes != task.notes ? notes : nil,
            project: project != (task.project ?? "") && !project.isEmpty ? project : nil,
            labels: labels != task.labels.joined(separator: ", ")
                ? labels.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                : nil
        )
        do {
            let change = try core.lumenna.editTask(id: id, edit: edit)
            load()
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            failure = error.sentence
        }
    }

    var actions: TaskActions? {
        guard let window else { return nil }
        return TaskActions(core: core, window: window, list: list)
    }
}

struct TaskDetailView: View {
    @ObservedObject var model: TaskDetailModel

    var body: some View {
        if let task = model.task {
            form(task)
        } else {
            Text("This task is no longer here.").foregroundStyle(Color.quietLabel)
        }
    }

    private func form(_ task: TaskDetail) -> some View {
        Form {
            Section {
                namedField("Title", text: $model.title, axis: .vertical)
                namedField("Due", text: $model.due, example: "tomorrow")
                    .help("A date, such as tomorrow or next Friday. Empty for none. A new date keeps how it repeats.")
                namedField("Repeats", text: $model.repetition, example: "every monday")
                    .help("Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition.")
                namedField("Estimate", text: $model.estimate, example: "45m")
                    .help("Such as 45m or 1h30m. Empty for none.")
                Named("Project") {
                    Picker("Project", selection: $model.project) {
                        ForEach(model.projects, id: \.self) { Text($0).tag($0) }
                    }
                }
                namedField("Labels", text: $model.labels, example: "calls, errands")
                    .help("Names separated by commas. A new name becomes a label.")
                Named("Priority") {
                    Picker("Priority", selection: $model.priority) {
                        Text("1, highest").tag(UInt8(1))
                        Text("2").tag(UInt8(2))
                        Text("3").tag(UInt8(3))
                        Text("4, none").tag(UInt8(4))
                    }
                }
            }
            Section("Notes") {
                TextEditor(text: $model.notes)
                    .frame(minHeight: 80)
                    .accessibilityLabel("Notes")
            }
            Section("Waits for") {
                ForEach(task.depends, id: \.id) { other in
                    LabeledContent(other.title) {
                        Button("Stop Waiting") {
                            model.actions?.perform(task.id) {
                                try model.core.lumenna.removeDependency(id: task.id, on: other.id)
                            }
                            model.load()
                        }
                        .accessibilityLabel("Stop waiting for \(other.title)")
                    }
                }
                Button("Add Something It Waits For…") { model.actions?.waitFor(task) }
            }
            Section("About") {
                if task.repetition == nil, let rule = task.recurrence {
                    LabeledContent("Repeats by the rule", value: rule)
                }
                LabeledContent("State", value: task.state.joined(separator: ", "))
            }
            Section {
                HStack {
                    // ⌘S is the menu bar's Task > Save Changes, which reaches this pane.
                    Button("Save") { model.save() }
                        .buttonStyle(.borderedProminent)
                    Button(task.state.contains("completed") ? "Mark Not Done" : "Mark Done") {
                        model.actions?.toggleDone(task)
                    }
                }
                HStack {
                    Button("Put in a Block…") { model.actions?.assign(task) }
                    Button("Make Subtask Of…") { model.actions?.makeSubtask(task) }
                    if task.parent != nil {
                        Button("Move to Top Level") {
                            model.actions?.perform(task.id) { try model.core.lumenna.moveTask(id: task.id, to: .top) }
                        }
                    }
                }
                Button("Move to Trash") {
                    model.actions?.perform(nil) { try model.core.lumenna.trashTask(id: task.id) }
                }
                .foregroundStyle(Color.warningLabel)
            }
        }
        .formStyle(.grouped)
        .tint(.lumennaTint)
        .alert(
            "Could not do that",
            isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }),
            presenting: model.failure
        ) { _ in
            Button("OK") {}
        } message: { failure in
            Text(failure)
        }
    }
}
