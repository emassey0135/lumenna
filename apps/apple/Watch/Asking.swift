import SwiftUI

/// How the watch asks an action's questions (`ActionRun`): each a sheet of its own, as the
/// phone's alerts and pickers are. A view holds one and shows its `asked`.
final class WatchAsker: ObservableObject, ActionAsking {
    @Published var asked: Asked?
    private let core: WatchCore

    init(core: WatchCore) {
        self.core = core
    }

    /// Shows `view`, once the sheet in front has closed, so the two do not collide.
    func show(_ view: some View) {
        let shown = AnyView(view)
        if asked == nil {
            asked = Asked(shown)
        } else {
            asked = nil
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [weak self] in self?.asked = Asked(shown) }
        }
    }

    func askConfirm(title: String, message: String, yes: String, destructive: Bool, then: @escaping () -> Void) {
        // Cancel first: what cannot be undone is never the first thing under a finger.
        show(ChoicePrompt(title: title, message: message, cancel: true, choices: [(yes, then)]))
    }

    func askText(title: String, label: String, initial: String, hint: String, problem: String?, then: @escaping (String) -> Void) {
        let message = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: " ")
        show(TextPrompt(title, message: message.isEmpty ? nil : message, initial: initial, placeholder: label, allowsEmpty: true, done: then))
    }

    func askPick(title: String, choices: [Choice], then: @escaping (Choice) -> Void) {
        show(PickPrompt(title: title, choices: choices, chosen: then))
    }

    func askChoose(title: String, message: String, answers: [Choice], then: @escaping (Choice) -> Void) {
        show(ChoicePrompt(title: title, message: message, choices: answers.map { answer in (answer.title, { then(answer) }) }))
    }

    /// From the lengths a sitting usually takes, or none: quicker on a watch than typing.
    func askLength(hint: String, then: @escaping (String) -> Void) {
        show(LengthChoice(title: "Planned length", without: "No Planned Length") { minutes in
            then(minutes.map { "\($0)m" } ?? "")
        })
    }

    func tell(title: String, _ sentence: String) {
        asked = nil
        core.failure = sentence
    }

    func fail(_ sentence: String) {
        asked = nil
        core.failure = sentence
    }

    /// Runs one of the core's actions; a form opens through `form`. What changed redraws every
    /// view, goes to the phone, and is said.
    func run(_ action: Action, form: (Action) -> Void = { _ in }, done: @escaping (Change) -> Void = { _ in }) {
        ActionRun.run(action, on: core.lumenna, asking: self, form: form) { [weak self] change, _ in
            self?.core.changed()
            Announcer.say(change.announcement, notices: change.notices)
            done(change)
        }
    }

    /// A row's actions offered when it is tapped, with any of the row's own after them.
    func offer(_ title: String, _ actions: [Action], extra: [(String, () -> Void)] = [], form: @escaping (Action) -> Void = { _ in }) {
        show(ChoicePrompt(title: title, choices: actions.map { action in
            (action.title, { [weak self] in self?.run(action, form: form) })
        } + extra))
    }
}

/// One of what the core offers for an action, each a button. A tree's depth is said where it
/// changes, as the watch's lists say it, never by indentation.
private struct PickPrompt: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let choices: [Choice]
    let chosen: (Choice) -> Void

    var body: some View {
        List {
            ForEach(Array(choices.enumerated()), id: \.offset) { index, choice in
                let previous = index > 0 ? choices[index - 1].depth : 0
                let level = choice.depth != previous ? "level \(choice.depth + 1)" : nil
                Button {
                    dismiss()
                    chosen(choice)
                } label: {
                    Text(choice.shownTitle)
                }
                .accessibilityValue([choice.shownDetail, level].compactMap { $0 }.joined(separator: ", "))
            }
        }
        .navigationTitle(title)
    }
}
