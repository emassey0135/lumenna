import AppKit
import SwiftUI

/// The details of the task in hand, editable, in the right-hand pane: the shared form
/// (`Shared/Forms/TaskForm.swift`) over the app's details, which follow the row in hand.
final class TaskDetailViewController: HostedForm<TaskDetailView>, TaskFormHost {
    private let model: TaskDetailModel
    let taskID: String

    init(core: Core, id: String) {
        let model = TaskDetailModel(core: core, id: id)
        self.model = model
        taskID = id
        super.init("Task details", rootView: TaskDetailView(model: model))
        model.host = self
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override var preferredFirstResponder: NSView? { view }

    /// Task > Save Changes, ⌘S.
    @objc func saveTask(_ sender: Any?) { model.save() }

    // MARK: - TaskFormHost

    func trashed() {
        // The list moves to whatever now holds the task's place, and this pane follows it.
    }
}
