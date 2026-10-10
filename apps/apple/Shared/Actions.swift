import Foundation

/// What can be done to a row is the core's: every record listed carries its `actions`, in
/// the order offered, under their spoken names, each with the question it asks. The Apple
/// apps offer them as swipe actions, menus and buttons; running one is `ActionRun`, here
/// once for the iPhone, the iPad, the Mac and the watch. Only the asking is each app's.

/// How an app asks an action's questions, its own way.
protocol ActionAsking: AnyObject {
    /// Whether to go ahead. Cancel is the app's, first, and the default.
    func askConfirm(title: String, message: String, yes: String, destructive: Bool, then: @escaping () -> Void)
    /// A line of text. `problem` is the core's sentence when it refused what was typed, which
    /// comes back as `initial`, so a refused answer stays in its dialog. Whatever is typed is
    /// handed on, empty included: the core refuses what it must.
    func askText(title: String, label: String, initial: String, hint: String, problem: String?, then: @escaping (String) -> Void)
    /// One of `choices`, in their order.
    func askPick(title: String, choices: [Choice], then: @escaping (Choice) -> Void)
    /// One of a few answers, each its own button.
    func askChoose(title: String, message: String, answers: [Choice], then: @escaping (Choice) -> Void)
    /// How long a sitting is meant to take, once one is picked: `hint` is the core's question.
    /// Empty is none.
    func askLength(hint: String, then: @escaping (String) -> Void)
    /// Says why a pick offers nothing, in the core's words: `title` is the pick's.
    func tell(title: String, _ sentence: String)
    /// Says why something could not be done, in the core's words.
    func fail(_ sentence: String)
}

extension ActionAsking {
    /// A length asked as a line of text, where the app has no better way.
    func askLength(hint: String, then: @escaping (String) -> Void) {
        askText(title: "Planned Length", label: "Planned length", initial: "", hint: hint, problem: nil, then: then)
    }
}

extension Action {
    /// Whether it asks something before it runs: a menu item for it ends in "…" on the Mac.
    var asks: Bool { question != .immediate }

    /// Whether it opens one of the app's own forms rather than running in the core.
    var isForm: Bool { question == .form }
}

extension Array where Element == Action {
    /// The first of these kinds: what a key or a menu command does to the row in hand.
    func first(_ kinds: ActionKind...) -> Action? {
        first { kinds.contains($0.kind) }
    }
}

/// Runs one action: asks its question, hands the answer to the core, and gives what changed
/// to `done` with the answer, for the app to keep focus where it belongs and say it.
enum ActionRun {
    static func run(
        _ action: Action,
        on lumenna: Lumenna,
        asking asker: ActionAsking,
        form: (Action) -> Void,
        done: @escaping (Change, Answer) -> Void
    ) {
        let answer = { (answer: Answer) in
            do {
                done(try lumenna.act(action: action, answer: answer), answer)
            } catch {
                asker.fail(error.sentence)
            }
        }
        switch action.question {
        case .form:
            form(action)
        case .immediate:
            answer(.yes)
        case let .confirm(title, message, yes):
            asker.askConfirm(title: title, message: message, yes: yes, destructive: action.destructive) { answer(.yes) }
        case let .text(title, label, initial, hint, _):
            askText(action, on: lumenna, asking: asker, title: title, label: label, typed: initial, hint: hint, problem: nil, done: done)
        case let .pick(title, length):
            let offered: Choices
            do {
                offered = try lumenna.choices(action: action)
            } catch {
                asker.fail(error.sentence)
                return
            }
            guard !offered.choices.isEmpty else {
                asker.tell(title: title, offered.announcement)
                return
            }
            asker.askPick(title: title, choices: offered.choices) { choice in
                guard let length else {
                    answer(.picked(id: choice.id, length: nil))
                    return
                }
                asker.askLength(hint: length) { typed in answer(.picked(id: choice.id, length: typed)) }
            }
        case let .choose(title, message, answers):
            asker.askChoose(title: title, message: message, answers: answers) { choice in
                answer(.picked(id: choice.id, length: nil))
            }
        }
    }

    /// Asks for the line, and again with what was typed and why when the core refuses it.
    private static func askText(
        _ action: Action, on lumenna: Lumenna, asking asker: ActionAsking,
        title: String, label: String, typed: String, hint: String, problem: String?,
        done: @escaping (Change, Answer) -> Void
    ) {
        asker.askText(title: title, label: label, initial: typed, hint: hint, problem: problem) { text in
            let answer = Answer.text(text: text)
            do {
                done(try lumenna.act(action: action, answer: answer), answer)
            } catch {
                askText(action, on: lumenna, asking: asker, title: title, label: label, typed: text, hint: hint, problem: error.sentence, done: done)
            }
        }
    }

    /// The name a change leaves its record under, when it names one: what a rename typed,
    /// what a new place is called, the label merged into. The app finds the row by it.
    static func name(after action: Action, answer: Answer) -> String? {
        switch (action.kind, answer) {
        case let (.rename, .text(text)), let (.newInside, .text(text)), let (.new, .text(text)):
            let name = text.trimmingCharacters(in: .whitespaces)
            return name.isEmpty ? nil : name
        case let (.mergeInto, .picked(id, _)):
            return id
        default:
            return nil
        }
    }
}

extension Choice {
    /// How it reads in a chooser: a block with its day and start, as the app says times.
    var shownTitle: String {
        guard let date, let start else { return title }
        return "\(Clock.spokenDay(date)), \(Clock.time(start)), \(title)"
    }

    /// What else tells it apart, beneath the title: a task's project, a block's hours.
    var shownDetail: String? {
        if let start, let end { return "\(Clock.time(start)) to \(Clock.time(end))" }
        return detail
    }
}
