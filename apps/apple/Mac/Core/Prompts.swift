import AppKit

/// The questions the app asks, as sheets on a window. Each is the system's own alert, which
/// VoiceOver reads as a dialog with its message, field and buttons in order.
extension NSWindow {
    /// Says that something could not be done, with the core's own sentence.
    func showFailure(_ message: String, title: String = "Could not do that") {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.beginSheetModal(for: self)
    }

    /// Asks for one line of text; `done` hears it only if confirmed with something in it.
    func askForText(
        _ title: String,
        message: String = "",
        initial: String = "",
        placeholder: String = "",
        action: String = "Save",
        done: @escaping (String) -> Void
    ) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        let field = NSTextField(string: initial)
        field.placeholderString = placeholder
        field.setAccessibilityLabel(title)
        field.frame = NSRect(x: 0, y: 0, width: 320, height: 24)
        alert.accessoryView = field
        alert.addButton(withTitle: action)
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: self) { response in
            let text = field.stringValue.trimmingCharacters(in: .whitespaces)
            if response == .alertFirstButtonReturn, !text.isEmpty {
                done(text)
            }
        }
    }

    /// Asks before something that cannot be taken back. Cancel is the default, so a stray
    /// Return does nothing.
    func confirm(_ title: String, message: String, action: String, done: @escaping () -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        let destructive = alert.addButton(withTitle: action)
        destructive.hasDestructiveAction = true
        let cancel = alert.addButton(withTitle: "Cancel")
        destructive.keyEquivalent = ""
        cancel.keyEquivalent = "\r"
        alert.beginSheetModal(for: self) { response in
            if response == .alertFirstButtonReturn { done() }
        }
    }

    /// Offers a few choices as buttons; more than three go in a list (`PickerSheet`).
    func choose(_ title: String, message: String = "", actions: [(String, () -> Void)]) {
        guard actions.count <= 3 else {
            PickerSheet.present(
                on: self, title: title,
                items: actions.enumerated().map { PickerItem(key: String($0.offset), title: $0.element.0) }
            ) { item in actions[Int(item.key)!].1() }
            return
        }
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        for (name, _) in actions { alert.addButton(withTitle: name) }
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: self) { response in
            let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
            if index >= 0, index < actions.count { actions[index].1() }
        }
    }

    /// Asks how long a sitting is meant to take: minutes, or `nil` for none — what
    /// `without` names. Cancel calls nothing.
    func askForLength(_ title: String, current: UInt32? = nil, without: String, done: @escaping (UInt32?) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = "In minutes. It is the plan; what you log is kept apart."
        let field = NSTextField(string: current.map(String.init) ?? "")
        field.placeholderString = "45"
        field.setAccessibilityLabel("Minutes")
        field.frame = NSRect(x: 0, y: 0, width: 120, height: 24)
        alert.accessoryView = field
        alert.addButton(withTitle: "Set")
        alert.addButton(withTitle: without)
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: self) { [weak self] response in
            switch response {
            case .alertFirstButtonReturn:
                let text = field.stringValue.trimmingCharacters(in: .whitespaces)
                if text.isEmpty {
                    done(nil)
                } else if let minutes = UInt32(text), minutes > 0 {
                    done(minutes)
                } else {
                    DispatchQueue.main.async { self?.showFailure("That is not a number of minutes.") }
                }
            case .alertSecondButtonReturn:
                done(nil)
            default:
                break
            }
        }
    }
}

extension NSViewController {
    /// Runs an operation and says what it did, or why it could not.
    @discardableResult
    func run(_ operation: () throws -> Change) -> Change? {
        do {
            let change = try operation()
            Announcer.say(change.announcement, notices: change.notices)
            return change
        } catch {
            view.window?.showFailure(error.sentence)
            return nil
        }
    }
}
