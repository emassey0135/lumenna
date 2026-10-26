import Foundation
import LumennaApp

/// One of what a pick offers, as the app words it (the turn's `lines`): its line's two parts,
/// its depth in a tree of them, and why it is offered.
struct Offered {
    let id: String
    let title: String
    let detail: String?
    let depth: UInt32
    let reason: String?
    /// A colour's, drawn before its name.
    var swatch: LumennaApp.Swatch? = nil
}

/// What came of a pick.
enum Picked {
    case chose(String)
    case cancelled
}

/// How a platform asks the app's questions, its own way: alerts and a picker on iOS, sheets on
/// the Mac and the watch. Every question reports its answer or that it was cancelled, which the
/// app needs to close it.
protocol QuestionAsking: AnyObject {
    /// Whether to go ahead. Cancel is the platform's, first, and the default.
    func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void)
    /// A line of text, starting from `initial`; `problem` is the app's sentence when it refused
    /// the last answer. Nil is cancelled. With `day` (ISO), it asks for a day (Go to Day, Another
    /// Day): the platform's calendar beside the line, open on that day, a day chosen there
    /// answered as its ISO date when nothing is typed.
    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, syntax: LumennaApp.Syntax?, day: String?, answered: @escaping (String?) -> Void)
    /// One of `offered`: Put in a Block's ends in the app's Another Day, which asks the day next.
    func pick(title: String, offered: [Offered], yes: String, answered: @escaping (Picked) -> Void)
    /// One of a few answers, each its own button. Nil is cancelled.
    func choose(title: String, message: String, answers: [(id: String, title: String)], destructive: Bool, answered: @escaping (String?) -> Void)
    /// Says something the person should hear and see before going on: why a pick offers nothing.
    func tell(title: String, _ sentence: String)
}

/// The question the app has open (`ViewModel.asking`), asked through the platform's
/// `QuestionAsking` and answered back to the app: which question is open, its words, what a pick
/// offers and why an answer was refused are all the app's. A refused answer is asked again with
/// the app's reason and what was typed. One per window, at its root.
final class AppQuestions {
    private let app: AppModel
    private weak var asker: QuestionAsking?
    /// Opens the app's own form for an action that asks one (`Question.form`): a task's
    /// details, a reminder list.
    private let form: (LumennaApp.Action) -> Void
    /// The questions a screen answers itself, which this leaves alone: the reminder list's Add
    /// Reminder on the Mac and the watch, whose field is the list's own.
    var answeredElsewhere: (LumennaApp.Asking) -> Bool = { _ in false }
    /// Whether a question is being asked and not yet answered.
    private var asking = false
    /// What was typed into the last line asked, for asking it again when refused.
    private var typed: (question: String, text: String)?
    private var observer: NSObjectProtocol?

    init(app: AppModel, asker: QuestionAsking, form: @escaping (LumennaApp.Action) -> Void) {
        self.app = app
        self.asker = asker
        self.form = form
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: app, queue: .main) { [weak self] _ in
            self?.update()
        }
        update()
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Asks what the app has open, unless something is being asked already. As the app holds it
    /// now: a turn from before the last answer, still on its way, shows the question answered.
    func update() {
        guard !asking, let asker, app.view.asking != nil else { return }
        let (view, lines) = app.current
        guard let open = view.asking, !answeredElsewhere(open) else { return }
        let action = open.action
        let cancel = { [app] in app.send(.cancel) }
        let answer = { [app] (given: LumennaApp.Answer) in app.send(.answer(given)) }
        let done = { [weak self] in self?.asking = false }
        switch open.question {
        case .immediate:
            return
        case .form:
            if let action { form(action) }
            cancel()
        case let .confirm(title, message, yes):
            asking = true
            asker.confirm(title: title, message: message, yes: yes, destructive: action?.destructive == true) { yes in
                done()
                yes ? answer(.yes) : cancel()
            }
        case let .text(title, label, initial, hint, _, yes, _):
            asking = true
            let question = "\(action?.kind.rawValue ?? -1) \(action?.target ?? "") \(title)"
            // Asked again after a refusal: with what was typed, and why it was refused.
            let start = open.refusal != nil && typed?.question == question ? typed?.text ?? initial : initial
            asker.text(title: title, label: label, initial: start, hint: hint, problem: open.refusal, yes: yes, syntax: open.syntax, day: open.day) { [weak self] text in
                done()
                guard let text else {
                    self?.typed = nil
                    cancel()
                    return
                }
                self?.typed = (question, text)
                answer(.text(text: text))
            }
        case let .pick(title, _, yes):
            // What a pick offers comes with the question once the app has worked it out.
            guard let offered = open.choices else { return }
            guard !offered.choices.isEmpty else {
                asker.tell(title: title, offered.announcement)
                cancel()
                return
            }
            // An answer the app refused, said before the pick again.
            if let refusal = open.refusal { asker.tell(title: title, refusal) }
            asking = true
            let worded = lines.choices
            let rows = offered.choices.enumerated().map { index, choice in
                let line = index < worded.count ? worded[index] : nil
                return Offered(
                    id: choice.id, title: line?.title ?? choice.title, detail: line.flatMap { $0.detail.isEmpty ? nil : $0.detail },
                    depth: choice.depth, reason: choice.reason, swatch: choice.swatch
                )
            }
            asker.pick(title: title, offered: rows, yes: yes) { picked in
                done()
                switch picked {
                case let .chose(id): answer(.picked(id: id, length: nil))
                case .cancelled: cancel()
                }
            }
        case let .choose(title, message, answers):
            asking = true
            asker.choose(title: title, message: message, answers: answers.map { ($0.id, $0.title) }, destructive: action?.destructive == true) { id in
                done()
                guard let id else { return cancel() }
                answer(.picked(id: id, length: nil))
            }
        }
    }
}
