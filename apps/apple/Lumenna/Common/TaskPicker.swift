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

    override var offersUndo: Bool { false }

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

/// Choosing one of a list already made — a work block for a task — in a sheet.
final class ListPicker: ItemListViewController {
    private let choices: [Item]
    private let chosen: (Item) -> Void

    init(core: Core, title: String, choices: [Item], chosen: @escaping (Item) -> Void) {
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
        (choices, choices.count == 1 ? "1 choice" : "\(choices.count) choices")
    }

    override func open(_ item: Item) {
        let chosen = self.chosen
        dismiss(animated: true) { chosen(item) }
    }

    static func present(from presenter: UIViewController, core: Core, title: String, choices: [Item], chosen: @escaping (Item) -> Void) {
        let picker = ListPicker(core: core, title: title, choices: choices, chosen: chosen)
        presenter.present(UINavigationController(rootViewController: picker), animated: true)
    }
}
