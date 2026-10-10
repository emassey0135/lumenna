import SwiftUI

/// What only the platform can do for the task form: ask the questions of the task's actions
/// in its own way, and leave once the task has gone to the trash.
protocol TaskFormHost: AnyObject {
    /// Who asks an action's question.
    var asker: ActionAsking { get }
    /// The task went to the trash.
    func trashed()
}

/// One task as stored, and its fields as edited — the model the Apple apps' task forms share.
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

    /// Runs one of the task's actions, the core's: after Move to Trash the form leaves.
    func run(_ action: Action) {
        guard let host else { return }
        ActionRun.run(action, on: core.lumenna, asking: host.asker, form: { _ in }) { [weak self] change, _ in
            guard let self else { return }
            self.load()
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
            if action.kind == .delete, change.changed { self.host?.trashed() }
        }
    }

    /// Mark Done or Mark Not Done, as a key asks.
    func toggleDone() {
        if let action = task?.actions.first(.markDone, .markNotDone) { run(action) }
    }

    /// Move to Trash, as a key asks.
    func trash() {
        if let action = task?.actions.first(.delete) { run(action) }
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
            // The priorities and their names are the core's.
            ChoiceSection("Priority", selection: $model.priority, choices: priorities().compactMap { choice in
                UInt8(choice.id).map { (choice.title, $0) }
            })
            Section {
                #if os(iOS) || os(watchOS)
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
                if task.repetition == nil, let rule = task.recurrence {
                    LabeledContent("Repeats by the rule", value: rule)
                }
                LabeledContent("State", value: task.state.joined(separator: ", "))
            } header: {
                FormParts.heading("About")
            }
            Section {
                #if os(macOS) || os(watchOS)
                // On the phone Save is in the navigation bar; on the Mac ⌘S reaches it too,
                // and a watch has no bar button for it.
                Button("Save") { model.save() }
                #endif
                // What can be done to it, the core's, in its order.
                ForEach(task.actions, id: \.self) { action in
                    if action.destructive {
                        WarningButton(Self.title(action)) { model.run(action) }
                    } else {
                        Button(Self.title(action)) { model.run(action) }
                    }
                }
            }
        }
        #if os(macOS)
        .formStyle(.grouped)
        #endif
        .modifier(FailureAlert(failure: $model.failure))
    }

    /// An action's name on its button: on the Mac, one that asks something ends in "…".
    private static func title(_ action: Action) -> String {
        #if os(macOS)
        action.asks ? action.title + "…" : action.title
        #else
        action.title
        #endif
    }

    /// The project: a pop-up of the projects on the Mac, where one is a click away; a name on
    /// the phone, where an inline list of every project would bury the form.
    @ViewBuilder private var project: some View {
        #if os(iOS) || os(watchOS)
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
        #if os(iOS) || os(watchOS)
        content.textInputAutocapitalization(.never).accessibilityHint(text)
        #else
        content.help(text)
        #endif
    }
}
