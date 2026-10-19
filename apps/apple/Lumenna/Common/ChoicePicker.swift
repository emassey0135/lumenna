import UIKit

/// Choosing one of what the core offers for an action (`Lumenna.choices`): a task to wait
/// for or go under, a project, a label, a work block. In a sheet, as a list with the task
/// list's look: a tree's depth is indentation for sight, and said in words where it changes.
final class ChoicePicker: ItemListViewController {
    private let offered: [Item]
    private let count: String
    private let chosen: (String) -> Void
    private let cancelled: () -> Void
    /// Whether the sheet was closed by a choice, not by Cancel or a swipe down.
    private var answered = false

    /// The surface's choices, worded here.
    convenience init(core: Core, title: String, choices: [Choice], chosen: @escaping (Choice) -> Void) {
        let items = choices.map { choice in
            Item(key: choice.id, title: choice.shownTitle, detail: choice.shownDetail, depth: choice.depth, reason: choice.reason)
        }
        self.init(core: core, title: title, items: items, chosen: { id in
            if let choice = choices.first(where: { $0.id == id }) { chosen(choice) }
        }, cancelled: {})
    }

    /// What the app offers, as it words it, and the app's "another day" as a row of its own at
    /// the foot, keyed `ChoicePicker.anotherDay`.
    init(core: Core, title: String, items: [Item], chosen: @escaping (String) -> Void, cancelled: @escaping () -> Void) {
        self.offered = items
        let choices = items.filter { $0.key != Self.anotherDay }.count
        count = choices == 1 ? "1 choice" : "\(choices) choices"
        self.chosen = chosen
        self.cancelled = cancelled
        super.init(core: core, title: title)
    }

    /// The key of the row that asks for another day's blocks.
    static let anotherDay = "\u{0}another-day"

    override var offersUndo: Bool { false }

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

    override func load() throws -> (items: [Item], count: String) {
        (offered, count)
    }

    override func open(_ item: Item) {
        answered = true
        let chosen = self.chosen
        dismiss(animated: true) { chosen(item.key) }
    }

    static func present(from presenter: UIViewController, core: Core, title: String, choices: [Choice], chosen: @escaping (Choice) -> Void) {
        let picker = ChoicePicker(core: core, title: title, choices: choices, chosen: chosen)
        presenter.present(UINavigationController(rootViewController: picker), animated: true)
    }
}
