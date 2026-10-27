import AppKit
import LumennaApp

/// The questions the app asks, as sheets on a window. Each is the system's own alert, which
/// VoiceOver reads as a dialog with its message, field and buttons in order.
extension NSWindow {
    /// Says that something could not be done, with the core's own sentence: under `title`, the
    /// question's own, or as the whole alert, which the sentence says by itself.
    func showFailure(_ message: String, title: String? = nil) {
        let alert = NSAlert()
        alert.messageText = title ?? message
        alert.informativeText = title == nil ? "" : message
        alert.alertStyle = .warning
        // The core's Close: an alert left without a button says OK, which is never the answer.
        alert.addButton(withTitle: LumennaApp.close(sentence: false))
        alert.beginSheetModal(for: self)
    }

    /// Offers a few choices as buttons; more than three go in a list (`PickerSheet`).
    func choose(_ title: String, message: String = "", actions: [(String, () -> Void)]) {
        guard actions.count <= 3 else {
            PickerSheet.present(
                on: self, title: title,
                items: actions.enumerated().map { PickerItem(key: String($0.offset), title: $0.element.0) },
                // Each item is what it does, so the button only goes ahead with it.
                yes: "Go Ahead"
            ) { item in actions[Int(item.key)!].1() }
            return
        }
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        for (name, _) in actions { alert.addButton(withTitle: name) }
        alert.addButton(withTitle: LumennaApp.cancel(sentence: false))
        alert.beginSheetModal(for: self) { response in
            let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
            if index >= 0, index < actions.count { actions[index].1() }
        }
    }

}

extension NSMenu {
    /// A menu of the app's actions, in its order: one that asks something ends in "…", as
    /// the Mac's menus say, and one that removes something comes after a separator.
    func add<A: RowAction>(_ actions: [A], run: @escaping (A) -> Void) {
        for action in actions {
            if action.destructive, !items.isEmpty, items.last?.isSeparatorItem == false { addItem(.separator()) }
            addItem(ClosureMenuItem(title: action.asksFirst ? action.title + "…" : action.title) { run(action) })
        }
    }
}

/// The app's questions (`AppQuestions`): sheets on the window, or on the sheet in front of it,
/// each saying when it was cancelled.
extension NSWindow: QuestionAsking {
    func tell(title: String, _ sentence: String) {
        front.showFailure(sentence, title: title)
    }

    func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        let go = alert.addButton(withTitle: yes)
        go.hasDestructiveAction = destructive
        let cancel = alert.addButton(withTitle: LumennaApp.cancel(sentence: false))
        // Cancel is the default, so a stray Return does nothing.
        go.keyEquivalent = ""
        cancel.keyEquivalent = "\r"
        alert.beginSheetModal(for: front) { response in
            let yes = response == .alertFirstButtonReturn
            DispatchQueue.main.async { answered(yes) }
        }
    }

    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, syntax: LumennaApp.Syntax?, day: String?, answered: @escaping (String?) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: "\n\n")
        // A line in the filter language completes, on Down, as the filter and quick add do.
        let core = AppDelegate.shared?.core
        let field: NSTextField = syntax.flatMap { syntax in core.map { CompletingField(core: $0, syntax: syntax, name: label) } } ?? NSTextField()
        field.stringValue = initial
        field.placeholderString = label
        field.setAccessibilityLabel(label)
        if !hint.isEmpty { field.setAccessibilityHelp(hint) }
        field.frame = NSRect(x: 0, y: 0, width: 320, height: 24)
        // A day (Go to Day, Another Day): the line first, the system's calendar beside it, open
        // on the day the app gives; a day chosen there is the answer when nothing is typed.
        let calendar = day.map { day -> NSDatePicker in
            let picker = NSDatePicker()
            picker.datePickerStyle = .clockAndCalendar
            picker.datePickerElements = .yearMonthDay
            picker.dateValue = Clock.day(day) ?? .now
            picker.sizeToFit()
            picker.setAccessibilityLabel(label)
            return picker
        }
        if let calendar {
            let width = max(calendar.frame.width, 240)
            let stack = NSStackView(views: [field, calendar])
            stack.orientation = .vertical
            stack.alignment = .leading
            field.widthAnchor.constraint(equalToConstant: width).isActive = true
            stack.frame = NSRect(x: 0, y: 0, width: width, height: calendar.frame.height + 32)
            alert.accessoryView = stack
        } else {
            alert.accessoryView = field
        }
        alert.addButton(withTitle: yes)
        alert.addButton(withTitle: LumennaApp.cancel(sentence: false))
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: front) { response in
            var text = response == .alertFirstButtonReturn ? field.stringValue : nil
            if let typed = text, let calendar, typed.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                text = Clock.isoDay(calendar.dateValue)
            }
            DispatchQueue.main.async { answered(text) }
        }
    }

    func pick(title: String, offered: [Offered], yes: String, answered: @escaping (Picked) -> Void) {
        let items = offered.map { PickerItem(key: $0.id, title: $0.title, detail: $0.detail, depth: Int($0.depth), reason: $0.reason, swatch: $0.swatch) }
        PickerSheet.present(on: front, title: title, items: items, yes: yes, chosen: { item in
            answered(.chose(item.key))
        }, cancelled: { answered(.cancelled) })
    }

    func choose(title: String, message: String, answers: [(id: String, title: String)], destructive: Bool, answered: @escaping (String?) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        for answer in answers {
            alert.addButton(withTitle: answer.title).hasDestructiveAction = destructive
        }
        alert.addButton(withTitle: LumennaApp.cancel(sentence: false))
        alert.beginSheetModal(for: front) { response in
            let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
            let id = index >= 0 && index < answers.count ? answers[index].id : nil
            DispatchQueue.main.async { answered(id) }
        }
    }
}

/// The app's questions asked where the person is: over Settings while it is in front, whose own
/// actions (Erase Everything Deleted, a device's Unpair) ask there, else over the main window.
/// Asked over the main window, Settings' questions opened behind it.
final class FrontAsker: QuestionAsking {
    private weak var main: NSWindow?

    init(main: NSWindow) {
        self.main = main
    }

    private var window: NSWindow? {
        var key = NSApp.keyWindow
        while let parent = key?.sheetParent { key = parent }
        return key?.identifier?.rawValue == "settings" ? key : main
    }

    func tell(title: String, _ sentence: String) {
        window?.tell(title: title, sentence)
    }

    func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void) {
        guard let window else { return answered(false) }
        window.confirm(title: title, message: message, yes: yes, destructive: destructive, answered: answered)
    }

    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, syntax: LumennaApp.Syntax?, day: String?, answered: @escaping (String?) -> Void) {
        guard let window else { return answered(nil) }
        window.text(title: title, label: label, initial: initial, hint: hint, problem: problem, yes: yes, syntax: syntax, day: day, answered: answered)
    }

    func pick(title: String, offered: [Offered], yes: String, answered: @escaping (Picked) -> Void) {
        guard let window else { return answered(.cancelled) }
        window.pick(title: title, offered: offered, yes: yes, answered: answered)
    }

    func choose(title: String, message: String, answers: [(id: String, title: String)], destructive: Bool, answered: @escaping (String?) -> Void) {
        guard let window else { return answered(nil) }
        window.choose(title: title, message: message, answers: answers, destructive: destructive, answered: answered)
    }
}
