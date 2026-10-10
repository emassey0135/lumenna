import UIKit

extension Folding {
    /// Expand or Collapse as a swipe action, for a row with something under it. `changed`
    /// redraws the list and says what happened.
    func action<Item>(
        for row: Shown<Item>, key: String, changed: @escaping (_ key: String, _ said: String) -> Void
    ) -> UIContextualAction? {
        guard let (title, said) = Folding.action(for: row) else { return nil }
        let action = UIContextualAction(style: .normal, title: title) { _, _, finished in
            changed(key, said)
            finished(true)
        }
        action.backgroundColor = .lumennaTint
        return action
    }
}
