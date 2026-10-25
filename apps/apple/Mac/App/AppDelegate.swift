import AppKit
import LumennaApp
import UserNotifications

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
    /// Gives this Mac's reminders while it runs (`Shared/Reminders.swift`).
    private var reminders: ReminderDelivery?
    /// Answers a reminder's Dismiss and Snooze; the notification center holds it weakly.
    private var responder: ReminderResponder?

    func applicationDidFinishLaunching(_ notification: Notification) {
        AppDelegate.shared = self
        do {
            core = try Core()
        } catch {
            // Nothing works without the store, so say why plainly and stop.
            let alert = NSAlert()
            alert.messageText = LumennaApp.storeUnopened(reason: "")
            alert.informativeText = error.sentence
            alert.runModal()
            NSApp.terminate(nil)
            return
        }
        // Siri, Shortcuts and Spotlight's intents run on this store.
        Assistant.serve(core.app.device) { [weak self] in self?.core.app.send(.launch($0)) }
        NSApp.mainMenu = MainMenu.build()
        showMainWindow(nil)
        installStatusItem()
        quickAdd = QuickAddPanel(core: core)
        HotKeys.start(
            summon: { [weak self] in self?.showMainWindow(nil) },
            quickAdd: { [weak self] in self?.quickAdd?.show() }
        )
        // Resident, so it gives reminders itself as they come due, as `lum notify` would.
        let responder = ReminderResponder(device: core.app.device) { [weak self] reminded in self?.open(reminded) }
        UNUserNotificationCenter.current().delegate = responder
        self.responder = responder
        reminders = ReminderDelivery(device: core.app.device)
        reminders?.follow()
        reminders?.start()
        core.watchForChanges()
        // The App Store edition's chosen backup folder, opened again before the first backup.
        BackupFolder.resume()
        backUp()
        // Resident for days at a time, so a backup that falls due while it runs is taken:
        // it checks hourly, as `lum rpc` does.
        backupTimer = Foundation.Timer.scheduledTimer(withTimeInterval: 60 * 60, repeats: true) { [weak self] _ in self?.backUp() }
    }

    func applicationWillTerminate(_ notification: Notification) {
        reminders?.stop()
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        core?.timeZoneMayHaveChanged()
        reminders?.now()
    }

    /// A widget tapped: the window, on the core's launch, which the window follows.
    func application(_ application: NSApplication, open urls: [URL]) {
        guard let core, let id = urls.lazy.compactMap(LaunchURL.id(in:)).first else { return }
        showMainWindow(nil)
        core.app.send(.launch(id))
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

    /// One round with every paired device, on this app's own endpoint: the app says how it
    /// went.
    @objc func syncNowFromMenu(_ sender: Any?) {
        Announcer.say("Syncing")
        core.app.send(.devices(.syncNow))
    }

    // MARK: - Undo and redo, of the store

    /// ⌘Z undoes typing while a field is being edited, and otherwise the last change this Mac
    /// made to the store — which is what a person means by it in each place.
    @objc func undoChange(_ sender: Any?) {
        if let editing = editingUndoManager, editing.canUndo {
            editing.undo()
            return
        }
        core.app.send(.undo)
    }

    @objc func redoChange(_ sender: Any?) {
        if let editing = editingUndoManager, editing.canRedo {
            editing.redo()
            return
        }
        core.app.send(.redo)
    }

    private var editingUndoManager: UndoManager? {
        (NSApp.keyWindow?.firstResponder as? NSTextView)?.undoManager
    }

    // MARK: - Places and the selected task

    @objc func goToToday(_ sender: Any?) { go(.today) }
    @objc func goToTasks(_ sender: Any?) { go(.tasks) }
    @objc func goToBlocks(_ sender: Any?) { go(.blocks) }
    @objc func goToTrash(_ sender: Any?) { go(.trash) }
    @objc func goToHistory(_ sender: Any?) { go(.history) }

    private func go(_ place: LumennaApp.Place) {
        showMainWindow(nil)
        if core.app.view.place != place { main?.go(place) }
    }

    /// A reminder chosen in Notification Center: its task, or its block on its day.
    private func open(_ reminded: RemindedOf) {
        if reminded.replan {
            // The overrun warning's Replan: today, with Replan from Now over it.
            NSApp.activate()
            go(.today)
            main?.day?.openReplan()
            return
        }
        if reminded.kind == "task" {
            go(.tasks)
            core.app.send(.focus(reminded.target))
            main?.showDetails()
        } else {
            go(.today)
            main?.day?.show(day: reminded.day, series: reminded.target)
        }
    }

    /// New Task: quick add, on the list shown, else from wherever the window is.
    @objc func newTask(_ sender: Any?) {
        showMainWindow(nil)
        if let list = main?.taskList, !list.trash {
            list.addTask()
        } else if let window = main?.window {
            QuickAddSheet.present(on: window, core: core)
        }
    }

    /// New Block: the app's block form, on the day shown or, from anywhere else, today.
    @objc func newBlock(_ sender: Any?) {
        showMainWindow(nil)
        core.app.send(.block(.new(start: nil, minutes: nil)))
    }

    @objc func newProject(_ sender: Any?) { showMainWindow(nil); main?.sidebar.newProject() }
    @objc func newLabel(_ sender: Any?) { showMainWindow(nil); main?.sidebar.newLabel() }
    @objc func newFilter(_ sender: Any?) { showMainWindow(nil); main?.sidebar.newSavedFilter() }

    @objc func focusFilter(_ sender: Any?) {
        if main?.taskList?.filters != true { go(.tasks) }
        main?.taskList?.focusFilter()
    }

    /// Each Task menu command is the in-hand task's own action of that kind — a task row's,
    /// or the task's in the details beside a sitting — so the menu never offers what the
    /// task's actions do not.
    private static func kinds(_ command: Selector?) -> [LumennaApp.ActionKind] {
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

    /// The task in hand's action for a Task menu command: its title, and running it on its row.
    /// A list's row offers it in short, the details whole.
    private func action(for command: Selector?) -> (title: String, run: () -> Void)? {
        let kinds = Self.kinds(command)
        let view = core.app.view
        let app = core.app
        if view.place != .today, view.place != .blocks, let focus = view.focus, let list = view.list,
           let item = list.items.first(where: { $0.row.id == focus }),
           let action = item.actions.lazy.compactMap({ Int($0) < list.offered.count ? list.offered[Int($0)] : nil })
               .first(where: { kinds.contains($0.kind) }) {
            return (action.title, { app.run(action, on: item.row.id) })
        }
        if let details = view.details, let action = details.task.offered.first(where: { kinds.contains($0.kind) }) {
            return (action.title, { app.act(action, on: .details) })
        }
        return nil
    }

    private func perform(_ command: Selector) {
        action(for: command)?.run()
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
        go(.today)
        main?.day?.goToToday(sender)
    }
    @objc func goToDay(_ sender: Any?) {
        go(.today)
        main?.day?.goToDay(sender)
    }

    /// The day's own actions for the Day menu: the day shown's, else none until it is shown;
    /// their keys go to the day first (`DayMenu`).
    var dayActions: [LumennaApp.Offered] {
        core.app.view.place == .today ? core.app.view.day?.offered ?? [] : []
    }

    /// One of the day's actions, from the Day menu.
    func performDay(_ action: LumennaApp.Offered) {
        core.app.act(action, on: .day)
    }

    /// The day's action of `kind`, from its key with the menu closed: today first, from
    /// anywhere else, then that action, or why the day has none, in the core's words.
    func runDay(_ kind: LumennaApp.ActionKind) {
        go(.today)
        core.app.send(.runDay([kind]))
    }

    @objc func backUpNow(_ sender: Any?) {
        do {
            let done = try core.app.device.backup()
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
                item.title = found?.title ?? "Mark Done"
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
