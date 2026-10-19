import SwiftUI
import UserNotifications

/// A task's or block's reminders, the core's list (`remindersOf`), as the watch shows them over
/// the surface until its screens move onto the app, as the Mac's have (`RemindersModel`). Its
/// own offer Remove Reminder; Add Reminder takes a phrase the core reads, and a refused one
/// stays in its field with the core's reason.
final class SurfaceRemindersModel: ObservableObject {
    let core: Core
    let target: String
    @Published private(set) var list: ReminderList?
    /// The phrase being typed for Add Reminder.
    @Published var typed = ""
    /// The core's reason it refused the phrase, shown with the field.
    @Published var problem: String?
    @Published var failure: String?
    /// An action asking to be confirmed first.
    @Published var confirming: Action?
    /// Whether the person turned Lumenna's notifications off, said under the list.
    @Published private(set) var silenced = false
    private var observer: NSObjectProtocol?

    init(core: Core, target: String) {
        self.core = core
        self.target = target
        load()
        observer = NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.load()
        }
    }

    func load() {
        do {
            list = try core.lumenna.remindersOf(target: target)
        } catch {
            failure = error.sentence
        }
    }

    /// Someone looking at reminders wants them shown: the moment to ask, if never asked.
    func appeared() {
        Reminders.askIfUndecided { [weak self] in
            UNUserNotificationCenter.current().authorization { status in self?.silenced = status == .denied }
        }
    }

    /// Add Reminder, the list's own action.
    var adding: Action? { list?.actions.first }
    var question: TextQuestion? { adding.flatMap { TextQuestion($0.question) } }

    func add() {
        guard let adding else { return }
        do {
            let change = try core.lumenna.act(action: adding, answer: .text(text: typed))
            typed = ""
            problem = nil
            changed(change)
        } catch {
            problem = error.sentence
            Announcer.say(error.sentence)
        }
    }

    /// One of a reminder's actions, the core's: Remove Reminder.
    func run(_ action: Action, confirmed: Bool = false) {
        if case .confirm = action.question, !confirmed {
            confirming = action
            return
        }
        do {
            changed(try core.lumenna.act(action: action, answer: .yes))
        } catch {
            failure = error.sentence
        }
    }

    private func changed(_ change: Change) {
        load()
        // Every view reads the store again and the reminders are scheduled anew.
        NotificationCenter.default.post(name: Core.changed, object: nil)
        Announcer.say(change.announcement, notices: change.notices)
    }
}

struct SurfaceRemindersForm: View {
    @ObservedObject var model: SurfaceRemindersModel
    /// Closes the Mac's sheet.
    var done: () -> Void = {}
    #if !os(macOS)
    @State private var offered: ReminderView?
    #endif

    var body: some View {
        Form {
            Section {
                ForEach(model.list?.reminders ?? [], id: \.id) { reminder in
                    row(reminder)
                }
            } header: {
                FormParts.heading(model.list?.announcement ?? "Reminders")
            } footer: {
                if model.silenced {
                    #if os(macOS)
                    FormParts.caption("Notifications for Lumenna are off in System Settings, so these are not shown.")
                    #else
                    FormParts.caption("Notifications for Lumenna are off in Settings, so these are not shown.")
                    #endif
                }
            }
            if let question = model.question, let adding = model.adding {
                Section {
                    Named(question.label) {
                        field(question).onSubmit { model.add() }
                    }
                    .modifier(FieldHint(question.hint))
                    if let problem = model.problem {
                        Text(problem)
                            .font(.footnote)
                            .foregroundStyle(Color.warningLabel)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    Button(question.yes) { model.add() }
                        .accessibilityLabel(adding.title)
                } header: {
                    FormParts.heading(question.title)
                }
            }
            #if os(macOS)
            Section {
                Button("Done", action: done).keyboardShortcut(.cancelAction)
            }
            #endif
        }
        #if os(macOS)
        .formStyle(.grouped)
        .frame(width: 440)
        .fixedSize(horizontal: false, vertical: true)
        #endif
        .onAppear { model.appeared() }
        .confirmationDialog(
            model.confirming.flatMap { if case let .confirm(title, _, _) = $0.question { title } else { nil } } ?? "",
            isPresented: Binding(get: { model.confirming != nil }, set: { if !$0 { model.confirming = nil } }),
            presenting: model.confirming
        ) { action in
            if case let .confirm(_, _, yes) = action.question {
                Button(yes, role: action.destructive ? .destructive : nil) { model.run(action, confirmed: true) }
            }
            Button("Cancel", role: .cancel) {}
        } message: { action in
            if case let .confirm(_, message, _) = action.question { Text(message) }
        }
        #if !os(macOS)
        .confirmationDialog(offered?.words ?? "", isPresented: Binding(get: { offered != nil }, set: { if !$0 { offered = nil } }), presenting: offered) { reminder in
            ForEach(reminder.actions, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action) }
            }
        }
        #endif
        .modifier(FailureAlert(failure: $model.failure))
    }

    @ViewBuilder private func field(_ question: TextQuestion) -> some View {
        #if os(macOS)
        TextField(question.label, text: $model.typed)
        #else
        TextField("", text: $model.typed)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
        #endif
    }

    /// A reminder: its words, and "Default" for one of the defaults. One of its own offers its
    /// actions: on the Mac as buttons beside it, on the watch when tapped.
    @ViewBuilder private func row(_ reminder: ReminderView) -> some View {
        let spoken = reminder.own ? reminder.words : "\(reminder.words), Default"
        #if os(macOS)
        // A Mac form's row: the words, "Default" beneath for a default, the buttons beside.
        LabeledContent {
            ForEach(reminder.actions, id: \.self) { action in
                Button(action.title) { model.run(action) }
                    .accessibilityLabel("\(action.title), \(reminder.words)")
            }
        } label: {
            Text(reminder.words)
            if !reminder.own { Text("Default").foregroundStyle(Color.quietLabel) }
        }
        #else
        if reminder.actions.isEmpty {
            Text(spoken)
        } else {
            Button { offered = reminder } label: {
                Text(reminder.words).frame(maxWidth: .infinity, alignment: .leading)
            }
            .accessibilityLabel(spoken)
        }
        #endif
    }
}
