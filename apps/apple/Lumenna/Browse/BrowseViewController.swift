import UIKit

/// Everything that is not the day or the task list: projects, labels, saved filters, every
/// block series, and the trash.
final class BrowseViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Browse")
    }

    override func load() throws -> (items: [Item], count: String) {
        let projects = try core.lumenna.listProjects().count
        let labels = try core.lumenna.listLabels().count
        let filters = try core.lumenna.listFilters().count
        let blocks = try core.lumenna.listBlocks().count
        let trash = try core.lumenna.listTasks(query: "deleted").count
        return ([
            Item(key: "projects", title: "Projects", detail: count(projects, "project")),
            Item(key: "labels", title: "Labels", detail: count(labels, "label")),
            Item(key: "filters", title: "Saved Filters", detail: count(filters, "filter")),
            Item(key: "blocks", title: "Blocks", detail: count(blocks, "block")),
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
        case "blocks": next = BlocksViewController(core: core)
        default: next = TaskListViewController(core: core, title: "Trash", query: "deleted", mode: .trash)
        }
        navigationController?.pushViewController(next, animated: true)
    }
}

extension ItemListViewController {
    /// Adds a project, label or saved filter as the sidebar's heading over them does: its
    /// New action, the core's, so the add button asks what every app asks.
    func addFromHeading(_ group: SidebarGroup, key: @escaping (String) -> String) {
        let heading = core.lumenna.places().entries.first { $0.kind == .group(group) }
        guard let action = heading?.actions.first(.new) else { return }
        run(action, core: core, form: { [weak self] _ in self?.addFilter(key: key) }) { [weak self] change, answer in
            self?.reload(focusing: ActionRun.name(after: action, answer: answer).map(key), saying: change)
        }
    }
}

/// The project tree, with weights.
final class ProjectsViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Projects")
    }

    override func load() throws -> (items: [Item], count: String) {
        let rows = try core.lumenna.listProjects()
        empty = rows.empty
        // The level is added as shown, against the item before it once folded.
        let items = rows.rows.map { row -> Item in
            Item(
                key: row.title,
                title: row.title,
                detail: ([RowSpeech.details(row)].compactMap { $0 } + row.state).joined(separator: ", "),
                depth: row.depth,
                spoken: RowSpeech.value(row, previousDepth: row.depth),
                actions: row.actions
            )
        }
        return (items, rows.announcement)
    }

    override var addTitle: String? { "Add project" }

    override func add() {
        addFromHeading(.projects) { $0 }
    }

    override func open(_ item: Item) {
        navigationController?.pushViewController(
            TaskListViewController(
                core: core,
                title: item.title,
                query: projectReference(name: item.title),
                quickAddPrefix: projectReference(name: item.title) + " "
            ),
            animated: true
        )
    }
}

/// Labels: a first-class axis, with their own list.
final class LabelsViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Labels")
    }

    override func load() throws -> (items: [Item], count: String) {
        let rows = try core.lumenna.listLabels()
        empty = rows.empty
        return (rows.rows.map { Item(key: $0.title, title: $0.title, detail: $0.value, actions: $0.actions) }, rows.announcement)
    }

    override var addTitle: String? { "Add label" }

    override func add() {
        addFromHeading(.labels) { $0 }
    }

    override func open(_ item: Item) {
        navigationController?.pushViewController(
            TaskListViewController(
                core: core,
                title: item.title,
                query: labelReference(name: item.title),
                quickAddPrefix: labelReference(name: item.title) + " "
            ),
            animated: true
        )
    }
}

/// Saved filters: run, and also created, renamed, re-queried, reordered and deleted.
final class FiltersViewController: ItemListViewController {
    private var queries: [String: String] = [:]

    init(core: Core) {
        // Short: a bar title is one line between its buttons, and "Saved Filters" overflows a
        // small phone at the largest text sizes. Browse lists it by its full name.
        super.init(core: core, title: "Filters")
    }

    override func load() throws -> (items: [Item], count: String) {
        let filters = try core.lumenna.listFilters()
        empty = filters.empty
        queries = Dictionary(uniqueKeysWithValues: filters.filters.map { ($0.name, $0.query) })
        return (filters.filters.map { Item(key: $0.name, title: $0.name, detail: $0.query, actions: $0.actions) }, filters.announcement)
    }

    override var addTitle: String? { "Add filter" }

    override func add() {
        addFromHeading(.filters) { $0 }
    }

    override func open(_ item: Item) {
        navigationController?.pushViewController(
            TaskListViewController(core: core, title: item.title, query: queries[item.key] ?? ""),
            animated: true
        )
    }
}


/// Every block series, by when it starts: for the ones not on any day near enough to find
/// from the planner.
final class BlocksViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Blocks")
    }

    override func load() throws -> (items: [Item], count: String) {
        let rows = try core.lumenna.listBlocks()
        empty = rows.empty
        // Said as a task row is: when, in this device's clock ("every weekday at 9:00 AM"),
        // then how long.
        return (rows.rows.map { Item(key: $0.id, title: $0.title, detail: RowSpeech.details($0), actions: $0.actions) }, rows.announcement)
    }

    override var addTitle: String? { "Add block" }

    override func add() {
        presentBlockForm(BlockFormModel(core: core, purpose: .add) { [weak self] change in
            self?.reload(saying: change)
        })
    }

    override func open(_ item: Item) {
        edit(item)
    }

    /// Edit Block is the block form, for every occurrence.
    override func form(_ action: Action, on item: Item) {
        edit(item)
    }

    private func edit(_ item: Item) {
        do {
            presentBlockForm(try .series(core: core, id: item.key) { [weak self] change in
                self?.reload(focusing: item.key, saying: change)
            })
        } catch {
            showFailure(error.sentence)
        }
    }
}
