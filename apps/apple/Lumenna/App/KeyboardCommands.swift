import UIKit

/// What a keyboard command asks for. Each travels up the responder chain from what has
/// focus, so the screen in front answers before `RootTabs`, which answers the rest: on the
/// day, New Block adds to the day shown; anywhere else, to today.
@objc protocol CommandActions {
    @objc optional func newTask()
    @objc optional func newBlock()
    @objc optional func newProject()
    @objc optional func newLabel()
    @objc optional func newFilter()
    @objc optional func syncNow()
    @objc optional func showSettings()
    @objc optional func undoChange()
    @objc optional func redoChange()
    @objc optional func filterTasks()
    @objc optional func goToToday()
    @objc optional func goToTasks()
    @objc optional func goToBlocks()
    @objc optional func goToTrash()
    @objc optional func toggleDone()
    @objc optional func moveToTrash()
    @objc optional func saveChanges()
    @objc optional func previousDay()
    @objc optional func nextDay()
    @objc optional func goToNow()
    @objc optional func goToDay()
}

/// Every keyboard command, once, with the Mac app's keys — an iPad or iPhone keyboard is a
/// Mac keyboard — named as the Mac's menus name them. On iPad they are the menu bar
/// (`build(_:)`); the iPhone has none, so there they are the root's key commands (`all`).
enum KeyboardCommands {
    private static func command(_ title: String, _ action: Selector, _ input: String, _ modifiers: UIKeyModifierFlags = .command) -> UIKeyCommand {
        UIKeyCommand(title: title, action: action, input: input, modifierFlags: modifiers)
    }

    private static func item(_ title: String, _ action: Selector) -> UICommand {
        UICommand(title: title, action: action)
    }

    static var newItems: [UIMenuElement] {
        [
            command("New Task", #selector(CommandActions.newTask), "n"),
            command("New Block", #selector(CommandActions.newBlock), "n", [.command, .shift]),
            item("New Project", #selector(CommandActions.newProject)),
            item("New Label", #selector(CommandActions.newLabel)),
            item("New Saved Filter", #selector(CommandActions.newFilter)),
        ]
    }

    static var syncItems: [UIMenuElement] {
        [command("Sync Now", #selector(CommandActions.syncNow), "r", [.command, .shift])]
    }

    static var settingsItems: [UIMenuElement] {
        [command("Settings", #selector(CommandActions.showSettings), ",")]
    }

    static var undoItems: [UIMenuElement] {
        [
            command("Undo", #selector(CommandActions.undoChange), "z"),
            command("Redo", #selector(CommandActions.redoChange), "z", [.command, .shift]),
        ]
    }

    static var filterItems: [UIMenuElement] {
        [command("Filter Tasks", #selector(CommandActions.filterTasks), "f")]
    }

    static var viewItems: [UIMenuElement] {
        [
            command("Today", #selector(CommandActions.goToToday), "1"),
            command("Tasks", #selector(CommandActions.goToTasks), "2"),
            command("Blocks", #selector(CommandActions.goToBlocks), "3"),
            command("Trash", #selector(CommandActions.goToTrash), "4"),
        ]
    }

    static var taskItems: [UIMenuElement] {
        [
            command("Mark Done", #selector(CommandActions.toggleDone), "k"),
            command("Save Changes", #selector(CommandActions.saveChanges), "s"),
            command("Move to Trash", #selector(CommandActions.moveToTrash), "\u{8}"),
        ]
    }

    static var dayItems: [UIMenuElement] {
        [
            command("Previous Day", #selector(CommandActions.previousDay), "["),
            command("Next Day", #selector(CommandActions.nextDay), "]"),
            command("Go to Now", #selector(CommandActions.goToNow), "t"),
            command("Go to Day", #selector(CommandActions.goToDay), "j"),
        ]
    }

    /// The iPhone's: every command with a key.
    static var all: [UIKeyCommand] {
        [newItems, syncItems, settingsItems, undoItems, filterItems, viewItems, taskItems, dayItems]
            .flatMap { $0 }
            .compactMap { $0 as? UIKeyCommand }
    }

    private static func inline(_ name: String, _ children: [UIMenuElement]) -> UIMenu {
        UIMenu(title: "", identifier: UIMenu.Identifier("io.github.emassey0135.lumenna.\(name)"), options: .displayInline, children: children)
    }

    /// The iPad's menu bar: the system's menus, with what does not apply taken out — Find and
    /// Format would take ⌘F and ⌘B — and Undo and Redo replaced by the store's, which hand
    /// an edit in a text field to the field's own undo (`RootTabs.undoChange`).
    static func build(_ builder: UIMenuBuilder) {
        // The iPhone's are `RootTabs.keyCommands`: in both, a key would have two commands.
        guard builder.system == .main, UIDevice.current.userInterfaceIdiom == .pad else { return }
        for gone: UIMenu.Identifier in [.format, .find, .newScene, .openRecent] where builder.menu(for: gone) != nil {
            builder.remove(menu: gone)
        }
        if builder.menu(for: .undoRedo) != nil {
            builder.replace(menu: .undoRedo, with: inline("undo", undoItems))
        } else {
            builder.insertChild(inline("undo", undoItems), atStartOfMenu: .edit)
        }
        builder.insertChild(inline("filter", filterItems), atEndOfMenu: .edit)
        builder.insertChild(inline("new", newItems), atStartOfMenu: .file)
        builder.insertChild(inline("sync", syncItems), atEndOfMenu: .file)
        if builder.menu(for: .preferences) != nil {
            builder.replace(menu: .preferences, with: inline("settings", settingsItems))
        } else {
            builder.insertChild(inline("settings", settingsItems), atStartOfMenu: .application)
        }
        builder.insertChild(inline("go", viewItems), atStartOfMenu: .view)
        let task = UIMenu(title: "Task", identifier: UIMenu.Identifier("io.github.emassey0135.lumenna.task"), children: taskItems)
        let day = UIMenu(title: "Day", identifier: UIMenu.Identifier("io.github.emassey0135.lumenna.day"), children: dayItems)
        builder.insertSibling(task, afterMenu: .view)
        builder.insertSibling(day, afterMenu: task.identifier)
    }
}

extension UIResponder {
    private static weak var found: UIResponder?

    /// Whatever has focus now: where the responder chain starts.
    static var first: UIResponder? {
        found = nil
        UIApplication.shared.sendAction(#selector(noteFirst), to: nil, from: nil, for: nil)
        return found
    }

    @objc private func noteFirst() {
        UIResponder.found = self
    }

    /// Text being edited, with something of its own to undo: ⌘Z there undoes the typing.
    /// Only a field still on screen: an alert's field stays first responder after the alert
    /// has gone, holding the name typed into it.
    static var editingWithUndo: UndoManager? {
        guard let first = first, first is UITextInput, (first as? UIView)?.window != nil,
              let manager = first.undoManager else { return nil }
        return manager
    }
}
