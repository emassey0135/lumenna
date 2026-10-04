import AppKit
import SwiftUI

/// One task's details, editable (§16.1), in the right-hand pane: the shared form
/// (`Shared/Forms/TaskForm.swift`), with the Mac's own sheets for choosing.
final class TaskDetailViewController: NSHostingController<TaskDetailView>, TaskFormHost {
    private let model: TaskDetailModel
    private let core: Core
    let taskID: String

    init(core: Core, id: String, window: MainWindowController) {
        let model = TaskDetailModel(core: core, id: id)
        self.model = model
        self.core = core
        taskID = id
        super.init(rootView: TaskDetailView(model: model))
        model.host = self
        title = "Task"
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    override var preferredFirstResponder: NSView? { view }

    /// Task > Save Changes, ⌘S.
    @objc func saveTask(_ sender: Any?) { model.save() }

    // MARK: - TaskFormHost

    func chooseTask(_ title: String, excluding: Set<String>, chosen: @escaping (String) -> Void) {
        guard let window = view.window else { return }
        let items = ((try? core.lumenna.listTasks(query: "").rows) ?? [])
            .filter { !excluding.contains($0.id) }
            .map { PickerItem(key: $0.id, title: $0.title, detail: $0.value) }
        PickerSheet.present(on: window, title: title, items: items) { chosen($0.key) }
    }

    func chooseBlock(for task: TaskDetail, chosen: @escaping (String, String, UInt32?) -> Void) {
        guard let window = view.window else { return }
        TaskActions.chooseBlock(core: core, window: window, for: task, chosen: chosen)
    }

    func trashed() {
        // The list moves to whatever now holds the task's place, and this pane follows it.
    }
}
