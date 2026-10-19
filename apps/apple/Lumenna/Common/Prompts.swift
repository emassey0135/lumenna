import UIKit

extension UIViewController {
    /// Asks for one line of text. `done` is called only if the person confirms with something
    /// in the field.
    func askForText(
        _ title: String,
        message: String? = nil,
        placeholder: String = "",
        initial: String = "",
        action: String = "Save",
        done: @escaping (String) -> Void
    ) {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addTextField { field in
            field.placeholder = placeholder
            field.text = initial
            // Named by what it holds where there is a placeholder to say it.
            field.accessibilityLabel = placeholder.isEmpty ? title : placeholder
            field.autocapitalizationType = .sentences
            field.clearButtonMode = .whileEditing
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        let answer = UIAlertAction(title: action, style: .default) { [weak alert] _ in
            let text = alert?.textFields?.first?.text?.trimmingCharacters(in: .whitespaces) ?? ""
            if !text.isEmpty {
                done(text)
            }
        }
        alert.addAction(answer)
        // Return answers, as in the system's own text alerts.
        alert.preferredAction = answer
        present(alert, animated: true)
    }

    /// Asks before something that cannot be taken back. The destructive choice is never the
    /// default, so a stray Return does nothing.
    func confirm(
        _ title: String,
        message: String,
        action: String,
        done: @escaping () -> Void
    ) {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: action, style: .destructive) { _ in done() })
        present(alert, animated: true)
    }

    /// Offers a choice among a few actions, as a sheet anchored to `source` on iPad.
    func choose(
        _ title: String,
        message: String? = nil,
        from source: UIView? = nil,
        actions: [(String, () -> Void)]
    ) {
        let sheet = UIAlertController(title: title, message: message, preferredStyle: .actionSheet)
        for (name, run) in actions {
            sheet.addAction(UIAlertAction(title: name, style: .default) { _ in run() })
        }
        sheet.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        sheet.popoverPresentationController?.sourceView = source ?? view
        present(sheet, animated: true)
    }

    /// Runs an operation and says what it did, or why it could not.
    @discardableResult
    func run(_ operation: () throws -> Change) -> Change? {
        do {
            let change = try operation()
            Announcer.say(change.announcement, notices: change.notices)
            return change
        } catch {
            showFailure(error.sentence)
            return nil
        }
    }
}

/// How the iPhone and iPad ask an action's questions (`ActionRun`): the system's alerts and
/// action sheets, and a sheet listing the choices.
final class PhoneAsker: ActionAsking {
    private let core: Core
    private weak var presenter: UIViewController?

    init(core: Core, from presenter: UIViewController) {
        self.core = core
        self.presenter = presenter
    }

    /// What questions are asked over: whatever is in front, so an answer asked after a sheet
    /// closes is not asked of a screen already covered.
    private var front: UIViewController? {
        var shown = presenter
        while let next = shown?.presentedViewController, !next.isBeingDismissed { shown = next }
        return shown
    }

    func askConfirm(title: String, message: String, yes: String, destructive: Bool, then: @escaping () -> Void) {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: yes, style: destructive ? .destructive : .default) { _ in then() })
        front?.present(alert, animated: true)
    }

    func askText(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, then: @escaping (String) -> Void) {
        let message = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: "\n\n")
        let alert = UIAlertController(title: title, message: message.isEmpty ? nil : message, preferredStyle: .alert)
        alert.addTextField { field in
            field.text = initial
            field.placeholder = label
            field.accessibilityLabel = label
            field.autocapitalizationType = .sentences
            field.clearButtonMode = .whileEditing
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        let answer = UIAlertAction(title: yes, style: .default) { [weak alert] _ in
            then(alert?.textFields?.first?.text ?? "")
        }
        alert.addAction(answer)
        // Return answers, as in the system's own text alerts; what it does is never destructive.
        alert.preferredAction = answer
        front?.present(alert, animated: true)
    }

    func askPick(title: String, choices: [Choice], yes: String, then: @escaping (Choice) -> Void) {
        guard let front else { return }
        ChoicePicker.present(from: front, core: core, title: title, choices: choices, chosen: then)
    }

    func askChoose(title: String, message: String, answers: [Choice], then: @escaping (Choice) -> Void) {
        guard let front else { return }
        front.choose(title, message: message, actions: answers.map { answer in (answer.title, { then(answer) }) })
    }

    func askLength(hint: String, then: @escaping (String) -> Void) {
        askText(title: "Planned Length", label: "Planned length", initial: "", hint: hint, problem: nil, yes: "Save", then: then)
    }

    func tell(title: String, _ sentence: String) {
        front?.showFailure(sentence, title: title)
    }

    func fail(_ sentence: String) {
        front?.showFailure(sentence)
    }

    func openReminders(_ action: Action) {
        presenter?.openReminders(action, core: core)
    }
}

extension UIViewController {
    /// Runs one of the core's actions, asking its question over this screen. A form opens
    /// through `form`; what changed goes to `done`.
    func run(
        _ action: Action, core: Core, form: (Action) -> Void = { _ in },
        done: @escaping (Change, Answer) -> Void
    ) {
        ActionRun.run(action, on: core.lumenna, asking: PhoneAsker(core: core, from: self), form: form, done: done)
    }
}

extension UIBarButtonItem {
    /// Undo or Redo for a navigation bar: the system's arrows, named in words for VoiceOver.
    /// Words would take the bar's room from the title, which is then clipped at large text sizes.
    static func undo(_ action: @escaping () -> Void) -> UIBarButtonItem {
        arrow("arrow.uturn.backward", name: "Undo", action)
    }

    static func redo(_ action: @escaping () -> Void) -> UIBarButtonItem {
        arrow("arrow.uturn.forward", name: "Redo", action)
    }

    private static func arrow(_ symbol: String, name: String, _ action: @escaping () -> Void) -> UIBarButtonItem {
        let item = UIBarButtonItem(image: UIImage(systemName: symbol), primaryAction: UIAction { _ in action() })
        item.accessibilityLabel = name
        // Shown with the name when a button is pressed and held, for those who read the arrows
        // with Large Content Viewer.
        item.title = name
        return item
    }
}

/// The app's questions (`AppQuestions`), asked as the surface's are: the system's alerts and
/// action sheets, and the picker sheet, each saying when it was cancelled.
extension PhoneAsker: QuestionAsking {
    func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void) {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in answered(false) })
        alert.addAction(UIAlertAction(title: yes, style: destructive ? .destructive : .default) { _ in answered(true) })
        present(alert) { answered(false) }
    }

    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, answered: @escaping (String?) -> Void) {
        let message = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: "\n\n")
        let alert = UIAlertController(title: title, message: message.isEmpty ? nil : message, preferredStyle: .alert)
        alert.addTextField { field in
            field.text = initial
            field.placeholder = label
            field.accessibilityLabel = label
            field.autocapitalizationType = .sentences
            field.clearButtonMode = .whileEditing
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in answered(nil) })
        let answer = UIAlertAction(title: yes, style: .default) { [weak alert] _ in
            answered(alert?.textFields?.first?.text ?? "")
        }
        alert.addAction(answer)
        // Return answers, as in the system's own text alerts; what it does is never destructive.
        alert.preferredAction = answer
        present(alert) { answered(nil) }
    }

    func pick(title: String, offered: [Offered], anotherDay: String?, yes: String, answered: @escaping (Picked) -> Void) {
        guard let front else { return answered(.cancelled) }
        var items = offered.map { Item(key: $0.id, title: $0.title, detail: $0.detail, depth: $0.depth, reason: $0.reason) }
        // Another day's blocks, after the week's.
        if let anotherDay { items.append(Item(key: ChoicePicker.anotherDay, title: anotherDay)) }
        let picker = ChoicePicker(core: core, title: title, items: items, chosen: { id in
            answered(id == ChoicePicker.anotherDay ? .anotherDay : .chose(id))
        }, cancelled: { answered(.cancelled) })
        front.present(UINavigationController(rootViewController: picker), animated: true)
    }

    func choose(title: String, message: String, answers: [(id: String, title: String)], destructive: Bool, answered: @escaping (String?) -> Void) {
        let sheet = UIAlertController(title: title, message: message, preferredStyle: .actionSheet)
        for answer in answers {
            sheet.addAction(UIAlertAction(title: answer.title, style: destructive ? .destructive : .default) { _ in answered(answer.id) })
        }
        sheet.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in answered(nil) })
        sheet.popoverPresentationController?.sourceView = front?.view
        present(sheet) { answered(nil) }
    }

    /// Presents `alert` over whatever is in front, or says it was cancelled when nothing is.
    private func present(_ alert: UIAlertController, otherwise: () -> Void) {
        guard let front else { return otherwise() }
        front.present(alert, animated: true)
    }
}
