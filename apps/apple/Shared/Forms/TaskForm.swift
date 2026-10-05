import SwiftUI

/// What only the platform can do for the task form: choose a task or a block in its own way,
/// and leave once the task has gone to the trash.
protocol TaskFormHost: AnyObject {
    /// Offers the open tasks, less `excluding`, and hands back the one chosen.
    func chooseTask(_ title: String, excluding: Set<String>, chosen: @escaping (String) -> Void)
    /// Offers the work blocks a task could go in and asks how long the sitting is meant to
    /// take (§3.7); hands back the block's identifier, its day, and the minutes or none.
    func chooseBlock(for task: TaskDetail, chosen: @escaping (_ block: String, _ date: String, _ minutes: UInt32?) -> Void)
    /// The task went to the trash.
    func trashed()
}

/// One task as stored, and its fields as edited (§16.1: task detail / edit) — the model both
/// Apple apps' task forms share.
final class TaskDetailModel: NSObject, ObservableObject {
    let core: Core
    let id: String
    weak var host: TaskFormHost?

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

    /// Another device, another process, or this app elsewhere changed the store. The fields
    /// follow unless the person is part way through editing them, which would lose their
    /// typing.
    @objc private func storeChanged() {
        if !hasChanges { load() }
    }

    /// Reads the task again and resets the fields to it.
    func load() {
        do {
            let shown = try core.lumenna.showTask(id: id)
            task = shown.task
            let fields = taskFields(task: shown.task)
            title = fields.title
            due = fields.due
            repetition = fields.repeat
            priority = fields.priority
            estimate = fields.estimate
            project = fields.project
            notes = fields.notes
            labels = fields.labels
            projects = ((try? core.lumenna.listProjects().rows) ?? []).map(\.title)
        } catch {
            task = nil
        }
    }

    /// The form's fields as the core compares them.
    private var fields: TaskFields {
        TaskFields(
            title: title, due: due, repeat: repetition, priority: priority, estimate: estimate,
            project: project, labels: labels, notes: notes
        )
    }

    /// What saving would send, or nil if nothing changed. The core decides (`taskEdit`), as
    /// it does for every client: a field sent unchanged would win a last-write-wins race and
    /// revert another device's edit to it.
    private var edit: TaskEdit? {
        task.flatMap { taskEdit(task: $0, fields: fields) }
    }

    var hasChanges: Bool { edit != nil }

    /// Sends only the fields that changed.
    func save() {
        guard let edit else {
            Announcer.say("Nothing changed")
            return
        }
        change { try $0.editTask(id: self.id, edit: edit) }
    }

    /// Runs a change, reads the task again, tells every view showing the store, and says
    /// what happened.
    func change(_ operation: (Lumenna) throws -> Change) {
        do {
            let change = try operation(core.lumenna)
            load()
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            failure = error.sentence
        }
    }

    func toggleDone() {
        let done = task?.state.contains("completed") == true
        change { done ? try $0.uncompleteTask(id: self.id) : try $0.completeTask(id: self.id) }
    }

    /// Chooses a task for this one to wait for (§3.3).
    func addDependency() {
        guard let task else { return }
        host?.chooseTask("Waits For", excluding: Set(task.depends.map(\.id) + [id])) { [weak self] other in
            self?.change { try $0.addDependency(id: task.id, on: other) }
        }
    }

    func removeDependency(_ other: Dependency) {
        change { try $0.removeDependency(id: self.id, on: other.id) }
    }

    /// Puts this task under another, joining that one's project.
    func makeSubtask() {
        host?.chooseTask("Make Subtask Of", excluding: [id]) { [weak self] parent in
            self?.change { try $0.moveTask(id: self!.id, to: .parent(id: parent)) }
        }
    }

    func moveToTop() {
        change { try $0.moveTask(id: self.id, to: .top) }
    }

    /// Puts this task into a block, for a sitting of the length chosen.
    func assign() {
        guard let task else { return }
        host?.chooseBlock(for: task) { [weak self] block, date, minutes in
            self?.change { try $0.assign(task: task.id, block: block, date: date, minutes: minutes) }
        }
    }

    func trash() {
        do {
            let change = try core.lumenna.trashTask(id: id)
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
            host?.trashed()
        } catch {
            failure = error.sentence
        }
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
                namedField("Title", text: $model.title, example: "What to do", axis: .vertical)
                namedField("Due", text: $model.due, example: "tomorrow")
                    .modifier(Hint("A date, such as tomorrow or next Friday. Empty for none. A new date keeps how it repeats."))
                namedField("Repeats", text: $model.repetition, example: "every monday")
                    .modifier(Hint("Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition."))
                namedField("Estimate", text: $model.estimate, example: "45m")
                    .modifier(Hint("Such as 45m or 1h30m. Empty for none."))
                project
                namedField("Labels", text: $model.labels, example: "calls, errands")
                    .modifier(Hint("Names separated by commas. A new name becomes a label."))
            }
            ChoiceSection("Priority", selection: $model.priority, choices: [
                ("Priority 1, highest", UInt8(1)), ("Priority 2", 2), ("Priority 3", 3), ("Priority 4, none", 4),
            ])
            Section {
                #if os(iOS)
                TextField("Notes", text: $model.notes, prompt: example("Anything else"), axis: .vertical)
                    .lineLimit(3...)
                    .accessibilityLabel("Notes")
                #else
                // A growing text field on the Mac draws its own name beside it, in a grey under
                // contrast; the editor does not.
                TextEditor(text: $model.notes)
                    .frame(minHeight: 80)
                    .accessibilityLabel("Notes")
                #endif
            } header: {
                FormParts.heading("Notes")
            }
            Section {
                ForEach(task.depends, id: \.id) { other in
                    Button("Stop Waiting for \(other.title)") { model.removeDependency(other) }
                }
                Button("Add Something It Waits For…") { model.addDependency() }
            } header: {
                FormParts.heading("Waits for")
            }
            Section {
                if task.repetition == nil, let rule = task.recurrence {
                    LabeledContent("Repeats by the rule", value: rule)
                }
                LabeledContent("State", value: task.state.joined(separator: ", "))
            } header: {
                FormParts.heading("About")
            }
            Section {
                #if os(macOS)
                // On the phone Save is in the navigation bar; here ⌘S reaches it too.
                Button("Save") { model.save() }
                #endif
                Button(task.state.contains("completed") ? "Mark Not Done" : "Mark Done") { model.toggleDone() }
                Button("Put in a Block…") { model.assign() }
                Button("Make Subtask Of…") { model.makeSubtask() }
                if task.parent != nil {
                    Button("Move to Top Level") { model.moveToTop() }
                }
                WarningButton("Move to Trash") { model.trash() }
            }
        }
        #if os(macOS)
        .formStyle(.grouped)
        #endif
        .modifier(FailureAlert(failure: $model.failure))
    }

    /// The project: a pop-up of the projects on the Mac, where one is a click away; a name on
    /// the phone, where an inline list of every project would bury the form.
    @ViewBuilder private var project: some View {
        #if os(iOS)
        namedField("Project", text: $model.project, example: "Inbox")
        #else
        Named("Project") {
            Picker("Project", selection: $model.project) {
                ForEach(model.projects, id: \.self) { Text($0).tag($0) }
            }
        }
        #endif
    }
}

/// What a field takes, said by VoiceOver after a pause on iOS and shown as a tooltip on macOS.
private struct Hint: ViewModifier {
    let text: String

    init(_ text: String) { self.text = text }

    func body(content: Content) -> some View {
        #if os(iOS)
        content.textInputAutocapitalization(.never).accessibilityHint(text)
        #else
        content.help(text)
        #endif
    }
}

/// A work block a task could be put in: what identifies it to the core, its day, and how it
/// reads in a list.
struct BlockChoice {
    let id: String
    let date: String
    let title: String
    let detail: String
}

extension Core {
    /// The work blocks a task could go in from the task itself (§3.7). Which ones — the work
    /// blocks of the coming week — is the core's (`workBlocks`), as for every app; how each
    /// reads is this one's. The planner reaches any other day.
    func workBlocksThisWeek() -> [BlockChoice] {
        let blocks = (try? lumenna.workBlocks(from: nil, days: nil).blocks) ?? []
        return blocks.map { block in
            BlockChoice(
                id: block.id,
                date: block.date,
                title: "\(Clock.spokenDay(block.date)), \(Clock.time(block.start)), \(block.title)",
                detail: "\(Clock.time(block.start)) to \(Clock.time(block.end))"
            )
        }
    }
}
