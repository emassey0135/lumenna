import AppKit
import SwiftUI

/// One task's details, editable, in the right-hand pane: the shared form
/// (`Shared/Forms/TaskForm.swift`), with the Mac's own sheets for its actions' questions.
final class TaskDetailViewController: HostedForm<TaskDetailView>, TaskFormHost {
    private let model: TaskDetailModel
    private let core: Core
    let taskID: String

    init(core: Core, id: String, window: MainWindowController) {
        let model = TaskDetailModel(core: core, id: id)
        self.model = model
        self.core = core
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

    /// The task's actions ask their questions as sheets on this window.
    var asker: ActionAsking { view.window ?? NSApp.mainWindow ?? NSWindow() }

    func trashed() {
        // The list moves to whatever now holds the task's place, and this pane follows it.
    }
}
