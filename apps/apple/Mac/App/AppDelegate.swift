import AppKit

/// The Mac app: resident in the menu bar, syncing for as long as it runs, with
/// a window that closes to the menu bar rather than quitting.
final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuItemValidation {
    static weak var shared: AppDelegate?

    private(set) var core: Core!
    private var main: MainWindowController?
    private var settings: SettingsWindowController?
    private var quickAdd: QuickAddPanel?
    private var statusItem: NSStatusItem?
    private var backupTimer: Foundation.Timer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        AppDelegate.shared = self
        do {
            core = try Core()
        } catch {
            // Nothing works without the store, so say why plainly and stop.
            let alert = NSAlert()
            alert.messageText = "Lumenna could not open its store"
            alert.informativeText = error.sentence
            alert.runModal()
            NSApp.terminate(nil)
            return
        }
        NSApp.mainMenu = MainMenu.build()
        showMainWindow(nil)
        installStatusItem()
        quickAdd = QuickAddPanel(core: core)
        HotKeys.start(
            summon: { [weak self] in self?.showMainWindow(nil) },
            quickAdd: { [weak self] in self?.quickAdd?.show() }
        )
        core.startSyncing()
        core.watchForOtherProcesses()
        backUp()
        // Resident for days at a time, so a backup that falls due while it runs is taken:
        // it checks hourly, as `lum rpc` does.
        backupTimer = Foundation.Timer.scheduledTimer(withTimeInterval: 60 * 60, repeats: true) { [weak self] _ in self?.backUp() }
    }

    func applicationWillTerminate(_ notification: Notification) {
        core?.stopSyncing(waiting: true)
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        core?.timeZoneMayHaveChanged()
    }

    /// Closing the window keeps the app running; clicking the Dock icon brings it back.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showMainWindow(nil)
        return true
    }

    private func backUp() {
        core.backUpIfDue { [weak self] message in self?.main?.window?.showFailure(message, title: "Backup") }
    }

    // MARK: - Windows

    @objc func showMainWindow(_ sender: Any?) {
        if main == nil { main = MainWindowController(core: core) }
        NSApp.activate(ignoringOtherApps: true)
        main?.showWindow(nil)
        main?.window?.makeKeyAndOrderFront(nil)
    }

    @objc func showSettings(_ sender: Any?) {
        if settings == nil { settings = SettingsWindowController(core: core) }
        NSApp.activate(ignoringOtherApps: true)
        settings?.showWindow(nil)
        settings?.window?.center()
    }

    @objc func showQuickAdd(_ sender: Any?) {
        quickAdd?.show()
    }

    // MARK: - The menu bar item

    private func installStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = NSImage(systemSymbolName: "checklist", accessibilityDescription: "Lumenna")
        item.button?.setAccessibilityLabel("Lumenna")
        let menu = NSMenu()
        menu.addItem(withTitle: "Show Lumenna", action: #selector(showMainWindow(_:)), keyEquivalent: "")
        menu.addItem(withTitle: "Quick Add…", action: #selector(showQuickAdd(_:)), keyEquivalent: "")
        menu.addItem(withTitle: "Sync Now", action: #selector(syncNowFromMenu(_:)), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit Lumenna", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "")
        item.menu = menu
        statusItem = item
    }

    // MARK: - Sync

    @objc func syncNowFromMenu(_ sender: Any?) {
        Announcer.say("Syncing")
        syncNow {}
    }

    /// One round with every paired device, on this app's own endpoint.
    func syncNow(then finished: @escaping () -> Void) {
        core.syncNow { [weak self] result in
            switch result {
            case let .success(report):
                Announcer.say(report.announcement, notices: report.notices + report.peers.compactMap { peer in
                    peer.error.map { "\(peer.name): \($0)" }
                })
                NotificationCenter.default.post(name: Core.changed, object: nil)
            case let .failure(error):
                (NSApp.keyWindow ?? self?.main?.window)?.showFailure(error.sentence)
            }
            finished()
        }
    }

    // MARK: - Undo and redo, of the store

    /// ⌘Z undoes typing while a field is being edited, and otherwise the last change this Mac
    /// made to the store — which is what a person means by it in each place.
    @objc func undoChange(_ sender: Any?) {
        if let editing = editingUndoManager, editing.canUndo {
            editing.undo()
            return
        }
        changeStore { try $0.undo() }
    }

    @objc func redoChange(_ sender: Any?) {
        if let editing = editingUndoManager, editing.canRedo {
            editing.redo()
            return
        }
        changeStore { try $0.redo() }
    }

    private var editingUndoManager: UndoManager? {
        (NSApp.keyWindow?.firstResponder as? NSTextView)?.undoManager
    }

    private func changeStore(_ operation: (Lumenna) throws -> Change) {
        do {
            let change = try operation(core.lumenna)
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            (NSApp.keyWindow ?? main?.window)?.showFailure(error.sentence)
        }
    }

    // MARK: - Places and the selected task

    @objc func goToToday(_ sender: Any?) { go(.today) }
    @objc func goToTasks(_ sender: Any?) { go(.tasks) }
    @objc func goToBlocks(_ sender: Any?) { go(.blocks) }
    @objc func goToTrash(_ sender: Any?) { go(.trash) }

    private func go(_ place: Place) {
        showMainWindow(nil)
        main?.sidebar.select(place)
    }

    @objc func newTask(_ sender: Any?) {
        showMainWindow(nil)
        if let list = main?.taskList, list.mode == .tasks {
            list.addTask()
        } else if let window = main?.window {
            QuickAddSheet.present(on: window, core: core) { change in
                Announcer.say(change.announcement, notices: change.notices)
            }
        }
    }

    @objc func newBlock(_ sender: Any?) {
        showMainWindow(nil)
        if let day = main?.day {
            day.addBlock()
        } else if let window = main?.window {
            BlockFormModel(core: core, purpose: .add).present(on: window) { change in
                NotificationCenter.default.post(name: Core.changed, object: nil)
                Announcer.say(change.announcement, notices: change.notices)
            }
        }
    }

    @objc func newProject(_ sender: Any?) { showMainWindow(nil); main?.sidebar.newProject() }
    @objc func newLabel(_ sender: Any?) { showMainWindow(nil); main?.sidebar.newLabel() }
    @objc func newFilter(_ sender: Any?) { showMainWindow(nil); main?.sidebar.newSavedFilter() }

    @objc func focusFilter(_ sender: Any?) {
        if main?.taskList == nil { go(.tasks) }
        main?.taskList?.focusFilter()
    }

    /// The task the Task menu acts on: the one selected in a task list, or a sitting's task
    /// in the day.
    private var selectedTask: TaskDetail? {
        guard let id = main?.taskList?.selectedID ?? main?.day?.selectedTaskID else { return nil }
        return try? core.lumenna.showTask(id: id).task
    }

    /// Each Task menu command is the selected task's own action of that kind, so the menu
    /// never offers what the task's actions do not.
    private static func kinds(_ command: Selector?) -> [ActionKind] {
        switch command {
        case #selector(toggleDone(_:)): [.markDone, .markNotDone]
        case #selector(putInBlock(_:)): [.putInBlock]
        case #selector(moveToProject(_:)): [.moveToProject]
        case #selector(makeSubtask(_:)): [.makeSubtaskOf]
        case #selector(waitFor(_:)): [.waitFor]
        case #selector(moveToTrash(_:)): [.delete]
        default: []
        }
    }

    private func action(for command: Selector?) -> (TaskDetail, Action)? {
        let kinds = Self.kinds(command)
        guard let task = selectedTask, let action = task.actions.first(where: { kinds.contains($0.kind) }) else { return nil }
        return (task, action)
    }

    private func perform(_ command: Selector) {
        guard let window = main?.window, let (task, action) = action(for: command) else { return }
        TaskActions(core: core, window: window, list: main?.taskList).perform(action, on: task.id)
    }

    @objc func toggleDone(_ sender: Any?) { perform(#selector(toggleDone(_:))) }
    @objc func putInBlock(_ sender: Any?) { perform(#selector(putInBlock(_:))) }
    @objc func moveToProject(_ sender: Any?) { perform(#selector(moveToProject(_:))) }
    @objc func makeSubtask(_ sender: Any?) { perform(#selector(makeSubtask(_:))) }
    @objc func waitFor(_ sender: Any?) { perform(#selector(waitFor(_:))) }
    @objc func moveToTrash(_ sender: Any?) { perform(#selector(moveToTrash(_:))) }

    @objc func nextPane(_ sender: Any?) { main?.nextPane(sender) }
    @objc func previousPane(_ sender: Any?) { main?.previousPane(sender) }
    @objc func previousDay(_ sender: Any?) { main?.day?.previousDay(sender) }
    @objc func nextDay(_ sender: Any?) { main?.day?.nextDay(sender) }
    @objc func goToNow(_ sender: Any?) {
        if main?.day == nil { go(.today) } else { main?.day?.goToToday(sender) }
    }
    @objc func goToDay(_ sender: Any?) {
        if main?.day == nil { go(.today) }
        main?.day?.goToDay(sender)
    }

    @objc func backUpNow(_ sender: Any?) {
        do {
            let done = try core.lumenna.backup(to: nil)
            Announcer.say(done.announcement, notices: done.notices)
        } catch {
            main?.window?.showFailure(error.sentence)
        }
    }

    @objc func exportOrImport(_ sender: Any?) {
        showSettings(nil)
        (settings?.window?.contentViewController as? NSTabViewController)?.selectedTabViewItemIndex = 4
    }

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(toggleDone(_:)), #selector(putInBlock(_:)), #selector(moveToProject(_:)),
             #selector(makeSubtask(_:)), #selector(waitFor(_:)), #selector(moveToTrash(_:)):
            let found = action(for: item.action)
            if item.action == #selector(toggleDone(_:)) {
                item.title = found?.1.title ?? "Mark Done"
            }
            return found != nil
        case #selector(previousDay(_:)), #selector(nextDay(_:)):
            return main?.day != nil
        case #selector(undoChange(_:)):
            item.title = editingUndoManager?.canUndo == true ? "Undo Typing" : "Undo"
            return true
        case #selector(redoChange(_:)):
            item.title = editingUndoManager?.canRedo == true ? "Redo Typing" : "Redo"
            return true
        default:
            return true
        }
    }
}
