import LumennaApp
import UIKit

/// The iPhone's top level: the core's tabs (`Arrangement.tabs`), in its order and words — the
/// day, the tasks, everything else to browse, and settings.
///
/// Each tab is its own navigation stack, so a person deep in a project's tasks can look at
/// today and come back to exactly where they were. The iPad has a sidebar instead
/// (`IPadRoot`).
final class RootTabs: UITabBarController, CommandActions, PlaceFollowing {
    private let core: Core
    private let day: DayViewController
    private let tasks: TaskListViewController
    private let browse: BrowseViewController
    private let settings: SettingsViewController
    /// What each tab shows, in the tab bar's order.
    private let kinds: [LumennaApp.TabKind]

    init(core: Core) {
        self.core = core
        day = DayViewController(core: core)
        tasks = TaskListViewController(core: core, place: .tasks)
        browse = BrowseViewController(core: core)
        settings = SettingsViewController(core: core)
        let tabs = core.app.arrangement.tabs
        kinds = tabs.map(\.kind)
        super.init(nibName: nil, bundle: nil)
        viewControllers = tabs.map { tab in
            let navigation = UINavigationController(rootViewController: screen(for: tab.kind))
            navigation.tabBarItem = UITabBarItem(title: tab.title, image: UIImage(systemName: Self.symbol(for: tab.kind)), selectedImage: nil)
            return navigation
        }
    }

    /// The screen at the root of a tab.
    private func screen(for kind: LumennaApp.TabKind) -> UIViewController {
        switch kind {
        case .place(.today): day
        case .place(.tasks): tasks
        case let .place(place): TaskListViewController(core: core, place: place)
        case .browse: browse
        case .settings: settings
        }
    }

    /// A tab's picture: the system's symbol for what it shows.
    private static func symbol(for kind: LumennaApp.TabKind) -> String {
        switch kind {
        case .place(.today): "calendar.day.timeline.left"
        case .place(.tasks): "checklist"
        case .place: "list.bullet"
        case .browse: "folder"
        case .settings: "gearshape"
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    /// The app's questions, asked over whatever is in front, and the forms they open, over the
    /// tab shown.
    private lazy var asker = PhoneAsker(core: core, from: self)
    private var questions: AppQuestions?
    private var sheets: AppSheets?

    override func viewDidLoad() {
        super.viewDidLoad()
        sheets = AppSheets(core: core, root: self) { [weak self] list in
            guard let self else { return }
            let shown = (self.selectedViewController as? UINavigationController)?.topViewController ?? self
            shown.showBeside(list)
        }
        questions = AppQuestions(app: core.app, asker: asker) { [weak self] action in
            guard let self else { return }
            let shown = (self.selectedViewController as? UINavigationController)?.topViewController ?? self
            shown.openForm(action, core: self.core)
        }
    }

    /// The tab showing `kind`, at its root, then `then` once it is in the window: a screen not
    /// yet shown can neither present nor take focus.
    private func select(_ kind: LumennaApp.TabKind, then: ((RootTabs) -> Void)? = nil) {
        guard let index = kinds.firstIndex(of: kind) else { return }
        if let presented = presentedViewController { presented.dismiss(animated: false) }
        selectedIndex = index
        (viewControllers?[index] as? UINavigationController)?.popToRootViewController(animated: false)
        if let then { DispatchQueue.main.async { [weak self] in self.map(then) } }
    }

    // The iPhone has no menu bar, so its keyboard commands are here.
    override var keyCommands: [UIKeyCommand]? { KeyboardCommands.all }

    /// The Home Screen's New Task (`SceneDelegate`), and ⌘N: quick add, over the task list it
    /// adds to, from wherever the app was.
    @objc func newTask() { select(.place(.tasks)) { $0.tasks.addTask() } }
    @objc func newBlock() { select(.place(.today)) { $0.day.newBlock() } }
    @objc func newProject() { openFromBrowse(.group(.projects)) { ($0 as? PlacesViewController)?.add() } }
    @objc func newLabel() { openFromBrowse(.group(.labels)) { ($0 as? PlacesViewController)?.add() } }
    @objc func newFilter() { openFromBrowse(.group(.filters)) { ($0 as? PlacesViewController)?.add() } }

    /// One of Browse's places, then `then` on its screen.
    private func openFromBrowse(_ kind: LumennaApp.SidebarKind, then: ((UIViewController) -> Void)? = nil) {
        select(.browse) { tabs in
            let shown = tabs.browse.show(kind)
            then?(shown)
        }
    }

    /// The tab showing the app's place: a tab's own, any other in Browse.
    func showAppPlace() {
        guard let place = core.app.view.place else { return }
        go(to: place)
    }

    /// The tab showing `place`: its own, else Browse's, wherever the core's arrangement puts it.
    private func go(to place: LumennaApp.Place) {
        if kinds.contains(.place(place)) { select(.place(place)) } else { openFromBrowse(.place(place)) }
    }

    @objc func filterTasks() { select(.place(.tasks)) { $0.tasks.focusFilter() } }
    @objc func goToToday() { select(.place(.today)) }
    @objc func goToUpcoming() { go(to: .upcoming) }
    @objc func goToTasks() { select(.place(.tasks)) }
    @objc func goToBlocks() { openFromBrowse(.place(.blocks)) }
    @objc func goToTrash() { openFromBrowse(.place(.trash)) }
    @objc func goToHistory() { openFromBrowse(.place(.history)) }
    @objc func goToReview() { openFromBrowse(.place(.review)) }
    @objc func goToReport() { openFromBrowse(.place(.report)) }
    @objc func showSettings() { select(.settings) }
    /// Time Left: the timer as it is, said, from any tab (the core's `time-left`).
    @objc func timeLeft() { core.app.key("time-left") }
    @objc func undoChange() { undoInStore(core) }
    @objc func redoChange() { redoInStore(core) }
    @objc func syncNow() { syncAndSay(core) }

    // The day's own actions from anywhere else: today's, on the day.
    @objc func planTheDay() { select(.place(.today)) { $0.day.runDay("plan-day") } }
    @objc func replanFromNow() { select(.place(.today)) { $0.day.runDay("replan") } }
    @objc func whatNow() { select(.place(.today)) { $0.day.runDay("what-now") } }
    @objc func endTheDay() { select(.place(.today)) { $0.day.runDay("end-day") } }
    @objc func clearTheDay() { select(.place(.today)) { $0.day.runDay("clear-day") } }

    /// The overrun warning's Replan, which the app has taken: the day, Replan from Now over it.
    @objc func openReplan() {
        select(.place(.today))
    }

    /// A reminder chosen in Notification Center: its task, or its block on its day, in hand.
    @objc func openReminded(kind: String, target: String, day: String) {
        if kind == "task" {
            select(.place(.tasks)) { $0.tasks.showBeside(TaskDetailViewController(core: $0.core, id: target)) }
        } else {
            core.app.send(.showBlock(series: target, date: day))
            select(.place(.today))
        }
    }
}
