import UIKit

/// How a name is written after a sigil in the filter and quick-add languages: quoted when it
/// has a space in it (§6.2).
func sigil(_ mark: Character, _ name: String) -> String {
    name.contains(" ") ? "\(mark)\"\(name)\"" : "\(mark)\(name)"
}

/// Everything that is not the day or the task list: projects, labels, saved filters and the
/// trash (§16.1).
final class BrowseViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Browse")
    }

    override func load() throws -> (items: [Item], count: String) {
        let projects = try core.lumenna.listProjects().count
        let labels = try core.lumenna.listLabels().count
        let filters = try core.lumenna.listFilters().count
        let trash = try core.lumenna.listTasks(query: "deleted").count
        return ([
            Item(key: "projects", title: "Projects", detail: count(projects, "project")),
            Item(key: "labels", title: "Labels", detail: count(labels, "label")),
            Item(key: "filters", title: "Saved filters", detail: count(filters, "filter")),
            Item(key: "trash", title: "Trash", detail: count(trash, "task")),
        ], "")
    }

    private func count(_ n: UInt32, _ noun: String) -> String {
        n == 0 ? "no \(noun)s" : n == 1 ? "1 \(noun)" : "\(n) \(noun)s"
    }

    override func open(_ item: Item) {
        let next: UIViewController
        switch item.key {
        case "projects": next = ProjectsViewController(core: core)
        case "labels": next = LabelsViewController(core: core)
        case "filters": next = FiltersViewController(core: core)
        default: next = TaskListViewController(core: core, title: "Trash", query: "deleted", mode: .trash)
        }
        navigationController?.pushViewController(next, animated: true)
    }
}

/// The project tree, with weights (§3.4).
final class ProjectsViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Projects")
    }

    override func load() throws -> (items: [Item], count: String) {
        let rows = try core.lumenna.listProjects()
        // Depth is said where it changes, never as indentation alone (§16.11).
        var previous: UInt32?
        let items = rows.rows.map { row -> Item in
            defer { previous = row.depth }
            return Item(
                key: row.title,
                title: row.title,
                detail: row.value,
                depth: row.depth,
                spoken: RowSpeech.value(row, previousDepth: previous)
            )
        }
        return (items, rows.announcement)
    }

    override var addTitle: String? { "Add project" }

    override func add() {
        askForText("New Project", placeholder: "Name", action: "Add") { [weak self] name in
            guard let self else { return }
            self.perform(on: Item(key: name, title: name)) {
                try self.core.lumenna.addProject(name: name, parent: nil)
            }
        }
    }

    override func open(_ item: Item) {
        navigationController?.pushViewController(
            TaskListViewController(
                core: core,
                title: item.title,
                query: sigil("#", item.title),
                quickAddPrefix: sigil("#", item.title) + " "
            ),
            animated: true
        )
    }

    override func actions(for item: Item) -> [ItemAction] {
        let lumenna = core.lumenna
        return [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(item.title)", initial: item.title) { name in
                    self?.perform(on: item, renamedTo: name) {
                        try lumenna.renameProject(name: item.key, to: name)
                    }
                }
            },
            ItemAction(title: "Move Up") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderProject(name: item.key, direction: .up) }
            },
            ItemAction(title: "Move Down") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderProject(name: item.key, direction: .down) }
            },
            ItemAction(title: "Move Under") { [weak self] item in self?.moveUnder(item) },
            ItemAction(title: "Weight") { [weak self] item in
                self?.askForText(
                    "Weight of \(item.title)",
                    message: "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.",
                    placeholder: "1.0"
                ) { text in
                    let weight: Weight = Float(text).map { .value(value: $0) } ?? .inherit
                    self?.perform(on: item) { try lumenna.weighProject(name: item.key, weight: weight) }
                }
            },
            ItemAction(title: "Archive") { [weak self] item in
                self?.perform(on: item) { try lumenna.archiveProject(name: item.key) }
            },
            ItemAction(title: "Delete", destructive: true) { [weak self] item in self?.delete(item) },
        ]
    }

    private func moveUnder(_ item: Item) {
        let others = items.filter { $0.key != item.key }
        var choices: [(String, () -> Void)] = [("Top Level", { [weak self] in
            self?.perform(on: item) { try self!.core.lumenna.moveProject(name: item.key, parent: nil) }
        })]
        choices += others.map { other in
            (other.title, { [weak self] in
                self?.perform(on: item) {
                    try self!.core.lumenna.moveProject(name: item.key, parent: other.key)
                }
            })
        }
        choose("Move \(item.title) under", actions: choices)
    }

    private func delete(_ item: Item) {
        choose("Delete \(item.title)?", message: "Its tasks can go to the trash with it, or move to the Inbox.", actions: [
            ("Delete and Trash Its Tasks", { [weak self] in
                self?.perform(on: item) { try self!.core.lumenna.deleteProject(name: item.key, keepTasks: false) }
            }),
            ("Delete and Keep Its Tasks", { [weak self] in
                self?.perform(on: item) { try self!.core.lumenna.deleteProject(name: item.key, keepTasks: true) }
            }),
        ])
    }
}

/// Labels: a first-class axis, with their own list (§16.1).
final class LabelsViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Labels")
    }

    override func load() throws -> (items: [Item], count: String) {
        let rows = try core.lumenna.listLabels()
        return (rows.rows.map { Item(key: $0.title, title: $0.title, detail: $0.value) }, rows.announcement)
    }

    override var addTitle: String? { "Add label" }

    override func add() {
        askForText("New Label", placeholder: "Name", action: "Add") { [weak self] name in
            guard let self else { return }
            self.perform(on: Item(key: name, title: name)) { try self.core.lumenna.addLabel(name: name) }
        }
    }

    override func open(_ item: Item) {
        navigationController?.pushViewController(
            TaskListViewController(
                core: core,
                title: item.title,
                query: sigil("@", item.title),
                quickAddPrefix: sigil("@", item.title) + " "
            ),
            animated: true
        )
    }

    override func actions(for item: Item) -> [ItemAction] {
        let lumenna = core.lumenna
        return [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(item.title)", initial: item.title) { name in
                    self?.perform(on: item, renamedTo: name) { try lumenna.renameLabel(name: item.key, to: name) }
                }
            },
            ItemAction(title: "Move Up") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderLabel(name: item.key, direction: .up) }
            },
            ItemAction(title: "Move Down") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderLabel(name: item.key, direction: .down) }
            },
            ItemAction(title: "Merge Into") { [weak self] item in
                guard let self else { return }
                // For when a typo made a near-duplicate: this one's tasks move to the other.
                let others = self.items.filter { $0.key != item.key }
                self.choose("Merge \(item.title) into", actions: others.map { other in
                    (other.title, { [weak self] in
                        self?.perform(on: item, renamedTo: other.key) {
                            try lumenna.mergeLabels(from: item.key, into: other.key)
                        }
                    })
                })
            },
            ItemAction(title: "Colour") { [weak self] item in
                self?.askForText(
                    "Colour for \(item.title)",
                    message: "A colour name, such as red or teal, or none. The name always shows too.",
                    placeholder: "teal"
                ) { colour in
                    let chosen = colour.lowercased() == "none" ? nil : colour
                    self?.perform(on: item) { try lumenna.recolourLabel(name: item.key, colour: chosen) }
                }
            },
            ItemAction(title: "Delete", destructive: true) { [weak self] item in
                self?.confirm("Delete \(item.title)?", message: "Tasks wearing it stay; they just stop showing it.", action: "Delete") {
                    self?.perform(on: item) { try lumenna.deleteLabel(name: item.key) }
                }
            },
        ]
    }
}

/// Saved filters: run, and also created, renamed, re-queried, reordered and deleted (§16.1).
final class FiltersViewController: ItemListViewController {
    private var queries: [String: String] = [:]

    init(core: Core) {
        super.init(core: core, title: "Saved Filters")
    }

    override func load() throws -> (items: [Item], count: String) {
        let filters = try core.lumenna.listFilters()
        queries = Dictionary(uniqueKeysWithValues: filters.filters.map { ($0.name, $0.query) })
        return (filters.filters.map { Item(key: $0.name, title: $0.name, detail: $0.query) }, filters.announcement)
    }

    override var addTitle: String? { "Add filter" }

    override func add() {
        askForText("New Filter", placeholder: "Name", action: "Next") { [weak self] name in
            self?.askForText("Query for \(name)", placeholder: "#Work & overdue", action: "Save") { query in
                guard let self else { return }
                self.perform(on: Item(key: name, title: name)) {
                    try self.core.lumenna.addFilter(name: name, query: query)
                }
            }
        }
    }

    override func open(_ item: Item) {
        navigationController?.pushViewController(
            TaskListViewController(core: core, title: item.title, query: queries[item.key] ?? ""),
            animated: true
        )
    }

    override func actions(for item: Item) -> [ItemAction] {
        let lumenna = core.lumenna
        return [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(item.title)", initial: item.title) { name in
                    self?.perform(on: item, renamedTo: name) {
                        try lumenna.editFilter(name: item.key, rename: name, query: nil)
                    }
                }
            },
            ItemAction(title: "Change Query") { [weak self] item in
                self?.askForText("Query for \(item.title)", initial: self?.queries[item.key] ?? "") { query in
                    self?.perform(on: item) { try lumenna.editFilter(name: item.key, rename: nil, query: query) }
                }
            },
            ItemAction(title: "Move Up") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderFilter(name: item.key, direction: .up) }
            },
            ItemAction(title: "Move Down") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderFilter(name: item.key, direction: .down) }
            },
            ItemAction(title: "Delete", destructive: true) { [weak self] item in
                self?.perform(on: item) { try lumenna.deleteFilter(name: item.key) }
            },
        ]
    }
}
