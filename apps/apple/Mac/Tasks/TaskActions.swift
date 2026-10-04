import AppKit

/// What can be done to one task, in one place: the list's context menu, the Task menu in the
/// menu bar, and the detail pane's buttons all offer these, so none can drift from the others.
struct TaskActions {
    let core: Core
    let window: NSWindow
    /// The list to keep focus in after a change, when there is one.
    weak var list: TaskListViewController?

    /// Runs a change, through the list when there is one so focus lands predictably.
    func perform(_ id: String?, _ operation: @escaping () throws -> Change) {
        if let list {
            list.perform(focusing: id, operation)
            return
        }
        do {
            let change = try operation()
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            window.showFailure(error.sentence)
        }
    }

    /// The menu for a task, read fresh so it matches what the task is now.
    func menu(for row: RowView) -> [(String, () -> Void)] {
        guard let task = try? core.lumenna.showTask(id: row.id).task else { return [] }
        let done = task.state.contains("completed")
        var actions: [(String, () -> Void)] = [
            (done ? "Mark Not Done" : "Mark Done", { toggleDone(task) }),
            ("Put in a Block…", { assign(task) }),
            ("Move to Project…", { moveToProject(task) }),
            ("Make Subtask Of…", { makeSubtask(task) }),
        ]
        if task.parent != nil {
            actions.append(("Move to Top Level", { perform(task.id) { try core.lumenna.moveTask(id: task.id, to: .top) } }))
        }
        actions.append(("Wait For…", { waitFor(task) }))
        for other in task.depends {
            actions.append(("Stop Waiting for \(other.title)", {
                perform(task.id) { try core.lumenna.removeDependency(id: task.id, on: other.id) }
            }))
        }
        actions.append(("-", {}))
        actions.append(("Move to Trash", { perform(nil) { try core.lumenna.trashTask(id: task.id) } }))
        return actions
    }

    func toggleDone(_ task: TaskDetail) {
        let done = task.state.contains("completed")
        perform(task.id) {
            done ? try core.lumenna.uncompleteTask(id: task.id) : try core.lumenna.completeTask(id: task.id)
        }
    }

    /// Open tasks other than these, to choose among.
    private func otherTasks(excluding: Set<String>) -> [PickerItem] {
        ((try? core.lumenna.listTasks(query: "").rows) ?? [])
            .filter { !excluding.contains($0.id) }
            .map { PickerItem(key: $0.id, title: $0.title, detail: $0.value) }
    }

    func makeSubtask(_ task: TaskDetail) {
        PickerSheet.present(on: window, title: "Make Subtask Of", items: otherTasks(excluding: [task.id])) { parent in
            perform(task.id) { try core.lumenna.moveTask(id: task.id, to: .parent(id: parent.key)) }
        }
    }

    func waitFor(_ task: TaskDetail) {
        let waiting = Set(task.depends.map(\.id) + [task.id])
        PickerSheet.present(on: window, title: "Waits For", items: otherTasks(excluding: waiting)) { other in
            perform(task.id) { try core.lumenna.addDependency(id: task.id, on: other.key) }
        }
    }

    func moveToProject(_ task: TaskDetail) {
        let projects = ((try? core.lumenna.listProjects().rows) ?? [])
            .filter { $0.title != task.project }
            .map { PickerItem(key: $0.title, title: $0.title, detail: $0.value) }
        PickerSheet.present(on: window, title: "Move to Project", items: projects) { project in
            perform(task.id) { try core.lumenna.moveTask(id: task.id, to: .project(name: project.key)) }
        }
    }

    /// Puts a task into a work block on today or the next six days (§3.7), asking how long
    /// the sitting is meant to take; the planner reaches any other day.
    func assign(_ task: TaskDetail) {
        var items: [PickerItem] = []
        var dates: [String: String] = [:]
        for offset in 0..<7 {
            let day = Calendar.current.date(byAdding: .day, value: offset, to: .now) ?? .now
            guard let plan = try? core.lumenna.plan(date: Clock.isoDay(day)) else { continue }
            for block in plan.blocks where block.kind == "work" {
                items.append(PickerItem(
                    key: block.id,
                    title: "\(Clock.spokenDay(plan.date)), \(Clock.time(block.start)), \(block.title)",
                    detail: "\(Clock.time(block.start)) to \(Clock.time(block.end))"
                ))
                dates[block.id] = plan.date
            }
        }
        guard !items.isEmpty else {
            window.showFailure("There are no work blocks this week. Add one from Today.", title: "Put in a Block")
            return
        }
        PickerSheet.present(on: window, title: "Put \(task.title) in a Block", items: items) { block in
            window.askForLength("How long is this sitting meant to take?", without: "Skip") { minutes in
                perform(task.id) {
                    try core.lumenna.assign(task: task.id, block: block.key, date: dates[block.key], minutes: minutes)
                }
            }
        }
    }
}
