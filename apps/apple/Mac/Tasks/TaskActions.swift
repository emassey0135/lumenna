import AppKit

/// Running a task's actions, the core's, in one place: the list's context menu, its keys, and
/// the Task menu in the menu bar all go through here, so none can drift from the others.
struct TaskActions {
    let core: Core
    let window: NSWindow
    /// The list to keep focus in after a change, when there is one.
    weak var list: TaskListViewController?
    /// Edit Details, the app's own form: the detail pane.
    var openDetail: () -> Void = {}

    /// Runs `action` on the task `id`, through the list when there is one so focus lands
    /// predictably: on the task, or — when it left the list — on what holds its place.
    func perform(_ action: Action, on id: String) {
        let leaves = [.delete, .restore, .deleteForGood].contains(action.kind)
        let list = self.list
        window.run(action, core: core, form: { _ in openDetail() }) { change, _ in
            if let list {
                list.acted(change, focusing: leaves ? nil : id)
            } else {
                NotificationCenter.default.post(name: Core.changed, object: nil)
                Announcer.say(change.announcement, notices: change.notices)
            }
        }
    }
}
