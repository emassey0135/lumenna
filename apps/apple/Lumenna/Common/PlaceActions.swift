import UIKit

/// What can be done to a project, a label or a saved filter, wherever one is listed: Browse's
/// lists on iPhone, the sidebar on iPad. Once, so the two never offer different things.
///
/// Each acts on the thing by name, and keeps VoiceOver on the list's row for it: `key` turns a
/// name into the row's key (Browse keys a row by its name; the sidebar by its place, since a
/// project and a label can share a name).
extension ItemListViewController {
    /// The project tree's, as every app offers them (`PlaceAction.ofProject`).
    func projectActions(
        _ name: String, archived: Bool, others: @escaping () -> [String], key: @escaping (String) -> String
    ) -> [ItemAction] {
        let lumenna = core.lumenna
        return PlaceAction.ofProject(archived: archived).map { action in
            ItemAction(title: action.title, destructive: action.destructive) { [weak self] item in
                guard let self else { return }
                switch action {
                case .rename:
                    askForText("Rename \(name)", initial: name) { renamed in
                        self.perform(on: item, renamedTo: key(renamed)) { try lumenna.renameProject(name: name, to: renamed) }
                    }
                case .moveUp: perform(on: item) { try lumenna.reorderProject(name: name, direction: .up) }
                case .moveDown: perform(on: item) { try lumenna.reorderProject(name: name, direction: .down) }
                case .moveUnder:
                    var choices: [(String, () -> Void)] = [("Top Level", { [weak self] in
                        self?.perform(on: item) { try lumenna.moveProject(name: name, parent: nil) }
                    })]
                    choices += others().filter { $0 != name }.map { other in
                        (other, { [weak self] in
                            self?.perform(on: item) { try lumenna.moveProject(name: name, parent: other) }
                        })
                    }
                    choose("Move \(name) under", actions: choices)
                case .addProjectInside: addProject(inside: name, key: key)
                case .weight: askWeight(of: name, on: item)
                case .archive, .unarchive: perform(on: item) { try lumenna.archiveProject(name: name) }
                case .delete:
                    choose("Delete \(name)?", message: PlaceAction.deletingProject, actions: [
                        (PlaceAction.deleteAndTrash, { [weak self] in
                            self?.perform(on: item) { try lumenna.deleteProject(name: name, keepTasks: false) }
                        }),
                        (PlaceAction.deleteAndKeep, { [weak self] in
                            self?.perform(on: item) { try lumenna.deleteProject(name: name, keepTasks: true) }
                        }),
                    ])
                case .mergeInto, .colour, .changeQuery: break
                }
            }
        }
    }

    /// Asks for a new project's name, at the top level or inside `parent`.
    func addProject(inside parent: String? = nil, key: @escaping (String) -> String) {
        let title = parent.map { "New Project in \($0)" } ?? "New Project"
        askForText(title, placeholder: "Name", action: "Add") { [weak self] name in
            guard let self else { return }
            self.perform(on: Item(key: key(name), title: name)) {
                try self.core.lumenna.addProject(name: name, parent: parent)
            }
        }
    }

    /// Asks a project's weight, and again with what was typed when it is not one: a typo
    /// must not quietly become "inherit" (the core reads it, `parseWeight`).
    private func askWeight(of name: String, on item: Item, typed: String = "", problem: String? = nil) {
        let help = PlaceAction.weightHelp
        askForText(
            "Weight of \(name)",
            message: problem.map { "\($0)\n\n\(help)" } ?? help,
            placeholder: "1.0",
            initial: typed
        ) { [weak self] text in
            guard let self else { return }
            do {
                let weight = try parseWeight(text: text)
                perform(on: item) { try self.core.lumenna.weighProject(name: name, weight: weight) }
            } catch {
                askWeight(of: name, on: item, typed: text, problem: error.sentence)
            }
        }
    }

    /// A label's, as every app offers them (`PlaceAction.ofLabel`).
    func labelActions(_ name: String, others: @escaping () -> [String], key: @escaping (String) -> String) -> [ItemAction] {
        let lumenna = core.lumenna
        return PlaceAction.ofLabel.map { action in
            ItemAction(title: action.title, destructive: action.destructive) { [weak self] item in
                guard let self else { return }
                switch action {
                case .rename:
                    askForText("Rename \(name)", initial: name) { renamed in
                        self.perform(on: item, renamedTo: key(renamed)) { try lumenna.renameLabel(name: name, to: renamed) }
                    }
                case .moveUp: perform(on: item) { try lumenna.reorderLabel(name: name, direction: .up) }
                case .moveDown: perform(on: item) { try lumenna.reorderLabel(name: name, direction: .down) }
                case .mergeInto:
                    // For when a typo made a near-duplicate: this one's tasks move to the other.
                    choose("Merge \(name) into", actions: others().filter { $0 != name }.map { other in
                        (other, { [weak self] in
                            self?.perform(on: item, renamedTo: key(other)) { try lumenna.mergeLabels(from: name, into: other) }
                        })
                    })
                case .colour:
                    askForText("Colour for \(name)", message: PlaceAction.colourHelp, placeholder: "teal") { colour in
                        let chosen = colour.lowercased() == "none" ? nil : colour
                        self.perform(on: item) { try lumenna.recolourLabel(name: name, colour: chosen) }
                    }
                case .delete:
                    confirm("Delete \(name)?", message: PlaceAction.deletingLabel, action: "Delete") {
                        self.perform(on: item) { try lumenna.deleteLabel(name: name) }
                    }
                default: break
                }
            }
        }
    }

    /// Asks for a new label's name.
    func addLabel(key: @escaping (String) -> String) {
        askForText("New Label", placeholder: "Name", action: "Add") { [weak self] name in
            guard let self else { return }
            self.perform(on: Item(key: key(name), title: name)) { try self.core.lumenna.addLabel(name: name) }
        }
    }

    /// A saved filter's, as every app offers them (`PlaceAction.ofFilter`).
    func filterActions(_ name: String, query: String, key: @escaping (String) -> String) -> [ItemAction] {
        let lumenna = core.lumenna
        return PlaceAction.ofFilter.map { action in
            ItemAction(title: action.title, destructive: action.destructive) { [weak self] item in
                guard let self else { return }
                switch action {
                case .rename:
                    askForText("Rename \(name)", initial: name) { renamed in
                        self.perform(on: item, renamedTo: key(renamed)) { try lumenna.editFilter(name: name, rename: renamed, query: nil) }
                    }
                case .changeQuery:
                    askForText("Query for \(name)", initial: query) { changed in
                        self.perform(on: item) { try lumenna.editFilter(name: name, rename: nil, query: changed) }
                    }
                case .moveUp: perform(on: item) { try lumenna.reorderFilter(name: name, direction: .up) }
                case .moveDown: perform(on: item) { try lumenna.reorderFilter(name: name, direction: .down) }
                case .delete: perform(on: item) { try lumenna.deleteFilter(name: name) }
                default: break
                }
            }
        }
    }

    /// Asks for a new saved filter's name, then its query.
    func addFilter(key: @escaping (String) -> String) {
        askForText("New Filter", placeholder: "Name", action: "Next") { [weak self] name in
            self?.askForText("Query for \(name)", placeholder: "#Work & overdue", action: "Save") { query in
                guard let self else { return }
                self.perform(on: Item(key: key(name), title: name)) {
                    try self.core.lumenna.addFilter(name: name, query: query)
                }
            }
        }
    }
}
