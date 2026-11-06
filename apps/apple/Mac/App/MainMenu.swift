import AppKit
import LumennaApp

/// The menu bar. Everything the app does is here, with its shortcut: the menu bar is
/// how a Mac user — and a VoiceOver user especially, with VO-M — finds what an app can do.
/// Each command is named as the core's menus name it (`Commands`); what is the Mac's alone
/// (Hide, Services, the Window menu) takes the core's words for it too (`Wording`).
enum MainMenu {
    static func build() -> NSMenu {
        let bar = NSMenu()
        bar.addItem(submenu(app()))
        bar.addItem(submenu(file()))
        bar.addItem(submenu(edit()))
        bar.addItem(submenu(view()))
        bar.addItem(submenu(task()))
        bar.addItem(submenu(day()))
        let window = window()
        bar.addItem(submenu(window))
        NSApp.windowsMenu = window
        let help = NSMenu(title: Commands.menu("help"))
        bar.addItem(submenu(help))
        NSApp.helpMenu = help
        return bar
    }

    private static func submenu(_ menu: NSMenu) -> NSMenuItem {
        let item = NSMenuItem(title: menu.title, action: nil, keyEquivalent: "")
        item.submenu = menu
        return item
    }

    private static func item(_ title: String, _ action: Selector, _ key: String = "", _ modifiers: NSEvent.ModifierFlags = .command) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.keyEquivalentModifierMask = key.isEmpty ? [] : modifiers
        return item
    }

    private static func app() -> NSMenu {
        let menu = NSMenu(title: Wording.metadata.name)
        menu.addItem(item(Commands.asking("about"), #selector(AppDelegate.showAbout(_:))))
        menu.addItem(.separator())
        menu.addItem(item(Commands.asking("settings"), #selector(AppDelegate.showSettings(_:)), ","))
        menu.addItem(.separator())
        let services = NSMenu(title: Wording.services)
        let servicesItem = NSMenuItem(title: Wording.services, action: nil, keyEquivalent: "")
        servicesItem.submenu = services
        NSApp.servicesMenu = services
        menu.addItem(servicesItem)
        menu.addItem(.separator())
        menu.addItem(item(Wording.hideApp, #selector(NSApplication.hide(_:)), "h"))
        menu.addItem(item(Wording.hideOthers, #selector(NSApplication.hideOtherApplications(_:)), "h", [.command, .option]))
        menu.addItem(item(Wording.showAll, #selector(NSApplication.unhideAllApplications(_:))))
        menu.addItem(.separator())
        menu.addItem(item(Wording.quitApp, #selector(NSApplication.terminate(_:)), "q"))
        return menu
    }

    private static func file() -> NSMenu {
        let menu = NSMenu(title: Commands.menu("file"))
        menu.addItem(item(Commands.asking("new-task"), #selector(AppDelegate.newTask(_:)), "n"))
        menu.addItem(item(Commands.asking("new-block"), #selector(AppDelegate.newBlock(_:)), "n", [.command, .shift]))
        menu.addItem(item(Commands.asking("new-project"), #selector(AppDelegate.newProject(_:))))
        menu.addItem(item(Commands.asking("new-label"), #selector(AppDelegate.newLabel(_:))))
        menu.addItem(item(Commands.asking("new-filter"), #selector(AppDelegate.newFilter(_:))))
        menu.addItem(.separator())
        menu.addItem(item(Commands.asking("sync-now"), #selector(AppDelegate.syncNowFromMenu(_:)), "r", [.command, .shift]))
        menu.addItem(item(Commands.asking("back-up"), #selector(AppDelegate.backUpNow(_:))))
        menu.addItem(item(Commands.asking("export-import"), #selector(AppDelegate.exportOrImport(_:))))
        menu.addItem(.separator())
        menu.addItem(item(Commands.asking("close-window"), #selector(NSWindow.performClose(_:)), "w"))
        return menu
    }

    private static func edit() -> NSMenu {
        let menu = NSMenu(title: Commands.menu("edit"))
        menu.addItem(item(Commands.asking("undo"), #selector(AppDelegate.undoChange(_:)), "z"))
        menu.addItem(item(Commands.asking("redo"), #selector(AppDelegate.redoChange(_:)), "z", [.command, .shift]))
        menu.addItem(.separator())
        menu.addItem(item(Wording.cut, #selector(NSText.cut(_:)), "x"))
        menu.addItem(item(Wording.copy, #selector(NSText.copy(_:)), "c"))
        menu.addItem(item(Wording.paste, #selector(NSText.paste(_:)), "v"))
        menu.addItem(item(Wording.selectAll, #selector(NSText.selectAll(_:)), "a"))
        menu.addItem(.separator())
        menu.addItem(item(Commands.asking("filter"), #selector(AppDelegate.focusFilter(_:)), "f"))
        menu.addItem(item(Wording.complete, #selector(NSTextView.complete(_:)), "\u{1b}", .option))
        return menu
    }

    private static func view() -> NSMenu {
        let menu = NSMenu(title: Commands.menu("view"))
        menu.addItem(item(Commands.asking("go-today"), #selector(AppDelegate.goToToday(_:)), "1"))
        menu.addItem(item(Commands.asking("go-tasks"), #selector(AppDelegate.goToTasks(_:)), "2"))
        menu.addItem(item(Commands.asking("go-blocks"), #selector(AppDelegate.goToBlocks(_:)), "3"))
        menu.addItem(item(Commands.asking("go-trash"), #selector(AppDelegate.goToTrash(_:)), "4"))
        menu.addItem(item(Commands.asking("go-history"), #selector(AppDelegate.goToHistory(_:)), "5"))
        menu.addItem(.separator())
        menu.addItem(item(Wording.toggleSidebar, #selector(NSSplitViewController.toggleSidebar(_:)), "s", [.command, .control]))
        return menu
    }

    private static func task() -> NSMenu {
        let menu = NSMenu(title: Commands.menu("task"))
        menu.addItem(item(Commands.asking("mark-done"), #selector(AppDelegate.toggleDone(_:)), "k"))
        menu.addItem(item(Commands.asking("put-in-block"), #selector(AppDelegate.putInBlock(_:)), "b"))
        menu.addItem(item(Commands.asking("move-to-project"), #selector(AppDelegate.moveToProject(_:)), "m", [.command, .shift]))
        menu.addItem(item(Commands.asking("make-subtask"), #selector(AppDelegate.makeSubtask(_:))))
        menu.addItem(item(Commands.asking("wait-for"), #selector(AppDelegate.waitFor(_:))))
        menu.addItem(item(Commands.asking("save-task"), #selector(TaskDetailViewController.saveTask(_:)), "s"))
        menu.addItem(.separator())
        menu.addItem(item(Commands.asking("trash-task"), #selector(AppDelegate.moveToTrash(_:)), "\u{8}"))
        return menu
    }

    private static func day() -> NSMenu {
        let menu = NSMenu(title: Commands.menu("day"))
        menu.addItem(item(Commands.asking("previous-day"), #selector(AppDelegate.previousDay(_:)), "["))
        menu.addItem(item(Commands.asking("next-day"), #selector(AppDelegate.nextDay(_:)), "]"))
        menu.addItem(item(Commands.asking("go-to-now"), #selector(AppDelegate.goToNow(_:)), "t"))
        menu.addItem(item(Commands.asking("go-to-day"), #selector(AppDelegate.goToDay(_:)), "j"))
        // Then the day's own actions, the core's, as the menu opens.
        menu.delegate = DayMenu.shared
        return menu
    }

    /// The Day menu's own part: the day's actions (`DayView.actions`), the app's, in its order,
    /// after what moves through days. Rebuilt each time the menu opens, since they change with
    /// the day and the time.
    final class DayMenu: NSObject, NSMenuDelegate {
        static let shared = DayMenu()
        /// What marks the items it added, to take them away again.
        static let tag = 7400

        func menuNeedsUpdate(_ menu: NSMenu) {
            for item in menu.items where item.tag == Self.tag { menu.removeItem(item) }
            guard let app = AppDelegate.shared else { return }
            let actions = app.dayActions
            guard !actions.isEmpty else { return }
            let separator = NSMenuItem.separator()
            separator.tag = Self.tag
            menu.addItem(separator)
            for action in actions {
                let item = ClosureMenuItem(title: action.asksFirst ? action.title + "…" : action.title) { app.performDay(action) }
                item.tag = Self.tag
                if let key = DayKeys.key(for: action.kind) {
                    item.keyEquivalent = key
                    item.keyEquivalentModifierMask = [.command, .option]
                }
                menu.addItem(item)
            }
        }

        /// The keys work with the menu closed, whatever the day offers: the day's action of
        /// that kind, or why it has none (`notOffered`), rather than a key that does nothing.
        func menuHasKeyEquivalent(
            _ menu: NSMenu, for event: NSEvent, target: AutoreleasingUnsafeMutablePointer<AnyObject?>,
            action: UnsafeMutablePointer<Selector?>
        ) -> Bool {
            let modifiers = event.modifierFlags.intersection([.command, .option, .shift, .control])
            let typed = event.charactersIgnoringModifiers?.lowercased() ?? ""
            // Implementing this means AppKit no longer looks through the menu's own items, so
            // the fixed ones (Previous Day, Go to Day) are matched here.
            if let item = menu.items.first(where: {
                $0.tag != Self.tag && !$0.keyEquivalent.isEmpty && $0.keyEquivalent == typed
                    && $0.keyEquivalentModifierMask.intersection([.command, .option, .shift, .control]) == modifiers
            }) {
                target.pointee = item.target
                action.pointee = item.action
                return true
            }
            guard modifiers == [.command, .option], let command = DayKeys.command(for: typed) else { return false }
            pending = command
            target.pointee = self
            action.pointee = #selector(runPending(_:))
            return true
        }

        private var pending: String?

        @objc private func runPending(_ sender: Any?) {
            guard let command = pending, let app = AppDelegate.shared else { return }
            pending = nil
            app.runDay(command)
        }
    }

    private static let f6 = String(Character(UnicodeScalar(UInt16(NSF6FunctionKey))!))

    private static func window() -> NSMenu {
        let menu = NSMenu(title: Wording.window)
        menu.addItem(item(Wording.minimize, #selector(NSWindow.performMiniaturize(_:)), "m"))
        menu.addItem(item(Wording.zoom, #selector(NSWindow.performZoom(_:))))
        menu.addItem(.separator())
        let next = item(Commands.asking("next-pane"), #selector(AppDelegate.nextPane(_:)), f6, [])
        menu.addItem(next)
        let previous = item(Commands.asking("previous-pane"), #selector(AppDelegate.previousPane(_:)), f6, .shift)
        menu.addItem(previous)
        menu.addItem(.separator())
        menu.addItem(item(Wording.metadata.name, #selector(AppDelegate.showMainWindow(_:)), "0"))
        return menu
    }
}
