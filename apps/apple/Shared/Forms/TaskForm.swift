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
    /// What the Project field offers, the core's, in tree order.
    @Published private(set) var projects: [Choice] = []
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
            var options = (try? core.lumenna.projectOptions()) ?? []
            // An archived project is not offered, but a task still in one shows it.
            if !fields.project.isEmpty, !options.contains(where: { $0.id == fields.project }) {
                options.insert(Choice(id: fields.project, title: fields.project, depth: 0, detail: nil, date: nil, start: nil, end: nil), at: 0)
            }
            projects = options
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
    private let words = FormWords.task

    var body: some View {
        if let task = model.task {
            form(task)
        } else {
            Text("This task is no longer here.").foregroundStyle(Color.quietLabel)
        }
    }

    private func form(_ task: TaskDetail) -> some View {
        Form {
            // Each field's name, hint and example are the core's, as in every app.
            Section {
                title
                namedField(words["due"], text: $model.due).modifier(Lowercase())
                namedField(words["repeat"], text: $model.repetition).modifier(Lowercase())
                namedField(words["estimate"], text: $model.estimate).modifier(Lowercase())
                project
                namedField(words["labels"], text: $model.labels).modifier(Lowercase())
            }
            // The priorities and their names are the core's.
            ChoiceSection(words["priority"].label, selection: $model.priority, choices: words["priority"].options.compactMap { choice in
                UInt8(choice.id).map { (choice.title, $0) }
            })
            Section {
                #if os(iOS) || os(watchOS)
                TextField(words["notes"].label, text: $model.notes, prompt: words["notes"].example.isEmpty ? nil : example(words["notes"].example), axis: .vertical)
                    .lineLimit(3...)
                    .accessibilityLabel(words["notes"].label)
                #else
                // A growing text field on the Mac draws its own name beside it, in a grey under
                // contrast; the editor does not.
                TextEditor(text: $model.notes)
                    .frame(minHeight: 80)
                    .accessibilityLabel(words["notes"].label)
                #endif
            } header: {
                FormParts.heading(words["notes"].label)
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

    /// The title: one line, and Return saves. On the phone it wraps, so a large text size
    /// shows all of it, but a Return there saves rather than starting a line.
    @ViewBuilder private var title: some View {
        #if os(iOS)
        namedField(words["title"], text: Binding(get: { model.title }, set: { typed in
            guard typed.contains(where: \.isNewline) else { model.title = typed; return }
            // Return saves rather than starting a line, and a line pasted with breaks in it
            // is joined into one. The field shows the break for a moment, so the title is set
            // again a turn later for it to be taken back out.
            let returned = typed.filter { !$0.isNewline } == model.title
            model.title = typed
            DispatchQueue.main.async {
                model.title = typed.split(whereSeparator: \.isNewline).joined(separator: " ")
                if returned { model.save() }
            }
        }), axis: .vertical)
        #elseif os(macOS)
        namedField(words["title"], text: $model.title).onSubmit { model.save() }
        #else
        // A watch's line comes from the system's input screen, which has no Return.
        namedField(words["title"], text: $model.title)
        #endif
    }

    /// The project, chosen from the projects (`projectOptions`), the chosen one read by its
    /// id: on the Mac a pop-up, which finds one by typing; on the phone and the watch a list
    /// of its own, since an inline list of every project would bury the form.
    @ViewBuilder private var project: some View {
        #if os(iOS) || os(watchOS)
        NavigationLink {
            ProjectChoices(name: words["project"].label, choices: model.projects, selection: $model.project)
        } label: {
            // The system's own value grey is under contrast; this one reads.
            LabeledContent {
                Text(model.projects.first { $0.id == model.project }?.title ?? model.project).foregroundStyle(Color.quietLabel)
            } label: {
                Text(words["project"].label)
            }
        }
        #else
        Named(words["project"].label) {
            Picker(words["project"].label, selection: $model.project) {
                ForEach(Array(model.projects.enumerated()), id: \.element.id) { index, choice in
                    Text(choice.title)
                        .accessibilityLabel(ProjectChoices.spoken(model.projects, index))
                        .tag(choice.id)
                }
            }
        }
        #endif
    }
}

/// The projects to choose from, in tree order. A project's depth is said in words where it
/// changes, as every list here says it, and indented for sight where the list draws it.
struct ProjectChoices: View {
    @Environment(\.dismiss) private var dismiss
    let name: String
    let choices: [Choice]
    @Binding var selection: String

    var body: some View {
        List {
            ForEach(Array(choices.enumerated()), id: \.element.id) { index, choice in
                Button {
                    selection = choice.id
                    dismiss()
                } label: {
                    HStack {
                        Text(choice.title).padding(.leading, CGFloat(choice.depth) * 16)
                        Spacer()
                        if choice.id == selection {
                            Image(systemName: "checkmark").foregroundStyle(Color.lumennaTint).accessibilityHidden(true)
                        }
                    }
                }
                .accessibilityLabel(Self.spoken(choices, index))
                .accessibilityAddTraits(choice.id == selection ? .isSelected : [])
            }
        }
        .navigationTitle(name)
    }

    /// A project's name, then its level where that differs from the one before it.
    static func spoken(_ choices: [Choice], _ index: Int) -> String {
        let choice = choices[index]
        let previous = index > 0 ? choices[index - 1].depth : 0
        return choice.depth != previous ? "\(choice.title), level \(choice.depth + 1)" : choice.title
    }
}

/// A field the core reads, where a capital the keyboard adds by itself is never wanted.
private struct Lowercase: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS) || os(watchOS)
        content.textInputAutocapitalization(.never)
        #else
        content
        #endif
    }
}
