import UIKit

/// What can be done to a project, a label or a saved filter, wherever one is listed: Browse's
/// lists on iPhone, the sidebar on iPad. Once, so the two never offer different things.
///
/// Each acts on the thing by name, and keeps VoiceOver on the list's row for it: `key` turns a
/// name into the row's key (Browse keys a row by its name; the sidebar by its place, since a
/// project and a label can share a name).
extension ItemListViewController {
    /// The project tree's: rename, reorder, nest, weigh, archive, delete.
    func projectActions(
        _ name: String, archived: Bool, others: @escaping () -> [String], key: @escaping (String) -> String
    ) -> [ItemAction] {
        let lumenna = core.lumenna
        return [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(name)", initial: name) { renamed in
                    self?.perform(on: item, renamedTo: key(renamed)) {
                        try lumenna.renameProject(name: name, to: renamed)
                    }
                }
            },
            ItemAction(title: "Move Up") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderProject(name: name, direction: .up) }
            },
            ItemAction(title: "Move Down") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderProject(name: name, direction: .down) }
            },
            ItemAction(title: "Move Under") { [weak self] item in
                var choices: [(String, () -> Void)] = [("Top Level", { [weak self] in
                    self?.perform(on: item) { try lumenna.moveProject(name: name, parent: nil) }
                })]
                choices += others().filter { $0 != name }.map { other in
                    (other, { [weak self] in
                        self?.perform(on: item) { try lumenna.moveProject(name: name, parent: other) }
                    })
                }
                self?.choose("Move \(name) under", actions: choices)
            },
            ItemAction(title: "Add Project Inside") { [weak self] _ in
                self?.addProject(inside: name, key: key)
            },
            ItemAction(title: "Weight") { [weak self] item in self?.askWeight(of: name, on: item) },
            ItemAction(title: archived ? "Unarchive" : "Archive") { [weak self] item in
                self?.perform(on: item) { try lumenna.archiveProject(name: name) }
            },
            ItemAction(title: "Delete", destructive: true) { [weak self] item in
                self?.choose("Delete \(name)?", message: "Its tasks can go to the trash with it, or move to the Inbox.", actions: [
                    ("Delete and Trash Its Tasks", { [weak self] in
                        self?.perform(on: item) { try lumenna.deleteProject(name: name, keepTasks: false) }
                    }),
                    ("Delete and Keep Its Tasks", { [weak self] in
                        self?.perform(on: item) { try lumenna.deleteProject(name: name, keepTasks: true) }
                    }),
                ])
            },
        ]
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
        let help = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again."
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

    /// A label's: rename, reorder, merge, colour, delete.
    func labelActions(_ name: String, others: @escaping () -> [String], key: @escaping (String) -> String) -> [ItemAction] {
        let lumenna = core.lumenna
        return [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(name)", initial: name) { renamed in
                    self?.perform(on: item, renamedTo: key(renamed)) { try lumenna.renameLabel(name: name, to: renamed) }
                }
            },
            ItemAction(title: "Move Up") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderLabel(name: name, direction: .up) }
            },
            ItemAction(title: "Move Down") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderLabel(name: name, direction: .down) }
            },
            ItemAction(title: "Merge Into") { [weak self] item in
                // For when a typo made a near-duplicate: this one's tasks move to the other.
                self?.choose("Merge \(name) into", actions: others().filter { $0 != name }.map { other in
                    (other, { [weak self] in
                        self?.perform(on: item, renamedTo: key(other)) { try lumenna.mergeLabels(from: name, into: other) }
                    })
                })
            },
            ItemAction(title: "Colour") { [weak self] item in
                self?.askForText(
                    "Colour for \(name)",
                    message: "A colour name, such as red or teal, or none. The name always shows too.",
                    placeholder: "teal"
                ) { colour in
                    let chosen = colour.lowercased() == "none" ? nil : colour
                    self?.perform(on: item) { try lumenna.recolourLabel(name: name, colour: chosen) }
                }
            },
            ItemAction(title: "Delete", destructive: true) { [weak self] item in
                self?.confirm("Delete \(name)?", message: "Tasks wearing it stay; they just stop showing it.", action: "Delete") {
                    self?.perform(on: item) { try lumenna.deleteLabel(name: name) }
                }
            },
        ]
    }

    /// Asks for a new label's name.
    func addLabel(key: @escaping (String) -> String) {
        askForText("New Label", placeholder: "Name", action: "Add") { [weak self] name in
            guard let self else { return }
            self.perform(on: Item(key: key(name), title: name)) { try self.core.lumenna.addLabel(name: name) }
        }
    }

    /// A saved filter's: rename, change its query, reorder, delete.
    func filterActions(_ name: String, query: String, key: @escaping (String) -> String) -> [ItemAction] {
        let lumenna = core.lumenna
        return [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(name)", initial: name) { renamed in
                    self?.perform(on: item, renamedTo: key(renamed)) {
                        try lumenna.editFilter(name: name, rename: renamed, query: nil)
                    }
                }
            },
            ItemAction(title: "Change Query") { [weak self] item in
                self?.askForText("Query for \(name)", initial: query) { changed in
                    self?.perform(on: item) { try lumenna.editFilter(name: name, rename: nil, query: changed) }
                }
            },
            ItemAction(title: "Move Up") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderFilter(name: name, direction: .up) }
            },
            ItemAction(title: "Move Down") { [weak self] item in
                self?.perform(on: item) { try lumenna.reorderFilter(name: name, direction: .down) }
            },
            ItemAction(title: "Delete", destructive: true) { [weak self] item in
                self?.perform(on: item) { try lumenna.deleteFilter(name: name) }
            },
        ]
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
