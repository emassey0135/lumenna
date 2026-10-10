import SwiftUI
import UIKit

/// One task's details, editable: the shared form (`Shared/Forms/TaskForm.swift`),
/// hosted with Save in the navigation bar and UIKit's own pickers.
final class TaskDetailViewController: UIHostingController<TaskDetailView>, TaskFormHost {
    private let model: TaskDetailModel
    private let core: Core

    init(core: Core, id: String) {
        let model = TaskDetailModel(core: core, id: id)
        self.model = model
        self.core = core
        super.init(rootView: TaskDetailView(model: model))
        title = "Task"
        model.host = self
        // A form gets the whole screen. Under the floating tab bar its last rows would be
        // read through glass.
        hidesBottomBarWhenPushed = true
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        // A UIKit button, always enabled: a disabled one fails contrast and is easy to miss
        // without sight, and saving with nothing changed says so.
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: "Save", primaryAction: UIAction { [weak self] _ in self?.model.save() }
        )
        navigationItem.rightBarButtonItem?.style = .done
    }

    override var canBecomeFirstResponder: Bool { true }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        takeKeyboardCommands()
    }

    // MARK: - TaskFormHost

    var asker: ActionAsking { PhoneAsker(core: core, from: self) }

    func trashed() {
        closeBeside()
    }

    // The Task menu, for the task shown (`KeyboardCommands`): beside its list on iPad, the
    // list is not in the responder chain, so the open task answers.

    @objc func saveChanges() {
        model.save()
    }

    @objc func toggleDone() {
        model.toggleDone()
    }

    @objc func moveToTrash() {
        model.trash()
    }
}
