import UIKit

/// Choosing one of what the core offers for an action (`Lumenna.choices`): a task to wait
/// for or go under, a project, a label, a work block. In a sheet, as a list with the task
/// list's look: a tree's depth is indentation for sight, and said in words where it changes.
final class ChoicePicker: ItemListViewController {
    private let choices: [Choice]
    private let chosen: (Choice) -> Void

    init(core: Core, title: String, choices: [Choice], chosen: @escaping (Choice) -> Void) {
        self.choices = choices
        self.chosen = chosen
        super.init(core: core, title: title)
    }

    override var offersUndo: Bool { false }

    override func viewDidLoad() {
        super.viewDidLoad()
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) }
        )
    }

    override func load() throws -> (items: [Item], count: String) {
        let items = choices.map { choice in
            Item(key: choice.id, title: choice.shownTitle, detail: choice.shownDetail, depth: choice.depth)
        }
        return (items, choices.count == 1 ? "1 choice" : "\(choices.count) choices")
    }

    override func open(_ item: Item) {
        guard let choice = choices.first(where: { $0.id == item.key }) else { return }
        let chosen = self.chosen
        dismiss(animated: true) { chosen(choice) }
    }

    static func present(from presenter: UIViewController, core: Core, title: String, choices: [Choice], chosen: @escaping (Choice) -> Void) {
        let picker = ChoicePicker(core: core, title: title, choices: choices, chosen: chosen)
        presenter.present(UINavigationController(rootViewController: picker), animated: true)
    }
}
