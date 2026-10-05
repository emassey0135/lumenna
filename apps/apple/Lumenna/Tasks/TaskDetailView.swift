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

    // MARK: - TaskFormHost

    func chooseTask(_ title: String, excluding: Set<String>, chosen: @escaping (String) -> Void) {
        TaskPicker.present(from: self, core: core, title: title, excluding: excluding) { item in chosen(item.key) }
    }

    /// The coming week's work blocks, as the Mac offers them; the planner reaches any other day.
    func chooseBlock(for task: TaskDetail, chosen: @escaping (String, String, UInt32?) -> Void) {
        let blocks = core.workBlocksThisWeek()
        guard !blocks.isEmpty else {
            showFailure("There are no work blocks this week. Add one from Today.")
            return
        }
        let items = blocks.map { Item(key: $0.id, title: $0.title, detail: $0.detail) }
        let dates = Dictionary(uniqueKeysWithValues: blocks.map { ($0.id, $0.date) })
        ListPicker.present(from: self, core: core, title: "Put in a Block", choices: items) { [weak self] block in
            self?.askForLength("How long is this sitting meant to take?", without: "Skip") { minutes in
                chosen(block.key, dates[block.key] ?? "", minutes)
            }
        }
    }

    func trashed() {
        navigationController?.popViewController(animated: true)
    }
}
