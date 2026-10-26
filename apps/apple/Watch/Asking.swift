import LumennaApp
import SwiftUI

/// How the watch asks the app's questions (`AppQuestions`): each a sheet of its own, as the
/// phone's alerts and pickers are. The root holds one and shows its `asked`.
final class WatchAsker: ObservableObject {
    @Published var asked: Asked?
    private let core: Core
    /// The sheet shown, which `dismissed` hears close.
    private var showing: Asked?
    /// What to show once the sheet in front has gone.
    private var next: Asked?
    /// The day asked for, while its sheet is up.
    private weak var dayAsked: DayAsked?

    init(core: Core) {
        self.core = core
    }

    /// Shows `view`, once the sheet in front has closed, so the two do not collide. `gone` is
    /// what its closing without an answer means — Cancel, or the sheet swiped away.
    func show(_ view: some View, gone: (() -> Void)? = nil) {
        let shown = Asked(view, gone: gone)
        if asked == nil, showing == nil {
            showing = shown
            asked = shown
        } else {
            next = shown
            asked = nil
        }
    }

    /// Closes the sheet shown, its closing meaning nothing: the app let it go.
    func close() {
        showing?.gone = nil
        asked = nil
    }

    /// The sheet closed: answered already, or not, which its `gone` says; then whatever waits.
    func dismissed() {
        let closed = showing
        showing = nil
        closed?.gone?()
        guard let next else { return }
        self.next = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [weak self] in
            guard let self, self.asked == nil, self.showing == nil else { return }
            self.showing = next
            self.asked = next
        }
    }

    func tell(title: String, _ sentence: String) {
        asked = nil
        core.failure = sentence
    }

    /// A row's actions, the app's, offered when it is tapped, in the app's order, with any of the
    /// row's own after them. Each runs on the row `key`, held where `side` says.
    func offer(
        _ title: String, _ actions: [LumennaApp.Offered], on key: String, side: LumennaApp.Side? = nil,
        extra: [(String, () -> Void)] = []
    ) {
        show(ChoicePrompt(title: title, choices: actions.map { action in
            (action.title, { [weak self] in self?.core.app.run(action, on: key, side: side) })
        } + extra))
    }
}

/// The app's questions (`AppQuestions`), each a sheet as the surface's are. A sheet that goes
/// without an answer — swiped away, or Cancel — says so: the view showing `asked` calls
/// `dismissed` as its sheet closes.
extension WatchAsker: QuestionAsking {
    /// Shows `view` for one of the app's questions, `gone` its answer if it closes unanswered.
    private func ask(_ view: some View, gone: @escaping () -> Void) {
        show(view, gone: gone)
    }

    func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void) {
        let reply = Once(answered)
        // Cancel first: what cannot be undone is never the first thing under a finger.
        ask(ChoicePrompt(title: title, message: message, cancel: true, choices: [(yes, { reply(true) })]), gone: { reply(false) })
    }

    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, syntax: LumennaApp.Syntax?, day: String?, answered: @escaping (String?) -> Void) {
        let reply = Once(answered)
        // A day (Go to Day, Another Day): the line and the watch's own picker. Asked again after
        // a refusal, the sheet still up says why, the typing kept.
        if let day {
            if let open = dayAsked, open.shown, !open.waiting {
                open.askAgain(problem: problem) { reply($0) }
                return
            }
            let asked = DayAsked(app: core.app, label: label, hint: hint, typed: initial, problem: problem, day: Clock.day(day) ?? .now) { reply($0) }
            asked.close = { [weak self] in self?.close() }
            dayAsked = asked
            ask(DayChoice(asked: asked, title: title, yes: yes), gone: { asked.cancel() })
            return
        }
        let message = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: " ")
        // A line in the filter language has the core's completions after it.
        ask(TextPrompt(title, message: message.isEmpty ? nil : message, initial: initial, placeholder: label, action: yes, syntax: syntax, allowsEmpty: true) { text in
            reply(text)
        }, gone: { reply(nil) })
    }

    // A watch's pick is a list of buttons, each answering by itself: no button names the answer.
    func pick(title: String, offered: [Offered], yes: String, answered: @escaping (Picked) -> Void) {
        let reply = Once(answered)
        ask(OfferedPrompt(title: title, offered: offered, picked: { reply($0) }), gone: { reply(.cancelled) })
    }

    func choose(title: String, message: String, answers: [(id: String, title: String)], destructive: Bool, answered: @escaping (String?) -> Void) {
        let reply = Once(answered)
        ask(ChoicePrompt(title: title, message: message, choices: answers.map { answer in (answer.title, { reply(answer.id) }) }), gone: { reply(nil) })
    }
}

/// An answer given once: a choice and the sheet closing after it are one answer, not two.
private final class Once<Answer> {
    private var answer: ((Answer) -> Void)?

    init(_ answer: @escaping (Answer) -> Void) {
        self.answer = answer
    }

    func callAsFunction(_ given: Answer) {
        let answer = self.answer
        self.answer = nil
        answer?(given)
    }
}

/// One of what the app offers, as it words it, each a button. A tree's depth is said where it
/// changes, never by indentation.
private struct OfferedPrompt: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let offered: [Offered]
    let picked: (Picked) -> Void

    var body: some View {
        List {
            ForEach(Array(offered.enumerated()), id: \.offset) { index, choice in
                let previous = index > 0 ? offered[index - 1].depth : 0
                let level = choice.depth != previous ? "level \(choice.depth + 1)" : nil
                Button {
                    picked(.chose(choice.id))
                    dismiss()
                } label: {
                    HStack {
                        SwatchMark(swatch: choice.swatch)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(choice.title)
                            // Why it is offered, the core's: a line beneath, and the hint.
                            if let reason = choice.reason {
                                Text(reason).font(.footnote).foregroundStyle(Color.quietLabel)
                            }
                        }
                    }
                }
                .accessibilityLabel(choice.title)
                .accessibilityValue([choice.detail, level].compactMap { $0 }.joined(separator: ", "))
                .accessibilityHint(choice.reason ?? "")
            }
        }
        .navigationTitle(title)
    }
}

/// A question asking for a day — Go to Day, Another Day's — the app's, its words the core's: the
/// day typed or said, the picker's, and why the last answer was refused. It stays up while the
/// app refuses what was given, and goes once the app has taken an answer.
final class DayAsked: ObservableObject {
    let label: String
    let hint: String
    @Published var typed: String
    @Published var day: Date
    @Published var problem: String?
    /// Whether its sheet is up.
    var shown = true
    var close: () -> Void = {}
    private var answer: ((String?) -> Void)?
    private let app: AppModel

    init(app: AppModel, label: String, hint: String, typed: String, problem: String?, day: Date, answered: @escaping (String?) -> Void) {
        self.app = app
        self.label = label
        self.hint = hint
        self.typed = typed
        self.problem = problem
        self.day = day
        answer = answered
    }

    /// Whether it waits for an answer: false once one is given, until asked again.
    var waiting: Bool { answer != nil }

    func askAgain(problem: String?, answered: @escaping (String?) -> Void) {
        self.problem = problem
        answer = answered
        if let problem { Announcer.say(problem) }
    }

    /// The day typed if there is one, else the picker's. A day the app cannot read is asked
    /// again (`askAgain`); once it asks nothing more, the sheet goes.
    func go() {
        guard waiting else { return }
        let typed = typed.trimmingCharacters(in: .whitespacesAndNewlines)
        give(typed.isEmpty ? Clock.isoDay(day) : typed)
        app.whenStill { [weak self] in
            guard let self, !self.waiting else { return }
            self.shown = false
            self.close()
        }
    }

    /// Closed without an answer.
    func cancel() {
        shown = false
        give(nil)
    }

    private func give(_ given: String?) {
        let answer = self.answer
        self.answer = nil
        answer?(given)
    }
}

/// A day to go to, chosen as the phone chooses one: a day said or typed in the core's words
/// ("next friday", "12 October") from the system's input screen, or a date picker, then the
/// question's own answer.
private struct DayChoice: View {
    @ObservedObject var asked: DayAsked
    let title: String
    let yes: String

    var body: some View {
        List {
            Section {
                TextField(asked.label, text: $asked.typed)
                    .textInputAutocapitalization(.never)
                    // Named explicitly: with text in it, a field's title gives way to the text.
                    .accessibilityLabel(asked.label)
                    .accessibilityHint(asked.hint)
                    .onSubmit { asked.go() }
                if let problem = asked.problem {
                    Text(problem).foregroundStyle(Color.warningLabel)
                }
            } footer: {
                FormParts.caption(asked.hint)
            }
            Section {
                NamedDatePicker(asked.label, selection: $asked.day, displayedComponents: .date)
                Button(yes) { asked.go() }
            }
        }
        .navigationTitle(title)
    }
}
