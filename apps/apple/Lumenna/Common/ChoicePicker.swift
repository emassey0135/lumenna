import LumennaApp
import UIKit

/// Choosing one of what the app offers for an action (`Asking.choices`): a task to wait for or
/// go under, a project, a label, a work block. In a sheet, as a list with the task list's look:
/// a tree's depth is indentation for sight, and said in words where it changes. Why each is
/// offered is its second line and its hint.
final class ChoicePicker: AppListViewController {
    private let offered: [AppItem]
    private let count: String
    private let chosen: (String) -> Void
    private let cancelled: () -> Void
    /// Whether the sheet was closed by a choice, not by Cancel or a swipe down.
    private var answered = false

    /// What the app offers, as it words it.
    init(core: Core, title: String, items: [AppItem], chosen: @escaping (String) -> Void, cancelled: @escaping () -> Void) {
        var previous: UInt32 = 0
        offered = items.map { item in
            var item = item
            let level = item.depth != previous ? "level \(item.depth + 1)" : nil
            previous = item.depth
            item.spoken = AppModel.spoken(detail: item.detail, expanded: nil, level: level)
            return item
        }
        count = items.count == 1 ? "1 choice" : "\(items.count) choices"
        self.chosen = chosen
        self.cancelled = cancelled
        super.init(core: core, title: title)
    }

    override var offersUndo: Bool { false }
    /// A pick's rows are its own: the app moves nothing in it.
    override var held: (focus: String?, landing: UInt32) { (nil, 0) }

    override func viewDidLoad() {
        super.viewDidLoad()
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) }
        )
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        // Gone without a choice: Cancel, or swiped down.
        if !answered, navigationController?.isBeingDismissed ?? isBeingDismissed {
            answered = true
            cancelled()
        }
    }

    override func rows() -> (items: [AppItem], count: String)? {
        (offered, count)
    }

    override func open(_ item: AppItem) {
        answered = true
        let chosen = self.chosen
        dismiss(animated: true) { chosen(item.key) }
    }
}
