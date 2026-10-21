import LumennaApp
import SwiftUI

/// How the watch asks the app's questions (`AppQuestions`): each a sheet of its own, as the
/// phone's alerts and pickers are. The root holds one and shows its `asked`.
final class WatchAsker: ObservableObject {
    @Published var asked: Asked?
    private let core: WatchCore
    /// The sheet shown, which `dismissed` hears close.
    private var showing: Asked?
    /// What to show once the sheet in front has gone.
    private var next: Asked?

    init(core: WatchCore) {
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

    /// A row's actions, the app's, offered when it is tapped, with any of the row's own after
    /// them: the row's primary ones first, as the phone swipes them, then the rest in the app's
    /// order, and whatever removes something last, never the first thing under a finger. Each
    /// runs on the row `key`, held where `side` says.
    func offer(
        _ title: String, _ actions: [LumennaApp.Action], on key: String, side: LumennaApp.Side? = nil,
        extra: [(String, () -> Void)] = []
    ) {
        let kept = actions.filter { !$0.destructive }
        let ordered = kept.filter(\.primary) + kept.filter { !$0.primary } + actions.filter(\.destructive)
        show(ChoicePrompt(title: title, choices: ordered.map { action in
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

    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, syntax: LumennaApp.Syntax?, answered: @escaping (String?) -> Void) {
        let reply = Once(answered)
        let message = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: " ")
        ask(TextPrompt(title, message: message.isEmpty ? nil : message, initial: initial, placeholder: label, action: yes, allowsEmpty: true) { text in
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
