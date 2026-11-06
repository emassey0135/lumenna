import LumennaApp
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
    @objc optional func goToHistory()
    @objc optional func toggleDone()
    @objc optional func putInBlock()
    @objc optional func moveToProject()
    @objc optional func makeSubtask()
    @objc optional func waitFor()
    @objc optional func moveToTrash()
    @objc optional func saveChanges()
    @objc optional func previousDay()
    @objc optional func nextDay()
    @objc optional func goToNow()
    @objc optional func goToDay()
    /// The day's own actions (`Plan.actions`), each the shown day's action of its kind.
    @objc optional func planTheDay()
    @objc optional func replanFromNow()
    @objc optional func whatNow()
    @objc optional func endTheDay()
    @objc optional func clearTheDay()
    /// A reminder's notification, chosen: its task, or its block on `day`.
    @objc optional func openReminded(kind: String, target: String, day: String)
    /// The overrun warning's Replan: today, with Replan from Now open over it.
    @objc optional func openReplan()
}

/// Every keyboard command, once, with the Mac app's keys — an iPad or iPhone keyboard is a
/// Mac keyboard — named as the core's menus name them (`Commands`), "…" after what asks first. On iPad they are the menu bar
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
            command(Commands.asking("new-task"), #selector(CommandActions.newTask), "n"),
            command(Commands.asking("new-block"), #selector(CommandActions.newBlock), "n", [.command, .shift]),
            item(Commands.asking("new-project"), #selector(CommandActions.newProject)),
            item(Commands.asking("new-label"), #selector(CommandActions.newLabel)),
            item(Commands.asking("new-filter"), #selector(CommandActions.newFilter)),
        ]
    }

    static var syncItems: [UIMenuElement] {
        [command(Commands.asking("sync-now"), #selector(CommandActions.syncNow), "r", [.command, .shift])]
    }

    static var settingsItems: [UIMenuElement] {
        [command(Commands.asking("settings"), #selector(CommandActions.showSettings), ",")]
    }

    static var undoItems: [UIMenuElement] {
        [
            command(Commands.asking("undo"), #selector(CommandActions.undoChange), "z"),
            command(Commands.asking("redo"), #selector(CommandActions.redoChange), "z", [.command, .shift]),
        ]
    }

    static var filterItems: [UIMenuElement] {
        [command(Commands.asking("filter"), #selector(CommandActions.filterTasks), "f")]
    }

    static var viewItems: [UIMenuElement] {
        [
            command(Commands.asking("go-today"), #selector(CommandActions.goToToday), "1"),
            command(Commands.asking("go-tasks"), #selector(CommandActions.goToTasks), "2"),
            command(Commands.asking("go-blocks"), #selector(CommandActions.goToBlocks), "3"),
            command(Commands.asking("go-trash"), #selector(CommandActions.goToTrash), "4"),
            command(Commands.asking("go-history"), #selector(CommandActions.goToHistory), "5"),
        ]
    }

    static var taskItems: [UIMenuElement] {
        [
            command(Commands.asking("mark-done"), #selector(CommandActions.toggleDone), "k"),
            command(Commands.asking("put-in-block"), #selector(CommandActions.putInBlock), "b"),
            command(Commands.asking("move-to-project"), #selector(CommandActions.moveToProject), "m", [.command, .shift]),
            item(Commands.asking("make-subtask"), #selector(CommandActions.makeSubtask)),
            item(Commands.asking("wait-for"), #selector(CommandActions.waitFor)),
            command(Commands.asking("save-task"), #selector(CommandActions.saveChanges), "s"),
            command(Commands.asking("trash-task"), #selector(CommandActions.moveToTrash), "\u{8}"),
        ]
    }

    static var dayItems: [UIMenuElement] {
        [
            command(Commands.asking("previous-day"), #selector(CommandActions.previousDay), "["),
            command(Commands.asking("next-day"), #selector(CommandActions.nextDay), "]"),
            command(Commands.asking("go-to-now"), #selector(CommandActions.goToNow), "t"),
            command(Commands.asking("go-to-day"), #selector(CommandActions.goToDay), "j"),
        ]
    }

    /// The day's own actions, the Mac's Day menu's, with its keys (`DayKeys`). Each runs the
    /// day's action of that kind, or says why the day has none (`notOffered`): the menu bar
    /// is built once, not for each day, so it cannot leave out what a day lacks.
    static var dayActionItems: [UIMenuElement] {
        [
            command(Commands.asking("plan-day"), #selector(CommandActions.planTheDay), DayKeys.planDay, [.command, .alternate]),
            command(Commands.asking("replan"), #selector(CommandActions.replanFromNow), DayKeys.replan, [.command, .alternate]),
            item(Commands.asking("what-now"), #selector(CommandActions.whatNow)),
            item(Commands.asking("end-day"), #selector(CommandActions.endTheDay)),
            item(Commands.asking("clear-day"), #selector(CommandActions.clearTheDay)),
        ]
    }

    /// The iPhone's: every command with a key.
    static var all: [UIKeyCommand] {
        [newItems, syncItems, settingsItems, undoItems, filterItems, viewItems, taskItems, dayItems, dayActionItems]
            .flatMap { $0 }
            .compactMap { $0 as? UIKeyCommand }
    }

    private static func inline(_ name: String, _ children: [UIMenuElement]) -> UIMenu {
        UIMenu(title: "", identifier: UIMenu.Identifier("dev.emassey0135.lumenna.\(name)"), options: .displayInline, children: children)
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
        let task = UIMenu(title: Commands.menu("task"), identifier: UIMenu.Identifier("dev.emassey0135.lumenna.task"), children: taskItems)
        let day = UIMenu(
            title: Commands.menu("day"), identifier: UIMenu.Identifier("dev.emassey0135.lumenna.day"),
            children: dayItems + [inline("day-actions", dayActionItems)]
        )
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
