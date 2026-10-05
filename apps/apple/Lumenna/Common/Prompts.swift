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
            field.accessibilityLabel = title
            field.autocapitalizationType = .sentences
            field.clearButtonMode = .whileEditing
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: action, style: .default) { [weak alert] _ in
            let text = alert?.textFields?.first?.text?.trimmingCharacters(in: .whitespaces) ?? ""
            if !text.isEmpty {
                done(text)
            }
        })
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

extension UIViewController {
    /// Asks how long a sitting is meant to take. `done` gets the minutes, or `nil`
    /// for none — what `without` names: "Skip" when assigning, "No Planned Length" when
    /// changing one. Cancel calls nothing.
    func askForLength(
        _ title: String,
        current: UInt32? = nil,
        without: String,
        done: @escaping (UInt32?) -> Void
    ) {
        let alert = UIAlertController(
            title: title, message: "In minutes. It is the plan; what you log is kept apart.",
            preferredStyle: .alert
        )
        alert.addTextField { field in
            field.placeholder = "45"
            field.text = current.map(String.init) ?? ""
            field.keyboardType = .numberPad
            field.accessibilityLabel = "Minutes"
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: without, style: .default) { _ in done(nil) })
        alert.addAction(UIAlertAction(title: "Set", style: .default) { [weak self, weak alert] _ in
            let text = alert?.textFields?.first?.text?.trimmingCharacters(in: .whitespaces) ?? ""
            guard let minutes = UInt32(text), minutes > 0 else {
                if text.isEmpty {
                    done(nil)
                } else {
                    self?.showFailure("That is not a number of minutes.")
                }
                return
            }
            done(minutes)
        })
        present(alert, animated: true)
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
