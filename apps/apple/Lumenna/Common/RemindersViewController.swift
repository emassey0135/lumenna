import UIKit
import UserNotifications

/// A task's or block's reminders, the core's list (`remindersOf`): the heading says how many
/// and whether they are the defaults, each reminder reads as the core words it, its own offer
/// Remove Reminder as a swipe, and Add Reminder asks for a phrase the core reads, asked again
/// with the core's reason when it refuses one.
final class RemindersViewController: ItemListViewController {
    private let target: String
    private var adding: Action?
    /// Whether the person turned Lumenna's notifications off in Settings, said in the heading:
    /// otherwise a reminder set here would seem to do nothing.
    private var silenced = false

    init(core: Core, target: String) {
        self.target = target
        super.init(core: core, title: "Reminders")
        adding = (try? core.lumenna.remindersOf(target: target))?.actions.first
        hidesBottomBarWhenPushed = true
    }

    override func load() throws -> (items: [Item], count: String) {
        let list = try core.lumenna.remindersOf(target: target)
        adding = list.actions.first
        let items = list.reminders.map { reminder in
            Item(key: reminder.id, title: reminder.words, detail: reminder.own ? nil : "Default", actions: reminder.actions)
        }
        let off = silenced ? "Notifications for Lumenna are off in Settings, so these are not shown" : nil
        return (items, ([list.announcement] + [off].compactMap { $0 }).joined(separator: ". "))
    }

    override var addTitle: String? { adding?.title ?? "Add Reminder" }

    override func add() {
        guard let adding else { return }
        run(adding, core: core) { [weak self] change, _ in
            NotificationCenter.default.post(name: Core.changed, object: nil)
            self?.reload(saying: change)
        }
    }

    /// A reminder of its own offers its actions when tapped too, as its swipe does.
    override func open(_ item: Item) {
        guard !item.actions.isEmpty else { return }
        choose(item.title, actions: item.actions.map { action in
            (action.title, { [weak self] in self?.perform(action, on: item) })
        })
    }

    override func acted(_ action: Action, on item: Item, answer: Answer, change: Change) {
        NotificationCenter.default.post(name: Core.changed, object: nil)
        super.acted(action, on: item, answer: answer, change: change)
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Someone looking at reminders wants them shown: the moment to ask, if never asked.
        Reminders.askIfUndecided { [weak self] in self?.checkSilenced() }
    }

    private func checkSilenced() {
        UNUserNotificationCenter.current().authorization { [weak self] status in
            guard let self else { return }
            let silenced = status == .denied
            guard silenced != self.silenced else { return }
            self.silenced = silenced
            reload()
        }
    }
}

extension UIViewController {
    /// Opens a task's or block's reminders: pushed over a task's details, which are already
    /// the thing open; beside the list otherwise, as a task's details are.
    func openReminders(_ action: Action, core: Core) {
        let reminders = RemindersViewController(core: core, target: action.target)
        if self is TaskDetailViewController, let navigation = navigationController {
            navigation.pushViewController(reminders, animated: true)
        } else {
            showBeside(reminders)
        }
    }
}
