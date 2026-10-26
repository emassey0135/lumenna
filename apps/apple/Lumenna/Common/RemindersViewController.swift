import LumennaApp
import UIKit
import UserNotifications

/// A task's or block's reminders, the app's open list (`ViewModel.reminders`): the heading says
/// how many and whether they are the defaults, each reminder reads as the core words it, its own
/// offer Remove Reminder as a swipe, and Add Reminder asks for a phrase the core reads, asked
/// again with the core's reason when it refuses one. Held in the reminder list's hand, so a
/// change lands back here; leaving it closes it in the app.
final class RemindersViewController: AppListViewController {
    /// Whether the person turned Lumenna's notifications off in Settings, said in the heading:
    /// otherwise a reminder set here would seem to do nothing.
    private var silenced = false

    init(core: Core) {
        super.init(core: core, title: "Reminders")
        hidesBottomBarWhenPushed = true
    }

    override var side: LumennaApp.Side? { .reminders }

    override func rows() -> (items: [AppItem], count: String)? {
        guard let list = core.app.view.reminders else { return nil }
        let lines = core.app.lines.reminders
        let items = list.reminders.enumerated().map { index, reminder in
            let line = index < lines.count ? lines[index] : nil
            return AppItem(
                key: reminder.id, title: line?.title ?? reminder.words,
                detail: line.flatMap { $0.detail.isEmpty ? nil : $0.detail }, actions: reminder.offered
            )
        }
        let off = silenced ? LumennaApp.notificationWords(thisDevice: AppModel.thisDevice, sentence: false).off : nil
        return (items, ([list.announcement] + [off].compactMap { $0 }).joined(separator: " "))
    }

    override var addTitle: String? { "Add Reminder" }

    /// The list's Add Reminder, the app's: a phrase, asked by the window's questions.
    override func add() {
        guard let adding = core.app.view.reminders?.offered.first else { return }
        core.app.act(adding, on: .reminders)
    }

    /// A reminder of its own offers its actions when tapped too, as its swipe does.
    override func open(_ item: AppItem) {
        guard !item.actions.isEmpty else { return }
        choose(item.title, actions: item.actions.map { action in
            (action.title, { [weak self] in self?.perform(action, on: item) })
        })
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Someone looking at reminders wants them shown: the moment to ask, if never asked.
        Reminders.askIfUndecided { [weak self] in self?.checkSilenced() }
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        // Left, not covered by a question: the list is put away in the app.
        if isMovingFromParent || isBeingDismissed || navigationController?.isBeingDismissed == true {
            core.app.send(.closeReminders)
        }
    }

    private func checkSilenced() {
        UNUserNotificationCenter.current().authorization { [weak self] status in
            guard let self else { return }
            let silenced = status == .denied
            guard silenced != self.silenced else { return }
            self.silenced = silenced
            draw()
        }
    }
}
