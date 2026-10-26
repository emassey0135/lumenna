import LumennaApp
import SwiftUI
import UserNotifications

/// A task's or block's reminders, the app's open list (`ViewModel.reminders`), as the Mac and
/// the watch show them; the iPhone's is a UIKit list (`RemindersViewController`). The heading
/// says how many and whether they are the defaults; each reads as the core words it; its own
/// offer Remove Reminder; Add Reminder takes a phrase the core reads, and a refused one stays in
/// its field with the app's reason. Held in the reminder list's hand, so a change lands here.
final class RemindersModel: ObservableObject {
    let core: Core
    @Published private(set) var list: LumennaApp.ReminderList?
    /// The phrase being typed for Add Reminder.
    @Published var typed = ""
    /// The app's reason it refused the phrase, shown with the field.
    @Published var problem: String?
    @Published var failure: String?
    /// Whether the person turned Lumenna's notifications off, said under the list.
    @Published private(set) var silenced = false
    /// Closes the list's screen once the app has put it away; set by whoever shows it.
    var close: () -> Void = {}
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        list = core.app.view.reminders
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            guard let self else { return }
            self.list = self.core.app.view.reminders
            if self.list == nil { self.close() }
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Someone looking at reminders wants them shown: the moment to ask, if never asked.
    func appeared() {
        Reminders.askIfUndecided { [weak self] in
            UNUserNotificationCenter.current().authorization { status in self?.silenced = status == .denied }
        }
    }

    /// Puts the list away in the app.
    func done() {
        core.app.send(.closeReminders)
    }

    /// Add Reminder, the list's own action, whole: its question's words are this list's field.
    var adding: LumennaApp.Action? { list?.actions.first }

    /// Add Reminder's line: its title, the field's name and hint, and its button.
    var question: (title: String, label: String, hint: String, yes: String)? {
        guard case let .text(title, label, _, hint, _, yes, _) = adding?.question else { return nil }
        return (title, label, hint, yes)
    }

    /// Adds the phrase typed, answering the app's Add Reminder itself (`AppQuestions` leaves it
    /// to the list: `answersItself`): once the app is still, a phrase it refused stays in the
    /// field with why, the question put away again.
    func add() {
        guard let adding = list?.offered.first else { return }
        core.app.act(adding, on: .reminders)
        core.app.send(.answer(.text(text: typed)))
        core.app.whenStill { [weak self] in
            guard let self else { return }
            if let refusal = self.core.app.view.asking?.refusal {
                self.core.app.send(.cancel)
                self.problem = refusal
                Announcer.say(refusal)
            } else {
                self.typed = ""
                self.problem = nil
            }
        }
    }

    /// Whether `asking` is the list's own Add Reminder, which it answers itself.
    static func answersItself(_ asking: LumennaApp.Asking) -> Bool {
        asking.action?.subject == .reminder && asking.action?.kind == .new
    }

    /// One of a reminder's actions, the app's: Remove Reminder, or Deliver On, whose pick the
    /// window asks.
    func run(_ action: LumennaApp.Offered, on reminder: String) {
        core.app.run(action, on: reminder, side: .reminders)
    }
}

struct RemindersForm: View {
    @ObservedObject var model: RemindersModel
    #if !os(macOS)
    @State private var offered: LumennaApp.ReminderView?
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
                    FormParts.caption(LumennaApp.notificationWords(thisDevice: AppModel.thisDevice, sentence: false).off)
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
                    // Named by the action, Add Reminder, not its question's bare Add: on the watch a
                    // button's own text wins over any label given it.
                    Button(adding.title) { model.add() }
                } header: {
                    FormParts.heading(question.title)
                }
            }
            #if os(macOS)
            Section {
                Button("Done") { model.done() }.keyboardShortcut(.cancelAction)
            }
            #endif
        }
        #if os(macOS)
        .formStyle(.grouped)
        .frame(width: 440)
        .fixedSize(horizontal: false, vertical: true)
        #endif
        .onAppear { model.appeared() }
        #if !os(macOS)
        .confirmationDialog(offered?.words ?? "", isPresented: Binding(get: { offered != nil }, set: { if !$0 { offered = nil } }), presenting: offered) { reminder in
            ForEach(reminder.offered, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) { model.run(action, on: reminder.id) }
            }
        }
        #endif
        .modifier(FailureAlert(failure: $model.failure))
    }

    @ViewBuilder private func field(_ question: (title: String, label: String, hint: String, yes: String)) -> some View {
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
    @ViewBuilder private func row(_ reminder: LumennaApp.ReminderView) -> some View {
        let spoken = reminder.own ? reminder.words : "\(reminder.words), Default"
        #if os(macOS)
        // A Mac form's row: the words, "Default" beneath for a default, the buttons beside.
        LabeledContent {
            ForEach(reminder.offered, id: \.self) { action in
                Button(action.title) { model.run(action, on: reminder.id) }
                    .accessibilityLabel("\(action.title), \(reminder.words)")
            }
        } label: {
            Text(reminder.words)
            if !reminder.own { Text("Default").foregroundStyle(Color.quietLabel) }
        }
        #else
        if reminder.offered.isEmpty {
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
