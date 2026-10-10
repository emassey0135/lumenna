import UIKit

/// The one place form the app asks itself (`Question.form`): a new saved filter, which takes
/// a name and then a query. Every other action on a project, label or saved filter is the
/// core's, asked by `ActionRun`.
extension ItemListViewController {
    /// Asks for a new saved filter's name, then its query.
    func addFilter(key: @escaping (String) -> String) {
        askForText("New Saved Filter", placeholder: "Name", action: "Next") { [weak self] name in
            self?.askForText("Query for \(name)", placeholder: "#Work & overdue", action: "Save") { query in
                guard let self else { return }
                self.perform(on: Item(key: key(name), title: name)) {
                    try self.core.lumenna.addFilter(name: name, query: query)
                }
            }
        }
    }
}
