import AppKit

/// The menu bar. Everything the app does is here, with its shortcut: the menu bar is
/// how a Mac user — and a VoiceOver user especially, with VO-M — finds what an app can do.
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
        let help = NSMenu(title: "Help")
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
        let menu = NSMenu(title: "Lumenna")
        menu.addItem(item("About Lumenna", #selector(NSApplication.orderFrontStandardAboutPanel(_:))))
        menu.addItem(.separator())
        menu.addItem(item("Settings…", #selector(AppDelegate.showSettings(_:)), ","))
        menu.addItem(.separator())
        let services = NSMenu(title: "Services")
        let servicesItem = NSMenuItem(title: "Services", action: nil, keyEquivalent: "")
        servicesItem.submenu = services
        NSApp.servicesMenu = services
        menu.addItem(servicesItem)
        menu.addItem(.separator())
        menu.addItem(item("Hide Lumenna", #selector(NSApplication.hide(_:)), "h"))
        menu.addItem(item("Hide Others", #selector(NSApplication.hideOtherApplications(_:)), "h", [.command, .option]))
        menu.addItem(item("Show All", #selector(NSApplication.unhideAllApplications(_:))))
        menu.addItem(.separator())
        menu.addItem(item("Quit Lumenna", #selector(NSApplication.terminate(_:)), "q"))
        return menu
    }

    private static func file() -> NSMenu {
        let menu = NSMenu(title: "File")
        menu.addItem(item("New Task…", #selector(AppDelegate.newTask(_:)), "n"))
        menu.addItem(item("New Block…", #selector(AppDelegate.newBlock(_:)), "n", [.command, .shift]))
        menu.addItem(item("New Project…", #selector(AppDelegate.newProject(_:))))
        menu.addItem(item("New Label…", #selector(AppDelegate.newLabel(_:))))
        menu.addItem(item("New Saved Filter…", #selector(AppDelegate.newFilter(_:))))
        menu.addItem(.separator())
        menu.addItem(item("Sync Now", #selector(AppDelegate.syncNowFromMenu(_:)), "r", [.command, .shift]))
        menu.addItem(item("Back Up Now", #selector(AppDelegate.backUpNow(_:))))
        menu.addItem(item("Export and Import…", #selector(AppDelegate.exportOrImport(_:))))
        menu.addItem(.separator())
        menu.addItem(item("Close Window", #selector(NSWindow.performClose(_:)), "w"))
        return menu
    }

    private static func edit() -> NSMenu {
        let menu = NSMenu(title: "Edit")
        menu.addItem(item("Undo", #selector(AppDelegate.undoChange(_:)), "z"))
        menu.addItem(item("Redo", #selector(AppDelegate.redoChange(_:)), "z", [.command, .shift]))
        menu.addItem(.separator())
        menu.addItem(item("Cut", #selector(NSText.cut(_:)), "x"))
        menu.addItem(item("Copy", #selector(NSText.copy(_:)), "c"))
        menu.addItem(item("Paste", #selector(NSText.paste(_:)), "v"))
        menu.addItem(item("Select All", #selector(NSText.selectAll(_:)), "a"))
        menu.addItem(.separator())
        menu.addItem(item("Filter Tasks", #selector(AppDelegate.focusFilter(_:)), "f"))
        menu.addItem(item("Complete", #selector(NSTextView.complete(_:)), "\u{1b}", .option))
        return menu
    }

    private static func view() -> NSMenu {
        let menu = NSMenu(title: "View")
        menu.addItem(item("Today", #selector(AppDelegate.goToToday(_:)), "1"))
        menu.addItem(item("Tasks", #selector(AppDelegate.goToTasks(_:)), "2"))
        menu.addItem(item("Blocks", #selector(AppDelegate.goToBlocks(_:)), "3"))
        menu.addItem(item("Trash", #selector(AppDelegate.goToTrash(_:)), "4"))
        menu.addItem(.separator())
        menu.addItem(item("Toggle Sidebar", #selector(NSSplitViewController.toggleSidebar(_:)), "s", [.command, .control]))
        return menu
    }

    private static func task() -> NSMenu {
        let menu = NSMenu(title: "Task")
        menu.addItem(item("Mark Done", #selector(AppDelegate.toggleDone(_:)), "k"))
        menu.addItem(item("Put in a Block…", #selector(AppDelegate.putInBlock(_:)), "b"))
        menu.addItem(item("Move to Project…", #selector(AppDelegate.moveToProject(_:)), "m", [.command, .shift]))
        menu.addItem(item("Make Subtask Of…", #selector(AppDelegate.makeSubtask(_:))))
        menu.addItem(item("Wait For…", #selector(AppDelegate.waitFor(_:))))
        menu.addItem(item("Save Changes", #selector(TaskDetailViewController.saveTask(_:)), "s"))
        menu.addItem(.separator())
        menu.addItem(item("Move to Trash", #selector(AppDelegate.moveToTrash(_:)), "\u{8}"))
        return menu
    }

    private static func day() -> NSMenu {
        let menu = NSMenu(title: "Day")
        menu.addItem(item("Previous Day", #selector(AppDelegate.previousDay(_:)), "["))
        menu.addItem(item("Next Day", #selector(AppDelegate.nextDay(_:)), "]"))
        menu.addItem(item("Go to Now", #selector(AppDelegate.goToNow(_:)), "t"))
        menu.addItem(item("Go to Day…", #selector(AppDelegate.goToDay(_:)), "j"))
        return menu
    }

    private static let f6 = String(Character(UnicodeScalar(UInt16(NSF6FunctionKey))!))

    private static func window() -> NSMenu {
        let menu = NSMenu(title: "Window")
        menu.addItem(item("Minimize", #selector(NSWindow.performMiniaturize(_:)), "m"))
        menu.addItem(item("Zoom", #selector(NSWindow.performZoom(_:))))
        menu.addItem(.separator())
        let next = item("Next Pane", #selector(AppDelegate.nextPane(_:)), f6, [])
        menu.addItem(next)
        let previous = item("Previous Pane", #selector(AppDelegate.previousPane(_:)), f6, .shift)
        menu.addItem(previous)
        menu.addItem(.separator())
        menu.addItem(item("Lumenna", #selector(AppDelegate.showMainWindow(_:)), "0"))
        return menu
    }
}
