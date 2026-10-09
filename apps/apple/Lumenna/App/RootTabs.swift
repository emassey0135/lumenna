import UIKit

/// The iPhone's top level: the day, the tasks, everything else to browse, and settings.
///
/// Four tabs, each its own navigation stack, so a person deep in a project's tasks can look at
/// today and come back to exactly where they were. The iPad has a sidebar instead
/// (`IPadRoot`).
final class RootTabs: UITabBarController, CommandActions {
    private let core: Core
    private let day: DayViewController
    private let tasks: TaskListViewController
    private let browse: BrowseViewController
    private let settings: SettingsViewController

    init(core: Core) {
        self.core = core
        day = DayViewController(core: core)
        tasks = TaskListViewController(core: core)
        browse = BrowseViewController(core: core)
        settings = SettingsViewController(core: core)
        super.init(nibName: nil, bundle: nil)
        let tab = { (root: UIViewController, title: String, symbol: String) -> UIViewController in
            let navigation = UINavigationController(rootViewController: root)
            navigation.tabBarItem = UITabBarItem(title: title, image: UIImage(systemName: symbol), selectedImage: nil)
            return navigation
        }
        viewControllers = [
            tab(day, "Today", "calendar.day.timeline.left"),
            tab(tasks, "Tasks", "checklist"),
            tab(browse, "Browse", "folder"),
            tab(settings, "Settings", "gearshape"),
        ]
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    /// The tab at `index`, at its root, then `then` once it is in the window: a screen not
    /// yet shown can neither present nor take focus.
    private func select(_ index: Int, then: ((RootTabs) -> Void)? = nil) {
        if let presented = presentedViewController { presented.dismiss(animated: false) }
        selectedIndex = index
        (viewControllers?[index] as? UINavigationController)?.popToRootViewController(animated: false)
        if let then { DispatchQueue.main.async { [weak self] in self.map(then) } }
    }

    // The iPhone has no menu bar, so its keyboard commands are here.
    override var keyCommands: [UIKeyCommand]? { KeyboardCommands.all }

    /// The Home Screen's New Task (`SceneDelegate`), and ⌘N: quick add, over the task list it
    /// adds to, from wherever the app was.
    @objc func newTask() { select(1) { $0.tasks.addTask() } }
    @objc func newBlock() { select(0) { $0.day.newBlock() } }
    @objc func newProject() { openFromBrowse("projects") { ($0 as? ProjectsViewController)?.add() } }
    @objc func newLabel() { openFromBrowse("labels") { ($0 as? LabelsViewController)?.add() } }
    @objc func newFilter() { openFromBrowse("filters") { ($0 as? FiltersViewController)?.add() } }

    /// Browse's list `key`, then `then` on it.
    private func openFromBrowse(_ key: String, then: ((UIViewController) -> Void)? = nil) {
        select(2) { tabs in
            tabs.browse.open(Item(key: key, title: key))
            if let shown = tabs.browse.navigationController?.topViewController { then?(shown) }
        }
    }

    @objc func filterTasks() { select(1) { $0.tasks.focusFilter() } }
    @objc func goToToday() { select(0) }
    @objc func goToTasks() { select(1) }
    @objc func goToBlocks() { openFromBrowse("blocks") }
    @objc func goToTrash() { openFromBrowse("trash") }
    @objc func showSettings() { select(3) }
    @objc func undoChange() { undoInStore(core) }
    @objc func redoChange() { redoInStore(core) }
    @objc func syncNow() { syncAndSay(core) }
}
