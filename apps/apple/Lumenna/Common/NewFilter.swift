import UIKit

/// The one place form the app asks itself (`Question.form`): a new saved filter, which takes
/// a name and then a query. Every other action on a project, label or saved filter is the
/// core's, asked by `ActionRun`.
extension ItemListViewController {
    /// Asks for a new saved filter's name, then its query.
    func addFilter(key: @escaping (String) -> String) {
        // The core's two questions: the name, then the query.
        let steps = TextQuestion.newFilter
        guard steps.count == 2 else { return }
        let (named, queried) = (steps[0], steps[1])
        askForText(named.title, message: named.hint.isEmpty ? nil : named.hint, placeholder: named.label, action: named.yes) { [weak self] name in
            self?.askForText(queried.title, message: queried.hint, placeholder: queried.label, action: queried.yes) { query in
                guard let self else { return }
                self.perform(on: Item(key: key(name), title: name)) {
                    try self.core.lumenna.addFilter(name: name, query: query)
                }
            }
        }
    }
}
