import UIKit

/// Choosing a task: to assign to a block, to wait for, to go under.
final class TaskPicker: ItemListViewController {
    private let excluding: Set<String>
    private let chosen: (Item) -> Void

    init(core: Core, title: String, excluding: Set<String> = [], chosen: @escaping (Item) -> Void) {
        self.excluding = excluding
        self.chosen = chosen
        super.init(core: core, title: title)
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) }
        )
    }

    override func load() throws -> (items: [Item], count: String) {
        let rows = try core.lumenna.listTasks(query: "")
        var previous: UInt32?
        let items = rows.rows.filter { !excluding.contains($0.id) }.map { row -> Item in
            defer { previous = row.depth }
            return Item(
                key: row.id, title: row.title, detail: row.value, depth: row.depth,
                spoken: RowSpeech.value(row, previousDepth: previous)
            )
        }
        return (items, rows.announcement)
    }

    override func open(_ item: Item) {
        let chosen = self.chosen
        dismiss(animated: true) { chosen(item) }
    }

    /// Shows the picker in a sheet of its own.
    static func present(
        from presenter: UIViewController,
        core: Core,
        title: String,
        excluding: Set<String> = [],
        chosen: @escaping (Item) -> Void
    ) {
        let picker = TaskPicker(core: core, title: title, excluding: excluding, chosen: chosen)
        presenter.present(UINavigationController(rootViewController: picker), animated: true)
    }
}
