import LumennaApp
import SwiftUI

/// What only the platform can do for the task form: leave once the task has gone to the
/// trash. Its actions' questions are the window's (`AppQuestions`).
protocol TaskFormHost: AnyObject {
    /// The task went to the trash.
    func trashed()
}

/// One task's details, the app's (`ViewModel.details`) — the model the Apple apps' task forms
/// share. The app holds the task, the fields as typed and what saving sends: each field goes to
/// it as the person changes it, and Save sends only what changed, since a field sent unchanged
/// would win a last-write-wins race and revert another device's edit to it. The fields follow
/// the app's, unless typing is still on its way, which an answer for earlier typing would undo.
final class TaskDetailModel: NSObject, ObservableObject {
    let core: Core
    let id: String
    weak var host: TaskFormHost?

    @Published private(set) var task: LumennaApp.TaskDetail?
    @Published var title = "" { didSet { typed("title", title) } }
    @Published var due = "" { didSet { typed("due", due) } }
    @Published var repetition = "" { didSet { typed("repeat", repetition) } }
    @Published var priority: UInt8 = 4 { didSet { typed("priority", String(priority)) } }
    @Published var estimate = "" { didSet { typed("estimate", estimate) } }
    @Published var project = "" { didSet { typed("project", project) } }
    @Published var notes = "" { didSet { typed("notes", notes) } }
    @Published var labels = "" { didSet { typed("labels", labels) } }
    /// What the Project field offers, the app's, in tree order.
    @Published private(set) var projects: [LumennaApp.Choice] = []
    /// The fields as the core words them: each one's name, what it takes and an example.
    @Published private(set) var form: [LumennaApp.FormField] = []
    /// What the details list besides the form, the app's: the state, the rule it repeats by,
    /// what it waits for, its history.
    @Published private(set) var lines: [LumennaApp.DetailsLine] = []
    /// The details' own words, the core's.
    let words = LumennaApp.detailsWords(sentence: false)
    @Published var failure: String?

    /// Whether the fields are being set from the app's, not typed.
    private var following = false
    /// Whether the app's fields are waiting for the app to be still.
    private var filling = false
    /// Whether the task was asked to go to the trash, to leave once it has.
    private var trashing = false
    private var observer: NSObjectProtocol?

    init(core: Core, id: String) {
        self.core = core
        self.id = id
        super.init()
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.follow()
        }
        show()
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Asks the app for this task's details: as the form opens, and as it comes back to the
    /// front, since another screen may have shown another task's.
    func show() {
        core.app.send(.details(.show(id)))
        follow()
    }

    /// The app's details of this task, into the fields: unless typing is still on its way.
    private func follow() {
        let details = core.app.view.details.flatMap { $0.task.id == id ? $0 : nil }
        // Gone to the trash, as asked — the app shows another task's details, or this one's
        // as trashed: the form leaves.
        if trashing, task != nil, details == nil || details?.task.state.contains("deleted") == true {
            trashing = false
            host?.trashed()
            return
        }
        guard let details else { return }
        task = details.task
        projects = details.projects
        if form != details.form { form = details.form }
        if lines != details.lines { lines = details.lines }
        guard !filling else { return }
        filling = true
        core.app.whenStill { [weak self] in
            self?.filling = false
            self?.fill()
        }
    }

    /// The app's fields, into the form's, once nothing is on its way.
    private func fill() {
        guard let details = core.app.view.details, details.task.id == id else { return }
        let fields = details.fields
        following = true
        defer { following = false }
        if title != fields.title { title = fields.title }
        if due != fields.due { due = fields.due }
        if repetition != fields.repeat { repetition = fields.repeat }
        if priority != fields.priority { priority = fields.priority }
        if estimate != fields.estimate { estimate = fields.estimate }
        if project != fields.project { project = fields.project }
        if notes != fields.notes { notes = fields.notes }
        if labels != fields.labels { labels = fields.labels }
    }

    /// A field the person changed, to the app.
    private func typed(_ key: String, _ value: String) {
        guard !following, task != nil else { return }
        core.app.send(.details(.field(key: key, value: value)))
    }

    var hasChanges: Bool { core.app.view.details?.dirty == true }

    /// Sends only the fields that changed; the app says if nothing did.
    func save() {
        core.app.send(.details(.save))
    }

    /// Runs one of the task's actions, the app's: after Move to Trash the form leaves.
    func run(_ action: LumennaApp.Offered) {
        if action.kind == .delete { trashing = true }
        core.app.act(action, on: .details)
    }

    /// The core's command `id` on this task ("mark-done", "put-in-block"), as a key asks, or the
    /// app says why it has none.
    func key(_ id: String) {
        core.app.send(.details(.key(id)))
    }

    /// Mark Done or Mark Not Done, as a key asks.
    func toggleDone() {
        key("mark-done")
    }

    /// Move to Trash, as a key asks; the form leaves once the task has gone.
    func trash() {
        trashing = task?.offered.contains { $0.kind == .delete } == true
        key("trash-task")
    }
}

struct TaskDetailView: View {
    @ObservedObject var model: TaskDetailModel
    private var words: FormWords { FormWords(model.form) }

    var body: some View {
        if let task = model.task {
            form(task).showingTask(task.id, titled: task.title)
        } else {
            Text(model.words.gone).foregroundStyle(Color.quietLabel)
        }
    }

    private func form(_ task: LumennaApp.TaskDetail) -> some View {
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
            // What the details list besides the fields, the app's lines in its words: the state,
            // the rule it repeats by, what it waits for, and its past, on its own screen only.
            Section {
                ForEach(model.lines, id: \.key) { line in
                    LabeledContent(line.label, value: line.value)
                }
            } header: {
                FormParts.heading(model.words.about)
            }
            Section {
                #if os(macOS) || os(watchOS)
                // On the phone Save is in the navigation bar; on the Mac ⌘S reaches it too,
                // and a watch has no bar button for it.
                Button(model.words.save) { model.save() }
                #endif
                // What can be done to it, the core's, in its order.
                ForEach(task.offered, id: \.self) { action in
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
    private static func title(_ action: LumennaApp.Offered) -> String {
        #if os(macOS)
        action.asksFirst ? action.title + "…" : action.title
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
    let choices: [LumennaApp.Choice]
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
    static func spoken(_ choices: [LumennaApp.Choice], _ index: Int) -> String {
        let choice = choices[index]
        let previous = index > 0 ? choices[index - 1].depth : 0
        return choice.depth != previous ? "\(choice.title), \(LumennaApp.levelWords(depth: choice.depth))" : choice.title
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
