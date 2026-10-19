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
        // Named by what it holds where there is a placeholder to say it.
        field.setAccessibilityLabel(placeholder.isEmpty ? title : placeholder)
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

/// How the Mac asks an action's questions (`ActionRun`): sheets on the window. Each follows
/// once the sheet before it has gone, since a sheet cannot open while the last is closing.
extension NSWindow: ActionAsking {
    func askConfirm(title: String, message: String, yes: String, destructive: Bool, then: @escaping () -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        let go = alert.addButton(withTitle: yes)
        go.hasDestructiveAction = destructive
        let cancel = alert.addButton(withTitle: "Cancel")
        // Cancel is the default, so a stray Return does nothing.
        go.keyEquivalent = ""
        cancel.keyEquivalent = "\r"
        alert.beginSheetModal(for: self) { response in
            if response == .alertFirstButtonReturn { DispatchQueue.main.async(execute: then) }
        }
    }

    func askText(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, then: @escaping (String) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: "\n\n")
        let field = NSTextField(string: initial)
        field.placeholderString = label
        field.setAccessibilityLabel(label)
        field.frame = NSRect(x: 0, y: 0, width: 320, height: 24)
        alert.accessoryView = field
        alert.addButton(withTitle: yes)
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: self) { response in
            let text = field.stringValue
            if response == .alertFirstButtonReturn { DispatchQueue.main.async { then(text) } }
        }
    }

    func askPick(title: String, choices: [Choice], yes: String, then: @escaping (Choice) -> Void) {
        let items = choices.map { PickerItem(key: $0.id, title: $0.shownTitle, detail: $0.shownDetail, depth: Int($0.depth), reason: $0.reason) }
        PickerSheet.present(on: self, title: title, items: items, yes: yes) { item in
            if let choice = choices.first(where: { $0.id == item.key }) { then(choice) }
        }
    }

    func askChoose(title: String, message: String, answers: [Choice], then: @escaping (Choice) -> Void) {
        choose(title, message: message, actions: answers.map { answer in
            (answer.title, { DispatchQueue.main.async { then(answer) } })
        })
    }

    func tell(title: String, _ sentence: String) {
        showFailure(sentence, title: title)
    }

    func fail(_ sentence: String) {
        showFailure(sentence)
    }

    /// A task's or block's reminders, as a sheet on this window (`Shared/Forms/ReminderList.swift`).
    func openReminders(_ action: Action) {
        openReminders(of: action.target)
    }

    /// The reminders of the task or block `target`, as a sheet on this window.
    func openReminders(of target: String) {
        guard let core = AppDelegate.shared?.core else { return }
        let model = RemindersModel(core: core, target: target)
        let sheet = NSWindow(contentViewController: NSViewController())
        let host = HostedForm("Reminders", rootView: RemindersForm(model: model) { [weak self, weak sheet] in
            if let sheet { self?.endSheet(sheet) }
        })
        sheet.contentViewController = host
        sheet.title = "Reminders"
        beginSheet(sheet)
    }

    /// Runs one of the core's actions, asking its question here. A form opens through
    /// `form`; what changed goes to `done`.
    func run(_ action: Action, core: Core, form: (Action) -> Void = { _ in }, done: @escaping (Change, Answer) -> Void) {
        ActionRun.run(action, on: core.lumenna, asking: self, form: form, done: done)
    }
}

extension NSMenu {
    /// A menu of the core's actions, in its order: one that asks something ends in "…", as
    /// the Mac's menus say, and one that removes something comes after a separator.
    func add(_ actions: [Action], run: @escaping (Action) -> Void) {
        for action in actions {
            if action.destructive, !items.isEmpty, items.last?.isSeparatorItem == false { addItem(.separator()) }
            addItem(ClosureMenuItem(title: action.asks ? action.title + "…" : action.title) { run(action) })
        }
    }
}

/// The app's questions (`AppQuestions`), asked as the surface's are: sheets on the window, each
/// saying when it was cancelled.
extension NSWindow: QuestionAsking {
    func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        let go = alert.addButton(withTitle: yes)
        go.hasDestructiveAction = destructive
        let cancel = alert.addButton(withTitle: "Cancel")
        // Cancel is the default, so a stray Return does nothing.
        go.keyEquivalent = ""
        cancel.keyEquivalent = "\r"
        alert.beginSheetModal(for: self) { response in
            let yes = response == .alertFirstButtonReturn
            DispatchQueue.main.async { answered(yes) }
        }
    }

    func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, answered: @escaping (String?) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = [problem, hint.isEmpty ? nil : hint].compactMap { $0 }.joined(separator: "\n\n")
        let field = NSTextField(string: initial)
        field.placeholderString = label
        field.setAccessibilityLabel(label)
        field.frame = NSRect(x: 0, y: 0, width: 320, height: 24)
        alert.accessoryView = field
        alert.addButton(withTitle: yes)
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: self) { response in
            let text = response == .alertFirstButtonReturn ? field.stringValue : nil
            DispatchQueue.main.async { answered(text) }
        }
    }

    func pick(title: String, offered: [Offered], anotherDay: String?, yes: String, answered: @escaping (Picked) -> Void) {
        var items = offered.map { PickerItem(key: $0.id, title: $0.title, detail: $0.detail, depth: Int($0.depth), reason: $0.reason) }
        // Another day's blocks, after the week's; asking, so it ends in "…" as the Mac's menus say.
        if let anotherDay { items.append(PickerItem(key: Self.anotherDay, title: anotherDay + "…")) }
        PickerSheet.present(on: self, title: title, items: items, yes: yes, chosen: { item in
            answered(item.key == Self.anotherDay ? .anotherDay : .chose(item.key))
        }, cancelled: { answered(.cancelled) })
    }

    /// The key of the picker's row that asks for another day's blocks.
    private static let anotherDay = "\u{0}another-day"

    func choose(title: String, message: String, answers: [(id: String, title: String)], destructive: Bool, answered: @escaping (String?) -> Void) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        for answer in answers {
            alert.addButton(withTitle: answer.title).hasDestructiveAction = destructive
        }
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: self) { response in
            let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
            let id = index >= 0 && index < answers.count ? answers[index].id : nil
            DispatchQueue.main.async { answered(id) }
        }
    }
}
