import UIKit

/// The app's top level: the day, the tasks, everything else to browse, and settings.
///
/// Four tabs, each its own navigation stack, so a person deep in a project's tasks can look at
/// today and come back to exactly where they were.
final class RootTabs: UITabBarController {
    init(core: Core) {
        super.init(nibName: nil, bundle: nil)
        let tab = { (root: UIViewController, title: String, symbol: String) -> UIViewController in
            let navigation = UINavigationController(rootViewController: root)
            navigation.tabBarItem = UITabBarItem(
                title: title, image: UIImage(systemName: symbol), selectedImage: nil
            )
            return navigation
        }
        viewControllers = [
            tab(DayViewController(core: core), "Today", "calendar.day.timeline.left"),
            tab(TaskListViewController(core: core), "Tasks", "checklist"),
            tab(BrowseViewController(core: core), "Browse", "folder"),
            tab(SettingsViewController(core: core), "Settings", "gearshape"),
        ]
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    // ⌘1 to ⌘4 go to the tabs, as in most apps with them.
    override var keyCommands: [UIKeyCommand]? {
        (viewControllers ?? []).enumerated().map { index, controller in
            UIKeyCommand(
                title: controller.tabBarItem.title ?? "",
                action: #selector(selectTab(_:)),
                input: "\(index + 1)",
                modifierFlags: .command,
                propertyList: index
            )
        }
    }

    /// The Home Screen's New Task (`SceneDelegate`): quick add, over the task list it adds
    /// to, from wherever the app was.
    func newTask() {
        selectedIndex = 1
        guard let navigation = selectedViewController as? UINavigationController,
              let tasks = navigation.viewControllers.first as? TaskListViewController
        else { return }
        let open = {
            navigation.popToRootViewController(animated: false)
            tasks.addTask()
        }
        if let presented = presentedViewController ?? navigation.presentedViewController {
            presented.dismiss(animated: false, completion: open)
        } else {
            open()
        }
    }

    @objc private func selectTab(_ command: UIKeyCommand) {
        if let index = command.propertyList as? Int {
            selectedIndex = index
        }
    }
}
