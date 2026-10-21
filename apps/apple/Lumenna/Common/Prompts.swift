import LumennaApp
import UIKit

extension UIViewController {
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
}

/// How the iPhone and iPad ask the app's questions (`AppQuestions`): the system's alerts and
/// action sheets, and a sheet listing the choices, each saying when it was cancelled.
final class PhoneAsker {
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

extension PhoneAsker: QuestionAsking {
    func tell(title: String, _ sentence: String) {
        front?.showFailure(sentence, title: title)
    }

    func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void) {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in answered(false) })
        alert.addAction(UIAlertAction(title: yes, style: destructive ? .destructive : .default) { _ in answered(true) })
        present(alert) { answered(false) }
    }

    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, syntax: LumennaApp.Syntax?, answered: @escaping (String?) -> Void) {
        let message = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: "\n\n")
        // The filter language completes, which an alert's field cannot: a sheet of its own.
        if let syntax, let front {
            let question = CompletingQuestionViewController(
                core: core, title: title, label: label, initial: initial, message: message, yes: yes, syntax: syntax, answered: answered
            )
            front.present(UINavigationController(rootViewController: question), animated: true)
            return
        }
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

    func pick(title: String, offered: [Offered], yes: String, answered: @escaping (Picked) -> Void) {
        guard let front else { return answered(.cancelled) }
        let items = offered.map { AppItem(key: $0.id, title: $0.title, detail: $0.detail, depth: $0.depth, reason: $0.reason, swatch: $0.swatch) }
        let picker = ChoicePicker(core: core, title: title, items: items, chosen: { id in
            answered(.chose(id))
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
